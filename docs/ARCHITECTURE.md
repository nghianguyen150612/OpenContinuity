# Architecture

## P001 workspace

```text
crates/opencontinuity-core   Portable serialized domain contracts
apps/opencontinuity-cli      opencontinuity command and doctor report
services/opencontinuityd    Nonprivileged foreground lifecycle
```

The Rust workspace uses edition 2024. `opencontinuity-core` depends only on serde and does not import BlueZ, systemd, PAM, Wayland, Qt, Linux paths or process-global system services. The CLI owns human-readable and JSON presentation. The daemon currently installs only a shutdown handler and idles until Ctrl-C/SIGTERM.

The core capability vocabulary is `supported`, `experimental`, `unsupported`, `blocked`, and `unknown`. A `TrustAssessment` keeps detection, trusted identity, cryptographic authentication, session freshness, proximity assurance and operational readiness distinct. Defaults are unknown. The JSON doctor schema is versioned and covered by tests.

P002 adds `opencontinuity_core::research`, a portable, sanitized fixture model and `DiscoveryAdapter` boundary with a deterministic mock backend. The CLI research command reads a local JSON fixture only. `DiscoveryReport` always carries `authorization: never_evaluated`; its type has no authorized state. This is research scaffolding, not a BlueZ adapter, actual discovery, or an unlock path. Raw addresses, payload bytes, and credentials are excluded from the fixture schema.

P003 adds `opencontinuity_core::watch_endpoint`, portable endpoint/transport traits, typed capability evidence, bounded versioned challenge/approval messages and deterministic mocks. The mock uses `ed25519-dalek` to exercise transcript-bound signatures and replay/expiry/presence rejection. `opencontinuity research watch-endpoint --fixture <path> [--json]` reports simulated transport/authentication state and always emits `authorization: never_evaluated`. Its test-only private seed and deterministic SHA-256 nonce sequence are public mock data, never production secrets or a CSPRNG. No Apple SDK, Bluetooth, network, keychain, device, PAM or host authorization integration exists. See the P003 validation and security design for boundaries and limitations.

P004 adds a separate `MockEnrollmentRecord` trust anchor with host/account/Watch identities, a tagged algorithm and version, lifecycle and revocation state. The verifier uses that record rather than taking its expected key from the endpoint under test; deterministic substitution tests reject a different signer. This remains mock-only with no persistent enrollment store. `apps/opencontinuity-cli/src/bluez.rs` retains **read-only** `busctl` capability inspection; it classifies interfaces as supported/unsupported/unknown/blocked and controller capability as not tested. The test uses injected strings and no longer expects this host to lack BlueZ.

P005 adds `crates/opencontinuity-core::gatt_wire`, a bounded versioned framing and reassembly contract, plus a Linux-only `apps/opencontinuity-cli/src/gatt.rs` zbus 5.12.0 D-Bus server. `opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental [--json]` explicitly exports a GATT ObjectManager, service and characteristics, registers them with `GattManager1`, registers a temporary advertisement through `LEAdvertisingManager1`, and tears down owned resources on timeout/signal/adapter loss. No bus or controller is available in this validation environment, so the real registration path has not been exercised. The portable mock lifecycle model still tests deterministic rollback and ordering; it is not evidence of real BlueZ behavior. Reports retain `authorization: never_evaluated`.

The CLI’s BlueZ adapter remains research-only. P007 adds experimental native watchOS SwiftUI/CoreBluetooth client source under `experiments/watchos/` with a pure protocol codec and a foreground-only GATT session flow. P008 adds a separate portable `watch_auth_research` module and Swift `WatchAuthProtocol` CryptoKit module for P-256 signed request/response transcripts. It does not reuse or modify GATT v1. Rust and Swift each verify independently pinned, test-only enrollment records; a small SwiftUI LocalAuthentication consent component accepts only a host request already verified by the Swift module. The fixture CLI has no Bluetooth, key-store, account-management or Linux authentication side effects and reports `authorization: never_evaluated`. Hosted Apple SDK compilation and actual cross-language execution remain separate gates. P001 production Bluetooth adapter scaffolding, the Qt6/QML Settings application, persistent enrollment, key custody and every Linux authorization integration remain absent.

## Planned module boundaries

Add modules when there is concrete implementation work:

- `crates/opencontinuity-core/src/`: portable contracts and state machines
- `crates/opencontinuity-protocols/src/`: protocol framing and interoperable protocol logic, with independent tests
- `crates/opencontinuity-platform/src/`: OS-facing adapter interfaces
- `platforms/linux/`: Linux and BlueZ-specific integration
- `platforms/freebsd/` and `platforms/windows/`: future platform adapters
- `apps/opencontinuity-settings/`: future Qt 6/QML UI and minimal C++ bridge
- `tests/fixtures/`: redacted protocol fixtures; no personal secrets or captures

These are planned boundaries, not P001 placeholder crates. Portable domain decisions must not depend on an adapter implementation. Protocol and UI code must report genuine Apple compatibility separately from experiments and custom replacements.

## Doctor contract

`opencontinuity doctor --json` emits `DoctorReport` with `schema_version`, `project`, `version`, `capabilities`, and `watch_auto_unlock_trust`. It reports native Apple integrations as unsupported; the P008 custom research protocol does not change those states. It does not inspect Bluetooth or infer host readiness. The production trust stages remain unknown. Consumers should reject unknown schema versions and treat trust as fail-closed.

## P006 provisional transport hardening

The research GATT server now has per-client phase-based result ownership, bounded sessions and method calls, explicit cleanup reporting, and a read-only host qualification script. See [session hardening](research/P006_GATT_SESSION_HARDENING.md). Device paths and BlueZ connection events are not authentication; the ordered event monitor invalidates reported disconnects while polling is supplementary. Real BlueZ and Watch behavior remain NOT RUN; P007 client source now exists but has not been compiled with an Apple SDK. The production adapter and Qt6/QML Settings framework remain absent. Reports retain `authorization: never_evaluated`.
