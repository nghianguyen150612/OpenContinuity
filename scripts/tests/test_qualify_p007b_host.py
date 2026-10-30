import importlib.util
import pathlib
import subprocess
import unittest

spec = importlib.util.spec_from_file_location("qualification", pathlib.Path(__file__).parents[1] / "qualify-p007b-host.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProbeTests(unittest.TestCase):
    def test_selected_adapter_is_explicit_and_validated(self):
        results = module.adapter_probes(None)
        self.assertTrue(all(row['status'] == 'BLOCKED' for row in results.values()))
        for adapter in ('hci0;power on', '../hci0', 'hci', 'hci12345'):
            with self.assertRaises(ValueError):
                module.adapter_probes(adapter)

    def test_raw_identifiers_never_returned(self):
        result = subprocess.CompletedProcess([], 1, b"AA:BB:CC:DD:EE:FF secret", b"private device")
        self.assertEqual(module.probe(["probe"], which=lambda _: True, run=lambda *a, **k: result),
                         {"status": "FAIL", "exit_code": 1})

    def test_missing_command_and_timeout(self):
        self.assertEqual(module.probe(["absent"], which=lambda _: None)["status"], "BLOCKED")
        def timeout(*args, **kwargs):
            raise subprocess.TimeoutExpired("probe", 5)
        self.assertEqual(module.probe(["probe"], which=lambda _: True, run=timeout)["reason"], "probe_timeout")


if __name__ == "__main__":
    unittest.main()
