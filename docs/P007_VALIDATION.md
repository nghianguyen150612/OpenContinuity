# P007 validation

`authorization: never_evaluated`

## Revision and source status

- Branch: `feat/p007-watchos-foreground-client`
- Preserved starting commit `1085a26b238ce911ff4eecb139a4a3c5b394fc86` (present as ancestor of the final branch head)
- Implementation commit under validation: `1e84fe8` (full SHA available in git history); documentation changes follow separately
- Native SwiftUI/CoreBluetooth foreground source: implemented; **NOT COMPILED**
- Protocol source: implemented; CryptoKit code is not executed in this environment
- Linux GATT server/BlueZ code: unchanged

## Rust checks

Environment: Debian GNU/Linux 13 x86_64; rustc 1.85.0 and Cargo 1.85.0 installed user-locally to match `rust-toolchain.toml`.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS, 75 tests, 0 failures |
| `cargo build --workspace` | PASS |
| Rust fixture generation (`cargo run -p opencontinuity-core --example generate_p007_fixture`) | PASS |
| Rust fixture validation (`p007_shared_swift_fixture_matches_rust_wire_encoders`) | PASS |
| `git diff --check` | PASS |

The preserved read-only `opencontinuity research watch-ble --capabilities --json` command was run locally; it reported `system_bus_unavailable`, no adapters inspected, hardware observations `not_run`, and `authorization: never_evaluated`. No Linux GATT server was started. The shared vector was generated from the Rust frame/control encoders and Rust SHA-256 implementation. Swift tests reference that JSON, but the Swift suite could not be run. Therefore **Swift fixture validation: NOT RUN; cross-language runtime parity: UNVERIFIED**.

## Apple and Swift qualification

| Stage | Result |
| --- | --- |
| Swift compiler / `swift test` | NOT RUN; `swift` and `swiftc` are unavailable |
| macOS | NOT AVAILABLE; host is Debian Linux |
| Xcode / watchOS SDK | NOT RUN / unavailable |
| watchOS target compilation and signing | NOT RUN |
| Apple Watch simulator | NOT RUN |
| Physical Apple Watch SE 2 and paired iPhone | NOT RUN |
| CoreBluetooth long reads (25-byte challenge and 44-byte result) | UNKNOWN; requires physical Watch/BlueZ test |

The Swift Package and documented Xcode target setup are source artifacts, not evidence of Apple SDK compatibility. No generic Linux compile is substituted for a watchOS build.

## Linux and hardware stages

| Stage | Result |
| --- | --- |
| Existing Rust BlueZ mock regression tests | PASS as part of workspace suite |
| Local BlueZ system bus and GATT registration | NOT RUN |
| Physical Linux BLE adapter and powered controller | NOT RUN |
| On-air advertisement and independent scan | NOT RUN |
| Physical service discovery / five characteristics | NOT RUN |
| Watch frame exchange, result digest and cancellation | NOT RUN |
| Physical teardown / reconnect | NOT RUN |

See `docs/research/P007_HARDWARE_TEST_PLAN.md` for the separated Linux and Watch evidence procedure. Mock D-Bus tests do not establish RF interoperability. P006A invalidates sessions when disconnect signals are received in its defined D-Bus ordering; it does not guarantee that every physical disconnect is observed instantaneously.

## Scope and security

P007 performs synthetic foreground transport only. `authorization: never_evaluated` remains the reported policy. No PAM, polkit, login manager, Linux unlock, Apple STS enrollment, Apple credential access, Watch identity verification, RSSI trust, pairing-based authentication, or background listener is added. P003/P004 mock signing remains separate. The original P001–P060 roadmap remains unavailable; Foundation/Qt6/QML Settings and production Bluetooth adapter work remain outstanding.

## Hosted review

- Pull request: to be recorded after creation
- Final pushed head SHA and Actions: to be recorded after push
- Review state: draft until Apple compilation and hardware-dependent gates are assessed
