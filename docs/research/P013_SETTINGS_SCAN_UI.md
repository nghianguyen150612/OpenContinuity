# P013 Settings scan UI

The Devices page has two independent sections. **Active Bluetooth LE discovery**
contains a radio disclosure, `Scan Bluetooth LE for 10 seconds`, progress with
elapsed time, `Cancel Scan`, sanitized event cards and outcome/cleanup text.
**Known-device inventory** preserves `Refresh known devices` and its P012 model.
Neither section is refreshed on startup or navigation. Doctor refresh and Ctrl+R
remain static capability reports and cannot start a scan.

P013 originally delivered one final JSON report. P014 upgrades this active
section to consume `--events-jsonl` incrementally; see the [P014 live UI
contract](P014_SETTINGS_LIVE_SCAN_UI.md). The P013 `--json` format remains
available for CLI consumers and retains its schema.

DeviceDiscoveryProvider owns its own asynchronous QProcess and typed
DeviceDiscoveryModel. The trusted sibling CLI (or explicit absolute `--cli` path)
runs fixed arguments `devices discover --seconds 10 --events-jsonl`, with no shell,
PATH lookup, elevated helper, arbitrary path argument or permanent daemon.
Callbacks check both process identity and generation. One scan can run at a time;
Scan is disabled while running and Cancel is disabled while cleanup is pending.

Elapsed progress includes adapter setup. The UI does not invent a radio-start
event; it shows observations after the validated session event. The terminal
record gives actual observation duration. A missing or invalid terminal leaves
observation duration explicitly unavailable; elapsed process time is not
substituted. Empty completion displays `No observations`, while
adapter/power/permission/LE-support/bus failures remain distinct. Cleanup
incomplete is a separate visible warning. Partial and previous results are
marked stale on a new attempt/cancellation/failure and after one minute. There
is no automatic restart or periodic discovery.

Cancel sends SIGTERM on Unix, allowing the Rust CLI to stop its session, clear
its filter and emit the terminal event. Navigation away, losing window activity and
closing the window request cancellation too. A 22-second process deadline
requests cancellation. If no terminal arrives within 6.5 seconds, QProcess
kills the process; the UI reports uncertain cleanup rather than claiming
StopDiscovery. Application shutdown also waits boundedly for graceful cleanup,
with a kill fallback because the normal event loop may be exiting.

Stdout is capped at 512 KiB total and 2 KiB per record; stderr is capped at 4 KiB. Excess output requests cleanup and
closes the overflowing read channel. Raw diagnostics never appear in QML.
Malformed JSON, inconsistent states, stale callbacks and process crashes cannot
replace results with trusted identities. Names render as plain text through the
existing Card component. The backend compiles with C++20 and Qt 6.4.2 on Ubuntu.

Apple Watch, AirDrop, AirPods and Research Lab behavior is unchanged. No Connect,
Pair, Trust, Forget, enrollment, PAM, polkit or unlock action is provided.
