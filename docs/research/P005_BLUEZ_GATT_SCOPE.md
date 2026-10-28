# P005 — Experimental Linux BlueZ GATT transport

Status: implementation complete for review / real hardware validation NOT RUN. A real D-Bus GATT server, advertisement and wire framing are implemented on this branch; they have not been exercised against a live BlueZ daemon or radio.
Branch: `feat/p005-experimental-bluez-gatt`
Base main: `11e99717dae8d32542ab6b7298dd64842ea3c74f` (P004 squash merge).

## Roadmap reconciliation

The original authoritative `OpenContinuity-Roadmap-P001-P060.md` is not in this repository; planning history groups P001–P006 as foundation/settings and P007–P018 as Watch work. P002–P004 have proceeded as provisional Watch research. Therefore this P005 is **a provisional implementation continuation**, not a verified copy of original P005 acceptance criteria. If the source is recovered, preserve it and explicitly reconcile milestones rather than silently redefining it.

Carry-over debt from P001: production Bluetooth adapter scaffolding and Qt6/QML Settings framework remain incomplete. P005 is an isolated research adapter, NOT a replacement for either full feature.

## Goal

Implement a **genuine, opt-in, non-authorizing Linux GATT peripheral experiment** using BlueZ D-Bus with the Watch proposed as a *foreground* BLE central. Establish demonstrable Linux-side advertisement, GATT registration and controlled synthetic data exchange *when suitable local hardware exists*. With no system bus/controller, keep all mocks and build/test gates useful and report real hardware as NOT RUN.

P004 exposed read-only capability inspection plus mock lifecycle only; no real radio server and no Swift app existed at its final head. P005 adds Linux-side D-Bus object export and registration code. This milestone must not claim Watch compatibility until tested on Apple hardware.

## Engineering boundaries

1. Preserve portable Rust core and P001–P004 CLI/daemon contracts. Real D-Bus details are in a Linux-only CLI module using zbus 5.12.0, whose published MSRV is Rust 1.77 and which builds on the pinned 1.85 toolchain.
2. Use `GattManager1.RegisterApplication`, exported application ObjectManager and GATT service/characteristic objects, and `LEAdvertisingManager1.RegisterAdvertisement` with correct object paths and BlueZ interface behavior. Verify precise API signatures and error handling from BlueZ documentation/source; do not guess.
3. Make command explicitly experimental, e.g. `opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental`, with maximum 300 s, no auto-run, no permanent systemd user/root service. Keep current `research watch-ble --capabilities` read-only.
4. Do not power on or change adapter settings, overwrite existing advertisements, alter BlueZ configuration, pair devices or require root merely to pass tests. If BlueZ policy blocks registration, report a clear authorization/policy error without weakening permissions.
5. Scope GATT characteristics to bounded synthetic health/challenge-transport data; no real authentication, approval, key enrollment, persistence or Linux session unlock. Use no production secrets. Both signed and unsigned experimental payloads must be labeled non-authorizing; never promote mock Ed25519 verification to real Watch identity.
6. Bound wire frames, handle fragmentation/reassembly when required by characteristic MTU, reject malformed/duplicate/out-of-order/overlong/expired data. Do not assume Linux and Watch share an MTU greater than 20 bytes. Preserve `authorization: never_evaluated`.
7. Set strict shutdown rules for success, timeout, signal, bus disconnect, adapter removal, registration failure, advertisement failure and client disconnect. Unregister advertisement before application, release only objects owned by the current process, and avoid orphaned advertisements. Test cleanup ordering.
8. Fix P004's environment-dependent tests: a real BlueZ host may legitimately return interfaces/adapters, so unit and integration tests must not assert that the system bus is absent. Add injectable D-Bus/process discovery boundaries and fixtures. Distinguish unsupported interface from blocked D-Bus access and unknown controller capabilities; reserve BLOCKED for a concrete inability to proceed.
9. Keep any later Watch/iPhone Swift app separate and experimental; no Apple build or SE 2 radio success unless real SDK/device evidence exists. A foreground Watch app should verify host identity before presenting an authentication request; P005 does **not** ask for production Watch approval.
10. Document privilege requirements, BlueZ API versions, controller feature checks, privacy of BLE discovery and safe test scope.

## Deliverables

- Linux-specific non-production GATT server/advertisement implementation and a reversible, time-bounded CLI experiment (implemented; real host validation not run).
- Mock D-Bus backend, deterministic failure/rollback tests, bounded frame protocol/fixtures and P001–P004 regression tests.
- `docs/research/P005_BLUEZ_GATT_SERVER.md` with real adapter preconditions, exact API evidence, CLI details and privacy warnings.
- `docs/research/P005_WIRE_CONTRACT.md` and `docs/research/P005_SECURITY_REVIEW.md`.
- `docs/P005_VALIDATION.md` with exact SHAs, local validation, hosted CI and hardware statuses. Update architecture/feature matrix/roadmap as warranted.

## Acceptance

- [x] Real BlueZ GATT object server and temporary advertisement registration code implemented; real host registration remains NOT RUN.
- [x] Existing read-only capability CLI and P001–P004 tests remain intact.
- [x] Capability parser tests and isolated private D-Bus mock integration tests do not assume a host bus/controller.
- [x] Explicit opt-in, adapter choice, bounded duration, unchanged power state, clear policy errors and owned-resource cleanup.
- [x] Frame size, minimum ATT MTU, fragmentation, timeout, invalid data, duplicate/session/version errors and cancellation are tested.
- [x] Experiment reports use `authorization: never_evaluated`; no unlock state exists.
- [x] Watch/iPhone/real BlueZ hardware claims are NOT RUN and marked as such.
- [x] No PAM, login-manager, polkit, Apple-private pairing, IRK extraction, or production enrollment.
- [x] Local format, Clippy, all-target tests (53 pass), workspace build and diff check.
- [ ] Final-head hosted CI and PR verification (recorded in P005 validation).

## Future blockers

- Mutual Linux-host-to-Watch authentication, trustworthy Watch-side consent/key operations, P-256 interop, secure enrollment persistence and anti-relay policy still need separate milestones/review.
- Apple-native private STS Auto Unlock is not implemented.
- No AirDrop or AirPods work in this milestone.
