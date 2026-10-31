# P012 — Linux BlueZ Known-Device Inventory & Native Settings Integration (Read-Only)

**Status:** STARTED — branch and specification only; runtime implementation, tests and CI NOT STARTED.
**Branch:** `feat/p012-linux-device-inventory`.
**Base:** `main` `2e06dd5a300e652d168bdb12e17ee510ab958af0` (P011 PR #12 squash merge).
**Provenance:** The original `OpenContinuity-Roadmap-P001-P060.md` was never recovered. P012 is an explicitly **provisional** milestone chosen to extend the now-merged real Qt Settings UI with trustworthy Linux device inventory and start filling the previously missing platform-adapter boundary. This is not a verified original P012 definition.

## Goal

Implement a *real, strictly read-only, snapshot-only* BlueZ known-device inventory on Linux, available through a new bounded versioned Rust CLI command and integrated into the P011 native Qt6/QML **Devices** page. Never pretend that a BlueZ-known/connected/paired peripheral is a genuine Apple device, securely identified Watch, nearby device, or authenticated Apple ecosystem feature.

P010 hardware qualification is still **NO-GO**. P008/P009 custom signed research protocols, P007 synthetic GATT, all Apple native features and Linux unlock remain unchanged. No automatic start of the research server, daemon, Bluetooth scanning, pairing, adapter power toggles or privileged configuration.

## Verified P011 baseline

- Independent CMake/C++20 `apps/opencontinuity-settings`, Qt6/QML UI, eight pages. `DoctorProvider` uses asynchronous, bounded `QProcess` with an absolute trusted CLI path, fixed `doctor --json`, 3 s deadline, stdout 16 KiB / stderr 4 KiB, strict DoctorReport v1, process-generation retirement, stale/error states and two `QAbstractListModel`s.
- `crates/opencontinuity-core/src/lib.rs` defines static DoctorReport v1: three native Apple capabilities (Watch Auto Unlock, AirDrop, AirPods Settings) **unsupported**; Watch trust stages **unknown**. Do not change those claims or break its schema.
- `apps/opencontinuity-cli/src/bluez.rs` already performs read-only BlueZ capability inspection via external `busctl`; `apps/opencontinuity-cli/src/gatt.rs` contains distinct opt-in P007/P009 GATT research server paths with zbus. P012 introduces a *separate inventory adapter*, not an extension of those server operations.
- Existing Qt CI (Ubuntu 24.04), Rust/Python CI, hosted macOS Swift/watchOS tests and unsigned artifact packaging all passed for P011 final head (PR job succeeded on rerun). Preserve these checks. Actual Arch/Hyprland and Watch radio tests remain NOT RUN.

## A. Linux read-only BlueZ adapter — real implementation

1. Prefer typed `zbus` D-Bus `org.freedesktop.DBus.ObjectManager.GetManagedObjects` against `org.bluez` on the **system bus**, and inspect only published `org.bluez.Adapter1` and `org.bluez.Device1` properties. Verify the exact zbus 5.12.0 API before coding. Never write properties, call `StartDiscovery`, `SetDiscoveryFilter`, `Connect`, `Pair`, `Trust`, `RemoveDevice`, `RegisterApplication`, or any other modifying method.
2. Add a dedicated inventory module (e.g. `apps/opencontinuity-cli/src/device_inventory.rs`), with portable schema/validation helpers in core **only** if they do not import BlueZ, D-Bus or Linux-specific code.
3. Allow only a manual, bounded, one-shot invocation: `opencontinuity devices list --json`. This command is read-only; no scan. Use a total deadline around 3 s and individually bounded D-Bus call(s). No unbounded hangs, all-or-nothing privileged fallback, `sudo`, shell or helper service. On non-Linux, emit a documented `unsupported_platform` status without attempting radio calls.
4. Distinguish successful **empty known-device list** from `no adapter`, `BlueZ unavailable`, `D-Bus unavailable`, `access denied`, `malformed data`, `timeout`. Distinct statuses must reach the UI, not silently become `0 devices`. A BlueZ snapshot reports previously known objects, not an active discovery scan, a full inventory of physical devices, or radio presence.
5. Parse precisely `Alias`/`Name` when available (do not assume presence), `Connected`, `Paired`, `Trusted`, optionally `ServicesResolved` as *nullable/unknown* if missing, plus adapter association. BlueZ `Trusted` is a local BlueZ property and **not** proof of Apple identity or an authenticated P008/P009 Watch key. Display `Connected` only as **BlueZ reports connected**, not independently verified proximity.
6. Never expose raw D-Bus paths, MAC addresses, IRKs, keys, raw UUID sets, advertising payload bytes, or persistent unique device identifiers in the public JSON. Map to ephemeral **snapshot-scoped** index IDs (`device-001` etc) and `adapter-001` IDs, with no persistent stable linkability promise. Device name/alias may itself be personally identifying: show only locally, sanitize bidirectional/control/format characters and length (max 80 code points or a similarly bounded rule); no logs, telemetry or network.
7. Cap adapters (e.g. <=16) and devices (e.g. <=128); bound all text/property lengths and output size; normalize variants/types correctly; handle partial/malformed BlueZ maps fail closed or mark individual optional fields `unknown` according to a written deterministic policy. No panic on hostile BlueZ names/properties.
8. Never classify Apple manufacturer/device identity purely by name, MAC OUI, RSSI, UUID or appearance. Category should be `bluetooth_known_device` or `unknown`; the P011 Apple Watch/AirPods pages stay unsupported.

## B. Explicit JSON contract and CLI

Implement schema v1, strict and documented, with this **illustrative** shape (choose the final exact schema and freeze it in docs/tests):

```json
{
  "schema_version": 1,
  "source": "bluez_known_device_snapshot",
  "scan_performed": false,
  "status": "ok",
  "adapters": [{"id": "adapter-001", "powered": true}],
  "devices": [{
    "id": "device-001",
    "adapter_id": "adapter-001",
    "display_name": "Headphones",
    "connected": false,
    "paired": true,
    "trusted": null,
    "services_resolved": null
  }],
  "authorization": "never_evaluated"
}
```

Status must distinguish `ok`, `no_adapter`, `bluez_unavailable`, `system_bus_unavailable`, `access_denied`, `timeout`, `malformed_data`, `unsupported_platform`. Do not imply this is the existing `DoctorReport` schema. `scan_performed` must always be `false`. On runtime error, expose a safe bounded reason/status without raw D-Bus exception strings or addresses.

A local absence of BlueZ should be a graceful user-visible state, not a fake empty successful snapshot. Tests should use an **injected/mock D-Bus ObjectManager** or trait that exercises the same extraction logic; no mock success marked as physical discovery.

## C. Native Qt Settings integration

1. Extend existing `apps/opencontinuity-settings` without breaking `DoctorProvider`, existing QML navigation, QtTest or install. Add `DeviceInventoryProvider` as a **separate** model-backed, async QProcess owner. Fixed program+arguments only: `opencontinuity devices list --json`; reuse the same trusted absolute executable resolution, no shell or arbitrarily supplied program. Do not change the semantics or schema of `doctor --json`.
2. Explicit `Refresh known devices` on the **Devices** page. Prefer no automatic inventory on app startup (doctor may still auto-load). No polling/monitoring by default; the UI should never call `StartDiscovery`. If a user clicks Refresh, show a conspicuous label **BlueZ known devices · no scan performed**.
3. New typed `QAbstractListModel` rows with constrained, plain-text display name and accurate per-device tri-state `Connected/Paired/Trusted`. Ensure ID/adapter key, caps and status are validated by C++ parser independently of Rust. Device actions `Connect`, `Pair`, `Trust`, `Forget`, `AirDrop`, `AirPods modes`, `Watch unlock` must not exist as enabled controls.
4. Use existing bounded process/lifecycle patterns: asynchronous QProcess, <=3 s deadline, 16 KiB stdout/4 KiB stderr or a justified schema-aligned higher bound (128 devices may exceed 16 KiB; measure and set a strictly bounded realistic maximum such as 64 KiB), generation gate, stale report and cancellation, no raw stderr to UI. Do not run the generic GATT research server or daemon. Invalidate/mark rows stale on disconnect/error; never present stale row state as live connectivity.
5. Rendering: zero devices vs missing BlueZ vs blocked vs loading, inaccessible permissions, malformed data, and stale snapshot all have **different** truthful labels; no faux Apple icons/labels. Apple Watch and AirPods pages continue to say **native feature unsupported**; the Devices list alone never upgrades doctor capability states. Research Lab still uses `authorization: never_evaluated`.
6. Keep feature modules portable at the UI/model level so future macOS/BSD/Windows adapters can replace the Linux BlueZ backend. Do not hardcode `/org/bluez/hci0`, specific keyboard/compositor paths, or any user data.

## D. Deterministic tests and real CI

- Rust tests: BlueZ known-object extraction with 0/1/many adapters/devices, malformed data, missing/null values, USB controller absent, disabled adapter, duplicate entries, out-of-bounds counts, cap/sanitize control/bidi/overlong names, privacy (no MAC/object path/secret in JSON), snapshot-scoped IDs, user error classes, non-Linux behavior and **no mutating D-Bus calls**.
- Rust CLI tests with injected fake ObjectManager/Dbus (or a private system bus if feasible), ensuring real command dispatch, strict JSON and bounded timeouts; no actual Bluetooth controller required. Avoid mocking parser in a separate copy.
- Qt tests: strict device snapshot v1 parser, limits, unknown schema, duplicate IDs, malformed/null booleans, access denied/no-adapter/empty distinction, process missing/hung/crashed/stderr overflow, cancellation and stale callbacks, verified model propagation to Devices QML.
- QML offscreen smoke: open actual app, navigate Devices, show zero/blocked/stale and a fixture-backed list, click Refresh known devices, confirm **not a scan**, and ensure Watch/AirPods native support state remains unsupported. Add focus/resize tests as appropriate.
- Preserve existing Rust fmt, Clippy `-D warnings`, workspace tests/build, Python scripts, Qt 6.4 Ubuntu CI, Rust→Swift→Rust CryptoKit interoperability, Swift XCTest and actual unsigned watchOS build/artifact. Add device snapshot tests to the existing Linux Qt job and maintain no extra Apple build dependencies.
- If available, collect **read-only actual Arch host evidence** on a connected/owned Arch box; specify exact commands and classify NO CONTROLLER vs NO PERMISSION vs KNOWN-DEVICES FOUND correctly. Cloud CI without BlueZ or Bluetooth is not physical hardware validation. User-provided local evidence is necessary for real-device claims.

## E. Documentation

Create:
- `docs/research/P012_BLUEZ_INVENTORY_ARCHITECTURE.md`
- `docs/research/P012_DEVICE_SNAPSHOT_SCHEMA.md`
- `docs/research/P012_SETTINGS_DEVICES_UI.md`
- `docs/research/P012_PRIVACY_AND_THREAT_MODEL.md`
- `docs/research/P012_ARCH_QUALIFICATION_PLAN.md`
- `docs/P012_VALIDATION.md`

Update `README.md`, `docs/ARCHITECTURE.md`, `docs/FEATURE_MATRIX.md`, `docs/ROADMAP.md`, `docs/THREAT_MODEL.md`, Settings README and CI source as warranted. Record external Qt/BlueZ licenses and keep upstream attribution honest.

## Acceptance

- [ ] `opencontinuity devices list --json` gives a **read-only, strictly versioned, bounded and privacy-sanitized snapshot** of actual BlueZ known `Device1` objects when accessible. No scan or mutation and no Linux authorization.
- [ ] P011 Settings Devices page can show a real refreshed snapshot through a safe asynchronous Qt bridge, distinguish status/error/empty/stale, and never invent Apple native capability.
- [ ] Rust parser/adapter/mock-bus and C++ parser/model/process/QML tests exercise production paths, including no-device environment.
- [ ] Ubuntu Qt6 and Rust/Python, macOS Swift/watchOS regression gates PASS on final head; any physical Arch claims separately verified.
- [ ] `DoctorReport` v1, Watch research transport, crypto, expiry, key custody and hardware NO-GO untouched.
- [ ] PR against `main`, no auto-merge or P013. Keep Draft if any real implementation, CI or cross-platform claim is blocked; if Arch host unavailable, distinguish optional physical qualification from the software implementation gate.

## Explicit non-goals

No automatic/interactive scan this milestone, no pairing/connect/trust/forget/remove, no device firmware writes, no AirDrop transfers, no AirPods battery or noise control, no genuine Apple Watch identity/Auto Unlock, no PAM/polkit/login unlock, no promiscuous capture, no BLE MAC/IRK leakage, no mandatory daemons, no privileged service, no system Bluetooth power or policy changes, no new vendor API support, no background monitoring or telemetry. All Watch research still says `authorization: never_evaluated`.

Deliver a **real but read-only** device inventory that the native Settings UI can truthfully consume, while keeping the remainder of Apple's ecosystem explicitly unsupported pending separate milestones.
