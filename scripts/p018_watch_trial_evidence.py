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
    return {"schema_version": 1, "validation": "valid", "stage": doc["stage"], "source": doc["source"], "claimed_result": doc["result"], "physical_qualification": "not_qualified", "authorization": "never_evaluated"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", help="explicit local JSON evidence file")
    args = parser.parse_args()
    try:
        with Path(args.evidence).open("rb") as stream:
            raw = stream.read(MAX_BYTES + 1)
        if len(raw) > MAX_BYTES:
            raise InvalidEvidence("file too large")
        doc = json.loads(raw, object_pairs_hook=pairs, parse_constant=lambda _: (_ for _ in ()).throw(InvalidEvidence("invalid number")))
        report = validate(doc)
    except (OSError, UnicodeError, ValueError, TypeError, RecursionError):
        print(json.dumps({"validation": "invalid", "authorization": "never_evaluated"}))
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
