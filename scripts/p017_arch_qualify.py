#!/usr/bin/env python3
"""Operator-controlled P017 qualification. Default: read-only P015 probes.

No hardware gate is certified by this tool. Raw CLI output stays private.
"""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time

import p015_arch_preflight as p015
import p016_evidence as p016

STAGES = ('S0', 'S1', 'S2', 'S3', 'S4', 'S5', 'W0', 'W1', 'W2', 'W3', 'W4', 'W5', 'W6')
PHYSICAL = ('S3', 'S4', 'S5', 'W0', 'W1', 'W2', 'W3', 'W4', 'W5', 'W6')
STATUSES = {'pass', 'fail', 'blocked', 'not_run'}
PROVENANCE = {'mock_ci', 'host_preflight', 'operator_hardware', 'independent_observer'}
GRACE = 2.0
REAP = 1.0


def trusted_executable(value=None, name='opencontinuity'):
    """Trust local account/root ownership, never shell syntax or writable parents.

    PATH resolution excludes relative entries. Explicit paths must be absolute,
    canonical and named for the intended program. This is an operator trust
    boundary, not code signing or protection from a compromised local account.
    """
    if value is None:
        search = os.pathsep.join(p for p in os.get_exec_path() if os.path.isabs(p))
        value = shutil.which(name, path=search)
        if value:
            value = os.path.realpath(value)
    if not value or not os.path.isabs(value) or not re.fullmatch(r'/[A-Za-z0-9_./+-]+', value):
        raise ValueError('unsafe executable')
    path = Path(value)
    if str(path) != value or str(path.resolve()) != value or path.name != name:
        raise ValueError('unsafe executable')
    for item in (path, *path.parents):
        info = item.stat()
        if info.st_uid not in {0, os.geteuid()} or info.st_mode & 0o022:
            raise ValueError('unsafe executable')
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError('unsafe executable')
    return str(path)


def lookup(name):
    try:
        return trusted_executable(name=name)
    except (OSError, ValueError):
        # Arch installs the daemon outside PATH. Keep the P015 version probe,
        # resolving only these fixed packaged locations through the same checks.
        if name == 'bluetoothd':
            for path in ('/usr/lib/bluetooth/bluetoothd', '/usr/libexec/bluetooth/bluetoothd'):
                try:
                    return trusted_executable(path, name=name)
                except (OSError, ValueError):
                    pass
        return None


def probe(args, cli=None):
    executable = cli if args[0] == 'opencontinuity' and cli else lookup(args[0])
    if executable is None:
        return None
    # Bounded output, including version probes. No stderr is retained or echoed.
    try:
        result = collect([executable, *args[1:]], 3, limit=16384)
    except OSError:
        return None
    if result['exit_code'] != 0 or result['outcome'] != 'exited':
        return None
    try:
        return result['data'].decode('utf-8')
    except UnicodeError:
        return None


def host(cli=None):
    raw = p015.preflight(probe=lambda args: probe(args, cli), lookup=lookup)
    safe = p016.preflight(json.dumps(raw).encode())
    # Kernel release may contain a private host label; publish numeric version only.
    kernel = re.match(r'[0-9]+\.[0-9]+(?:\.[0-9]+)?', raw['environment']['kernel'])
    os_info = raw['environment']['os']
    os_id = os_info.get('id')
    if os_id not in {'arch', 'debian', 'ubuntu', 'fedora', 'alpine', 'opensuse'}:
        os_id = 'other'
    os_version = os_info.get('version_id', '')
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){0,3}', os_version):
        os_version = 'rolling' if os_id == 'arch' else 'unreported'
    result = {
        'kernel_version': kernel.group() if kernel else 'unknown',
        'os': {'id': os_id, 'version': os_version},
        'system_bus': raw['system_bus'],
        'bluez_access': raw.get('bluez_access', 'blocked_or_unavailable'),
        'bluez_version': raw['bluez_version'],
        'cli_version': 'unavailable',
        'settings': raw['settings'],
        'adapter_count': len(raw['adapters']), 'adapters': raw['adapters'],
        'readiness': safe['bluez_readiness'],
    }
    # Publish numeric CLI version only, never vendor/user supplied version labels.
    version = re.fullmatch(r'opencontinuity ([0-9]+\.[0-9]+\.[0-9]+)', raw['cli_version'])
    if version:
        result['cli_version'] = version[1]
    return result


def send_group(process, sig):
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        pass


def collect(argv, deadline_seconds, cancelled=lambda: False, limit=p016.MAX_BYTES):
    """Drain stdout without communicate's unbounded allocation; bound exit too.

    Start a private process group. TERM then KILL applies even if descendants
    retain stdout after the CLI exits. Cancellation never implies cleanup success.
    """
    data = bytearray()
    outcome = 'exited'
    process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, start_new_session=True)
    deadline = time.monotonic() + deadline_seconds
    stopping = None
    eof = False
    with selectors.DefaultSelector() as selector:
        os.set_blocking(process.stdout.fileno(), False)
        selector.register(process.stdout, selectors.EVENT_READ)
        try:
            while True:
                now = time.monotonic()
                if stopping is None and (cancelled() or now >= deadline):
                    outcome = 'cancelled' if cancelled() else 'timeout'
                    stopping = now
                    send_group(process, signal.SIGTERM)
                if stopping is not None and now >= stopping + GRACE:
                    send_group(process, signal.SIGKILL)
                    break
                if eof and process.poll() is not None:
                    break
                for key, _ in selector.select(0.05):
                    chunk = os.read(key.fd, 8192)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        eof = True
                    elif len(data) + len(chunk) > limit:
                        outcome = 'output_limit'
                        stopping = stopping if stopping is not None else now
                        send_group(process, signal.SIGTERM)
                    elif outcome != 'output_limit':
                        data.extend(chunk)
        finally:
            # Also remove any unexpected descendants after a normal child exit.
            send_group(process, signal.SIGKILL)
            process.stdout.close()
            try:
                code = process.wait(timeout=REAP)
            except subprocess.TimeoutExpired:
                code = None
    return {'data': bytes(data), 'exit_code': code, 'outcome': outcome}


@contextmanager
def cancellation():
    stopped = [False]
    def stop(_sig, _frame):
        stopped[0] = True
    previous = {sig: signal.signal(sig, stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        yield lambda: stopped[0]
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def private_output(directory):
    """Reserve a fresh 0600 transcript in an existing, canonical 0700 folder."""
    path = Path(directory)
    p016.require(path.is_absolute() and str(path.resolve()) == str(path))
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        p016.require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        output = os.open('scan.jsonl', os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o600, dir_fd=fd)
        return os.fdopen(output, 'wb')
    finally:
        os.close(fd)


def envelope(readiness=None):
    result = {'schema_version': 1, 'milestone': 'P017', 'authorization': 'never_evaluated',
              'stages': {s: {'status': 'not_run', 'provenance': 'host_preflight'} for s in STAGES},
              'execution_started': False, 'observed_events': 0, 'reported_cleanup': 'not_run',
              'terminal_valid': False, 'exit_verified': False, 'exit_code': None,
              'process_outcome': 'not_run', 'physical_qualification': 'blocked',
              'independent_rf': 'not_run',
              'hardware_attestation': 'not_verified', 'apple_auto_unlock': 'no_go'}
    if readiness is not None:
        result['host'] = readiness
        ready = (readiness['os'].get('id') == 'arch' and readiness['readiness'] == 'reported_ready'
                 and readiness['cli_version'] != 'unavailable' and readiness['settings'] != 'unavailable')
        result['stages']['S0']['status'] = 'pass' if ready else 'blocked'
    return result


def apply_transcript(result, data, execution=None, seconds=None):
    try:
        summary = p016.transcript(data)
        if seconds is not None:
            p016.require(p016.decode(data.splitlines()[0])['requested_duration_seconds'] == seconds)
    except (ValueError, TypeError, KeyError, RecursionError):
        result['stages']['S2']['status'] = 'fail'
        result['reported_cleanup'] = 'unknown'
        return
    result['terminal_valid'] = True
    result['observed_events'] = summary['device_event_count']
    result['reported_cleanup'] = summary['cleanup']
    result['summary'] = summary
    # An imported transcript cannot prove execution or exit, even when valid.
    ok = (execution is not None and execution['outcome'] == 'exited'
          and execution['exit_code'] == 0 and summary['terminal_status'] == 'completed'
          and summary['cleanup'] == 'reported_complete')
    result['stages']['S2']['status'] = 'pass' if ok else ('fail' if execution else 'blocked')


def operator_evidence(data):
    """Validate declarations, not authenticity; never copy private arbitrary fields."""
    obj = p016.decode(data)
    p016.require(type(obj) is dict and set(obj) == {'schema_version', 'authorization', 'stages'})
    p016.require(type(obj['schema_version']) is int and obj['schema_version'] == 1)
    p016.require(obj['authorization'] == 'never_evaluated')
    p016.require(type(obj['stages']) is dict and set(obj['stages']) == set(PHYSICAL))
    safe = {}
    for stage, row in obj['stages'].items():
        p016.require(type(row) is dict and set(row) == {'status', 'provenance', 'evidence_ref'})
        p016.require(row['status'] in STATUSES and row['provenance'] in PROVENANCE)
        ref = row['evidence_ref']
        p016.require(ref is None or (type(ref) is str and re.fullmatch(r'proof-[0-9]{3}\.json', ref)))
        if row['status'] == 'pass':
            p016.require(ref is not None and row['provenance'] in {'operator_hardware', 'independent_observer'})
            if stage == 'S3':
                p016.require(row['provenance'] == 'independent_observer')
        safe[stage] = dict(row)
    return safe


class Parser(argparse.ArgumentParser):
    def error(self, _message):
        # argparse's usual error can echo malicious argument/path values.
        self.exit(2, 'Qualification rejected: invalid options. Use --help.\n')


def main(argv=None):
    parser = Parser(description=__doc__, allow_abbrev=False)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--check', action='store_true', help='read-only readiness (default)')
    modes.add_argument('--active-discovery', action='store_true', help='explicit bounded radio trial')
    modes.add_argument('--validate-existing', action='store_true', help='validate local transcript without probes or radio')
    modes.add_argument('--evidence-template', action='store_true', help='emit private operator declaration template; no probes')
    parser.add_argument('--input', help='local JSONL file, only with --validate-existing')
    parser.add_argument('--operator-evidence', help='local operator declarations; never certified as hardware evidence')
    parser.add_argument('--cli', help='trusted canonical absolute opencontinuity executable')
    parser.add_argument('--seconds', type=int, default=10, help='1–30; active mode only')
    parser.add_argument('--i-consent-to-radio-scan', action='store_true')
    parser.add_argument('--i-own-host', action='store_true', help='confirm authority over the test host/adapter')
    parser.add_argument('--transcript-dir', help='optional existing private 0700 folder; creates scan.jsonl with 0600')
    args = parser.parse_args(argv)
    try:
        p016.require(1 <= args.seconds <= 30)
        p016.require(bool(args.input) == args.validate_existing)
        p016.require(not args.transcript_dir or args.active_discovery)
        p016.require(args.active_discovery or not (args.i_consent_to_radio_scan or args.i_own_host))
        if args.evidence_template:
            p016.require(not (args.cli or args.operator_evidence))
            obj = {'schema_version': 1, 'authorization': 'never_evaluated', 'stages': {
                s: {'status': 'not_run', 'provenance': 'independent_observer' if s == 'S3' else 'operator_hardware',
                    'evidence_ref': None} for s in PHYSICAL}}
            print(json.dumps(obj, indent=2, sort_keys=True))
            return 0
        if args.active_discovery:
            p016.require(args.i_consent_to_radio_scan and args.i_own_host)
        cli = trusted_executable(args.cli) if args.cli else None
        result = envelope(None if args.validate_existing else host(cli))
        if args.operator_evidence:
            result['operator_reported'] = operator_evidence(p016.read_local(args.operator_evidence))
            # Declarations do not change observed stages or physical gate outcomes.
        if args.validate_existing:
            apply_transcript(result, p016.read_local(args.input))
        elif args.active_discovery:
            if result['stages']['S0']['status'] != 'pass':
                result['stages']['S1']['status'] = 'blocked'
                print(json.dumps(result, indent=2, sort_keys=True))
                return 1
            cli = cli or trusted_executable()
            # Reserve before radio operation; unsafe outputs must never trigger scan.
            output = private_output(args.transcript_dir) if args.transcript_dir else None
            try:
                with cancellation() as cancelled:
                    execution = collect([cli, 'devices', 'discover', '--seconds', str(args.seconds), '--events-jsonl'],
                                        args.seconds + 12, cancelled)
                result.update(execution_started=True, exit_code=execution['exit_code'],
                              exit_verified=execution['exit_code'] is not None,
                              process_outcome=execution['outcome'])
                result['stages']['S1']['status'] = 'pass'
                if output:
                    output.write(execution['data'])
                apply_transcript(result, execution['data'], execution, args.seconds)
            finally:
                if output:
                    output.close()
        print(json.dumps(result, indent=2, sort_keys=True))
        return 1 if any(r['status'] == 'fail' for r in result['stages'].values()) else 0
    except (OSError, ValueError, TypeError, KeyError, RecursionError):
        print('Qualification rejected: unsafe options, unavailable prerequisites, or invalid evidence.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
