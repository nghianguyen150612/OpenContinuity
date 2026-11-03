import contextlib
import copy
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import p017_arch_qualify as q


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[2])
        self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name)
        self.cli = self.directory / 'opencontinuity'
        common = dict(schema_version=1, source='bluez_le_discovery_events', authorization='never_evaluated', scan_session_id='a'*32)
        self.events = [dict(common, type='session', sequence=1, state='started', status='started', scan_started=True, requested_duration_seconds=1, adapter_index=1),
                       dict(common, type='device', sequence=2, id='scan-'+'a'*16+'-001', display_name='Personal peripheral', provenance='bluez_event_during_session', change='observed'),
                       dict(common, type='terminal', sequence=3, scan_requested=True, scan_started=True, scan_completed=True, requested_duration_seconds=1, actual_duration_ms=1000, adapter_index=1, status='completed', cleanup_status='complete', device_count=1, device_event_count=1, termination_reason='duration_elapsed')]
        self.ready = dict(os={'id': 'arch'}, readiness='reported_ready', cli_version='0.1.0', settings='installed_version_unknown')

    def data(self, events=None):
        return b''.join(json.dumps(e).encode()+b'\n' for e in (self.events if events is None else events))

    def executable(self, body):
        self.cli.write_text('#!/usr/bin/python3\n'+body)
        self.cli.chmod(0o700)
        return str(self.cli)

    def invoke(self, args):
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = q.main(args)
        return code, json.loads(stdout.getvalue()) if stdout.getvalue() else None, stderr.getvalue()

    def test_default_reuses_read_only_preflight_without_scan(self):
        calls = []
        def probe(args, _cli=None):
            calls.append(args)
            return None
        with patch.object(q, 'probe', side_effect=probe), patch.object(q, 'lookup', return_value=None):
            code, report, _ = self.invoke([])
        self.assertEqual(code, 0)
        self.assertEqual(report['stages']['S0']['status'], 'blocked')
        self.assertFalse(report['execution_started'])
        self.assertTrue(calls)
        self.assertFalse(any('discover' in args for args in calls))

    def test_missing_bus_bluez_adapter_and_le_are_blocked(self):
        for absent in ['bus', 'bluez', 'adapter', 'powered', 'le']:
            def probe(args, _cli=None):
                if args[0] == 'opencontinuity': return 'opencontinuity 0.1.0'
                if args[0] == 'bluetoothd': return '5.82'
                if 'list' in args: return None if absent == 'bus' else 'org.bluez 123 bluetoothd root :1.2 bluetooth.service - -\n'
                if 'tree' in args: return None if absent == 'bluez' else ('' if absent == 'adapter' else '/org/bluez/hci0\n')
                if 'get-property' in args: return 'b false' if absent == 'powered' else 'b true'
                return 'as 0' if absent == 'le' else 'as 1 "Transport"'
            with self.subTest(absent=absent), patch.object(q, 'probe', side_effect=probe), patch.object(q, 'lookup', return_value='/usr/bin/unused'):
                report = q.host()
                self.assertEqual(report['readiness'], 'blocked_or_unavailable')
                with patch.object(q, 'host', return_value=report), patch.object(q, 'collect') as collect:
                    code, result, _ = self.invoke(['--active-discovery', '--i-consent-to-radio-scan', '--i-own-host'])
                    self.assertEqual(code, 1)
                    self.assertEqual(result['stages']['S1']['status'], 'blocked')
                    collect.assert_not_called()

    def test_invalid_consent_duration_options_never_probe(self):
        for args in [ ['--active-discovery'], ['--active-discovery', '--i-consent-to-radio-scan'],
                      ['--active-discovery', '--i-own-host'], ['--seconds', '0'], ['--seconds', '31'],
                      ['--transcript-dir', '/private'], ['--i-own-host'] ]:
            with self.subTest(args=args), patch.object(q, 'host') as host:
                self.assertEqual(self.invoke(args)[0], 1)
                host.assert_not_called()
        completed = subprocess.run([sys.executable, str(Path(q.__file__)), '--seconds', 'secret=private'], capture_output=True)
        self.assertNotIn(b'secret', completed.stderr)
        self.assertEqual(completed.returncode, 2)

    def test_executable_path_validation(self):
        valid = self.executable('pass\n')
        self.assertEqual(q.trusted_executable(valid), valid)
        for value in ['opencontinuity', '/absent/opencontinuity', valid+';touch', valid+'$(id)', valid+'/../opencontinuity']:
            with self.subTest(value=value), self.assertRaises((ValueError, OSError)):
                q.trusted_executable(value)
        self.cli.chmod(0o777)
        with self.assertRaises(ValueError): q.trusted_executable(valid)
        self.cli.chmod(0o700)
        link = self.directory / 'link'; link.symlink_to(self.cli)
        with self.assertRaises(ValueError): q.trusted_executable(str(link))
        self.directory.chmod(0o777)
        with self.assertRaises(ValueError): q.trusted_executable(valid)
        self.directory.chmod(0o700)

    def test_relative_path_entry_is_never_searched(self):
        self.executable('pass\n')
        with patch.dict(os.environ, {'PATH': '.'}), self.assertRaises(ValueError):
            q.trusted_executable()

    def test_packaged_bluez_daemon_outside_path_uses_same_trust_checks(self):
        with patch.object(q, 'trusted_executable', side_effect=[ValueError(), '/usr/lib/bluetooth/bluetoothd']) as resolve:
            self.assertEqual(q.lookup('bluetoothd'), '/usr/lib/bluetooth/bluetoothd')
        self.assertEqual(resolve.call_args.args, ('/usr/lib/bluetooth/bluetoothd',))
        self.assertEqual(resolve.call_args.kwargs, {'name': 'bluetoothd'})

    def test_valid_active_trial_checks_argv_exit_and_no_physical_promotion(self):
        expected = ['devices', 'discover', '--seconds', '1', '--events-jsonl']
        cli = self.executable(f'import sys\nassert sys.argv[1:] == {expected!r}\nsys.stdout.buffer.write({self.data()!r})\n')
        with patch.object(q, 'host', return_value=self.ready):
            code, result, _ = self.invoke(['--active-discovery', '--seconds', '1', '--cli', cli, '--i-own-host', '--i-consent-to-radio-scan'])
        self.assertEqual(code, 0)
        self.assertTrue(result['execution_started'])
        self.assertTrue(result['exit_verified'])
        self.assertEqual(result['stages']['S2']['status'], 'pass')
        self.assertEqual(result['reported_cleanup'], 'reported_complete')
        for stage in q.PHYSICAL: self.assertEqual(result['stages'][stage]['status'], 'not_run')
        self.assertEqual(result['physical_qualification'], 'blocked')
        self.assertNotIn('Personal peripheral', json.dumps(result))
        self.assertNotIn('a'*16, json.dumps(result))
        with patch.object(q, 'host', return_value=self.ready):
            code, result, _ = self.invoke(['--active-discovery', '--seconds', '1', '--cli', cli, '--i-own-host', '--i-consent-to-radio-scan', '--transcript-dir', str(self.directory)])
        self.assertEqual(code, 0)
        self.assertEqual((self.directory/'scan.jsonl').read_bytes(), self.data())
        self.assertEqual((self.directory/'scan.jsonl').stat().st_mode & 0o777, 0o600)
        self.executable(f'import sys\nsys.stdout.buffer.write({self.data()!r})\nsys.exit(9)\n')
        with patch.object(q, 'host', return_value=self.ready):
            code, result, _ = self.invoke(['--active-discovery', '--seconds', '1', '--cli', cli, '--i-own-host', '--i-consent-to-radio-scan'])
        self.assertEqual(code, 1)
        self.assertEqual(result['exit_code'], 9)
        self.assertEqual(result['stages']['S2']['status'], 'fail')

    def test_timeout_cancel_and_kill_fallback_are_bounded(self):
        cli = self.executable('import signal,time\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\ntime.sleep(60)\n')
        with patch.object(q, 'GRACE', 0.08), patch.object(q, 'REAP', 0.3):
            started = time.monotonic()
            result = q.collect([cli], 0.15)
            self.assertLess(time.monotonic()-started, 2)
            self.assertEqual(result['outcome'], 'timeout')
            self.assertEqual(result['exit_code'], -signal.SIGKILL)
            result = q.collect([cli], 10, cancelled=lambda: True)
            self.assertEqual(result['outcome'], 'cancelled')
            self.assertIsNotNone(result['exit_code'])

    def test_descendant_holding_pipe_cannot_extend_deadline(self):
        cli = self.executable('import os,time\nif os.fork() == 0: time.sleep(60)\n')
        with patch.object(q, 'GRACE', 0.05):
            started = time.monotonic(); result = q.collect([cli], 0.15)
        self.assertLess(time.monotonic()-started, 2)
        self.assertEqual(result['outcome'], 'timeout')

    def test_signal_handlers_restore_and_cancel(self):
        previous = signal.getsignal(signal.SIGTERM)
        with q.cancellation() as cancelled:
            os.kill(os.getpid(), signal.SIGTERM)
            self.assertTrue(cancelled())
        self.assertEqual(signal.getsignal(signal.SIGTERM), previous)

    def test_output_limit_and_stderr_redaction(self):
        cli = self.executable('import sys\nsys.stderr.write("AA:BB:CC:DD:EE:FF /home/private secret=hidden")\nsys.stdout.write("x"*100000)\n')
        with patch.object(q, 'GRACE', 0.05): result = q.collect([cli], 2, limit=1024)
        self.assertEqual(result['outcome'], 'output_limit')
        self.assertLessEqual(len(result['data']), 1024)
        self.assertNotIn(b'hidden', result['data'])

    def test_abnormal_exit_missing_terminal_cleanup_and_duration_mismatch(self):
        incomplete = copy.deepcopy(self.events)
        incomplete[-1].update(status='cleanup_incomplete', cleanup_status='incomplete', scan_completed=False, termination_reason='cleanup_failed')
        for data, code, outcome, seconds in [(self.data(), 9, 'exited', 1), (self.data(), -9, 'timeout', 1),
                (self.data(self.events[:-1]), 0, 'exited', 1), (self.data(incomplete), 1, 'exited', 1),
                (self.data(), 0, 'exited', 2), (b'', 0, 'exited', 1)]:
            result = q.envelope()
            q.apply_transcript(result, data, dict(exit_code=code, outcome=outcome), seconds)
            self.assertEqual(result['stages']['S2']['status'], 'fail')
            self.assertEqual(result['physical_qualification'], 'blocked')

    def test_offline_valid_invalid_and_private_inputs(self):
        path = self.directory/'input'; path.write_bytes(self.data())
        with patch.object(q, 'host') as host, patch.object(q, 'collect') as collect:
            code, result, _ = self.invoke(['--validate-existing', '--input', str(path)])
            host.assert_not_called(); collect.assert_not_called()
        self.assertEqual(code, 0)
        self.assertEqual(result['stages']['S2']['status'], 'blocked')
        self.assertFalse(result['exit_verified'])
        path.write_bytes(self.data().replace(b'Personal peripheral', b'AA:BB:CC:DD:EE:FF'))
        code, result, error = self.invoke(['--validate-existing', '--input', str(path)])
        self.assertEqual(code, 1)
        self.assertNotIn('AA:BB', json.dumps(result)+error)
        path.write_bytes(b'x'*(q.p016.MAX_BYTES+1))
        self.assertEqual(self.invoke(['--validate-existing', '--input', str(path)])[0], 1)

    def test_private_transcript_creation_refuses_overwrite_symlink_and_permissions(self):
        with q.private_output(self.directory) as output: output.write(b'private')
        path = self.directory/'scan.jsonl'
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(FileExistsError): q.private_output(self.directory)
        path.unlink(); path.symlink_to(self.directory/'target')
        with self.assertRaises(FileExistsError): q.private_output(self.directory)
        self.assertFalse((self.directory/'target').exists())
        self.directory.chmod(0o755)
        with self.assertRaises(ValueError): q.private_output(self.directory)
        self.directory.chmod(0o700)

    def test_unsafe_transcript_directory_rejected_before_scan(self):
        cli = self.executable('pass\n')
        with patch.object(q, 'host', return_value=self.ready), patch.object(q, 'collect') as collect:
            code, _, _ = self.invoke(['--active-discovery', '--cli', cli, '--i-own-host', '--i-consent-to-radio-scan', '--transcript-dir', '/missing/private'])
            self.assertEqual(code, 1); collect.assert_not_called()

    def test_operator_envelope_strict_and_never_attests(self):
        code, obj, _ = self.invoke(['--evidence-template'])
        self.assertEqual(code, 0)
        for stage, row in obj['stages'].items():
            row.update(status='pass', evidence_ref='proof-001.json')
        safe = q.operator_evidence(json.dumps(obj).encode())
        path = self.directory/'operator.json'; path.write_text(json.dumps(obj))
        with patch.object(q, 'host', return_value=self.ready):
            _, result, _ = self.invoke(['--operator-evidence', str(path)])
        self.assertEqual(result['operator_reported'], safe)
        self.assertEqual(result['hardware_attestation'], 'not_verified')
        self.assertEqual(result['physical_qualification'], 'blocked')
        self.assertEqual(result['stages']['S3']['status'], 'not_run')
        for mutate in [lambda x: x.update(address='AA:BB:CC:DD:EE:FF'),
                       lambda x: x.update(schema_version=True),
                       lambda x: x['stages']['S3'].update(provenance='mock_ci'),
                       lambda x: x['stages']['S3'].update(evidence_ref='/home/private'),
                       lambda x: x['stages']['S3'].update(evidence_ref=None),
                       lambda x: x['stages']['W0'].update(status='certified')]:
            bad = copy.deepcopy(obj); mutate(bad)
            with self.assertRaises(ValueError): q.operator_evidence(json.dumps(bad).encode())

    def test_host_suppresses_kernel_hostname_version_and_device_paths(self):
        raw = dict(schema_version=1, system_bus='available', bluez_access='available', bluez_version='5.82', cli_version='personal_host 0.1.0', settings='unavailable',
                   environment=dict(kernel='6.12.9-personal-host', os={'id': 'private-host', 'version_id': 'personal'}),
                   adapters=[dict(index=1, powered=True, transport_filter=True)])
        with patch.object(q.p015, 'preflight', return_value=raw): result = q.host()
        self.assertNotIn('personal', json.dumps(result))
        self.assertNotIn('private-host', json.dumps(result))
        self.assertEqual(result['kernel_version'], '6.12.9')
        self.assertEqual(result['cli_version'], 'unavailable')


if __name__ == '__main__': unittest.main()
