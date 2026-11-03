# P016 evidence schema and privacy

Run only with explicit local files:

```sh
python3 scripts/p015_arch_preflight.py > private-preflight.json
python3 scripts/p016_evidence.py --preflight private-preflight.json
python3 scripts/p016_evidence.py --preflight private-preflight.json --transcript private-existing-scan.jsonl
```

The validator never starts a process, connects to a network, uploads data or
invokes discovery. P015 preflight now emits schema_version 1; legacy unversioned
P015 files are accepted as v1. Other versions are rejected. Summary schema v1
contains authorization=never_evaluated, separate host_environment,
bluez_readiness, discovery_session, stream_delivery, cleanup, independent_rf,
wayland_gui, apple_watch_se2 and (with a transcript) terminal_status,
device_count, device_event_count, record_count.

Inputs are at most 512 KiB each, JSON depth at most 8 (a conservative lexical
bound), 514 transcript records and 2048 bytes per record including LF. Require
strict UTF-8 JSON, no duplicate or escaped-equivalent duplicate keys, contiguous
sequence, common session, exact field sets, sequential ephemeral observation
IDs, changed updates, <=128 devices, <=512 device events, duration 1–30 seconds,
consistent terminal state, cleanup and counts. Reject trailing partial records,
missing terminals and post-terminal data. Names are limited to 80 printable
characters and rejected for address-like patterns, paths, email/IP-like values
and labeled secret values. Invalid-input errors never echo input or paths.

Only fixed vocabulary and bounded counts are emitted. All environment strings,
local usernames, versions, paths, labels, identifiers and unrecognized preflight
metadata are omitted. Arbitrary names can contain serials or secrets that no
pattern can recognize; omission from output is the privacy boundary, not a
claim of universal sensitive-string detection. Do not publish raw inputs.

A valid file gives complete_transcript_exit_unverified; it cannot verify the
CLI exit status, file authenticity, physical origin, RF, identity or proximity.
Cleanup is reported_complete, never independently confirmed. Host readiness is
reported, not attested. Independent RF and Wayland remain not_run and Watch SE 2
remains no_go regardless of input claims. Keep a separate reviewed operator
attestation for hardware work: software revision, bounded action, controlled
peripheral, independent observer and limitations, without identifiers. This
validator deliberately cannot upgrade physical gates from an attestation field.
