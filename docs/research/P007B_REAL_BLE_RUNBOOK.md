# P007B real BLE operator runbook

`authorization: never_evaluated`

Use an owned Linux controller and Watch. Every evidence row must have UTC time, stage ID, PASS/FAIL/BLOCKED/NOT RUN, expected/actual metadata and an evidence reference. Do not publish MAC addresses, D-Bus Device paths, host names, serial numbers, persistent UUIDs, signing IDs, session IDs, raw challenges or captures. Store unredacted working logs privately; inspect and redact before publication.

## Read-only preflight

1. Build with Rust 1.85: `cargo build --workspace`. Record Arch release (`/etc/os-release`), kernel release, BlueZ and controller firmware versions without host/controller identifiers. Privately run `uname -a`, `bluetoothctl --version`, `bluetoothctl list`, `systemctl --no-pager status bluetooth`, `btmgmt info`, and `scripts/qualify-bluez-host.sh`. These can expose persistent identifiers: do not publish their raw outputs. `python3 scripts/qualify-p007b-host.py` runs these bounded probes and emits only timestamp, kernel version and exit classifications. A successful probe command is not a hardware qualification.
2. Run `target/debug/opencontinuity research watch-ble --capabilities --json`. Privately inspect adapter inventory. Select the actual `hciN`; never assume hci0. Inspect Powered, GattManager1, LEAdvertisingManager1, SupportedInstances and ActiveInstances using the existing qualification script. Verify LE peripheral support in controller management output and firmware in host logs. Missing firmware information is UNKNOWN, not an inferred PASS.
3. Record bus, BlueZ owner, controller/firmware, powered state, peripheral role, GATT manager, advertising manager/capacity and permission eligibility independently. Introspection success does not prove registration permission. Do not power controllers, pair, alter policy, enable services or invoke sudo automatically.

## Explicit bounded trial

The operator must approve the selected adapter and 120-second trial before this step. Start `target/debug/opencontinuity research watch-gatt serve --adapter <actual-hciN> --duration-seconds 120 --experimental --json`. Preserve the terminal JSON privately, then redact persistent identifiers/errors before sharing. Record successful GATT and advertisement registration independently from cleanup and RF. Use a second owned BLE scanner to record the expected service UUID over air. Registration success alone is insufficient.

If BlueZ returns InvalidLength, retain its exact error privately and inspect actual advertisement composition, controller legacy/extended support, existing slots and a sanitized length budget. The current advertisement has a 128-bit UUID and `OC Research` name. Do not assume the controller's packing behavior from this source alone. Reproduce with a minimal service-only advertisement before proposing a tested, compatible remedy; P007B changes no advertisement or wire bytes without that evidence. Record policy errors verbatim in private logs and publish sanitized classifications; do not weaken D-Bus policy.

## Watch trial

Install using P007B_APPLE_SIGNED_INSTALL.md. Enable Diagnostics explicitly before each trial (disabled by default; turning off clears history). The display retains at most 64 allowlisted events; note observations as they occur because old events are evicted. Do not photograph the peripheral-name UI without redaction. The trace contains no raw values, persistent IDs, free-form errors or device names. Record wall-clock UTC separately. It records accepted callbacks and observed lengths, not underlying ATT packets.

Verify: launch; Bluetooth ready; no pre-action scan; explicit scan; advertisement visibility; selected connection; exact service and five characteristics; version 1; actual challenge length 25; frame count and each acknowledgement; Complete; actual response length 44; digest verdict; explicit disconnect and terminal cleanup. Each item needs separate evidence. Error callback/timeout records operation_failed; record the current stage and sanitized UI error classification. For cancellation and reconnect follow P007B_LONG_READ_AND_RECONNECT.md.

After server termination, inspect cleanup success and BlueZ object/advertisement removal. Test normal expiry and SIGINT/SIGTERM separately. BlueZ restart is an optional disruptive operator-controlled trial only on an owned isolated test host with explicit approval. Never infer immediate RF disconnect notification from P006A's conditional event-ordering behavior.

## Evidence template

| UTC | Stage | Hardware/OS/tool versions (redacted) | Expected / actual | Status | Evidence |
| --- | --- | --- | --- | --- | --- |
| fill | host / API / registration / RF / discovery / version / challenge / writes / Complete / result / cancel / reconnect / teardown | model and versions only | counts, lengths and classifications only | PASS/FAIL/BLOCKED/NOT RUN | sanitized log or manual observation |
