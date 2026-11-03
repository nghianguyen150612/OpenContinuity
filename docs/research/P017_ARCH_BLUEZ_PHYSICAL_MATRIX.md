# P017 Arch BlueZ physical matrix

Software qualification is separate from hardware evidence. Current physical
status: **NOT RUN**; no owned Arch adapter, independent receiver or native
Wayland/Hyprland display is available in the cloud environment. S0 is BLOCKED on
this non-Arch host with unavailable system bus/BlueZ and zero reported adapters.
S1–S5 are NOT RUN. P010 Watch SE 2 stays NO-GO; native Apple Auto Unlock stays NO-GO.

| Gate | Required operator evidence | Current outcome |
| --- | --- | --- |
| S0 readiness | Read-only tool, actual Arch kernel/BlueZ/controller firmware, installed CLI and Qt | BLOCKED here |
| S1 execution | Local consent, owned controlled host, exact bounded argv, start/exit | NOT RUN physically |
| S2 stream and exit | Live stream, terminal, duration, counts, exit and cleanup claim | Software tests only; physical NOT RUN |
| S3 RF | Independent receiver, controlled advertising intervals, timing and discrepancies | NOT RUN |
| S4 Wayland/Hyprland | Native window and complete interaction worksheet below | NOT RUN |
| S5 ownership/cleanup | Bounded independent method/reply observer, owned Stop/filter/connection | Private-bus software tests only; physical NOT RUN |

## Reproducible native Settings procedure

1. On the owned Arch session, build/install using the Settings README. Record
   source revision, numeric Qt version, compositor version and available Wayland
   plugin privately. Check `XDG_SESSION_TYPE=wayland` locally; do not export the
   environment or screenshots containing device lists. Confirm the real CLI is
   installed in the Settings executable directory or provide its trusted absolute
   path. Run the read-only P017 tool first. Missing GUI/plugin/permissions is BLOCKED.
2. Launch explicitly in the existing Hyprland session:
   `QT_QPA_PLATFORM=wayland /trusted/bin/opencontinuity-settings --cli /trusted/bin/opencontinuity`.
   Confirm the window opens, resizes and navigates. This is a real compositor
   trial; `offscreen` cannot pass S4.
3. At startup and on every page, verify no scan is initiated. On Devices, click
   Refresh known devices. Confirm independent cached inventory/error/empty states
   without StartDiscovery or radio scanning. It is not a live proximity result.
4. With local consent and controlled peripheral/receiver ready, click **Scan
   Bluetooth LE for 10 seconds** once. Record disclosure, progress, live
   unverified observations before terminal completion, unique count, elapsed time
   and the final cleanup state. No sightings is not a failed parser and is not RF
   success. After completion wait and verify no automatic repeat scan.
5. Start a fresh consented trial and click Cancel during observation. Verify
   graceful exit, terminal cancellation when deliverable, stale rows and cleanup
   warning if confirmation is absent. Confirm bounded child termination locally.
6. Repeat separately while navigating away; then while closing the window.
   Confirm cleanup is requested, no orphan CLI remains and owned resources are
   released. Loss of window activity is also a cleanup trigger; record it.
7. On an isolated host with an independently approved failure scenario, observe
   missing/failed cleanup. Confirm visible uncertain/stale state, never a clean
   completed result. Do not cause a system-wide BlueZ restart without separate
   consent. If this scenario is unavailable mark its row NOT RUN.
8. After a terminal callback, open a new session. Verify fresh rows/session state
   and that delayed old callbacks cannot update or finish it. Record a separately
   controlled delayed-exit scenario if available; software fixtures alone do not
   establish this physical observation.

Use a row for every step and trial: UTC, stage, expected behavior, observed
behavior/counts, pass/fail/blocked/not_run, provenance, private proof reference.
Record all missing prerequisites explicitly. Review private logs before publishing
sanitized results. The [RF/cleanup worksheet](P017_RF_AND_CLEANUP_EVIDENCE.md)
specifies independent evidence and shared BlueZ semantics. Discovery and GATT
server trials run sequentially on one adapter because the project guard excludes
concurrent ownership. Successful third-party BLE discovery does not qualify Watch.
