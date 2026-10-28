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

P004 adds a separate `MockEnrollmentRecord` trust anchor with host/account/Watch identities, a tagged algorithm and version, lifecycle and revocation state. The verifier uses that record rather than taking its expected key from the endpoint under test; deterministic substitution tests reject a different signer. This remains mock-only with no persistent enrollment store. `apps/opencontinuity-cli/src/bluez.rs` performs **read-only** `busctl` object/interface inspection. `opencontinuity research watch-ble --capabilities [--json]` cannot register services or advertise. No BlueZ D-Bus object server, Swift/watchOS target, host-signed challenge, pairing, or Linux authorization path is implemented. Reports always say `authorization: never_evaluated`.

`opencontinuity_core::bluez_research` is a deterministic lifecycle model for explicit opt-in, bounded-duration registration, unsupported capabilities, application/advertisement registration failure, and idempotent cleanup on timeout, signal, adapter loss, or manual stop. It is a test model only and never calls BlueZ; real registration remains blocked in this environment.

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

`opencontinuity doctor --json` emits `DoctorReport` with `schema_version`, `project`, `version`, `capabilities`, and `watch_auto_unlock_trust`. It reports protocol features as unsupported because no implementation exists. It does not inspect Bluetooth or infer host readiness. The trust stages remain unknown. Consumers should reject unknown schema versions and treat trust as fail-closed.
