# P004 validation

**Milestone:** foreground Apple Watch ↔ Linux BLE GATT feasibility spike.
**Branch:** `research/p004-watch-ble-gatt-spike`.
**Base:** P004 start commit `8ef4e2f85a30fcc23e32bef8394439806a4501b6`, parent P003 merge `1d5d3f5a19f81727a5d3deef61d24de5f82d4ed4`.
**Final implementation head SHA:** reported with the pull request after commit; this file cannot contain its own commit hash.

## Implementation and evidence classification

- **VERIFIED:** separate mock enrollment record binds enrollment ID, Linux host, account, Watch endpoint, Ed25519 mock algorithm/version, lifecycle and revocation. Verification does not read the untrusted endpoint's public key.
- **VERIFIED:** a portable mock BlueZ lifecycle model covers explicit opt-in, duration bounds, unsupported capabilities, application/advertisement registration failures, reverse-order idempotent cleanup, timeout, SIGINT, SIGTERM and adapter loss. It is not connected to D-Bus and does not perform registration.
- **VERIFIED:** negative mock tests reject unknown/revoked/inactive enrollment, unsupported algorithm, wrong host/account/Watch binding, invalid signatures, key substitution, replay, stale/future response, missing confirmation, denial, transport loss, malformed or oversized protocol data, and incompatible version.
- **VERIFIED:** `opencontinuity research watch-ble --capabilities --json` exists and is read-only. On this environment it reported `bluez_service_or_system_bus_unavailable`, `bluez_version: null`, `adapters: []`, `hardware_watch_observations: not_run`, `authentication_test_outcome: not_run`, and `authorization: never_evaluated`.
- **DOCUMENTED:** BlueZ D-Bus application/advertisement APIs and the need to discover both manager interfaces and actual controller support. No GATT service/advertisement runtime was implemented.
- **BLOCKED:** host environment lacks a system bus, BlueZ daemon/tools and Bluetooth controller; no adapter API or RF operation was exercised.
- **DOCUMENTED:** watchOS foreground `CBCentralManager` feasibility and public CryptoKit P-256 types. **BLOCKED / NOT RUN:** no Xcode/watchOS SDK/Apple signing environment. **NOT RUN:** no Apple Watch SE 2, paired iPhone or Apple hardware observation. No Swift application source/build claim exists.
- **DOCUMENTED:** host-to-Watch mutual authentication requirement. Host-signed challenge verification is **NOT IMPLEMENTED**; no real Watch app may display approval until host identity is independently authenticated.
- **VERIFIED:** Linux authorization remains unavailable by construction in this branch. All P004 reports use `authorization: never_evaluated`; no PAM, polkit, login manager, lock screen, or privileged authentication service was modified.
- The authoritative original P001–P060 planning roadmap remains unavailable. P004 scope is provisional. The Qt6/QML Settings framework and production Bluetooth adapter scaffolding remain unresolved.

## Local commands and results

Environment: Debian 13 container, Linux x86_64 kernel `6.18.55`; Rust `1.85.0`. Rust toolchain was installed for this workspace session. `bluetoothctl`, `btmgmt`, `bluetoothd`, `swift`, and `xcodebuild` were not available. `/usr/bin/busctl` exists but cannot connect to the system bus.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `cargo test --workspace --all-targets` | **PASS** — 38 tests, 0 failed (4 CLI unit, 7 CLI command, 26 core, 1 daemon command; daemon unit target has 0 tests) |
| `cargo build --workspace` | **PASS** |
| `git diff --check` | **PASS** |
| `cargo run -q -p opencontinuity-cli -- research watch-ble --capabilities --json` | **PASS** diagnostic command; accurately reports no BlueZ service/system bus and no controller claims |
| Existing doctor/watch/watch-endpoint commands | **PASS** in workspace tests, including deterministic P003 mock behavior |
| BlueZ adapter/GATT hardware test | **BLOCKED / NOT RUN** |
| watchOS compile, signing, simulator, or physical Watch test | **BLOCKED / NOT RUN** |

Host-only tests establish Rust contracts and CLI behavior only. They do not establish BLE transport interoperability, controller support, Secure Enclave behavior, or Apple Watch compatibility.

## Hosted CI and pull request

The implementation is committed and pushed to the P004 feature branch; the PR targets `main`. Hosted CI result and run URL must be read from GitHub Actions for the final PR head SHA. A pending or earlier-commit run is not a pass. See the final delivery report for the exact head SHA, PR URL, and final-head run conclusion.

No merge is authorized by this milestone. No P005 work is started.
