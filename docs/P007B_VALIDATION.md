# P007B validation and physical blockers

`authorization: never_evaluated`

Starting branch `research/p007b-watch-bluez-physical-interop` at `7963d756fae0aa862c25ae5ffc502fd512856b7d`; base P007 merge `1cf956bd7356df2e7e2800da5657c01d12f7b256` verified. This is the feasible diagnostics/runbook portion of P007B. Physical interoperability is not qualified. The authoritative P001–P060 roadmap is still unavailable; production Bluetooth adapter and Foundation/Qt6/QML Settings remain outstanding.

## Observed host evidence

UTC preflight 2026-10-29T08:09:29Z. Local host: Debian 13, x86_64, kernel 6.18.55, not the owned Arch test host. Sanitized raw command-result evidence is in `research/fixtures/P007B_HOST_PREFLIGHT_2026-10-29.json`. The existing host script returns zero even when subprobes are unavailable; its exit code is never a controller PASS. This is a command inventory only.

| Stage | Evidence / prerequisite | Outcome |
| --- | --- | --- |
| Kernel command | uname -a executed, persistent hostname excluded from published output | PASS |
| BlueZ version / controller list | bluetoothctl absent | BLOCKED |
| Controller firmware, roles, power | btmgmt absent; no exposed controller inventory | BLOCKED |
| Bluetooth service query | systemctl exit 1: system not booted with systemd as PID 1; host is down | FAIL (query) |
| System bus / BlueZ owner | busctl exit 1: system bus socket unavailable, No such file or directory | BLOCKED (bus); FAIL (query) |
| GattManager1, advertising manager and capacity | no system bus/controller | BLOCKED |
| D-Bus permission / GATT registration eligibility | cannot evaluate without actual BlueZ host | BLOCKED |
| GATT application / advertisement registration | no operator-selected physical adapter/trial | NOT RUN |
| RF advertisement observation | independent owned scanner unavailable | NOT RUN |
| Mac / Xcode / Watch OS / iPhone versions | no physical Mac or paired devices attached; hosted CI is separate | BLOCKED |
| Signed build / install / launch / Bluetooth permission | signing and physical destination unavailable | NOT RUN |
| Service and five-characteristic discovery | physical path unavailable | NOT RUN |
| 25-byte challenge long read / 44-byte result long read | no physical Watch/BlueZ trial | NOT RUN; behavior UNKNOWN |
| Frames / Complete / digest / Cancel | no physical exchange | NOT RUN |
| Reconnect / stale frames / suspension / cleanup / restart | no physical exchange | NOT RUN |

No GATT trial, pairing, power change, service enablement, policy edit or signing credential access was performed. No InvalidLength failure or long-read incompatibility was reproduced, so neither advertisement composition nor protocol bytes were changed.

## Software qualification

Local Rust 1.85: fmt, Clippy with warnings denied, 75 workspace tests and workspace build PASS. Python preflight tests: 2 PASS (identifier-output exclusion, missing-tool and timeout handling). Swift/Xcode unavailable locally. [Hosted run 40840501549](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40840501549) on implementation/documentation head `dbc6a8baf2e609f77b2d8f2171a156e8c998a090` passed both jobs: 75 Rust tests, 2 Python tests, and 11 Swift XCTest cases (2 diagnostics, 4 lifecycle, 5 codec including shared Rust fixtures). Apple Swift 6.1.2, Xcode 16.4 (16F6), watchOS SDK 11.5: scheme OpenContinuityWatch, Debug, generic/platform=watchOS, CODE_SIGNING_ALLOWED=NO, BUILD SUCCEEDED at 2026-10-29T08:18:09Z. This is an actual Apple compile, not a signed installation or physical trial. Subsequent documentation-only final-head CI is reported in the delivery report. PR [#8](https://github.com/nghianguyen150612/OpenContinuity/pull/8) is draft.

The preserved local `opencontinuity research watch-ble --capabilities --json` command reported system_bus_unavailable, no adapters inspected, hardware_watch_observations not_run, authorization never_evaluated.

The Watch diagnostics are explicitly enabled in the UI, bounded to 64 in-memory allowlisted events, cleared on opt-out, and never accept raw bytes, free-form errors, names or identifiers. Length events are observed callback lengths before parsing; frame acknowledgements are counted only after pending-operation ownership checks. The diagnostic view includes authorization: never_evaluated. Trace eviction means a single snapshot is not a complete long trial; record observations during each stage. No Linux GATT/BlueZ runtime behavior was changed.

## Acceptance and next operator actions

- [x] Preserve protocol, P006A protections and existing software gates.
- [x] Add bounded opt-in Watch instrumentation and sanitized read-only host probe with deterministic tests.
- [x] Provide signed-install, physical-run and long-read/reconnect procedures.
- [ ] Execute preflight on the owned Arch host and record model/software/firmware versions privately, then publish sanitized metadata.
- [ ] Install and launch a signed app on SE 2; explicitly approve a selected-adapter 120-second server trial.
- [ ] Independently observe RF, discover service/characteristics, verify complete long reads, synthetic Complete/Cancel results and terminal cleanup.
- [ ] Run interruption/reconnect matrix and attach sanitized timestamped evidence.

PR remains draft while physical gates are outstanding. Any future failure requires a reproducible error and focused regression test before changing the working protocol. Even successful BLE transport would not establish identity, wearer presence, proximity, Linux authorization or unlock.
