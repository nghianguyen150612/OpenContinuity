#!/usr/bin/env python3
"""Validate explicitly supplied, sanitized P018 trial evidence offline.

This checker validates structure and consistency, not authenticity or hardware.
"""
import argparse
import json
import re
import sys
from pathlib import Path

MAX_BYTES = 128 * 1024
MAX_OUTPUT_BYTES = 4096
MAX_EVENTS = 64
STAGES = {f"W{i}" for i in range(7)}
SOURCES = {"mock_ci", "host_runtime_observed", "watch_runtime_observed", "operator_declared", "independent_observer"}
RESULTS = {"pass", "fail", "blocked", "not_run"}
PHASES = {"start": 0, "registration": 1, "discovery": 2, "transport": 3, "decision": 4, "cleanup": 5, "terminal": 6}
OUTCOMES = {"attempted", "confirmed", "failed", "cancelled", "expired", "unknown", "complete"}
ID = re.compile(r"[0-9a-f]{32}\Z")


class InvalidEvidence(ValueError):
    pass


def pairs(items):
    result = {}
    for key, value in items:
        if key in result:
            raise InvalidEvidence("duplicate field")
        result[key] = value
    return result


def fields(value, expected):
    if not isinstance(value, dict) or set(value) != expected:
        raise InvalidEvidence("invalid fields")


def validate(doc):
    fields(doc, {"schema_version", "trial_id", "stage", "source", "result", "authorization", "events"})
    if type(doc["schema_version"]) is not int or doc["schema_version"] != 1:
        raise InvalidEvidence("invalid schema")
    if not isinstance(doc["trial_id"], str) or not ID.fullmatch(doc["trial_id"]):
        raise InvalidEvidence("invalid trial identifier")
    if any(not isinstance(doc[key], str) for key in ("stage", "source", "result")):
        raise InvalidEvidence("invalid category")
    if doc["stage"] not in STAGES or doc["source"] not in SOURCES or doc["result"] not in RESULTS:
        raise InvalidEvidence("invalid category")
    if doc["authorization"] != "never_evaluated":
        raise InvalidEvidence("invalid authorization")
    events = doc["events"]
    if not isinstance(events, list) or len(events) > MAX_EVENTS:
        raise InvalidEvidence("invalid event count")
    if doc["result"] in {"pass", "fail"} and not events:
        raise InvalidEvidence("missing events")
    previous_phase, previous_time = -1, -1
    for index, event in enumerate(events):
        fields(event, {"sequence", "elapsed_ms", "phase", "outcome"})
        if type(event["sequence"]) is not int or event["sequence"] != index:
            raise InvalidEvidence("invalid sequence")
        elapsed = event["elapsed_ms"]
        if type(elapsed) is not int or not previous_time <= elapsed <= 3_600_000:
            raise InvalidEvidence("invalid elapsed time")
        phase = event["phase"]
        if not isinstance(phase, str) or phase not in PHASES or PHASES[phase] < previous_phase:
            raise InvalidEvidence("invalid phase order")
        if not isinstance(event["outcome"], str) or event["outcome"] not in OUTCOMES:
            raise InvalidEvidence("invalid outcome")
        if phase == "terminal" and index != len(events) - 1:
            raise InvalidEvidence("terminal must be last")
        previous_phase, previous_time = PHASES[phase], elapsed
    if events:
        if events[0]["phase"] != "start" or events[0]["outcome"] != "attempted" or events[-1]["phase"] != "terminal":
            raise InvalidEvidence("missing boundary")
        if not any(event["phase"] == "cleanup" for event in events):
            raise InvalidEvidence("missing cleanup")
        required_phase = {"W1": "registration", "W2": "discovery", "W3": "transport", "W4": "decision", "W5": "decision", "W6": "cleanup"}.get(doc["stage"])
        if doc["result"] == "pass" and required_phase and not any(e["phase"] == required_phase for e in events):
            raise InvalidEvidence("missing stage phase")
        if doc["result"] == "pass" and (events[-1]["outcome"] != "complete" or not any(e["phase"] == "cleanup" and e["outcome"] == "confirmed" for e in events)):
            raise InvalidEvidence("inconsistent completion")
        if doc["result"] == "fail" and events[-1]["outcome"] not in {"failed", "cancelled", "expired", "unknown"}:
            raise InvalidEvidence("inconsistent failure")
    return {"schema_version": 1, "validation": "valid", "stage": doc["stage"], "source": doc["source"], "provenance": "user_supplied_record_unverified", "claimed_result": doc["result"], "physical_qualification": "not_qualified", "authorization": "never_evaluated"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    if len(sys.argv) > 1 and sys.argv[1] in {"--host", "--watch"}:
        pair_result = _main_pair(sys.argv[1:])
        if pair_result is not None:
            return pair_result
    parser.add_argument("evidence", help="explicit local JSON evidence file")
    args = parser.parse_args()
    try:
        doc = load_json(args.evidence)
        report = validate(doc)
    except (OSError, UnicodeError, ValueError, TypeError, RecursionError):
        print(json.dumps({"validation": "invalid", "authorization": "never_evaluated"}))
        return 1
    emit(report)
    return 0


def load_json(path):
    with Path(path).open("rb") as stream:
        raw = stream.read(MAX_BYTES + 1)
    return parse_json(raw)


def load_host_json(path):
    with Path(path).open("rb") as stream:
        raw = stream.read(MAX_BYTES + 1)
    # Host diagnostics are a single JSONL write. Requiring the final newline
    # rejects a partial non-pipe write that happened to stop after valid JSON.
    if not raw.endswith(b"\n") or raw.count(b"\n") != 1:
        raise InvalidEvidence("incomplete host line")
    return parse_json(raw)


def parse_json(raw):
    if len(raw) > MAX_BYTES:
        raise InvalidEvidence("file too large")
    text = raw.decode("utf-8", errors="strict")
    return json.loads(text, object_pairs_hook=pairs, parse_constant=lambda _: (_ for _ in ()).throw(InvalidEvidence("invalid number")))


def emit(report):
    output = json.dumps(report, sort_keys=True, separators=(",", ":"))
    if len(output.encode("utf-8")) > MAX_OUTPUT_BYTES:
        print(json.dumps({"validation": "invalid", "authorization": "never_evaluated"}))
    else:
        print(output)


def _strict_int(value, maximum):
    return type(value) is int and 0 <= value <= maximum


def validate_host_runtime(doc):
    fields(doc, {"schemaVersion", "kind", "source", "trialId", "authorization", "incomplete", "frameWriteCount", "signedWriteCount", "signedRequestFrameCount", "signedResponseFrameCount", "acknowledgementCount", "events"})
    if type(doc["schemaVersion"]) is not int or doc["schemaVersion"] != 1 or doc["kind"] != "gatt_trial_diagnostics" or doc["source"] != "host_runtime_observed" or doc["authorization"] != "never_evaluated":
        raise InvalidEvidence("invalid host record")
    if doc["trialId"] is None:
        if not doc["incomplete"]:
            raise InvalidEvidence("missing host trial label")
    elif not isinstance(doc["trialId"], str) or not ID.fullmatch(doc["trialId"]):
        raise InvalidEvidence("invalid host trial")
    if type(doc["incomplete"]) is not bool or any(not _strict_int(doc[key], 65535) for key in ("frameWriteCount", "signedWriteCount", "signedRequestFrameCount", "signedResponseFrameCount", "acknowledgementCount")):
        raise InvalidEvidence("invalid host metadata")
    events = doc["events"]
    if not isinstance(events, list) or not 1 <= len(events) <= 16:
        raise InvalidEvidence("invalid host event count")
    phases = {"start": 0, "adapter_ready": 1, "application_registration": 2, "advertisement_registration": 3, "session": 4, "frame": 5, "signed_exchange": 6, "signed_request_read": 6, "consent_decision": 7, "synthetic_exchange": 7, "cancellation": 8, "expiration": 8, "disconnect": 8, "failure": 8, "cleanup_advertisement": 9, "cleanup_application": 10, "terminal": 11}
    outcomes = {"attempted", "confirmed", "failed", "observed", "cancelled", "expired", "unknown", "approved", "rejected"}
    elapsed, phase = -1, -1
    previous_phase = None
    registration = {"application_registration": None, "advertisement_registration": None}
    cleanup = {"cleanup_advertisement": None, "cleanup_application": None}
    cleanup_started = False
    session_seen = False
    adapter_ready = False
    for index, event in enumerate(events):
        fields(event, {"sequence", "elapsedMs", "phase", "outcome"})
        if not _strict_int(event["sequence"], 63) or event["sequence"] != index or not _strict_int(event["elapsedMs"], 3_600_000) or event["elapsedMs"] < elapsed:
            raise InvalidEvidence("invalid host sequence")
        next_phase = phases.get(event["phase"]) if isinstance(event["phase"], str) else None
        if next_phase is None:
            raise InvalidEvidence("invalid host phase")
        phase_name = event["phase"]
        outcome = event["outcome"]
        if not isinstance(outcome, str) or outcome not in outcomes:
            raise InvalidEvidence("invalid host outcome")
        allowed_outcomes = {
            "start": {"attempted"}, "adapter_ready": {"confirmed"},
            "application_registration": {"attempted", "confirmed", "failed"},
            "advertisement_registration": {"attempted", "confirmed", "failed"},
            "session": {"confirmed"}, "frame": {"observed", "failed"},
            "signed_exchange": {"observed", "failed"}, "synthetic_exchange": {"confirmed"},
            "signed_request_read": {"failed"},
            "consent_decision": {"approved", "rejected"}, "cancellation": {"cancelled"},
            "expiration": {"expired"}, "disconnect": {"observed"}, "failure": {"failed"},
            "cleanup_advertisement": {"attempted", "confirmed", "failed", "unknown"},
            "cleanup_application": {"attempted", "confirmed", "failed", "unknown"},
            "terminal": {"confirmed", "failed", "unknown"},
        }
        if outcome not in allowed_outcomes[phase_name]:
            raise InvalidEvidence("invalid host phase outcome")
        if phase_name.startswith("cleanup_"):
            cleanup_started = True
        elif cleanup_started and phase_name != "terminal":
            raise InvalidEvidence("host event after cleanup began")
        if phase_name == "session" and phase > 4:
            if previous_phase not in {"synthetic_exchange", "cancellation", "expiration", "disconnect", "failure", "session"}:
                raise InvalidEvidence("host session restarted during an active phase")
            phase = 3
        if next_phase < phase:
            raise InvalidEvidence("invalid host phase")
        if phase_name == "adapter_ready":
            adapter_ready = True
        elif phase_name in {"application_registration", "advertisement_registration"}:
            state = registration[phase_name]
            if outcome == "attempted":
                if state is not None:
                    raise InvalidEvidence("duplicate host registration attempt")
                if not adapter_ready:
                    raise InvalidEvidence("host registration before adapter readiness")
                if phase_name == "advertisement_registration" and registration["application_registration"] != "confirmed":
                    raise InvalidEvidence("advertisement attempted before application registration")
                registration[phase_name] = "attempted"
            elif outcome in {"confirmed", "failed"}:
                if state != "attempted":
                    raise InvalidEvidence("host registration reply without attempt")
                registration[phase_name] = outcome
            else:
                raise InvalidEvidence("invalid host registration outcome")
        elif phase_name == "session":
            if registration["application_registration"] != "confirmed" or registration["advertisement_registration"] != "confirmed":
                raise InvalidEvidence("host session before both registrations")
            session_seen = True
        elif phase_name in {"frame", "synthetic_exchange", "signed_exchange", "signed_request_read", "signed_response_frame", "acknowledgement", "consent_decision"} and not session_seen:
            raise InvalidEvidence("host transport event before session")
        elif phase_name.startswith("cleanup_"):
            state = cleanup[phase_name]
            if outcome == "attempted":
                if state is not None:
                    raise InvalidEvidence("duplicate host cleanup attempt")
                if phase_name == "cleanup_application" and cleanup["cleanup_advertisement"] == "attempted":
                    raise InvalidEvidence("application cleanup before advertisement cleanup reply")
                cleanup[phase_name] = "attempted"
            elif outcome in {"confirmed", "failed"}:
                if state != "attempted":
                    raise InvalidEvidence("host cleanup reply without attempt")
                cleanup[phase_name] = outcome
            elif outcome == "unknown" and state is None:
                cleanup[phase_name] = "unknown"
            else:
                raise InvalidEvidence("invalid host cleanup outcome")
        if phase_name == "terminal" and index != len(events) - 1:
            raise InvalidEvidence("host terminal must be last")
        previous_phase, phase, elapsed = phase_name, phases[phase_name], event["elapsedMs"]
    if events[0]["phase"] != "start" or events[0]["outcome"] != "attempted" or events[-1]["phase"] != "terminal":
        raise InvalidEvidence("missing host boundaries")
    if not doc["incomplete"] and (cleanup["cleanup_advertisement"] == "attempted" or cleanup["cleanup_application"] == "attempted"):
        raise InvalidEvidence("host cleanup attempt has no result")
    if not doc["incomplete"]:
        if "attempted" in registration.values():
            raise InvalidEvidence("host registration attempt has no result")
        if registration["application_registration"] == "confirmed" and cleanup["cleanup_application"] not in {"confirmed", "failed", "unknown"}:
            raise InvalidEvidence("owned application cleanup state missing")
        if registration["advertisement_registration"] == "confirmed" and cleanup["cleanup_advertisement"] not in {"confirmed", "failed", "unknown"}:
            raise InvalidEvidence("owned advertisement cleanup state missing")
    terminal_outcome = events[-1]["outcome"]
    if not doc["incomplete"] and terminal_outcome == "confirmed" and any(
        registration[key] == "confirmed" and cleanup[cleanup_key] != "confirmed"
        for key, cleanup_key in (("application_registration", "cleanup_application"), ("advertisement_registration", "cleanup_advertisement"))
    ):
        raise InvalidEvidence("host confirmed terminal without confirmed owned cleanup")
    if terminal_outcome == "failed" and not any(
        event["outcome"] == "failed"
        for event in events
    ) and not doc["incomplete"]:
        raise InvalidEvidence("host failed terminal without a failure event")
    phases_present = {event["phase"] for event in events}
    if not doc["incomplete"]:
        if doc["frameWriteCount"] and "frame" not in phases_present:
            raise InvalidEvidence("host frame count without frame event")
        if doc["signedWriteCount"] and "signed_exchange" not in phases_present:
            raise InvalidEvidence("host signed count without signed event")
        if doc["acknowledgementCount"] and "signed_exchange" not in phases_present:
            raise InvalidEvidence("host acknowledgement count without signed event")
        if (doc["signedRequestFrameCount"] or doc["signedResponseFrameCount"] or doc["acknowledgementCount"]) and "session" not in phases_present:
            raise InvalidEvidence("host P009 counts without a session")
        if any(event["phase"] == "synthetic_exchange" and event["outcome"] == "confirmed" for event in events) and not doc["frameWriteCount"]:
            raise InvalidEvidence("host synthetic completion without a frame")
    return doc


WATCH_EVENT_NAMES = {
    "scan_started", "scan_stopped", "advertisement_discovered", "connecting", "connected",
    "service_discovered", "five_characteristics_verified", "session_invalidated", "disconnected",
    "challenge_length", "response_length", "frame_count", "frame_ack", "protocol_version_verified",
    "control_write_acknowledged", "synthetic_digest_verified", "cancel_requested", "deadline_expired",
    "operation_failed", "stale_callback_rejected", "verified_request_accepted", "request_validation_failed",
    "session_terminal", "consent_approved", "consent_rejected", "consent_cancelled", "consent_failed",
    "signed_request_frame_count", "signed_response_frame_count", "signed_request_frame_ack", "signed_response_frame_ack",
    "local_authentication_succeeded", "local_authentication_failed", "local_authentication_unsupported",
}
WATCH_STAGES = {"discovery", "connection", "services", "write_limit", "request_frames", "request_delivery", "get_read_ack", "write_ack", "host_signature", "consent", "local_authentication", "response_delivery", "cleanup", "failure", "challenge_read", "result_read", "request_read"}


def validate_watch_runtime(doc):
    fields(doc, {"schemaVersion", "kind", "authorization", "trialId", "source", "incomplete", "aggregates", "samples", "events"})
    if type(doc["schemaVersion"]) is not int or doc["schemaVersion"] != 2 or doc["kind"] != "diagnostic_trace" or doc["authorization"] != "never_evaluated":
        raise InvalidEvidence("invalid Watch record")
    if not isinstance(doc["source"], str) or doc["source"] not in {"physical", "simulated"} or not isinstance(doc["trialId"], str) or not ID.fullmatch(doc["trialId"]):
        raise InvalidEvidence("invalid Watch source")
    if type(doc["incomplete"]) is not bool:
        raise InvalidEvidence("invalid Watch completeness")
    aggregates = doc["aggregates"]
    if not isinstance(aggregates, dict) or len(aggregates) > len(WATCH_STAGES) or not set(aggregates).issubset(WATCH_STAGES):
        raise InvalidEvidence("invalid Watch aggregates")
    for aggregate in aggregates.values():
        fields(aggregate, {"count", "totalMs", "maximumMs"})
        if not _strict_int(aggregate["count"], 65535) or not _strict_int(aggregate["totalMs"], 65535 * 3_600_000) or not _strict_int(aggregate["maximumMs"], 3_600_000):
            raise InvalidEvidence("invalid Watch aggregate")
        if aggregate["maximumMs"] > aggregate["totalMs"] or (aggregate["count"] == 0 and (aggregate["totalMs"] or aggregate["maximumMs"])) or aggregate["totalMs"] > aggregate["count"] * 3_600_000:
            raise InvalidEvidence("inconsistent Watch aggregate")
    samples = doc["samples"]
    if not isinstance(samples, list) or len(samples) > MAX_EVENTS:
        raise InvalidEvidence("invalid Watch sample count")
    for sample in samples:
        allowed = {"stage", "elapsedMs", "outcome", "expected", "actual"}
        if not isinstance(sample, dict) or not {"stage", "elapsedMs", "outcome"}.issubset(sample) or not set(sample).issubset(allowed):
            raise InvalidEvidence("invalid Watch sample")
        if not isinstance(sample["stage"], str) or sample["stage"] not in WATCH_STAGES or not isinstance(sample["outcome"], str) or sample["outcome"] not in {"PASS", "FAIL", "BLOCKED", "NOT RUN"}:
            raise InvalidEvidence("invalid Watch sample category")
        if not _strict_int(sample["elapsedMs"], 3_600_000):
            raise InvalidEvidence("invalid Watch sample timing")
        for key in ("expected", "actual"):
            if key in sample and not _strict_int(sample[key], 65535):
                raise InvalidEvidence("invalid Watch sample count")
    events = doc["events"]
    if not isinstance(events, list) or not 1 <= len(events) <= MAX_EVENTS:
        raise InvalidEvidence("invalid Watch events")
    elapsed = -1
    terminal_names = {"session_terminal"}
    phase_order = {
        "scan_started": 0, "scan_stopped": 1, "advertisement_discovered": 1,
        "connecting": 2, "connected": 2, "service_discovered": 3,
        "five_characteristics_verified": 3, "protocol_version_verified": 4,
        "challenge_length": 4, "response_length": 4, "frame_count": 4,
        "frame_ack": 4, "control_write_acknowledged": 4,
        "signed_request_frame_count": 4, "signed_request_frame_ack": 4,
        "signed_response_frame_count": 5, "signed_response_frame_ack": 5,
        "synthetic_digest_verified": 4, "verified_request_accepted": 5,
        "consent_approved": 5, "consent_rejected": 5,
        "consent_cancelled": 5, "consent_failed": 5,
        "local_authentication_succeeded": 5, "local_authentication_failed": 5,
        "local_authentication_unsupported": 5,
        "request_validation_failed": 5, "cancel_requested": 4,
        "deadline_expired": 5, "operation_failed": 5,
        "session_invalidated": 6, "disconnected": 6, "session_terminal": 7,
    }
    current_phase = 0
    saw_scan = False
    active_scan = False
    terminal_seen = False
    for index, event in enumerate(events):
        fields(event, {"sequence", "elapsedMs", "event"})
        if not _strict_int(event["sequence"], 63) or event["sequence"] != index or not _strict_int(event["elapsedMs"], 3_600_000) or event["elapsedMs"] < elapsed:
            raise InvalidEvidence("invalid Watch sequence")
        if not isinstance(event["event"], str):
            raise InvalidEvidence("invalid Watch event")
        name = event["event"].split(" ", 1)[0].split("=", 1)[0]
        if name not in WATCH_EVENT_NAMES:
            raise InvalidEvidence("unknown Watch event")
        if name == "scan_started":
            if active_scan and not doc["incomplete"]:
                raise InvalidEvidence("Watch session restarted before terminal")
            current_phase, saw_scan = 0, True
            active_scan, terminal_seen = True, False
        elif terminal_seen:
            if name in {"session_invalidated", "stale_callback_rejected"}:
                pass
            else:
                raise InvalidEvidence("Watch event after terminal without a new scan")
        elif name == "session_invalidated" and not saw_scan:
            current_phase = 0
        elif name != "stale_callback_rejected":
            next_phase = phase_order[name]
            if next_phase < current_phase:
                raise InvalidEvidence("invalid Watch phase order")
            current_phase = next_phase
        # Numeric event values have exact forms and are deliberately bounded.
        if name in {"challenge_length", "response_length"}:
            match = re.fullmatch(r"(?:challenge_length expected=25 actual=([0-9]{1,5})|response_length expected=44_or_11 actual=([0-9]{1,5}))", event["event"])
            if not match:
                raise InvalidEvidence("invalid Watch length event")
        elif name in {"frame_count", "frame_ack"}:
            match = re.fullmatch(r"(?:frame_count|frame_ack)=([0-9]{1,5})", event["event"])
            if not match or int(match.group(1)) > 65535:
                raise InvalidEvidence("invalid Watch frame event")
        elif name in {"signed_request_frame_count", "signed_response_frame_count", "signed_request_frame_ack", "signed_response_frame_ack"}:
            match = re.fullmatch(r"(?:signed_request_frame_count|signed_response_frame_count|signed_request_frame_ack|signed_response_frame_ack)=([0-9]{1,5})", event["event"])
            if not match or int(match.group(1)) > 65535:
                raise InvalidEvidence("invalid Watch signed frame event")
        elif name == "protocol_version_verified":
            if event["event"] != "protocol_version_verified=1":
                raise InvalidEvidence("invalid Watch version event")
        elif event["event"] != name:
            raise InvalidEvidence("invalid Watch event label")
        if name in terminal_names:
            if terminal_seen:
                raise InvalidEvidence("duplicate Watch terminal")
            terminal_seen = True
            active_scan = False
        elapsed = event["elapsedMs"]
    if not events or (not doc["incomplete"] and (not terminal_seen or events[-1]["event"] != "session_terminal")):
        raise InvalidEvidence("missing Watch terminal")
    return doc


def validate_pair(host, watch):
    host = validate_host_runtime(host)
    watch = validate_watch_runtime(watch)
    if host["trialId"] is None or host["trialId"] != watch["trialId"]:
        raise InvalidEvidence("mismatched trial labels")
    watch_events = {e["event"].split(" ", 1)[0].split("=", 1)[0] for e in watch["events"]}
    host_p006 = any(e["phase"] == "synthetic_exchange" and e["outcome"] == "confirmed" for e in host["events"])
    watch_p006 = "synthetic_digest_verified" in watch_events
    p006_alignment = (
        "both_sources_report_success" if host_p006 and watch_p006
        else "one_source_reports_success" if host_p006 != watch_p006
        else "no_bilateral_success_evidence"
    )
    host_decision = next((e["outcome"] for e in host["events"] if e["phase"] == "consent_decision"), None)
    watch_decisions = watch_events & {"consent_approved", "consent_rejected", "consent_cancelled", "consent_failed"}
    p009_alignment = (
        "decision_categories_agree" if host_decision and len(watch_decisions) == 1 and next(iter(watch_decisions)) == f"consent_{host_decision}"
        else "decision_categories_differ" if host_decision and watch_decisions
        else "one_side_only" if host_decision or watch_decisions
        else "no_decision_observed"
    )
    host_response = host["signedResponseFrameCount"] > 0
    watch_response = "signed_response_frame_count" in watch_events and "signed_response_frame_ack" in watch_events
    return {
        "schema_version": 1,
        "validation": "valid_partial" if host["incomplete"] or watch["incomplete"] else "valid_pair_structure",
        "trial_id": host["trialId"],
        "host_source": host["source"],
        "host_provenance": "user_supplied_host_export_unverified",
        "watch_source_claim": watch["source"],
        "watch_provenance": "user_supplied_watch_export_unverified",
        "host_incomplete": host["incomplete"],
        "watch_incomplete": watch["incomplete"],
        "shared_phases": {
            "P006_exchange_observed_both_sides": host_p006 and watch_p006,
            "P009_request_and_consent_observed_on_watch": "verified_request_accepted" in watch_events and bool(watch_events & {"consent_approved", "consent_rejected", "consent_cancelled", "consent_failed"}),
            "P009_response_delivery_observed_both_sides": host_response and watch_response,
        },
        "claim_alignment": {"P006": p006_alignment, "P009_decision": p009_alignment},
        "pairing_basis": "matching local trial label only; endpoint communication is not authenticated",
        "physical_qualification": "not_qualified",
        "authorization": "never_evaluated",
    }


def _main_pair(argv):
    if not argv or argv[0] not in {"--host", "--watch"}:
        return None
    if len(argv) not in {2, 4}:
        emit({"validation": "invalid", "authorization": "never_evaluated"})
        return 1
    try:
        if len(argv) == 2:
            if argv[0] == "--host":
                record = validate_host_runtime(load_host_json(argv[1]))
                source = "host_runtime_observed"
            else:
                record = validate_watch_runtime(load_json(argv[1]))
                source = "watch_runtime_observed" if record["source"] == "physical" else "mock_ci"
            result = {
                "schema_version": 1,
                "validation": "valid_partial",
                "available_source": source,
                "source_claim": record["source"],
                "provenance": "user_supplied_runtime_export_unverified",
                "trial_id": record["trialId"],
                "missing_source": "watch_runtime_observed" if source == "host_runtime_observed" else "host_runtime_observed",
                "physical_qualification": "not_qualified",
                "authorization": "never_evaluated",
            }
        elif argv[0] == "--host" and argv[2] == "--watch":
            result = validate_pair(load_host_json(argv[1]), load_json(argv[3]))
        elif argv[0] == "--watch" and argv[2] == "--host":
            result = validate_pair(load_host_json(argv[3]), load_json(argv[1]))
        else:
            raise InvalidEvidence("invalid options")
    except (OSError, UnicodeError, ValueError, TypeError, RecursionError):
        emit({"validation": "invalid", "authorization": "never_evaluated"})
        return 1
    emit(result)
    return 0


if __name__ == "__main__":
    sys.exit(main())
