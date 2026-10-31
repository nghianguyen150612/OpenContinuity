# P013 — Explicit, Bounded Bluetooth LE Discovery & Native Settings UI (Provisional)

**Status:** IMPLEMENTED — software validation and physical limitations are recorded in `docs/P013_VALIDATION.md`; delivered as a Draft PR.
**Branch:** `feat/p013-opt-in-bluetooth-discovery`.
**Base `main`:** `64b98ba7eb6a80e6969a531a2e924a38f33bb391` (P012 PR #13 squash merge).
**Original-roadmap caveat:** The named original P001–P060 planning document remains unavailable. P013 is a clearly provisional, Linux-first continuation of the shipped Qt Settings + read-only BlueZ inventory, **not** a verified preexisting P013 scope.
**P012 CI verification:** Final-head `3dc2f3459e826ef1a0bb9c84cae35e69f517fa3c` completed successfully in run `41161372066` (Rust, Qt, Swift/interoperability/watchOS/artifact packaging). Physical qualification remains separate.

## Objective and architectural boundary

Implement **user-initiated, time-bounded Bluetooth LE discovery** as a separate explicitly mutable BlueZ session with reliable ownership-based cleanup, and expose it in the existing native Qt6/QML Devices page. Preserve P012 `devices list --json` as an independent **read-only, zero-radio-scan** snapshot. Discovering a Bluetooth peripheral is not pairing, connecting, recognizing a genuine Apple Watch or AirPods, verifying physical proximity, or authorizing Linux.

The user must understand that `Scan nearby Bluetooth LE devices` uses the real radio and can interfere with concurrent Bluetooth activity. This is **not** a passive, read-only operation. It is allowed only through a deliberate visible click/CLI command with an explicit bounded duration, and never at Settings launch, background refresh, cron or daemon startup.

## Baseline

- Rust 2024 core/CLI and zbus Linux research code. P012 `apps/opencontinuity-cli/src/device_inventory.rs` accesses `org.freedesktop.DBus.ObjectManager.GetManagedObjects` once and emits strictly validated privacy-sanitized snapshot schema v1.
- P011 Qt6/QML desktop app has 8 pages, read-only `DoctorProvider`, and P012 separate `DeviceInventoryProvider` with asynchronous bounded `QProcess` and model.
- P007/P009 GATT research and test-key protocol are separate from BlueZ inventory and discovery. P010 Apple Watch SE 2 physical BlueZ qualification remains **NO-GO**. Native Auto Unlock, AirDrop, AirPods and all Linux authorization remain unsupported.
- BlueZ `org.bluez.Adapter1.StartDiscovery()` acquires **one discovery session per client per adapter**; `StopDiscovery()` releases the caller's session, not the sessions of other clients. A discovery procedure is shared; `InterfacesAdded` and `PropertiesChanged` may reflect other clients or cached devices. Authoritative upstream: https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst

## A. P013 Linux discovery runtime (real D-Bus implementation)

1. Define a separate `device_discovery` adapter, not an extension of the read-only inventory call, and prefer the already locked zbus 5.x API; verify exact methods and signatures against pinned versions. Connect to system bus, enumerate adapters read-only, accept an explicit **snapshot-indexed** adapter choice (e.g. `--adapter-index 1`) or carefully documented deterministic default. No raw D-Bus object-path CLI injection.
2. Require `Adapter1` and `Powered=true`, and needed LE discovery/filter support. Never power an adapter, pair, connect, configure global BlueZ or change system D-Bus policy. **Fail closed** if BLE-only transport filtering is unsupported; do not silently substitute BR/EDR.
3. Use a per-client `SetDiscoveryFilter({Transport:"le"})` only if required to guarantee LE-only discovery; document its per-client and shared discovery semantics. Restore/clear **only this client's** filter on termination. Never change another process's filter/session. No global controller scan settings.
4. Register relevant D-Bus event subscriptions BEFORE `StartDiscovery`; capture bounded `ObjectManager.InterfacesAdded` and `org.freedesktop.DBus.Properties.PropertiesChanged` observations associated with the selected adapter while the session is active. Apply strict event validation; distinguish **event observed during own scan interval** from BlueZ's previously cached devices and from independently verified RF sightings. Do not label a device `nearby` merely because it has a `Device1` object.
5. Call `StartDiscovery` exactly once per accepted session; on normal expiry/cancel/SIGINT/SIGTERM/adapter loss/BlueZ owner change, attempt exactly one matching `StopDiscovery` if start succeeded, clear owned per-client filter and close D-Bus connection. Bound cleanup RPCs and disclose failures. Where process is forcibly killed, do not claim deterministic StopDiscovery execution; rely only on documented D-Bus disconnect cleanup as a fallback and report the limitation.
6. Duration strictly 1–30 seconds, default 10 seconds; include setup and cleanup in a bounded outer runtime (for example <=40 seconds), with explicit per-call timeout, bounded memory, max 128 observable devices and bounded event counts. Do not run a detached unbounded D-Bus worker that can keep scanning after CLI exit; if the worker cannot be canceled, redesign to preserve ownership cleanup and explicit uncertainty.
7. Avoid scanning if a known owned P007/P009 research GATT experiment holds the selected adapter; investigate a narrowly scoped collision gate and truthful `adapter_busy` status rather than forcibly stopping other services. Do not claim universal global exclusion of other BlueZ clients without actual support.
8. Never automatically trust an observed peripheral, automatically enroll research keys, accept identity from BLE names, or take action on Linux auth state.

## B. Explicit CLI and discovery schema

Add a distinct command such as `opencontinuity devices discover --seconds 10 --json`; do **not** alter `opencontinuity devices list --json` behavior or v1 schema.

Freeze a separate `bluez_le_discovery_session` schema v1, with:
- `schema_version=1`, `source=bluez_le_discovery_session`, `scan_requested=true`, `status` and `authorization=never_evaluated`.
- `scan_started` true **only after StartDiscovery returns success**; `scan_completed` true only after clean bounded expiry and attempted owned StopDiscovery/cleanup (separate `cleanup_status` field).
- elapsed duration / requested duration, optional adapter index, explicit provenance of observed events, scan result count, bounded sanitized device display names and per-scan ephemeral IDs, no stable identifiers.
- Status categories for `completed`, `cancelled`, `no_adapter`, `adapter_not_powered`, `unsupported_le`, `adapter_busy`, `bluez_unavailable`, `system_bus_unavailable`, `access_denied`, `timeout`, `malformed_data`, `cleanup_incomplete` and `unsupported_platform`, without raw D-Bus exception strings or addresses.

**Do not** serialize MAC addresses, device object paths, IRKs, raw RSSI/advertising data, P008/P009 crypto material, session secrets or persistent IDs. Locally display names only after P012-equivalent sanitization and length bounds. Avoid presenting a scan result as a verified or authenticated Apple device. A zero-results scan may be successful without proving that no device was in range.

On the real CLI, support graceful cancellation with an accurately classified outcome and nonzero exit code for non-complete results. JSON emission and exit-code contract must be independently tested and documented.

## C. Native Qt Settings — explicit scan UI

1. Keep the P012 `Refresh known devices` button and read-only inventory independent. Add an explicitly labeled `Scan Bluetooth LE for 10 seconds` action on the Devices page with a short disclosure that it **actively uses Bluetooth**. It must never trigger from app startup, navigation, report refresh or periodic timers.
2. Implement a separate `DeviceDiscoveryProvider`/model with asynchronous `QProcess`, fixed `devices discover --seconds <bounded integer> --json` program+args, known absolute CLI path, stdout/stderr caps, sane Qt timeout (duration + setup/cleanup grace), generation-scoped callbacks, cancellation and one active scan at a time. No shell, env-injected command or privileged helper.
3. Display clear `Scan in progress`, `Cancel scan`, `Scan completed`, `No observations`, `Adapter not powered`, `Permission denied`, `Scan failed`, `Cleanup incomplete` and `Previous scan results (stale)` states. Distinguish scan results from BlueZ's **known-device inventory**; do not merge the two datasets into a claim about physical nearby devices.
4. On Cancel/close/background or user navigation away, request graceful shutdown and verify/communicate cleanup status when observable; have a bounded force-termination fallback labeled as uncertain cleanup. If the CLI only emits final JSON, on cancellation a separate safe communication mechanism may be needed, but do not run a permanent daemon or interactive shell.
5. No Pair/Connect/Trust/Forget/Settings write actions in P013. Apple Watch, AirDrop and AirPods pages remain `unsupported` and do not inherit any new trust classification from a scan result.
6. Native Qt UI must remain compatible with Qt 6.4.2 Ubuntu runners and later Qt 6.x on Arch. Preserve Wayland/X11 platform neutrality.

## D. Tests and CI

- Rust: unit/state-machine + mocked per-client D-Bus session tests for filter/setup/start/event accumulation/stop/clear/error/cancel/timeout/owner lost, other client's session unaffected, no adapter/not powered/LE unsupported, bounded records/names, D-Bus error categories, no leakage of MAC/object paths/raw advertisements, exact schema/exit codes. Mocked D-Bus PASS is not physical RF PASS.
- Qt: parser/model/QProcess tests for all status values, missing CLI, invalid JSON/oversize/timeout/cancel/stale callback, no shell execution, 10-second bounded duration, one active scan, shutdown when UI closes; real QML smoke for manual click/Cancel/no-results/errors and 8 existing pages.
- Regression: P012 inventory stays strictly read-only; P011 DoctorReport schema unchanged; P007–P010 Watch research tests/CI untouched. Run cargo fmt, Clippy `-D warnings`, cargo workspace test/build, Python suites, CMake/Ninja/CTest on Ubuntu Qt6.4, Swift package + Rust→Swift→Rust interoperability, unsigned watchOS build and artifact packaging on macOS. Verify all final-head GitHub Actions.
- Real Arch BlueZ controller/independent scanner trials are optional **separate physical gates** in this software milestone; report `NOT RUN/BLOCKED` when unavailable. P010 Watch SE 2 physical qualification remains NO-GO regardless of P013 scan behavior.
- P012 upstream CI gate: do not automatically assert the P012 pending final-head workflows passed; inspect any post-merge results and fix any inherited regression within P013 if necessary.

## E. Documents and delivery

Create:
- `docs/research/P013_BLUEZ_DISCOVERY_LIFECYCLE.md`
- `docs/research/P013_DISCOVERY_SCHEMA.md`
- `docs/research/P013_SETTINGS_SCAN_UI.md`
- `docs/research/P013_DISCOVERY_SECURITY_REVIEW.md`
- `docs/research/P013_ARCH_PHYSICAL_TEST_PLAN.md`
- `docs/P013_VALIDATION.md`

Update README, architecture, feature matrix, roadmap, threat model, Qt Settings README and BlueZ attribution as appropriate. Ensure all new discovery UI controls have genuine wired behavior or are disabled with an explanation.

Create a Draft PR against `main` from existing branch after code/test/docs. Do not merge automatically or begin P014. Declare software acceptance separately from Arch physical RF proof.

**Hard boundaries:** No Apple Watch Auto Unlock, Apple Account, pairing/connect, production key provisioning, AirDrop/AWDL, AirPods modes, PAM/polkit, login manager, Linux unlock, unrestricted background scanning, telemetry, external device logs, real proximity assurance or hardware identity claims. Maintain `authorization: never_evaluated`.
