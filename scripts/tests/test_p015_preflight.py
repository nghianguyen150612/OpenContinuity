import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("preflight", Path(__file__).parents[1] / "p015_arch_preflight.py")
preflight = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preflight)


class PreflightTests(unittest.TestCase):
    def test_sanitized_read_only_adapter_queries(self):
        calls = []
        def run(args):
            calls.append(args)
            if "tree" in args:
                return "/org/bluez/hci0\n/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF\n"
            if "get-property" in args:
                return "b true\n"
            if "call" in args:
                return 'as 1 "Transport"\n'
            if "list" in args:
                return "org.bluez 123 bluetoothd root :1.2 bluetooth.service - -\n"
            return ""
        with patch.object(preflight, "run", side_effect=run), patch.object(preflight.shutil, "which", return_value=None):
            report = preflight.preflight()
        self.assertEqual(report["adapters"], [{"index": 1, "powered": True, "transport_filter": True}])
        self.assertNotIn("/org/bluez", str(report))
        self.assertTrue(all(not any(x in a for x in ("StartDiscovery", "SetDiscoveryFilter", "StopDiscovery")) for a in calls))
        self.assertTrue(all(":1.2" in a for a in calls if any(x in a for x in ("tree", "get-property", "call"))))

    def test_unavailable_bus_is_not_qualified(self):
        with patch.object(preflight, "run", return_value=None), patch.object(preflight.shutil, "which", return_value=None):
            report = preflight.preflight()
        self.assertEqual(report["system_bus"], "blocked_or_unavailable")
        self.assertEqual(report["independent_rf"], "not_run")

    def test_activatable_unowned_bluez_is_not_started(self):
        with patch.object(preflight, "run", return_value="org.bluez - - - (activatable) - - -") as run, patch.object(preflight.shutil, "which", return_value=None):
            report = preflight.preflight()
        self.assertEqual(report["bluez_access"], "blocked_or_unavailable")
        self.assertFalse(any("tree" in call.args[0] for call in run.call_args_list))
