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
    def host_export(self, trial):
        return {"schemaVersion": 1, "kind": "gatt_trial_diagnostics", "source": "host_runtime_observed", "trialId": trial, "authorization": "never_evaluated", "incomplete": False, "frameWriteCount": 2, "signedWriteCount": 0, "signedRequestFrameCount": 0, "signedResponseFrameCount": 0, "acknowledgementCount": 0, "events": [
            {"sequence": 0, "elapsedMs": 0, "phase": "start", "outcome": "attempted"},
            {"sequence": 1, "elapsedMs": 1, "phase": "adapter_ready", "outcome": "confirmed"},
            {"sequence": 2, "elapsedMs": 2, "phase": "application_registration", "outcome": "attempted"},
            {"sequence": 3, "elapsedMs": 3, "phase": "application_registration", "outcome": "confirmed"},
            {"sequence": 4, "elapsedMs": 4, "phase": "advertisement_registration", "outcome": "attempted"},
            {"sequence": 5, "elapsedMs": 5, "phase": "advertisement_registration", "outcome": "confirmed"},
            {"sequence": 6, "elapsedMs": 6, "phase": "session", "outcome": "confirmed"},
            {"sequence": 7, "elapsedMs": 7, "phase": "frame", "outcome": "observed"},
            {"sequence": 8, "elapsedMs": 8, "phase": "synthetic_exchange", "outcome": "confirmed"},
            {"sequence": 9, "elapsedMs": 9, "phase": "cleanup_advertisement", "outcome": "attempted"},
            {"sequence": 10, "elapsedMs": 10, "phase": "cleanup_advertisement", "outcome": "confirmed"},
            {"sequence": 11, "elapsedMs": 11, "phase": "cleanup_application", "outcome": "attempted"},
            {"sequence": 12, "elapsedMs": 12, "phase": "cleanup_application", "outcome": "confirmed"},
            {"sequence": 13, "elapsedMs": 13, "phase": "terminal", "outcome": "confirmed"}]}

    def watch_export(self, trial):
        return {"schemaVersion": 2, "kind": "diagnostic_trace", "authorization": "never_evaluated", "trialId": trial, "source": "physical", "incomplete": False, "aggregates": {}, "samples": [], "events": [
            {"sequence": 0, "elapsedMs": 0, "event": "scan_started"},
            {"sequence": 1, "elapsedMs": 1, "event": "connected"},
            {"sequence": 2, "elapsedMs": 2, "event": "synthetic_digest_verified"},
            {"sequence": 3, "elapsedMs": 3, "event": "disconnected"},
            {"sequence": 4, "elapsedMs": 4, "event": "session_terminal"}]}

    def test_actual_host_watch_exports_correlate_only_by_label(self):
        trial = "b" * 32
        report = p018.validate_pair(self.host_export(trial), self.watch_export(trial))
        self.assertTrue(report["shared_phases"]["P006_exchange_observed_both_sides"])
        self.assertEqual(report["physical_qualification"], "not_qualified")
        self.assertEqual(report["authorization"], "never_evaluated")
        self.assertIn("not authenticated", report["pairing_basis"])
        self.assertEqual(report["host_provenance"], "user_supplied_host_export_unverified")
        self.assertEqual(report["watch_provenance"], "user_supplied_watch_export_unverified")

    def test_pair_rejects_mismatched_ids_and_unknown_fields(self):
        trial = "c" * 32
        host, watch = self.host_export(trial), self.watch_export("d" * 32)
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_pair(host, watch)

    def test_incomplete_host_export_can_report_unavailable_local_label(self):
        host = self.host_export("a" * 32)
        host["trialId"] = None
        host["incomplete"] = True
        self.assertIsNone(p018.validate_host_runtime(host)["trialId"])
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_pair(host, self.watch_export("a" * 32))
        host["incomplete"] = False
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_host_runtime(host)

    def test_pair_summary_marks_partial_and_differing_claims(self):
        trial = "a" * 32
        host, watch = self.host_export(trial), self.watch_export(trial)
        watch["events"] = [
            {"sequence": 0, "elapsedMs": 0, "event": "scan_started"},
            {"sequence": 1, "elapsedMs": 1, "event": "connected"},
            {"sequence": 2, "elapsedMs": 2, "event": "operation_failed"},
            {"sequence": 3, "elapsedMs": 3, "event": "disconnected"},
            {"sequence": 4, "elapsedMs": 4, "event": "session_terminal"},
        ]
        report = p018.validate_pair(host, watch)
        self.assertEqual(report["claim_alignment"]["P006"], "one_source_reports_success")
        with tempfile.NamedTemporaryFile(mode="w") as f:
            f.write(json.dumps(host))
            f.flush()
            result = subprocess.run([sys.executable, str(SCRIPT), "--host", f.name], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0)
        self.assertIn("valid_partial", result.stdout)
        self.assertIn("not_qualified", result.stdout)
        watch["trialId"] = trial
        watch["events"][0]["unexpected"] = "private"
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_pair(host, watch)

    def test_p009_response_and_decision_categories_are_reported_without_selecting_a_winner(self):
        trial = "9" * 32
        host = self.host_export(trial)
        host["frameWriteCount"] = 0
        host["signedWriteCount"] = 3
        host["signedRequestFrameCount"] = 2
        host["signedResponseFrameCount"] = 1
        host["acknowledgementCount"] = 1
        host["events"] = [
            {"sequence": 0, "elapsedMs": 0, "phase": "start", "outcome": "attempted"},
            {"sequence": 1, "elapsedMs": 1, "phase": "adapter_ready", "outcome": "confirmed"},
            {"sequence": 2, "elapsedMs": 2, "phase": "application_registration", "outcome": "attempted"},
            {"sequence": 3, "elapsedMs": 3, "phase": "application_registration", "outcome": "confirmed"},
            {"sequence": 4, "elapsedMs": 4, "phase": "advertisement_registration", "outcome": "attempted"},
            {"sequence": 5, "elapsedMs": 5, "phase": "advertisement_registration", "outcome": "confirmed"},
            {"sequence": 6, "elapsedMs": 6, "phase": "session", "outcome": "confirmed"},
            {"sequence": 7, "elapsedMs": 7, "phase": "signed_exchange", "outcome": "observed"},
            {"sequence": 8, "elapsedMs": 8, "phase": "consent_decision", "outcome": "rejected"},
            {"sequence": 9, "elapsedMs": 9, "phase": "cleanup_advertisement", "outcome": "attempted"},
            {"sequence": 10, "elapsedMs": 10, "phase": "cleanup_advertisement", "outcome": "confirmed"},
            {"sequence": 11, "elapsedMs": 11, "phase": "cleanup_application", "outcome": "attempted"},
            {"sequence": 12, "elapsedMs": 12, "phase": "cleanup_application", "outcome": "confirmed"},
            {"sequence": 13, "elapsedMs": 13, "phase": "terminal", "outcome": "confirmed"},
        ]
        watch = self.watch_export(trial)
        names = [
            "scan_started", "advertisement_discovered", "scan_stopped", "connecting",
            "connected", "service_discovered", "five_characteristics_verified",
            "protocol_version_verified=1", "signed_request_frame_count=2",
            "signed_request_frame_ack=0", "verified_request_accepted",
            "consent_approved", "signed_response_frame_count=1",
            "signed_response_frame_ack=0", "session_invalidated", "disconnected",
            "session_terminal",
        ]
        watch["events"] = [
            {"sequence": index, "elapsedMs": index, "event": name}
            for index, name in enumerate(names)
        ]
        summary = p018.validate_pair(host, watch)
        self.assertEqual(summary["claim_alignment"]["P009_decision"], "decision_categories_differ")
        self.assertTrue(summary["shared_phases"]["P009_response_delivery_observed_both_sides"])
        self.assertEqual(summary["physical_qualification"], "not_qualified")

    def test_pair_cli_returns_sanitized_summary(self):
        trial = "e" * 32
        with tempfile.TemporaryDirectory() as directory:
            host_path = Path(directory) / "host.json"
            watch_path = Path(directory) / "watch.json"
            host_path.write_text(json.dumps(self.host_export(trial)))
            watch_path.write_text(json.dumps(self.watch_export(trial)))
            result = subprocess.run([sys.executable, str(SCRIPT), "--host", str(host_path), "--watch", str(watch_path)], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0)
        self.assertNotIn(str(host_path), result.stdout + result.stderr)
        self.assertIn("not_qualified", result.stdout)

    def test_watch_partial_trace_and_aggregate_bounds(self):
        watch = self.watch_export("f" * 32)
        watch["incomplete"] = True
        watch["events"].pop()
        watch["aggregates"] = {"get_read_ack": {"count": 2, "totalMs": 3_700_000, "maximumMs": 3_600_000}}
        self.assertTrue(p018.validate_watch_runtime(watch)["incomplete"])
        watch["incomplete"] = False
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_watch_runtime(watch)

    def test_watch_rejects_wall_clock_samples_and_impossible_aggregates(self):
        watch = self.watch_export("1" * 32)
        watch["samples"] = [{"stage": "connection", "timestampUnixMs": 123, "elapsedMs": 4, "outcome": "PASS"}]
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_watch_runtime(watch)

    def test_watch_accepts_p009_response_after_consent_and_delayed_p006_ack(self):
        trial = "4" * 32
        watch = self.watch_export(trial)
        watch["events"] = [
            "scan_started", "advertisement_discovered", "scan_stopped", "connecting",
            "connected", "service_discovered", "five_characteristics_verified",
            "protocol_version_verified=1", "signed_request_frame_count=2",
            "signed_request_frame_ack=0", "verified_request_accepted",
            "local_authentication_succeeded", "consent_approved",
            "signed_response_frame_count=3", "signed_response_frame_ack=0",
            "signed_response_frame_ack=1", "session_invalidated", "disconnected",
            "session_terminal",
        ]
        watch["events"] = [
            {"sequence": index, "elapsedMs": index, "event": label}
            for index, label in enumerate(watch["events"])
        ]
        self.assertEqual(p018.validate_watch_runtime(watch)["source"], "physical")

        delayed = self.watch_export(trial)
        delayed["events"] = [
            {"sequence": 0, "elapsedMs": 0, "event": "scan_started"},
            {"sequence": 1, "elapsedMs": 1, "event": "connected"},
            {"sequence": 2, "elapsedMs": 2, "event": "frame_count=2"},
            {"sequence": 3, "elapsedMs": 3, "event": "cancel_requested"},
            {"sequence": 4, "elapsedMs": 4, "event": "frame_ack=1"},
            {"sequence": 5, "elapsedMs": 5, "event": "session_invalidated"},
            {"sequence": 6, "elapsedMs": 6, "event": "disconnected"},
            {"sequence": 7, "elapsedMs": 7, "event": "session_terminal"},
        ]
        self.assertEqual(p018.validate_watch_runtime(delayed)["trialId"], trial)
        watch = self.watch_export("2" * 32)
        watch["aggregates"] = {"connection": {"count": 1, "totalMs": 1, "maximumMs": 2}}
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_watch_runtime(watch)

    def test_host_rejects_cleanup_out_of_order_and_session_before_registration(self):
        host = self.host_export("3" * 32)
        host["events"][6]["phase"] = "session"
        host["events"][6]["sequence"] = 6
        host["events"][6]["elapsedMs"] = 6
        host["events"].pop(5)
        for index, event in enumerate(host["events"]):
            event["sequence"] = index
        with self.assertRaises(p018.InvalidEvidence):
            p018.validate_host_runtime(host)

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
