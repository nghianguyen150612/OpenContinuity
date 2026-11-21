# OpenContinuity

OpenContinuity is an open-source effort to build native Apple ecosystem interoperability for Linux first, with other Unix-like systems and Windows as future targets. The reference desktop is Arch Linux with Hyprland and Caelestia.

- **Desktop app:** OpenContinuity Settings (Qt 6/QML)
- **CLI:** `opencontinuity`
- **Daemon:** `opencontinuityd`
- **Core:** portable Rust 2024 contracts
- **Settings:** native Qt 6/QML with a small C++ bridge

## P001 status

The CLI and idle foreground daemon are usable. P019 adds an experimental optional Owly receiver process for an already-prepared AWDL interface; it does not implement an AWDL radio driver, activate Wi-Fi, guarantee iPhone discovery or establish genuine AirDrop interoperability. Apple Watch Auto Unlock and AirPods configuration remain unsupported. P008/P009 are separate signed Watch research protocols, not Apple Auto Unlock, and do not authorize Linux. Physical BLE, AirDrop and LocalAuthentication validation remain unrun. The static doctor does not scan, and `airdrop doctor` is read-only. See [Feature Matrix](docs/FEATURE_MATRIX.md) and [Roadmap](docs/ROADMAP.md).

## Build and run

Install Rust 1.85.0 or newer with rustfmt and Clippy. Then run:

```sh
cargo build --workspace
cargo run -p opencontinuity-cli -- --help
cargo run -p opencontinuity-cli -- doctor
cargo run -p opencontinuity-cli -- doctor --json
cargo run -p opencontinuity-cli -- airdrop doctor --json
cargo run -p opencontinuity-cli -- devices list --json
cargo run -p opencontinuity-cli -- research watch --fixture tests/fixtures/watch-discovery.json --json
cargo run -p opencontinuity-cli -- research watch-endpoint --fixture tests/fixtures/watch-endpoint.json --json
cargo run -p opencontinuity-cli -- research watch-auth --fixture path/to/signed-test-fixture.json --json
cargo run -p opencontinuity-cli -- research watch-ble --capabilities --json
cargo run -p opencontinuity-cli -- research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental --json
cargo run -p opencontinuityd -- --help
cargo run -p opencontinuityd
```

The daemon runs in the foreground in a safe idle state and exits on Ctrl-C or SIGTERM. Research fixture commands are offline and always report `authorization: never_evaluated`. The Watch GATT and signed transport remain separate experimental flows; physical signed BLE remains unverified. For P019, `airdrop doctor --json` only reads local state. The optional receive command requires an already-prepared `awdl0`, explicit operator flags, an existing destination, and a terminal prompt for each incoming request. It does not bring up Owly's radio or auto-accept. See the [P019 scope](docs/research/P019_AIRDROP_AWDL_FOUNDATION_SCOPE.md), [upstream/license audit](docs/research/P019_UPSTREAM_AND_LICENSE_AUDIT.md), [receive security review](docs/research/P019_AIRDROP_RECEIVE_SECURITY.md), [RTL8852BE physical plan](docs/research/P019_RTL8852BE_PHYSICAL_QUALIFICATION.md), and [validation](docs/P019_VALIDATION.md).

## Architecture

`crates/opencontinuity-core` contains portable capability and trust contracts. The CLI presents the report; the daemon currently provides a nonprivileged lifecycle only. The native Qt 6/QML Settings application lives in `apps/opencontinuity-settings/` and reads the CLI through a bounded C++ bridge. OS adapters and protocol implementations remain separate future modules outside portable core. Read [Architecture](docs/ARCHITECTURE.md) and [Threat Model](docs/THREAT_MODEL.md) before adding sensitive functionality.

## Project policies

The project prioritizes genuine Apple protocol compatibility. A custom transport or replacement must never be labelled as a native Apple implementation. Device detection, trusted identity, cryptographic authentication, freshness, proximity assurance and operational readiness are separate claims. Nearby Bluetooth advertisements do not authorize OS unlock.

There is no telemetry. See [Security](SECURITY.md), [Contributing](CONTRIBUTING.md), and [Upstream Manifest](docs/UPSTREAM_MANIFEST.md) for security and licensing rules. The supplied P001–P060 roadmap was not present in the checkout or workspace during bootstrap; the P001 objectives in the project request are the planning source used here.

## License

OpenContinuity is MIT-licensed, as established by the repository's original [LICENSE](LICENSE). That license does not grant rights to third-party source, protocol specifications, trademarks, keys, or media. GPLv3 dependencies can impose source and licensing obligations on combined distributions; evaluate every dependency and linking boundary before adoption. See the upstream manifest.

### P008 provisional signed approval research

P008 adds Rust P-256 request/response verification, CryptoKit-compatible Swift transcripts, test-only independent enrollment pins and a foreground LocalAuthentication consent component that is separate from GATT v1. The component does not unlock Linux. Hosted Apple compilation and physical Watch/BlueZ behavior remain separate gates. See the [protocol](docs/research/P008_AUTH_PROTOCOL.md), [enrollment boundaries](docs/research/P008_ENROLLMENT_TRUST.md), [consent research](docs/research/P008_WATCH_CONSENT.md), [security review](docs/research/P008_SECURITY_REVIEW.md) and [validation](docs/P008_VALIDATION.md). The original roadmap remains unavailable and the production adapter is unresolved. P011 now provides the separate Qt6/QML Settings foundation.

### P007B physical BLE qualification infrastructure

P007B provides a [sanitized read-only BlueZ preflight](scripts/qualify-p007b-host.py), [signed Watch installation guide](docs/research/P007B_APPLE_SIGNED_INSTALL.md), [physical BLE runbook](docs/research/P007B_REAL_BLE_RUNBOOK.md), and [validation](docs/P007B_VALIDATION.md). The Watch synthetic transport UI offers a disabled-by-default, bounded (64 event), allowlisted metadata trace, erased when disabled. The P007B software infrastructure has been merged; actual controller RF, Watch SE 2 long reads and physical exchanges remain **NOT RUN**. This is not Apple Watch identity verification or Linux authorization.

## P010 physical qualification (provisional)

Bounded opt-in timing diagnostics, strict evidence manifests and unsigned development
build artifacts support physical qualification. Physical Watch/Arch BLE, signed
exchange, LocalAuthentication and cleanup acceptance remain BLOCKED in the cloud
workspace. Simulation and CI do not establish hardware compatibility. Both research
wire formats and the 60-second signed bound remain unchanged. No Linux authorization is implemented; `authorization: never_evaluated`. See [P010 validation](docs/P010_VALIDATION.md).

## P011 native Settings (provisional)

A real `opencontinuity-settings` Qt 6/QML desktop application now provides Overview,
Devices, Apple Watch, AirDrop, AirPods, Continuity, Research Lab / Diagnostics and
About pages. P011 introduced an asynchronous C++20 bridge for static `opencontinuity doctor --json`
with strict schema/size/deadline validation; P012 inventory and P013 discovery
use separate providers; P014 makes only discovery live through JSONL streaming. Unsupported
features remain unsupported; no devices, battery readings, transfers or unlock are
invented. The Devices page now provides manual BlueZ known-device snapshots; it
keeps that read-only inventory independent from P013 opt-in discovery. No enabled protocol toggles, daemon control or
research server startup are provided.

See [Settings build/install instructions](apps/opencontinuity-settings/README.md),
[architecture](docs/research/P011_SETTINGS_ARCHITECTURE.md),
[desktop dependencies and tests](docs/research/P011_DESKTOP_BUILD_AND_TEST.md) and
[P011 validation](docs/P011_VALIDATION.md). P010 hardware qualification remains
**NO-GO**.

## P012 Linux known-device inventory (provisional)

The Settings Devices page provides a manual read-only BlueZ snapshot through
`opencontinuity devices list --json`. It lists BlueZ-known objects, not nearby
devices, and never scans, pairs or connects. Apple Watch, AirDrop and AirPods
capabilities remain unsupported. See the [snapshot schema](docs/research/P012_DEVICE_SNAPSHOT_SCHEMA.md),
[UI behavior](docs/research/P012_SETTINGS_DEVICES_UI.md), [privacy boundary](docs/research/P012_PRIVACY_AND_THREAT_MODEL.md)
and [P012 validation](docs/P012_VALIDATION.md). P010 physical qualification remains NO-GO.

## P013 bounded Bluetooth LE discovery (provisional)

Run `opencontinuity devices discover --seconds 10 --json` to explicitly use the
Bluetooth radio for a bounded session (1–30 seconds; default 10). Settings Devices
has **Scan Bluetooth LE for 10 seconds**, progress, Cancel, sanitized event cards
and cleanup warnings. Nothing scans automatically. The separate known-device
inventory and DoctorReport v1 are unchanged.

Results are BlueZ events received during the session, not verified RF sightings,
Apple identity, proximity or authentication. No addresses, paths or raw advertising
data enter the report. Every report retains `authorization: never_evaluated`.
See the [lifecycle](docs/research/P013_BLUEZ_DISCOVERY_LIFECYCLE.md),
[schema](docs/research/P013_DISCOVERY_SCHEMA.md),
[security review](docs/research/P013_DISCOVERY_SECURITY_REVIEW.md) and
[validation](docs/P013_VALIDATION.md).

## P014 live discovery streaming (provisional)

The CLI adds the separate `devices discover --events-jsonl` format while keeping
P013 `--json` unchanged. Settings consumes the versioned event stream as it
arrives and updates **Unverified BlueZ observations** live, including changed
sanitized names, elapsed time, unique count, Cancel and validated cleanup
outcome. A bounded writer stops safely on blocked output; the same P013 owner,
filter, Start/Stop and connection lifecycle remains authoritative. Known-device
inventory stays in its own P012 model. Scanning remains explicit and never starts
on launch or navigation.

See the [event protocol](docs/research/P014_DISCOVERY_STREAM_PROTOCOL.md),
[lifecycle/backpressure design](docs/research/P014_STREAM_LIFECYCLE_AND_BACKPRESSURE.md),
[Settings behavior](docs/research/P014_SETTINGS_LIVE_SCAN_UI.md),
[privacy review](docs/research/P014_PRIVACY_SECURITY_REVIEW.md),
[Arch physical plan](docs/research/P014_ARCH_PHYSICAL_PLAN.md), and
[validation](docs/P014_VALIDATION.md). Physical Arch discovery is NOT RUN;
Watch SE 2 qualification remains NO-GO. Native Apple features remain unsupported.

For a read-only, sanitized Arch/BlueZ preflight and a separately opt-in physical qualification procedure, see [P015 Arch qualification](docs/research/P015_ARCH_BLUEZ_QUALIFICATION.md). No scan is started by the preflight.

P016 adds adversarial production stream and Qt lifecycle tests plus a local,
privacy-safe evidence validator. See [P016 validation](docs/P016_VALIDATION.md). Software fixtures
do not qualify physical RF, native Wayland/Hyprland or Apple Watch SE 2; those
hardware gates remain NOT RUN and Watch remains NO-GO.

## P017 operator-controlled qualification (provisional)

P017 adds a read-only-by-default host qualification runner with an explicitly
consented bounded discovery mode, P016 evidence validation, bounded child
termination and privacy-safe summaries. [Operator guide](docs/research/P017_SAFE_HOST_QUALIFICATION_TOOL.md) and
[validation](docs/P017_VALIDATION.md) separate software results from physical gates.
Actual Arch RF/cleanup, native Wayland/Hyprland and signed Watch SE 2 trials are
NOT RUN; P010 Watch acceptance and native Apple Auto Unlock remain NO-GO.
Linux remains the experimental GATT peripheral and Watch the foreground central.
No authorization is evaluated; no pairing/trust, policy or login changes occur.

## P018 foreground GATT trial diagnostics (provisional)

The existing P006 `watch-gatt serve` and P009 `watch-auth-gatt serve` accept
`--diagnostics` only under `--experimental`. This emits bounded host lifecycle
metadata to stderr without changing stdout JSON. The foreground Watch reuses the
P007B opt-in diagnostics screen, uses a local trial label and clears capture on
opt-out/background. Pass the label displayed on Watch to Linux with
`--trial-id <32-lowercase-hex>` to let the offline checker group the files.
Matching labels are operator-controlled and do not prove that the devices
communicated. See the [diagnostic schema](docs/research/P018_WATCH_GATT_DIAGNOSTIC_SCHEMA.md),
[phase map](docs/research/P018_HOST_AND_WATCH_PHASE_MAPPING.md),
[privacy review](docs/research/P018_OFFLINE_EVIDENCE_AND_PRIVACY.md),
[lifecycle security review](docs/research/P018_GATT_LIFECYCLE_SECURITY_REVIEW.md),
[runbook](docs/research/P018_WATCH_SE2_PHYSICAL_TRIAL_RUNBOOK.md) and
[validation](docs/P018_VALIDATION.md). W0–W6 real hardware trials remain NOT RUN;
P010 remains NO-GO and `authorization: never_evaluated`.
