# P014 Arch physical discovery plan

**Evidence state: NOT RUN.** This plan is for a later owned Arch Linux host with
a Bluetooth LE adapter. Private D-Bus tests and offscreen Qt smoke tests are not
physical radio evidence. The plan must not be recorded as PASS until each manual
step has a dated operator record and raw captures have been reviewed for privacy.

## Preconditions and record

Use a personally controlled Arch workstation and a controller the operator is
authorized to test. Record the OpenContinuity commit, kernel, BlueZ version,
adapter model/firmware, Qt version, compositor/session type, and whether another
Bluetooth client is active. Do not collect or publish addresses, raw advertising
data, nearby-device names, or user/account details. Use a controlled test
peripheral with permission; no pairing is needed.

Confirm before scanning that the app is idle, does not auto-start discovery, and
that `Refresh known devices` remains read-only. If a baseline BlueZ client already
owns discovery, record only that fact; do not stop or reconfigure it.

## Test sequence

1. Run one explicit 1-second scan, one 10-second scan, and one 30-second scan.
   Confirm each emits a single stream session and one terminal when stdout is
   consumed normally. Check the selected adapter index is invocation-scoped.
2. Generate one controlled LE event and verify the UI shows an unverified row
   before the terminal event. Rename the controlled peripheral and verify the
   same ephemeral row updates. Do not interpret the row as distance or identity.
3. During another scan, click Cancel. Verify the UI waits for the CLI terminal
   and reports cleanup only after the validated exit. Repeat by navigating away
   and closing the window. Forced termination must show cleanup unknown/incomplete.
4. Run `Refresh known devices` before and after discovery. Confirm the snapshot
   model is independent from streamed observations and does not start a scan.
5. If safe, observe an already-running separate BlueZ client before and after
   the OpenContinuity session. Confirm OpenContinuity issues only one owned
   StopDiscovery, clears its own filter, and does not stop the other client.
6. Repeat once with stdout connected to a deliberately non-reading consumer.
   Confirm the CLI returns within the documented write/cleanup bounds and does
   not leave an OpenContinuity-owned session. Record output and terminal absence
   without retaining private names or addresses.

## Evidence and stop conditions

Record only command exit codes, event category/sequence counts, status enums,
cleanup enums, duration, and the presence of expected owned method counts. Keep
operator-local logs private; redact addresses, paths, names, advertisements,
and account data before any project attachment.

Stop and mark the trial inconclusive if the BlueZ owner changes, the system bus
fails, StartDiscovery reply is uncertain, cleanup is incomplete, the test
peripheral cannot be independently controlled, another client's state changes,
or any address/path/payload is exposed. Do not repeat StopDiscovery to force the
radio off.

Physical LE observations do not qualify Watch SE 2 interoperability. P010's
Apple Watch SE 2 physical qualification stays **NO-GO** until its separate
hardware plan passes. No Arch result can establish Apple identity, secure
proximity, Apple Auto Unlock, or Linux authorization.
