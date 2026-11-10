import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "p018_watch_trial_evidence.py"
spec = importlib.util.spec_from_file_location("p018", SCRIPT)
p018 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p018)


def example():
    return {"schema_version": 1, "trial_id": "a" * 32, "stage": "W3", "source": "mock_ci", "result": "pass", "authorization": "never_evaluated", "events": [
        {"sequence": 0, "elapsed_ms": 0, "phase": "start", "outcome": "attempted"},
        {"sequence": 1, "elapsed_ms": 1, "phase": "transport", "outcome": "complete"},
        {"sequence": 2, "elapsed_ms": 2, "phase": "cleanup", "outcome": "confirmed"},
        {"sequence": 3, "elapsed_ms": 3, "phase": "terminal", "outcome": "complete"}]}


class EvidenceTests(unittest.TestCase):
    def test_mock_never_qualifies(self):
        self.assertEqual(p018.validate(example())["physical_qualification"], "not_qualified")

    def test_bad_fields_and_order(self):
        for mutate in (lambda d: d.update(address="secret"),
                       lambda d: d["events"][1].update(sequence=8),
                       lambda d: d["events"][1].update(elapsed_ms=9),
                       lambda d: d["events"].pop(2),
                       lambda d: d["events"][-1].update(outcome="failed")):
            doc = copy.deepcopy(example())
            mutate(doc)
            with self.assertRaises(p018.InvalidEvidence):
                p018.validate(doc)

    def test_duplicate_and_secret_not_echoed(self):
        with tempfile.NamedTemporaryFile() as f:
            f.write(b'{"secret":"private-value","secret":1}')
            f.flush()
            result = subprocess.run([sys.executable, str(SCRIPT), f.name], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertNotIn("private-value", result.stdout + result.stderr)
        self.assertNotIn(f.name, result.stdout + result.stderr)

    def test_bounded_and_no_boolean_numbers(self):
        doc = example()
        doc["events"][0]["sequence"] = False
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate(doc)
        doc = example()
        doc["events"] *= 17
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate(doc)

    def test_invalid_categories_and_stage_phase(self):
        for key, value in (("source", []), ("stage", "W7"), ("result", "PASS"), ("authorization", "allowed")):
            doc = example()
            doc[key] = value
            with self.assertRaises(p018.InvalidEvidence):
                p018.validate(doc)
        doc = example()
        doc["stage"] = "W1"
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate(doc)

    def test_oversize_file_is_rejected(self):
        with tempfile.NamedTemporaryFile() as f:
            f.write(b" " * (p018.MAX_BYTES + 1))
            f.flush()
            result = subprocess.run([sys.executable, str(SCRIPT), f.name], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertNotIn(f.name, result.stdout + result.stderr)

    def test_all_provenance_stays_unqualified(self):
        for source in p018.SOURCES:
            doc = example()
            doc["source"] = source
            self.assertEqual(p018.validate(doc)["physical_qualification"], "not_qualified")


if __name__ == "__main__":
    unittest.main()
