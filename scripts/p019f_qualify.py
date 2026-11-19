#!/usr/bin/env python3
"""Disposable-host qualification runner. Default operation is read-only.

No service, mount, quota or delegation provisioning is performed. The only
active commands are two fixed, ignored Rust test names. Production Receive
is never invoked. Public output contains no configured paths or raw logs.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import secrets
import signal
import stat
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[1]
STATUSES = {'MOCK_PASS', 'HOST_PREFLIGHT_PASS', 'KERNEL_PASS', 'NOT_RUN', 'BLOCKED', 'FAILED'}
TESTS = {
    'quota': 'airdrop::quota::tests::kernel_project_quota_atomic_publication_and_accounting',
    'placement_rejection': 'airdrop::containment::tests::real_kernel_cgroup_integration',
}
UNIMPLEMENTED = ('trusted_placement', 'descendant_escape_and_migration',
                 'cancellation_and_timeout', 'supervisor_sigkill_guardian_cleanup',
                 'guardian_failure_restart', 'write_confinement',
                 'publication_commit_boundary', 'cross_project_mount_publication',
                 'quota_configuration_change', 'positive_short_write', 'physical_airdrop')


def read_json(path, *, administrator=False):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    with os.fdopen(fd, 'rb') as source:
        info = os.fstat(source.fileno())
        if (not stat.S_ISREG(info.st_mode) or info.st_size > 16384
                or (administrator and (info.st_uid != 0 or info.st_mode & 0o022))):
            raise ValueError('invalid provisioning record')
        data = source.read(16385)
        if len(data) > 16384:
            raise ValueError('oversized provisioning record')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate key')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def open_directory(path):
    """Pin each component; reject symlink and parent traversal."""
    path = Path(path)
    if not path.is_absolute() or '..' in path.parts or path == Path('/'):
        raise ValueError('unsafe root')
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for component in path.parts[1:]:
            next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
                              | os.O_CLOEXEC, dir_fd=fd)
            os.close(fd)
            fd = next_fd
        return fd
    except BaseException:
        os.close(fd)
        raise


def mount_record(path, text=None):
    if text is None:
        text = Path('/proc/self/mountinfo').read_text()
    matches = []
    for line in text.splitlines():
        fields = line.split()
        split = fields.index('-')
        mount = re.sub(r'\\([0-7]{3})', lambda m: chr(int(m[1], 8)), fields[4])
        if Path(path).is_relative_to(mount):
            matches.append((len(mount), fields, split))
    if not matches:
        raise ValueError('mount unavailable')
    _, fields, split = max(matches, key=lambda item: item[0])
    # Sources, labels, mount paths and arbitrary mount options stay private.
    allowed = {'rw', 'ro', 'nodev', 'nosuid', 'noexec', 'prjquota', 'pquota', 'noquota'}
    options = set(fields[5].split(',')) | set(fields[split + 3].split(','))
    return {'id': int(fields[0]), 'device': fields[2], 'filesystem': fields[split + 1],
            'options': sorted(options & allowed)}


def host():
    release = os.uname().release
    return {'kernel': release if re.fullmatch(r'[A-Za-z0-9._+-]{1,100}', release) else 'redacted',
            'linux': os.uname().sysname == 'Linux',
            'cgroup_v2_visible': Path('/sys/fs/cgroup/cgroup.controllers').is_file(),
            'systemd_manager_visible': Path('/run/systemd/system').is_dir()}


def commit_identity():
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO,
                                  stderr=subprocess.DEVNULL, timeout=5).decode().strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=REPO,
                                        stderr=subprocess.DEVNULL, timeout=5))
    if not re.fullmatch('[0-9a-f]{40}', sha):
        raise ValueError('invalid commit')
    return sha, dirty


def validate_manifest(config, boot, uid):
    expected = {'schema', 'disposable', 'boot_id', 'supervisor_uid', 'backend_uid',
                'guardian_uid', 'cgroup', 'quota', 'project_id', 'hard_bytes', 'hard_inodes'}
    if (set(config) != expected or type(config['schema']) is not int
            or config['schema'] != 1 or config['disposable'] is not True):
        raise ValueError('invalid manifest')
    if config['boot_id'] != boot:
        raise ValueError('stale boot reservation')
    uids = [config[name] for name in ('supervisor_uid', 'backend_uid', 'guardian_uid')]
    if any(type(value) is not int or value <= 0 for value in uids) or len(set(uids)) != 3:
        raise ValueError('distinct non-root credentials required')
    if config['supervisor_uid'] != uid:
        raise ValueError('supervisor credential mismatch')
    for name, maximum in [('project_id', 2**32 - 1), ('hard_bytes', 64 * 1024 * 1024),
                          ('hard_inodes', 4096)]:
        if type(config[name]) is not int or not 0 < config[name] <= maximum:
            raise ValueError('unsafe quota configuration')
    for name in ('cgroup', 'quota'):
        if set(config[name]) != {'path', 'device', 'inode'}:
            raise ValueError('missing resource identity')
        if any(type(config[name][field]) is not int or config[name][field] <= 0
               for field in ('device', 'inode')):
            raise ValueError('invalid resource identity')


def preflight(config):
    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    validate_manifest(config, boot, os.geteuid())
    pinned = []
    observations = {}
    try:
        for name, filesystem in [('cgroup', 'cgroup2'), ('quota', 'xfs')]:
            resource = config[name]
            fd = open_directory(resource['path'])
            pinned.append(fd)
            info = os.fstat(fd)
            if (info.st_dev, info.st_ino) != (resource['device'], resource['inode']):
                raise ValueError('resource replaced')
            if info.st_uid != os.geteuid() or info.st_mode & 0o077:
                raise ValueError('resource ownership mismatch')
            record = mount_record(resource['path'])
            if record['filesystem'] != filesystem or 'ro' in record['options']:
                raise ValueError('unsupported mount')
            observations[name] = dict(record, inode=info.st_ino, owner=info.st_uid)
            names = os.listdir(fd)
            if name == 'quota' and names:
                raise ValueError('quota root is not disposable and empty')
            if name == 'cgroup':
                if any(stat.S_ISDIR(os.stat(entry, dir_fd=fd, follow_symlinks=False).st_mode)
                       for entry in names):
                    raise ValueError('existing child cgroups are not disposable')
                if any(entry.startswith('.opencontinuity') for entry in names):
                    raise ValueError('stale cgroup reservation')
                events_fd = os.open('cgroup.events', os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
                with os.fdopen(events_fd) as source:
                    if source.read(4096).splitlines().count('populated 0') != 1:
                        raise ValueError('writer subtree not empty')
                if not {'cgroup.procs', 'cgroup.kill', 'cgroup.controllers'} <= set(names):
                    raise ValueError('missing cgroup controls')
        return observations, pinned
    except BaseException:
        for fd in pinned:
            os.close(fd)
        raise


def bounded_test(command, environment, timeout=180, evidence_fd=None, mock=False):
    """Bounded relay; raw paths/panic text never enter public evidence.

    Only terminate our own cargo process group. On uncertainty, the runner
    does not touch any cgroup or filesystem allocation; retain for inspection.
    """
    process = subprocess.Popen(command, cwd=REPO, env=environment, start_new_session=True,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    digest = hashlib.sha256()
    output = bytearray()
    started = time.monotonic()
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    try:
        while selector.get_map():
            if time.monotonic() - started > timeout:
                raise TimeoutError('test deadline')
            for key, _ in selector.select(0.05):
                data = os.read(key.fileobj.fileno(), 65536)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                digest.update(data)
                output.extend(data)
                if len(output) > 4 * 1024 * 1024:
                    raise ValueError('test output limit')
                if evidence_fd is not None:
                    evidence_fd.write(data)
                    evidence_fd.flush()
        code = process.wait(timeout=max(0.1, timeout - (time.monotonic() - started)))
        # A skipped/filtered/zero-test cargo exit must never become KERNEL_PASS.
        passed = (code == 0 and b'1 passed; 0 failed; 0 ignored;' in output
                  and b'P019F_KERNEL_ASSERTIONS_COMPLETE' in output)
        if mock:
            passed = code == 0 and re.search(rb'Ran [1-9][0-9]* tests? in ', output) and b'\nOK\n' in output
        result = {'status': ('MOCK_PASS' if mock else 'KERNEL_PASS') if passed else 'FAILED',
                'exit_code': code, 'log_sha256': digest.hexdigest(),
                'elapsed_ms': round((time.monotonic() - started) * 1000)}
        counts = re.search(rb'^P019F_QUOTA_COUNTS (\d+) (\d+) (\d+) (\d+)$',
                           output, re.MULTILINE)
        if counts:
            result['quota_accounting'] = dict(zip(
                ('hard_bytes', 'used_bytes_at_exhaustion', 'hard_inodes', 'used_inodes_at_exhaustion'),
                (int(value) for value in counts.groups())))
        return result
    except (TimeoutError, ValueError, subprocess.TimeoutExpired):
        # Popen has not reaped the leader, preventing PID reuse here.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass  # Uninterruptible fixture: preserve, do not claim cleanup.
        return {'status': 'FAILED', 'reason': 'runner_timeout_or_output_limit',
                'log_sha256': digest.hexdigest(), 'retention': 'inspection_required'}
    finally:
        selector.close()
        process.stdout.close()


def run_suite(suite, config, pins, evidence):
    # Pinned identities are rechecked just before launch; the actual Rust
    # adapters independently open and validate. This still assumes a trusted
    # host excludes concurrent resource replacement; it is not a sandbox.
    for fd, name in zip(pins, ('cgroup', 'quota')):
        resource = config[name]
        fresh = open_directory(resource['path'])
        try:
            original, current = os.fstat(fd), os.fstat(fresh)
            if (original.st_dev, original.st_ino) != (current.st_dev, current.st_ino):
                raise ValueError('resource changed')
        finally:
            os.close(fresh)
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(('OPENCONTINUITY_', 'OC_P019'))}
    environment.update({
        'OPENCONTINUITY_RUN_QUOTA_INTEGRATION': '1',
        'OPENCONTINUITY_RUN_CGROUP_INTEGRATION': '1',
        'OPENCONTINUITY_AIRDROP_CGROUP_ROOT': config['cgroup']['path'],
        'OPENCONTINUITY_AIRDROP_QUOTA_ROOT': config['quota']['path'],
        'OPENCONTINUITY_AIRDROP_QUOTA_PROJECT_ID': str(config['project_id']),
        'OPENCONTINUITY_AIRDROP_QUOTA_HARD_BYTES': str(config['hard_bytes']),
        'OPENCONTINUITY_AIRDROP_QUOTA_HARD_INODES': str(config['hard_inodes']),
    })
    return bounded_test(['cargo', 'test', '-p', 'opencontinuity-cli', '--bin',
                         'opencontinuity', TESTS[suite], '--', '--ignored', '--exact',
                         '--nocapture', '--test-threads=1'], environment, evidence_fd=evidence)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest')
    parser.add_argument('--run', choices=TESTS)
    parser.add_argument('--private-evidence-dir')
    parser.add_argument('--mock', action='store_true', help='run offline contract/harness tests')
    parser.add_argument('--i-own-disposable-host', action='store_true')
    args = parser.parse_args(argv)
    sha, dirty = commit_identity()
    report = {'schema': 1, 'commit': sha, 'dirty': dirty, 'host': host(),
              'execution_id': secrets.token_hex(16),
              'observed_at': datetime.now(timezone.utc).isoformat(),
              'authorization': 'never_evaluated', 'physicalQualification': 'not_qualified',
              'productionQualified': False, 'records': []}
    records = report['records']
    pins = []
    result = 0
    if args.mock:
        records.append(dict(test='offline_contract_and_harness', **bounded_test(
            [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests',
             '-p', 'test_p019f_qualify.py', '-v'], os.environ.copy(), mock=True)))
        result = int(records[-1]['status'] != 'MOCK_PASS')
    try:
        if not args.manifest:
            raise ValueError('explicit_manifest_required')
        config = read_json(args.manifest, administrator=True)
        report['configuration_sha256'] = hashlib.sha256(
            json.dumps(config, sort_keys=True).encode()).hexdigest()
        observed, pins = preflight(config)
        report['host']['resources'] = observed
        records.append({'test': 'static_host_preflight', 'status': 'HOST_PREFLIGHT_PASS'})
        if args.run:
            if not args.i_own_disposable_host or dirty or not args.private_evidence_dir:
                raise ValueError('explicit_opt_in_and_clean_commit_required')
            evidence_root = open_directory(args.private_evidence_dir)
            try:
                info = os.fstat(evidence_root)
                if info.st_uid != os.geteuid() or info.st_mode & 0o077:
                    raise ValueError('evidence directory must be private')
                evidence_fd = os.open(report['execution_id'] + '.log',
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                    0o600, dir_fd=evidence_root)
                with os.fdopen(evidence_fd, 'wb') as evidence:
                    records.append(dict(test=TESTS[args.run],
                                        **run_suite(args.run, config, pins, evidence)))
            finally:
                os.close(evidence_root)
            result = int(records[-1]['status'] != 'KERNEL_PASS')
    except (OSError, ValueError, KeyError, TypeError):
        # Exception strings may contain private paths. Emit only fixed reasons.
        records.append({'test': 'host_preflight_or_execution', 'status': 'BLOCKED',
                        'reason': 'prerequisites_unavailable_or_unverifiable'})
        result |= int(bool(args.run))
    finally:
        for fd in pins:
            os.close(fd)
    for suite, identity in TESTS.items():
        if not any(record['test'] == identity for record in records):
            records.append({'test': identity, 'status': 'NOT_RUN'})
    records.extend({'test': identity, 'status': 'NOT_RUN' if identity == 'physical_airdrop'
                    else 'BLOCKED'} for identity in UNIMPLEMENTED)
    # Each record is tied to the enclosing exact commit and sanitized host
    # observation. No arbitrary imported result can promote qualification.
    print(json.dumps(report, sort_keys=True))
    return result


if __name__ == '__main__':
    raise SystemExit(main())
