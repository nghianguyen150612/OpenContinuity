# P014 validation — live Bluetooth LE discovery stream

Date: 2026-10-31. Branch: `feat/p014-live-ble-discovery-stream`.
Starting head: `21dbae3b0d3957c365c6de954c60b0aa13982c98`.
Base main: `c7a7bc58958e9b112580df39477096ef29152e54`.
This record distinguishes software evidence from physical radio qualification.

## Local software validation

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, Rust 1.85.0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS, 172 tests |
| `cargo build --workspace` | PASS |
| `python3 -m unittest discover -s scripts/tests -v` | PASS, 7 tests |
| `git diff --check` | PASS |
| Qt 6.4.2 CMake/Ninja/CTest and QML smoke | NOT RUN locally: CMake, Ninja, and Qt development headers are absent; hosted CI pending |
| Swift/watchOS regression and Rust→Swift→Rust checks | NOT RUN locally: Swift/Xcode are unavailable on this Linux execution host; hosted macOS CI pending |

Rust mock BlueZ tests use private `dbus-daemon` instances and the production zbus
discovery path. JSONL tests cover ordering, updates, cancellation, closed stdout,
blocked stdout with a reduced pipe, limits, preflight failure, and a single
Start/Stop lifecycle. They do not require or establish Bluetooth hardware. Qt
fixtures and offscreen QML tests, when run in CI, are software evidence only.

## GitHub delivery

Draft PR URL, final head, and final-head push/PR workflow results are recorded
after publication. Earlier P013 green checks are inherited baseline evidence,
not a substitute for checking this P014 head.

## Physical evidence and merge boundary

**Arch Bluetooth LE physical testing: NOT RUN.** No owned Arch controller,
controlled peripheral, or live radio trial is available in this execution
workspace. See the [Arch physical plan](research/P014_ARCH_PHYSICAL_PLAN.md).

P010 Apple Watch SE 2 qualification remains **NO-GO**. P014 software tests do
not establish watchOS behavior, RF provenance, Apple identity, proximity,
Apple Watch Auto Unlock, AirDrop/AWDL, AirPods controls, or Linux authorization.
Every event keeps `authorization: never_evaluated`.

## Final results

This section is completed after all requested local gates and final-head GitHub
checks have run. Any unavailable gate, failure, or remaining merge blocker is
listed explicitly.
