# OpenContinuity

OpenContinuity is an open-source effort to build native Apple ecosystem interoperability for Linux first, with other Unix-like systems and Windows as future targets. The reference desktop is Arch Linux with Hyprland and Caelestia.

- **Desktop app:** OpenContinuity Settings (planned; not included in P001)
- **CLI:** `opencontinuity`
- **Daemon:** `opencontinuityd`
- **Core:** portable Rust 2024 contracts
- **Planned GUI:** Qt 6/QML with a small C++ bridge

## P001 status

P001 is the project foundation. The CLI and idle foreground daemon are usable. No Apple protocol has been implemented yet: Watch Auto Unlock, native AWDL/AirDrop, AirPods configuration and switching are all **unsupported in this build**. The doctor command reports that honestly and does not scan for devices. See [Feature Matrix](docs/FEATURE_MATRIX.md) and [Roadmap](docs/ROADMAP.md).

## Build and run

Install Rust 1.85.0 or newer with rustfmt and Clippy. Then run:

```sh
cargo build --workspace
cargo run -p opencontinuity-cli -- --help
cargo run -p opencontinuity-cli -- doctor
cargo run -p opencontinuity-cli -- doctor --json
cargo run -p opencontinuity-cli -- research watch --fixture tests/fixtures/watch-discovery.json --json
cargo run -p opencontinuity-cli -- research watch-endpoint --fixture tests/fixtures/watch-endpoint.json --json
cargo run -p opencontinuity-cli -- research watch-ble --capabilities --json
cargo run -p opencontinuity-cli -- research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental --json
cargo run -p opencontinuityd -- --help
cargo run -p opencontinuityd
```

The daemon runs in the foreground in a safe idle state and exits on Ctrl-C or SIGTERM. `research watch` and `research watch-endpoint` use only local fixtures and deterministic mocks. `research watch-ble --capabilities` remains read-only. `research watch-gatt serve` is a separate experimental command requiring explicit `--experimental`, an adapter, and a 1–300 second runtime; it temporarily registers a synthetic GATT service and advertisement, without powering on the adapter or authorizing Linux. This repository has no Apple Watch app, and real BlueZ/Watch communication is not claimed. See [P005 server](docs/research/P005_BLUEZ_GATT_SERVER.md), [wire contract](docs/research/P005_WIRE_CONTRACT.md), [security review](docs/research/P005_SECURITY_REVIEW.md), and [validation](docs/P005_VALIDATION.md).

## Architecture

`crates/opencontinuity-core` contains portable capability and trust contracts. The CLI presents the report; the daemon currently provides a nonprivileged lifecycle only. OS adapters, protocol implementations and the Settings UI are future modules and must remain outside portable core. Read [Architecture](docs/ARCHITECTURE.md) and [Threat Model](docs/THREAT_MODEL.md) before adding sensitive functionality.

## Project policies

The project prioritizes genuine Apple protocol compatibility. A custom transport or replacement must never be labelled as a native Apple implementation. Device detection, trusted identity, cryptographic authentication, freshness, proximity assurance and operational readiness are separate claims. Nearby Bluetooth advertisements do not authorize OS unlock.

There is no telemetry. See [Security](SECURITY.md), [Contributing](CONTRIBUTING.md), and [Upstream Manifest](docs/UPSTREAM_MANIFEST.md) for security and licensing rules. The supplied P001–P060 roadmap was not present in the checkout or workspace during bootstrap; the P001 objectives in the project request are the planning source used here.

## License

OpenContinuity is MIT-licensed, as established by the repository's original [LICENSE](LICENSE). That license does not grant rights to third-party source, protocol specifications, trademarks, keys, or media. GPLv3 dependencies can impose source and licensing obligations on combined distributions; evaluate every dependency and linking boundary before adoption. See the upstream manifest.

### P006 provisional BlueZ qualification

Run `scripts/qualify-bluez-host.sh` for read-only host inventory; see [host qualification](docs/research/P006_BLUEZ_HOST_QUALIFICATION.md) before any explicitly opted-in, time-bounded GATT trial. P007 provides experimental foreground watchOS SwiftUI/CoreBluetooth source and a portable pure protocol module; Apple compilation and Watch tests are NOT RUN in the development environment. See [P007 implementation](docs/research/P007_WATCHOS_CLIENT.md) and [validation](docs/P007_VALIDATION.md). BlueZ controller tests remain NOT RUN. The synthetic transport does not authenticate or authorize. P006 continues the provisional BLE research track; the missing original Foundation/Settings roadmap, production adapter and Qt6/QML framework remain unresolved.

P007B provides a [physical BLE runbook](docs/research/P007B_REAL_BLE_RUNBOOK.md), [signed Watch installation guide](docs/research/P007B_APPLE_SIGNED_INSTALL.md), and [evidence/blocker report](docs/P007B_VALIDATION.md). The Watch client has optional bounded metadata diagnostics; actual Watch/BlueZ physical interoperability remains unverified.
