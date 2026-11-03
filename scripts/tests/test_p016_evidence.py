import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'p016_evidence.py'
spec = importlib.util.spec_from_file_location('evidence', SCRIPT)
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.pre = {'schema_version': 1, 'system_bus': 'available', 'bluez_access': 'available',
                    'adapters': [{'index': 1, 'powered': True, 'transport_filter': True}]}
        common = dict(schema_version=1, source='bluez_le_discovery_events', authorization='never_evaluated', scan_session_id='a'*32)
        self.events = [dict(common, type='session', sequence=1, state='started', status='started', scan_started=True, requested_duration_seconds=1, adapter_index=1),
                       dict(common, type='device', sequence=2, id='scan-'+'a'*16+'-001', display_name='Peripheral', provenance='bluez_event_during_session', change='observed'),
                       dict(common, type='terminal', sequence=3, scan_requested=True, scan_started=True, scan_completed=True, requested_duration_seconds=1, actual_duration_ms=1000, adapter_index=1, status='completed', cleanup_status='complete', device_count=1, device_event_count=1, termination_reason='duration_elapsed')]

    def data(self, events=None):
        return b''.join(json.dumps(x).encode()+b'\n' for x in (self.events if events is None else events))

    def test_valid_summary_never_claims_physical_qualification(self):
        result = e.summarize(json.dumps(self.pre).encode(), self.data())
        self.assertEqual(result['independent_rf'], 'not_run')
        self.assertEqual(result['apple_watch_se2'], 'no_go')
        self.assertEqual(result['device_event_count'], 1)
        self.assertNotIn('Peripheral', json.dumps(result))
        self.assertNotIn('a'*16, json.dumps(result))

    def test_no_scan_defaults(self):
        result = e.summarize(json.dumps(self.pre).encode())
        self.assertEqual(result['discovery_session'], 'not_run')
        self.assertEqual(result['cleanup'], 'not_run')

    def test_mutations_fail_closed(self):
        cases = [(0, 'schema_version', 2), (1, 'sequence', 4), (1, 'scan_session_id', 'b'*32),
                 (2, 'device_count', 0), (2, 'device_event_count', 2), (2, 'cleanup_status', 'incomplete'),
                 (2, 'scan_completed', False), (2, 'actual_duration_ms', 0), (1, 'id', 'raw'),
                 (1, 'display_name', 'AA:BB:CC:DD:EE:FF'), (1, 'display_name', '/home/private'),
                 (1, 'display_name', '192.168.1.2'), (1, 'display_name', 'cookie=private'),
                 (2, 'authorization', 'authorized'), (2, 'termination_reason', 'ok'), (2, 'sequence', True), (2, 'adapter_index', True)]
        for index, key, value in cases:
            with self.subTest(key=key, value=value):
                events = copy.deepcopy(self.events); events[index][key] = value
                with self.assertRaises(ValueError): e.transcript(self.data(events))

    def test_full_record_budget(self):
        events = [self.events[0]]
        for i in range(512):
            device = dict(self.events[1], sequence=i+2, id='scan-'+'a'*16+f'-{i%100+1:03}',
                          display_name=f'Observation {i}', change='observed' if i < 100 else 'updated')
            events.append(device)
        events.append(dict(self.events[2], sequence=514, status='data_limit', scan_completed=False,
                           device_count=100, device_event_count=512, termination_reason='stream_or_input_limit'))
        result = e.transcript(self.data(events))
        self.assertEqual(result['record_count'], 514)
        self.assertEqual(result['device_count'], 100)
        self.assertEqual(result['device_event_count'], 512)

    def test_duplicate_keys_and_escaped_duplicate(self):
        for key in [b'schema_version', b'schema_\\u0076ersion']:
            data = self.data().replace(b'"schema_version": 1', b'"schema_version": 1,"'+key+b'":1', 1)
            with self.assertRaises(ValueError): e.transcript(data)

    def test_missing_partial_extra_terminal(self):
        for data in [self.data()[:-1], self.data(self.events[:-1]), self.data()+b'{}\n', self.data()[:-30]]:
            with self.assertRaises(ValueError): e.transcript(data)

    def test_limits_and_utf8(self):
        for data in [b'['*9+b']'*9, b'\xff', b'{}\n'*515, b'x'*2049+b'\n{}\n']:
            with self.assertRaises(ValueError): e.transcript(data)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'large'; path.write_bytes(b'x'*(e.MAX_BYTES+1))
            with self.assertRaises(ValueError): e.read_local(path)

    def test_nonregular_input_rejected_without_blocking(self):
        import os
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'pipe'; os.mkfifo(path)
            with self.assertRaises(ValueError): e.read_local(path)

    def test_preflight_schema_and_adapter_bounds(self):
        for key, value in [('schema_version', 2), ('schema_version', True), ('adapters', [{}]), ('adapters', [self.pre['adapters'][0]]*17), ('system_bus', 'secret')]:
            pre = dict(self.pre); pre[key] = value
            with self.assertRaises(ValueError): e.preflight(json.dumps(pre).encode())

    def test_cli_never_echoes_rejected_data_or_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'private-user'; path.write_text('{secret}')
            result = subprocess.run([sys.executable, str(SCRIPT), '--preflight', str(path)], capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 1)
            self.assertNotIn('private-user', result.stderr)
            self.assertNotIn('secret', result.stderr)
            self.assertEqual(result.stdout, '')
