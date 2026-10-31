# P013 Arch physical discovery plan

**Current evidence: NOT RUN / BLOCKED.** This cloud workspace has no accessible
Arch controller or independently observed RF session. Private D-Bus fixtures and
Qt offscreen tests are software evidence only. No physical scan was initiated.
P010 Watch SE 2 remains NO-GO.

Before a physical trial, obtain explicit approval for the selected local adapter,
duration and radio use. Record sanitized OS/kernel/BlueZ versions, controller LE
support and whether other Bluetooth work is active. Do not collect addresses,
object paths, raw advertisements, RSSI, keys or private device names as evidence.
Do not automatically power on the adapter or stop another client's session.

1. Run read-only Doctor and inventory. Confirm no discovery begins at Settings
   startup, navigation or `Refresh known devices`.
2. With the operator's adapter already powered on, explicitly invoke
   `opencontinuity devices discover --seconds 10 --json` or click Scan. Record
   only schema/status/flags/durations/count/cleanup and authorization. Redact
   local labels before sharing. Keep raw output local and do not upload it.
3. Compare an independent scanner or local controller observation with the
   session's start/end. Distinguish RF observations from BlueZ event reporting.
   A zero-result completion does not prove absence of peripherals.
4. Repeat one approved bounded run each for CLI SIGINT, CLI SIGTERM and Settings
   Cancel/navigation/close. Confirm the owned session/filter disappears; inspect
   cleanup warnings. Never infer cleanup merely from another client's shared
   Discovering state.
5. If separately approved, have a known independent client keep discovery active
   while this CLI completes. Verify that client's session continues. Restore only
   state owned by the trial. Do not forcibly interrupt unrelated Bluetooth work.
6. Exercise powered-off/no-adapter/permission/unsupported-LE cases where safe,
   without changing system policy. BlueZ restart/adapter removal is a separate
   disruptive trial and needs explicit operator approval and recovery planning.
7. Record PASS/FAIL/NOT RUN for each case, sanitized durations and cleanup status,
   and any interference. Never infer genuine Watch/AirPods/iPhone identity,
   native Apple support, proximity or Linux authorization from scan results.

Physical acceptance requires observed bounded radio behavior and ownership
cleanup on actual hardware. Software acceptance alone does not satisfy it.
