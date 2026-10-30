# P011 — Native OpenContinuity Settings Foundation (Qt 6/QML, Linux First)

**Status:** STARTED — branch and specification only, implementation NOT STARTED.
**Branch:** `feat/p011-qt6-settings-foundation`
**Base `main`:** `e3afed25e9d8b8b63896c9117254cc86611855d2` (P010 software infrastructure squash merge).
**Provenance:** The requested `OpenContinuity-Roadmap-P001-P060.md` was unavailable when the repository was bootstrapped. This P011 is an **explicitly provisional milestone** selected to close the repeatedly documented P001 Settings foundation gap. It is not asserted to match an unseen original P011.

## Goal

Create the **first actually runnable, native Linux desktop application** `OpenContinuity Settings` built with **Qt 6 / Qt Quick / QML** and a small C++ bridge to the existing **Rust 2024** CLI/domain contracts. The product aims for an integrated macOS-inspired, original design and future Apple ecosystem interoperability; this milestone is UI foundation and truthful read-only status, **not** Apple-protocol support.

The reference Linux desktop is Arch/CachyOS + Hyprland/Wayland with Caelestia Shell, but the app must not depend on Hyprland, Caelestia, or a particular compositor. Other Linux desktops should remain feasible; FreeBSD/Unix-like and Windows are future portability targets, without false platform claims.

## Mandatory current architecture

- Rust workspace `crates/opencontinuity-core`, `apps/opencontinuity-cli`, `services/opencontinuityd` with pinned Rust 1.85 and edition 2024.
- `opencontinuity doctor --json` is a **static, side-effect-free**, schema v1 `DoctorReport`: `{schema_version, project, version, capabilities:[{id,state,summary}], watch_auto_unlock_trust:{device_detection,trusted_identity,cryptographic_authentication,session_freshness,proximity_assurance,operational_readiness}}`.
- `CapabilityState`: `supported`, `experimental`, `unsupported`, `blocked`, `unknown`. The only current doctor capability IDs are `apple_watch_auto_unlock`, `airdrop`, and `airpods_settings`, all unsupported. All Watch auto-unlock trust stages default to unknown. Never reinterpret them as runtime device observations.
- P007 synthetic transport, P008 P-256 signed research, P009A test-only enrollment, P010 diagnostics/evidence infrastructure are merged as **research-only**. Physical Apple Watch SE 2 ↔ BlueZ, signed Watch installation, on-device `LocalAuthentication` and full BLE exchanges remain **NOT RUN/BLOCKED**. P010 software merge does not promote physical qualification (NO-GO).
- `authorization: never_evaluated` must be visible for security research. There is no Linux authorization/unlock behavior.

## Deliverables — buildable, working app, not mockups

### A. Qt 6 desktop application

1. Create `apps/opencontinuity-settings/` with an independent CMake project (C++20) and QML module/resources. Canonical executable `opencontinuity-settings`. Require a tested Qt6 baseline (prefer 6.5+; verify APIs and versions installed before choosing). Use `Qt6::Quick`, `Qt6::QuickControls2`, `Qt6::Qml` and `Qt6::Core` (plus explicit test deps). **Do not make Qt a Cargo workspace dependency or import Qt into portable Rust core.**
2. Implement a real `ApplicationWindow`, responsive navigable sidebar, keyboard accessibility and content pages. Source must be new and project-owned; do not copy Apple proprietary SVGs, icons, app assets, CSS, source or trademarks. Aim for polished macOS-like settings *interaction model* while keeping native Qt rendering, clean typography, thoughtful contrast and clear state labels. Do not clone Apple Settings exactly.
3. Pages (read-only/stub states are OK if truthful): Overview; Devices (empty state; no fabricated discovered devices); Apple Watch; AirDrop; AirPods; Continuity; Research Lab / Diagnostics (read-only, explicit safety boundary); About. Navigation selections, window resizing, empty states, scroll, theme response and status presentation must function. Avoid dummy enabled toggles and buttons implying nonexistent functionality.
4. Provide native color/theme behavior (system light/dark where available), safe contrast, adaptive layout at laptop window sizes, sensible focus/tab order and a neutral fallback when a QML control or optional icon theme resource is absent. No external network/fonts required to render.
5. Desktop integration: `opencontinuity-settings.desktop` with categories, executable, StartupWMClass if appropriate; CMake install rules for binary, desktop file and custom original icon or fallback. Support Wayland and X11 by Qt platform plugin choice without forcing environment variables globally. Do not request root, change kernel/BlueZ, install services, auto-start daemon, alter window manager config or alter lock screen.

### B. Safe Rust/Qt read-only bridge

1. Create small C++ `DoctorProvider`/`SettingsBackend` interface exposed to QML via registered type or root context singleton (use verified Qt APIs). It runs **only** the existing `opencontinuity doctor --json` using `QProcess` with program and arguments separated, **no shell**. Do not start the daemon, Bluetooth scans, GATT server, Watch research, key setup or any system auth action. Do not use the QProcess synchronous waiting API on the GUI thread; use asynchronous completion and a bounded timeout.
2. Discovery of CLI executable should be predictable and safe (explicit absolute path argument or executable-relative bundled binary; optionally a clearly labeled development override). Avoid shell/PATH injection and arbitrary user-supplied command strings. Report absence/permission failure/timeout to UI, not as a fake unsupported capability. Put a strict maximum on stdout/stderr bytes, bounded runtime and concurrent refresh handling; cancel/retire old process and ignore stale callbacks.
3. Strictly parse and validate schema v1: required root keys, project `OpenContinuity`, strict capability states, unknown/repeated capability IDs, reasonable sizes, trust fields and types. Do not invent default `supported` states. Unknown/future schema must fail closed into `status unavailable` with a visible error; raw CLI stderr and unexpected JSON text must **not** be rendered without sanitization. Escape untrusted UI text through normal QML text semantics. Separate `report loaded` from `physical hardware connected`.
4. Expose readonly structured `QAbstractListModel` (preferred) / typed `Q_PROPERTY` data for capability rows and trust stages, plus `loading`, `lastError`, `schemaVersion`, `reportVersion`, `lastUpdated` and explicit `refresh()`. Avoid a giant model auto-discovery or a fake live device list; no polling by default. When a refresh fails, mark report stale/unavailable; never present old unsupported/unknown information as current without clear stale marker.
5. If `opencontinuity doctor --json` exposes only 3 native capability IDs, keep the remaining page states **not implemented / unknown**, rather than deriving Apple Watch/airpods integration from P010 research source. Research page can show build-time documented research status separately with explicit `experimental / hardware unverified` markers and links to project docs; no interactive signed Watch controls in P011.
6. Either implement a small direct read-only Rust C ABI **only if** its lifetime/ABI/build toolchain can be thoroughly tested, or prefer the simpler bounded QProcess bridge above. Document why. Keep Rust contracts the sole truth for capability classification.

### C. Testing and CI

1. Test actual C++/Qt JSON parsing, missing CLI, timeout/large output, nonzero exit, malformed JSON, wrong schema, invalid trust states, duplicate/unknown IDs, truncated payload, stale refresh and concurrency isolation using dependency injection/mock process output as appropriate. Ensure tests don't execute shell or Bluetooth.
2. Qt offscreen/QML smoke test must **instantiate** the real top-level UI at least once; ensure pages load and navigation works (Qt Quick Test / CTest or equivalent). If a headless Qt platform is unavailable, distinguish dependency BLOCKED rather than claim PASS. Use `QT_QPA_PLATFORM=offscreen` with a verified software rendering fallback; never infer X11/Wayland compositor behavior from offscreen tests.
3. Add a standalone `Qt Settings (Linux)` GitHub Actions job installing verified Ubuntu Qt6 packages, CMake/Ninja, offscreen smoke, QtTest and CTest. Preserve all existing Rust/Python/Swift/watchOS CI jobs, including Rust→Swift→Rust integration and unsigned Watch artifact; don't break Apple CI.
4. Actual Linux build must succeed with documented commands, e.g. `cmake -S apps/opencontinuity-settings -B build/settings -G Ninja -DCMAKE_BUILD_TYPE=Debug && cmake --build build/settings && ctest --test-dir build/settings --output-on-failure`. Confirm relative Rust CLI location or injected test command works. Add `cmake --install ... --prefix ...` smoke where possible. Document Arch packages based on verified distro names, and Wayland/X11/manual launch steps.
5. Run unchanged Rust gates (`cargo fmt --all -- --check`, Clippy `-D warnings`, workspace tests/build), Python suite, `git diff --check`, Swift package and actual unsigned watchOS compile on hosted Apple SDK. Record per-platform evidence and precise CI URLs.

### D. Documentation

- `docs/research/P011_SETTINGS_ARCHITECTURE.md` — Qt QML/C++/Rust boundaries, executable resolution, build graph and feature status semantics.
- `docs/research/P011_DESKTOP_DESIGN_SYSTEM.md` — original visual system, navigation, keyboard/accessibility, light/dark, responsive/empty/error states; note no Apple copyrighted UI/assets copied.
- `docs/research/P011_DOCTOR_BRIDGE.md` — exact strict doctor v1 parser contract, timeout, byte caps, stale/failure behavior and QProcess threat boundary.
- `docs/research/P011_DESKTOP_BUILD_AND_TEST.md` — Arch Linux, Ubuntu CI, Wayland/X11 headless commands, install/run, manual test matrix and known Qt dependencies.
- `docs/P011_VALIDATION.md` — exact CLI/schema, actual Qt build/test/CI counts, visible UI states and explicit physical Watch NO-GO.
- Update `README.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/FEATURE_MATRIX.md`, `docs/THREAT_MODEL.md`, `docs/UPSTREAM_MANIFEST.md` if dependencies/license evidence require.

## Acceptance criteria

- [ ] Real `opencontinuity-settings` Qt6/QML executable compiled and launched on headless Linux CI; a human can navigate an actual multi-page Settings UI on Linux.
- [ ] `doctor --json` v1 integrated via safe, asynchronous, strictly bounded and validated read-only C++ bridge; status values match Rust exactly; invalid data/timeout/error produce truthful stale/unavailable states.
- [ ] No enabled controls or fake device entries for unsupported Apple Watch Auto Unlock/AirDrop/AirPods; P007–P010 research is not promoted to native support.
- [ ] Qt unit tests + actual QML smoke tests and Rust/Python/Swift/watchOS regression gates PASS at the final head.
- [ ] Desktop launcher/install and Arch/Wayland instructions exist, with bounded dependency assumptions.
- [ ] No Bluetooth hardware scans, test key creation, daemon activation, system privilege, PAM/polkit or Linux unlock from GUI. No telemetry.
- [ ] Prior P010 physical qualification remains NO-GO until real evidence; `authorization: never_evaluated` in any research status displayed.
- [ ] PR to `main`, source/CI evidence and no false physical claims. If Qt dependencies/test runtime cannot be verified, leave PR Draft and report blockers. No auto-merge or P012.

## Exclusions

Reimplementing Apple proprietary Mac Settings UI or bundled assets, native Watch Auto Unlock/STS, Apple AWDL/AirDrop or AirPods device protocols, auto Bluetooth scan/pairing, device control/settings writes, daemon control, Linux login/unlock, privileged helper, PAM/polkit, active research GATT transport, Secure Enclave device attestations, live hardware status without probes, background monitoring and physical qualification claims.

This is a **working UI and read-only status milestone**. The program must be useful and honest even with no Apple devices or BlueZ adapter connected.
