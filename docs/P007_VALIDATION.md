# P007 / P007A validation

`authorization: never_evaluated`

## P007A qualification (2026-10-29)

The P007A results supersede the historical P007 source-only status below. Starting head `bfc6c88e272f86b7c8dbdc411848faed09ab42ad` and preserved commit `1085a26b238ce911ff4eecb139a4a3c5b394fc86` remain ancestors.

- Local Rust 1.85: fmt, Clippy, all 75 workspace tests and build PASS; diff check PASS.
- [Hosted Apple/Rust CI](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40805562115) on `863a81b4e954210ae0fac9ae26d24a62ea19b114`: PASS.
- Apple Swift 6.1.2 (`swiftlang-6.1.2.1.2`); Xcode 16.4, build 16F6; watchOS SDK 11.5.
- Swift package: 9 XCTest tests PASS (4 lifecycle, 5 codec), including shared Rust fixture validation with CryptoKit.
- XcodeGen 2.46.0 generated the application from `experiments/watchos/project.yml`. Scheme `OpenContinuityWatch`, Debug, `generic/platform=watchOS`, deployment target 10.0, `CODE_SIGNING_ALLOWED=NO`: BUILD SUCCEEDED using the actual Apple SDK. This compile does not install or run the app.
- PR #7 remains draft while the final connection-delegate change is qualified; final pushed SHA and CI conclusions appear in the delivery report.
- Physical Linux BlueZ, Apple Watch SE 2, paired iPhone and simulator execution remain NOT RUN. CoreBluetooth long-read delivery over RF remains UNKNOWN.

The lifecycle tests exercise the production pure ownership model, including active-operation scan denial, terminal-event draining, same-ID reconnect, stale/duplicate ticket rejection, and 100 repeated cancel/timeout/reconnect cycles. They do not simulate Apple's radio or delegate delivery internals. Pending operations enforce peripheral, operation kind and exact characteristic object ownership. A per-connection delegate captures an immutable generation; a new central instance after the terminal connection event separates subsequent lifetimes. CoreBluetooth provides no custom operation ID in write callbacks: within a connection, the adapter relies on the documented single completion for each serialized write-with-response. It does not invent an ATT transaction ID or retry a timed-out write in a new operation. A missing terminal connection callback leaves scanning blocked rather than abandoning the connection.

## Historical P007 results (before P007A)

## Revision and source status

- Branch: `feat/p007-watchos-foreground-client`
- Preserved starting commit `1085a26b238ce911ff4eecb139a4a3c5b394fc86` (present as ancestor of the final branch head)
- Implementation commit covered by the passing local Rust gates: `1e84fe84e6e09bf357f2fe8a92c7e7f302587231`; final branch head also includes documentation-only commit(s)
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

- Pull request: [#7](https://github.com/nghianguyen150612/OpenContinuity/pull/7) (OPEN, draft)
- Pushed head at PR creation: `fceec95770825a9ae90dd513270f5afbd4995c85`; current head and Actions status are reported in the delivery report
- Review state: draft until Apple compilation and hardware-dependent gates are assessed
