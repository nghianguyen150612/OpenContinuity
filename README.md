# OpenContinuity

OpenContinuity is an open-source effort to build native Apple ecosystem interoperability for Linux first, with other Unix-like systems and Windows as future targets. The reference desktop is Arch Linux with Hyprland and Caelestia.

- **Desktop app:** OpenContinuity Settings (planned; not included in P001)
- **CLI:** `opencontinuity`
- **Daemon:** `opencontinuityd`
- **Core:** portable Rust 2024 contracts
- **Planned GUI:** Qt 6/QML with a small C++ bridge

## P001 status

P001 is the project foundation. The CLI and idle foreground daemon are usable. No native Apple protocol has been implemented: Watch Auto Unlock, native AWDL/AirDrop, AirPods configuration and switching are all **unsupported in this build**. P008 adds a separate signed Watch approval research profile. P009 adds a separately versioned BlueZ/CoreBluetooth transport, bounded host request and Watch response transfers, and a Rust/Swift software fixture exchange. P009A adds a foreground test-only enrollment workflow with an ephemeral in-memory Watch key and manual verification of both public-key fingerprints. Physical BLE and LocalAuthentication validation remain unrun. This research profile is not Apple Auto Unlock and does not authorize Linux. The doctor command reports capabilities honestly and does not scan for devices. See [Feature Matrix](docs/FEATURE_MATRIX.md) and [Roadmap](docs/ROADMAP.md).

## Build and run

Install Rust 1.85.0 or newer with rustfmt and Clippy. Then run:

```sh
cargo build --workspace
cargo run -p opencontinuity-cli -- --help
cargo run -p opencontinuity-cli -- doctor
cargo run -p opencontinuity-cli -- doctor --json
cargo run -p opencontinuity-cli -- research watch --fixture tests/fixtures/watch-discovery.json --json
cargo run -p opencontinuity-cli -- research watch-endpoint --fixture tests/fixtures/watch-endpoint.json --json
cargo run -p opencontinuity-cli -- research watch-auth --fixture path/to/signed-test-fixture.json --json
cargo run -p opencontinuity-cli -- research watch-ble --capabilities --json
cargo run -p opencontinuity-cli -- research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental --json
cargo run -p opencontinuityd -- --help
cargo run -p opencontinuityd
```

The daemon runs in the foreground in a safe idle state and exits on Ctrl-C or SIGTERM. `research watch` and `research watch-endpoint` use only local fixtures and deterministic mocks. `research watch-auth` verifies signed test-only research fixtures without system side effects and always reports `authorization: never_evaluated`. `research watch-auth-transport prepare` and `verify-response` are test-only tools for the serialized Rust/Swift CI exchange; they do not start BLE or authorize Linux. On macOS, `OC_P008_INTEROP_OUTPUT=/tmp/watch-auth-response.json swift test --package-path experiments/watchos` creates a complete P008 test fixture; pass that path to the CLI to check both signatures. `research watch-ble --capabilities` remains read-only. `research watch-gatt serve` is a separate experimental synthetic GATT service. P009A adds `research watch-auth-gatt generate-host-key`, `export-host-pin` and `check-enrollment`; `serve` requires explicit `--experimental`, a selected adapter, a 1–60 second runtime, schema-2 test-only enrollments, a separate mode-0600 host key, and exact host/Watch SHA-256 confirmation flags. It signs a fresh 60-second P008 request only after BlueZ registration succeeds. Setup is bounded; request delivery, consent and response delivery have separate monotonic deadlines. The Watch app has an explicit Authenticated Research screen and keeps its ephemeral P-256 private key in memory. Physical signed BLE remains unverified. See [P009A deadline model](docs/research/P009A_DEADLINE_MODEL.md), [temporary enrollment](docs/research/P009A_TEMPORARY_ENROLLMENT.md), [security review](docs/research/P009A_SECURITY_REVIEW.md), and [validation](docs/P009A_VALIDATION.md).

## Architecture

`crates/opencontinuity-core` contains portable capability and trust contracts. The CLI presents the report; the daemon currently provides a nonprivileged lifecycle only. OS adapters, protocol implementations and the Settings UI are future modules and must remain outside portable core. Read [Architecture](docs/ARCHITECTURE.md) and [Threat Model](docs/THREAT_MODEL.md) before adding sensitive functionality.

## Project policies

The project prioritizes genuine Apple protocol compatibility. A custom transport or replacement must never be labelled as a native Apple implementation. Device detection, trusted identity, cryptographic authentication, freshness, proximity assurance and operational readiness are separate claims. Nearby Bluetooth advertisements do not authorize OS unlock.

There is no telemetry. See [Security](SECURITY.md), [Contributing](CONTRIBUTING.md), and [Upstream Manifest](docs/UPSTREAM_MANIFEST.md) for security and licensing rules. The supplied P001–P060 roadmap was not present in the checkout or workspace during bootstrap; the P001 objectives in the project request are the planning source used here.

## License

OpenContinuity is MIT-licensed, as established by the repository's original [LICENSE](LICENSE). That license does not grant rights to third-party source, protocol specifications, trademarks, keys, or media. GPLv3 dependencies can impose source and licensing obligations on combined distributions; evaluate every dependency and linking boundary before adoption. See the upstream manifest.

### P008 provisional signed approval research

P008 adds Rust P-256 request/response verification, CryptoKit-compatible Swift transcripts, test-only independent enrollment pins and a foreground LocalAuthentication consent component that is separate from GATT v1. The component does not unlock Linux. Hosted Apple compilation and physical Watch/BlueZ behavior remain separate gates. See the [protocol](docs/research/P008_AUTH_PROTOCOL.md), [enrollment boundaries](docs/research/P008_ENROLLMENT_TRUST.md), [consent research](docs/research/P008_WATCH_CONSENT.md), [security review](docs/research/P008_SECURITY_REVIEW.md) and [validation](docs/P008_VALIDATION.md). The missing original Foundation/Settings roadmap, production adapter and Qt6/QML framework remain unresolved.

### P007B physical BLE qualification infrastructure

P007B provides a [sanitized read-only BlueZ preflight](scripts/qualify-p007b-host.py), [signed Watch installation guide](docs/research/P007B_APPLE_SIGNED_INSTALL.md), [physical BLE runbook](docs/research/P007B_REAL_BLE_RUNBOOK.md), and [validation](docs/P007B_VALIDATION.md). The Watch synthetic transport UI offers a disabled-by-default, bounded (64 event), allowlisted metadata trace, erased when disabled. The P007B software infrastructure has been merged; actual controller RF, Watch SE 2 long reads and physical exchanges remain **NOT RUN**. This is not Apple Watch identity verification or Linux authorization.

## P010 physical qualification (provisional)

Bounded opt-in timing diagnostics, strict evidence manifests and unsigned development
build artifacts support physical qualification. Physical Watch/Arch BLE, signed
exchange, LocalAuthentication and cleanup acceptance remain BLOCKED in the cloud
workspace. Simulation and CI do not establish hardware compatibility. Both research
wire formats and the 60-second signed bound remain unchanged. No Linux authorization
or P011 is implemented; `authorization: never_evaluated`. See [P010 validation](docs/P010_VALIDATION.md).
