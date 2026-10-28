# P005 validation

**Branch:** `feat/p005-experimental-bluez-gatt`  
**Verified base:** P004 initial commit `3c2a6bb09e8fd40236038d580c8ebb0039b31cb6`; `main` / P004 merge `11e99717dae8d32542ab6b7298dd64842ea3c74f`.  
**Final branch SHA:** see final delivery report and the GitHub PR head (this file is committed before the final branch SHA is known).  
**Scope status:** provisional P005 continuation; the original P001–P060 roadmap is not in the repository.

## Evidence classes

### Automated unit tests

`opencontinuity-core::gatt_wire` tests verify version/session checks, strict header and flags, sequence and duplicate rejection, a minimum ATT MTU 23 boundary (20-byte attribute write, five-byte payload), reassembly, message cap, MTU cap, partial timeout, completion/cancellation, and invalid lengths. Existing P001–P004 fixture, signature, enrollment-substitution, revocation, replay, freshness, and authorization regressions remain in the workspace.

### Mock D-Bus integration tests

`apps/opencontinuity-cli/tests/bluez_mock.rs` starts a private `dbus-daemon` and an in-process fake `org.bluez` service. It exercises the real zbus server process against the standard D-Bus method signatures and exported object manager/properties. The fake central reads version and an MTU-fragmented challenge, sends minimum-MTU data frames, completes the exchange, and reads the bounded response/digest. The test verifies the service and characteristic property names, explicit adapter selection with two mock adapters, application/advertisement registration, advertisement-failure rollback, application-registration failure without unregistering unowned state, missing system bus, absent BlueZ name, absent adapter, unsupported manager interfaces, timeout cleanup, controller power loss, and SIGINT/SIGTERM cleanup ordering. These are deterministic **mock integration tests**, not a system BlueZ daemon or RF test.

### Real BlueZ host validation

Environment: Debian GNU/Linux 13 (trixie), Linux x86_64, kernel `6.18.55`; Rust `1.85.0` installed in a temporary workspace tool directory.

| Probe | Result | Classification |
| --- | --- | --- |
| `uname -a` | `Linux 006a81821be4 6.18.55 #1 SMP Wed Oct 14 19:45:18 UTC 2026 x86_64 GNU/Linux` | VERIFIED |
| `bluetoothctl --version` / `bluetoothctl list` | executable not found | NOT AVAILABLE |
| `btmgmt info` | executable not found | NOT AVAILABLE |
| `bluetoothd --version` | executable not found | NOT AVAILABLE |
| `busctl --system --no-pager tree org.bluez` | `Failed to connect to system scope bus via local transport: No such file or directory` | VERIFIED: system bus unavailable |
| `busctl --system list` | same missing system-bus socket | VERIFIED |
| BlueZ version, adapter/controller model, firmware/capability bits | no daemon, utilities or system bus | UNKNOWN |
| Real `GattManager1.RegisterApplication` / advertisement / discovery / radio | not attempted; no real BlueZ host | NOT RUN |

The D-Bus mock tests above run independently of these missing host prerequisites. No real BlueZ registration or advertising result is claimed.

### Apple validation

`swift`, `xcodebuild`, watchOS SDK, signing identity, Apple Watch SE 2 and paired iPhone are unavailable in this environment. No Watch app was built or signed, and no service discovery, connection, exchange, or disconnect was tested on Apple hardware. Apple compilation and physical testing are **NOT RUN**.

## Commands and results

Rust toolchain: `rustc 1.85.0 (4d91de4e4 2025-02-17)`. Checks required by the P005 request:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `cargo test --workspace --all-targets` | **PASS** — 53 tests, 0 failures (7 CLI unit, 8 private D-Bus mock integration, 8 CLI command, 29 core, 1 daemon command; daemon unit target has 0 tests) |
| `cargo build --workspace` | **PASS** |
| `git diff --check` | **PASS** |
| `cargo run -q -p opencontinuity-cli -- research watch-ble --capabilities --json` | **PASS** — reports `system_bus_unavailable`, no adapters, Watch observations `not_run`, authorization `never_evaluated` |
| `cargo run -q -p opencontinuity-cli -- research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental --json` | Correctly exits 1 with JSON `error_class: system_bus_unavailable`; no registration was attempted; authorization `never_evaluated` |

## GitHub Actions and pull request

P005 branch push and PR target `main` are required. Hosted CI must be checked on the final pushed head SHA. PR URL, final head SHA, workflow run URL and conclusion will be recorded after publishing this branch; an earlier commit’s run is not evidence for the final head. Do not merge the PR automatically.

## Remaining P001/P005 gaps

- P001 production Bluetooth adapter scaffolding remains incomplete. This separate experiment does not make the product Bluetooth adapter complete.
- The Qt6/QML Settings framework remains absent.
- The authoritative original P001–P060 roadmap remains unavailable; P005 is a provisional continuation.
- Host authentication, production enrollment/revocation storage, P-256 Watch interoperability, relay defenses, Watch approval UI and all Linux authorization remain unimplemented.
