import contextlib
from dataclasses import replace
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import p019f_contract as c
import p019f_qualify as q


def binding():
    return c.Binding('a' * 32, 'b' * 32, 'boot-identity', (1, 2, 3),
                     (4, 5, 6), (7, 8, 9), (10, 11, 12), (13, 14, 15))


def observation(sequence=1, **changes):
    return c.Observation(binding(), sequence, **dict(
        placed_before_exec=True, authority_fds_closed=True, distinct_credentials=True,
        guardian_alive=True, ingress_closed=True, recursively_empty=True,
        quota_unchanged=True, write_confinement=True, **changes))


class ContractTests(unittest.TestCase):
    def launch(self):
        contract = c.Contract(binding())
        self.assertTrue(contract.launched(observation()))
        return contract

    def sealed(self):
        contract = self.launch()
        self.assertTrue(contract.seal(Mock(seal_and_observe=lambda _: observation(2))))
        return contract

    def test_complete_contract_does_not_publish(self):
        contract = self.sealed()
        self.assertTrue(contract.commit(consent=True, before_deadline=True,
            cancelled=False, backend_success=True, relays_success=True,
            limits_valid=True, staging_valid=True, authority_live=True))
        self.assertEqual(contract.phase, c.Phase.COMMITTED)

    def test_each_uncertain_publication_gate_retains(self):
        gates = dict(consent=True, before_deadline=True, cancelled=False,
                     backend_success=True, relays_success=True, limits_valid=True,
                     staging_valid=True, authority_live=True)
        for gate in gates:
            for invalid in [None, not gates[gate], 1 if gates[gate] else 0]:
                with self.subTest(gate=gate, invalid=invalid):
                    contract = self.sealed()
                    self.assertFalse(contract.commit(**dict(gates, **{gate: invalid})))
                    self.assertEqual(contract.phase, c.Phase.RETAINED)

    def test_missing_placement_credentials_or_fd_closure_retains(self):
        for field in ('placed_before_exec', 'authority_fds_closed', 'distinct_credentials',
                      'guardian_alive', 'quota_unchanged', 'write_confinement'):
            contract = c.Contract(binding())
            self.assertFalse(contract.launched(replace(observation(), **{field: False})))

    def test_seal_requires_closed_ingress_and_recursive_empty(self):
        for field in ('ingress_closed', 'recursively_empty', 'quota_unchanged',
                      'guardian_alive', 'write_confinement'):
            contract = self.launch()
            manager = Mock(seal_and_observe=lambda _: replace(observation(2), **{field: False}))
            self.assertFalse(contract.seal(manager))
            self.assertEqual(contract.phase, c.Phase.RETAINED)

    def test_every_identity_change_blocks_replay_and_restart(self):
        for field in binding().__dataclass_fields__:
            replacement = 'c' * 32 if field in ('session', 'manager_epoch', 'boot') else (20, 21, 22)
            changed = replace(binding(), **{field: replacement})
            contract = self.launch()
            manager = Mock(seal_and_observe=lambda _: replace(observation(2), binding=changed))
            self.assertFalse(contract.seal(manager))
        for sequence in (0, 1, 3):
            contract = self.launch()
            self.assertFalse(contract.seal(Mock(seal_and_observe=lambda _: observation(sequence))))

    def test_guardian_timeout_crash_and_supervisor_loss_never_publish(self):
        for phase in ('startup', 'active', 'finalization'):
            contract = c.Contract(binding()) if phase == 'startup' else self.launch()
            if phase == 'finalization':
                contract = self.sealed()
            sequence = contract.sequence + 1
            manager = Mock(stop_and_observe=lambda _: observation(sequence))
            self.assertTrue(contract.supervisor_lost(manager))
            self.assertEqual(contract.phase, c.Phase.RETAINED)
        contract = self.launch()
        manager = Mock()
        manager.seal_and_observe.side_effect = TimeoutError()
        self.assertFalse(contract.seal(manager))
        manager.stop_and_observe.side_effect = OSError()
        self.assertFalse(contract.supervisor_lost(manager))

    def test_incomplete_identity_is_not_authority(self):
        for change in [dict(session='pid-123'), dict(supervisor_identity=(123,)),
                       dict(staging_identity=binding().destination_identity)]:
            with self.assertRaises(ValueError):
                replace(binding(), **change)


class HarnessTests(unittest.TestCase):
    def config(self):
        return dict(schema=1, disposable=True, boot_id='boot', supervisor_uid=1000,
                    backend_uid=1001, guardian_uid=1002,
                    cgroup=dict(path='/test/cgroup', device=1, inode=2),
                    quota=dict(path='/test/quota', device=3, inode=4),
                    project_id=42, hard_bytes=8 * 1024 * 1024, hard_inodes=128)

    def invoke(self, args):
        output = io.StringIO()
        with patch.object(q, 'commit_identity', return_value=('a' * 40, False)), \
             patch.object(q, 'host', return_value={'linux': True}), \
             contextlib.redirect_stdout(output):
            code = q.main(args)
        return code, json.loads(output.getvalue())

    def test_default_preflight_is_read_only_and_unqualified(self):
        with patch.object(q, 'run_suite') as run, patch.object(q, 'preflight') as probe:
            code, report = self.invoke([])
        run.assert_not_called()
        probe.assert_not_called()
        self.assertEqual(code, 0)
        self.assertFalse(report['productionQualified'])
        self.assertTrue(all(row['status'] in {'NOT_RUN', 'BLOCKED'} for row in report['records']))

    def test_run_needs_trusted_manifest_and_explicit_optin(self):
        with patch.object(q, 'run_suite') as run, \
             patch.object(q, 'read_json', return_value=self.config()), \
             patch.object(q, 'preflight', return_value=({}, [])):
            code, report = self.invoke(['--manifest', '/private/config', '--run', 'quota'])
        run.assert_not_called()
        self.assertEqual(code, 1)
        self.assertNotIn('/private', json.dumps(report))

    def test_preflight_cannot_promote_kernel_cases(self):
        with patch.object(q, 'read_json', return_value=self.config()), \
             patch.object(q, 'preflight', return_value=({}, [])):
            _, report = self.invoke(['--manifest', '/private/config'])
        self.assertEqual(report['records'][0]['status'], 'HOST_PREFLIGHT_PASS')
        self.assertNotIn('KERNEL_PASS', json.dumps(report))

    def test_actual_runner_result_keeps_unimplemented_cases_blocked(self):
        with tempfile.TemporaryDirectory() as evidence, \
             patch.object(q, 'read_json', return_value=self.config()), \
             patch.object(q, 'preflight', return_value=({}, [])), \
             patch.object(q, 'run_suite', return_value={'status': 'KERNEL_PASS', 'exit_code': 0}):
            code, report = self.invoke(['--manifest', '/private/config', '--run', 'quota',
                                        '--i-own-disposable-host', '--private-evidence-dir', evidence])
        self.assertEqual(code, 0)
        self.assertFalse(report['productionQualified'])
        for row in report['records']:
            if row['test'] in q.UNIMPLEMENTED:
                self.assertEqual(row['status'], 'NOT_RUN' if row['test'] == 'physical_airdrop'
                                 else 'BLOCKED')

    def test_stale_boot_shared_uid_and_oversized_limits_fail(self):
        for changes in [dict(boot_id='old'), dict(backend_uid=1000), dict(guardian_uid=0),
                        dict(hard_bytes=128 * 1024 * 1024), dict(hard_inodes=0),
                        dict(disposable=False)]:
            with self.assertRaises(ValueError):
                q.validate_manifest(dict(self.config(), **changes), 'boot', 1000)

    def test_mount_reporting_omits_private_sources(self):
        record = q.mount_record('/private/quota',
            '1 0 8:1 / / rw - ext4 /dev/private-name rw\n'
            '2 1 8:2 / /private/quota rw,nosuid - xfs SECRET rw,prjquota,secret=value')
        self.assertEqual(record['filesystem'], 'xfs')
        self.assertIn('prjquota', record['options'])
        self.assertNotIn('SECRET', json.dumps(record))
        self.assertNotIn('secret', json.dumps(record))

    def test_no_symlink_or_parent_roots(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'target').mkdir()
            (root / 'link').symlink_to(root / 'target')
            for path in ('/', 'relative', str(root / 'link'), str(root / '..')):
                with self.assertRaises((OSError, ValueError)):
                    q.open_directory(path)

    def test_malformed_duplicate_and_private_manifest_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'record'
            path.write_text('{"schema":1,"schema":1}')
            with self.assertRaises(ValueError):
                q.read_json(path)
            path.unlink()
            path.symlink_to('/absent')
            with self.assertRaises(OSError):
                q.read_json(path)

    def test_bounded_runner_requires_test_count_and_completion_marker(self):
        for output, status in [('1 passed; 0 failed; 0 ignored;', 'FAILED'),
                               ('P019F_KERNEL_ASSERTIONS_COMPLETE', 'FAILED'),
                               ('1 passed; 0 failed; 0 ignored;\nP019F_KERNEL_ASSERTIONS_COMPLETE',
                                'KERNEL_PASS')]:
            command = [sys.executable, '-c', 'print(' + repr(output) + ')']
            self.assertEqual(q.bounded_test(command, os.environ.copy())['status'], status)

    def test_failed_and_timed_out_runner_preserves_uncertainty(self):
        result = q.bounded_test([sys.executable, '-c', 'raise SystemExit(7)'], os.environ.copy())
        self.assertEqual(result['status'], 'FAILED')
        result = q.bounded_test([sys.executable, '-c', 'import time; time.sleep(2)'],
                                os.environ.copy(), timeout=0.05)
        self.assertEqual(result['status'], 'FAILED')
        self.assertEqual(result['retention'], 'inspection_required')

    def test_raw_evidence_stays_private_and_public_report_is_sanitized(self):
        with tempfile.TemporaryFile() as evidence:
            result = q.bounded_test([sys.executable, '-c',
                'print("private/path/secret"); raise SystemExit(7)'],
                os.environ.copy(), evidence_fd=evidence)
            evidence.seek(0)
            self.assertIn(b'private/path/secret', evidence.read())
        self.assertNotIn('private', json.dumps(result))
        self.assertEqual(result['status'], 'FAILED')

    def test_resource_replacement_is_rejected_before_kernel_command(self):
        with tempfile.TemporaryDirectory() as original, tempfile.TemporaryDirectory() as changed:
            pinned = q.open_directory(original)
            config = self.config()
            config['cgroup']['path'] = changed
            try:
                with self.assertRaises(ValueError), patch.object(q, 'bounded_test') as runner:
                    q.run_suite('quota', config, [pinned], None)
                runner.assert_not_called()
            finally:
                os.close(pinned)

    def test_new_execution_id_and_status_allowlist(self):
        _, first = self.invoke([])
        _, second = self.invoke([])
        self.assertNotEqual(first['execution_id'], second['execution_id'])
        self.assertTrue(all(row['status'] in q.STATUSES for row in first['records']))
        self.assertEqual(first['authorization'], 'never_evaluated')
        self.assertEqual(first['physicalQualification'], 'not_qualified')

    def test_mock_execution_never_promotes_kernel_evidence(self):
        with patch.object(q, 'bounded_test', return_value={'status': 'MOCK_PASS'}):
            code, report = self.invoke(['--mock'])
        self.assertEqual(code, 0)
        self.assertEqual(report['records'][0]['status'], 'MOCK_PASS')
        self.assertNotIn('KERNEL_PASS', json.dumps(report))


if __name__ == '__main__':
    unittest.main()
