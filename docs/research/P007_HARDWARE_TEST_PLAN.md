# P007 hardware test plan

`authorization: never_evaluated`

Record each item independently as PASS, FAIL, BLOCKED or NOT RUN. Attach tool versions, logs with secrets removed, adapter identity, Watch model/OS, and timestamps. A mocked D-Bus test is not evidence of RF or physical interoperability.

## Linux host

1. Run `scripts/qualify-bluez-host.sh` and retain read-only JSON. Record system bus, BlueZ version, adapter presence/power, GATT manager and LE advertising manager separately.
2. With operator approval, start `opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental --json` (duration must be 1–300 seconds). Record GATT registration and teardown.
3. Observe the advertisement from an independent BLE scanner. Record RF visibility, exact service UUID and scan time.
4. Connect from the Watch and separately record service discovery, all five characteristics/properties, version, complete 25-byte challenge, ordered frame writes, control command, complete result, session and digest checks.
5. Test cancellation after an acknowledged partial frame, disconnect during transfer, reconnect with a fresh ID, operation timeout, repeated trial, server expiry and clean teardown. Never infer that every physical disconnect will be signaled instantaneously; P006A handles observed D-Bus disconnect signals in its defined ordering.

## Apple Watch SE 2

1. Record Xcode, Swift, watchOS SDK, Watch OS and iPhone OS versions; install a signed experimental Watch app and accept its Bluetooth permission.
2. Verify no scan occurs before explicit Start Scan. Test powered-off, unauthorized, scan timeout, selection, connection failure, explicit disconnect and reconnect.
3. Verify exact service/characteristics and properties, version byte, and full challenge (25 bytes). A partial long read is a failure.
4. Send at least two frames; verify writes are sequential and acknowledged, then verify Complete/result (44 bytes). Separately test Cancel/status 2/length 9/digest. A partial result is a failure.
5. Test wrong version/session/digest where a controllable fixture peripheral is available. Test disconnect mid-write, timeout, app suspension/background, then fresh session on reconnect.
6. Report foreground and background observations separately. Background execution is not assumed or required.

## Current evidence

No physical Linux BLE adapter, BlueZ system bus, Apple SDK, simulator or Watch SE 2 was available in the implementation environment. Hardware stages remain NOT RUN pending the evidence table in `docs/P007_VALIDATION.md`.
