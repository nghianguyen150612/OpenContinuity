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
| Qt 6.4.2 CMake/Ninja/CTest and QML smoke | NOT RUN locally: CMake, Ninja, and Qt development headers are absent; passed in hosted CI |
| Swift/watchOS regression and Rust→Swift→Rust checks | NOT RUN locally: Swift/Xcode are unavailable on this Linux execution host; passed in hosted macOS CI |

Rust mock BlueZ tests use private `dbus-daemon` instances and the production zbus
discovery path. JSONL tests cover ordering, updates, cancellation, closed stdout,
blocked stdout with a reduced pipe, limits, preflight failure, and a single
Start/Stop lifecycle. They do not require or establish Bluetooth hardware. Qt
fixtures and offscreen QML tests, when run in CI, are software evidence only.

## GitHub delivery

Draft PR: https://github.com/nghianguyen150612/OpenContinuity/pull/15

Validated code head: `45a4855b4f3e93e15e8681d7baaa3ed206402998`.

| Workflow | Run | Result |
| --- | --- | --- |
| Push CI | [41206377962](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/41206377962) | PASS — Rust checks, Swift/watchOS, Qt Settings, C++/CTest, and QML smoke |
| Pull request CI | [41206383539](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/41206383539) | Swift/watchOS and Qt Settings passed. The duplicate Rust job remained queued when this record was written; the push workflow completed all Rust gates on the same head. |

The hosted Settings build exposed and verified fixes for a narrowing conversion
in the malformed-UTF-8 fixture, an unsupported QProcess API call, and a QtTest
fixture argument being interpreted as a test filter. The final push workflow
passed after those fixes. Earlier P013 checks are baseline evidence only.

## Physical evidence and merge boundary

**Arch Bluetooth LE physical testing: NOT RUN.** No owned Arch controller,
controlled peripheral, or live radio trial is available in this execution
workspace. See the [Arch physical plan](research/P014_ARCH_PHYSICAL_PLAN.md).

P010 Apple Watch SE 2 qualification remains **NO-GO**. P014 software tests do
not establish watchOS behavior, RF provenance, Apple identity, proximity,
Apple Watch Auto Unlock, AirDrop/AWDL, AirPods controls, or Linux authorization.
Every event keeps `authorization: never_evaluated`.

## Final results

Local Rust, Python, formatting, and whitespace gates passed. Hosted CI passed
all push jobs on the validated code head; Qt, C++, QML, Swift, watchOS, and
Rust→Swift→Rust coverage passed. The PR-triggered duplicate Rust job was still
queued, while that same Rust job passed in the push run.

Arch physical BLE testing remains unrun and P010 Watch SE 2 remains NO-GO, so
the Draft PR is not evidence for hardware, watchOS behavior, proximity, identity,
or authorization qualification. The P014 code is ready for review; physical
qualification remains outside this software validation.
