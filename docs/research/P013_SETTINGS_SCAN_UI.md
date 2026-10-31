# P013 Settings scan UI

The Devices page has two independent sections. **Active Bluetooth LE discovery**
contains a radio disclosure, `Scan Bluetooth LE for 10 seconds`, progress with
elapsed time, `Cancel Scan`, sanitized event cards and outcome/cleanup text.
**Known-device inventory** preserves `Refresh known devices` and its P012 model.
Neither section is refreshed on startup or navigation. Doctor refresh and Ctrl+R
remain static capability reports and cannot start a scan.

DeviceDiscoveryProvider owns its own asynchronous QProcess and typed
DeviceDiscoveryModel. The trusted sibling CLI (or explicit absolute `--cli` path)
runs fixed arguments `devices discover --seconds 10 --json`, with no shell,
PATH lookup, elevated helper, arbitrary path argument or permanent daemon.
Callbacks check both process identity and generation. One scan can run at a time;
Scan is disabled while running and Cancel is disabled while cleanup is pending.

Elapsed progress includes adapter setup until the final report is available;
the UI does not invent a radio-start event. The validated final report gives
actual observation duration. A missing or invalid final report leaves observation
duration explicitly unavailable; elapsed process time is not substituted. Empty completion displays `No observations`, while
adapter/power/permission/LE-support/bus failures remain distinct. Cleanup
incomplete is a separate visible warning. Partial and previous results are
marked stale on a new attempt/cancellation/failure and after one minute. There
is no automatic restart or periodic discovery.

Cancel sends SIGTERM on Unix, allowing the Rust CLI to stop its session, clear
its filter and emit final JSON. Navigation away, losing window activity and
closing the window request cancellation too. A 22-second process deadline
requests cancellation. If no final report arrives within 6.5 seconds, QProcess
kills the process; the UI reports uncertain cleanup rather than claiming
StopDiscovery. Application shutdown also waits boundedly for graceful cleanup,
with a kill fallback because the normal event loop may be exiting.

Stdout is capped at 64 KiB and stderr at 4 KiB. Excess output requests cleanup and
closes the overflowing read channel. Raw diagnostics never appear in QML.
Malformed JSON, inconsistent states, stale callbacks and process crashes cannot
replace results with trusted identities. Names render as plain text through the
existing Card component. The backend compiles with C++20 and Qt 6.4.2 on Ubuntu.

Apple Watch, AirDrop, AirPods and Research Lab behavior is unchanged. No Connect,
Pair, Trust, Forget, enrollment, PAM, polkit or unlock action is provided.
