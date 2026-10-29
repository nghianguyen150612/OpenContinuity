# OpenContinuity

OpenContinuity is an open-source effort to build native Apple ecosystem interoperability for Linux first, with other Unix-like systems and Windows as future targets. The reference desktop is Arch Linux with Hyprland and Caelestia.

- **Desktop app:** OpenContinuity Settings (planned; not included in P001)
- **CLI:** `opencontinuity`
- **Daemon:** `opencontinuityd`
- **Core:** portable Rust 2024 contracts
- **Planned GUI:** Qt 6/QML with a small C++ bridge

## P001 status

P001 is the project foundation. The CLI and idle foreground daemon are usable. No native Apple protocol has been implemented: Watch Auto Unlock, native AWDL/AirDrop, AirPods configuration and switching are all **unsupported in this build**. P008 adds a separate signed Watch approval research profile; it is not Apple Auto Unlock and does not authorize Linux. The doctor command reports capabilities honestly and does not scan for devices. See [Feature Matrix](docs/FEATURE_MATRIX.md) and [Roadmap](docs/ROADMAP.md).

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

The daemon runs in the foreground in a safe idle state and exits on Ctrl-C or SIGTERM. `research watch` and `research watch-endpoint` use only local fixtures and deterministic mocks. `research watch-auth` verifies signed test-only research fixtures without system side effects and always reports `authorization: never_evaluated`. On macOS, `OC_P008_INTEROP_OUTPUT=/tmp/watch-auth-response.json swift test --package-path experiments/watchos` creates a complete test fixture; pass that path to the CLI to check both signatures. `research watch-ble --capabilities` remains read-only. `research watch-gatt serve` is a separate experimental command requiring explicit `--experimental`, an adapter, and a 1–300 second runtime; it temporarily registers a synthetic GATT service and advertisement, without powering on the adapter or authorizing Linux. Experimental Watch source exists, but real BlueZ/Watch communication is not claimed. See [P005 server](docs/research/P005_BLUEZ_GATT_SERVER.md), [wire contract](docs/research/P005_WIRE_CONTRACT.md), [P008 protocol](docs/research/P008_AUTH_PROTOCOL.md), [security review](docs/research/P008_SECURITY_REVIEW.md), and [validation](docs/P008_VALIDATION.md).

## Architecture

`crates/opencontinuity-core` contains portable capability and trust contracts. The CLI presents the report; the daemon currently provides a nonprivileged lifecycle only. OS adapters, protocol implementations and the Settings UI are future modules and must remain outside portable core. Read [Architecture](docs/ARCHITECTURE.md) and [Threat Model](docs/THREAT_MODEL.md) before adding sensitive functionality.

## Project policies

The project prioritizes genuine Apple protocol compatibility. A custom transport or replacement must never be labelled as a native Apple implementation. Device detection, trusted identity, cryptographic authentication, freshness, proximity assurance and operational readiness are separate claims. Nearby Bluetooth advertisements do not authorize OS unlock.

There is no telemetry. See [Security](SECURITY.md), [Contributing](CONTRIBUTING.md), and [Upstream Manifest](docs/UPSTREAM_MANIFEST.md) for security and licensing rules. The supplied P001–P060 roadmap was not present in the checkout or workspace during bootstrap; the P001 objectives in the project request are the planning source used here.

## License

OpenContinuity is MIT-licensed, as established by the repository's original [LICENSE](LICENSE). That license does not grant rights to third-party source, protocol specifications, trademarks, keys, or media. GPLv3 dependencies can impose source and licensing obligations on combined distributions; evaluate every dependency and linking boundary before adoption. See the upstream manifest.

### P008 provisional signed approval research

P008 adds Rust P-256 request/response verification, CryptoKit-compatible Swift transcripts, test-only independent enrollment pins and a foreground LocalAuthentication consent component that is separate from GATT v1. The component does not unlock Linux. Hosted Apple compilation and physical Watch/BlueZ behavior remain separate gates. See the [protocol](docs/research/P008_AUTH_PROTOCOL.md), [enrollment boundaries](docs/research/P008_ENROLLMENT_TRUST.md), [consent research](docs/research/P008_WATCH_CONSENT.md), [security review](docs/research/P008_SECURITY_REVIEW.md) and [validation](docs/P008_VALIDATION.md). The missing original Foundation/Settings roadmap, production adapter and Qt6/QML framework remain unresolved.
