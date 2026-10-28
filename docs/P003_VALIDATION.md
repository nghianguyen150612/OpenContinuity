# P003 validation record

Validation snapshot: 2026-10-28. Repository base refs were rechecked before edits: `main` `365444eea4a776b92dc2dfa3315275929e09ef63`, P003 branch start `6147c931565988184a31dca4022cbf5bd157b08e`. Work is isolated on `research/p003-watch-endpoint-feasibility`.

## Local Rust checks

Environment: Linux x86_64; Rust 1.85.0 (`4d91de4e4`); workspace edition 2024. Rust was installed into the execution environment because the supplied workspace initially had no Rust toolchain. No repository toolchain files were changed. A CLI test initially exposed a fixture enum serialization spelling mismatch; after correcting the explicit serde name, the full run passed.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `cargo test --workspace --all-targets` | **PASS**: 28 tests across core, CLI and daemon targets; P001/P002 regression tests included |
| `cargo build --workspace` | **PASS** |
| `git diff --check` | **PASS** |
| `opencontinuity doctor --json` behavior | **PASS** in existing CLI tests; unsupported protocol and unknown trust states unchanged |
| `research watch` offline fixture | **PASS** in existing CLI tests; authorization remains `never_evaluated` |
| `research watch-endpoint` fixture | **PASS** in CLI tests; deterministic `simulated_verified` result, replay rejection, authorization `never_evaluated` |

P003 unit tests cover deterministic mock output, capability serialization, strict unknown-field handling, JSON size/version bounds, challenge/approval schema checks, nonce uniqueness, replay rejection, expiry/future response, signature tampering, identity/session mismatch, missing presence, user denial, transport disconnect/relay failure, and fail-closed output. This validates portable mock code only, not Apple hardware.

## Apple and hardware validation

| Target | Result |
| --- | --- |
| macOS / Xcode / watchOS SDK build | **BLOCKED**: `xcodebuild` unavailable in this Linux environment; no Swift target was added |
| Apple Watch SE 2 physical tests | **NOT RUN**: no Watch available |
| Paired iPhone / WatchConnectivity tests | **NOT RUN**: no iPhone available |
| Linux BlueZ BLE hardware tests | **NOT RUN**: `bluetoothctl` and `btmgmt` unavailable; no controller evidence captured |
| Background, screen-off, locked, off-wrist, timeout and relay-delay tests | **NOT RUN**; reproducible procedures are in [P003 hardware plan](research/P003_HARDWARE_TEST_PLAN.md) |

No mock, simulator, or host test is reported as a successful Apple Watch SE 2 test.

## Hosted CI and PR

The final hosted CI result and PR URL are checked after push and recorded with the delivery report. A queued or in-progress GitHub Actions run is not a pass. P002's queued checks are not used as evidence for P003.

## P003 acceptance checklist

- [x] Direct Watch–Linux transport feasibility and blockers documented.
- [x] Watch–iPhone–Linux relay feasibility and delivery limits documented.
- [x] Explicit foreground Watch confirmation assessed.
- [x] watchOS public API availability researched from Apple primary documentation.
- [x] Background execution constraints and lack of continuous listener documented.
- [x] Endpoint identity, host/account/session binding, key custody, replay, relay and presence threats modeled.
- [x] Portable Rust mocks and deterministic negative tests added.
- [x] P001/P002 command and report contracts regression-tested.
- [x] Hardware claims align with evidence; all unavailable Apple/hardware tests marked BLOCKED or NOT RUN.
- [x] Feasibility, transport/API, security and hardware-plan documents created; architecture, feature matrix, roadmap and threat model updated.
- [ ] Hosted CI reaches a terminal conclusion on the final pushed head.
- [ ] P003 PR open for human review; not merged.

The full original P001–P060 roadmap remains unavailable. P003 remains a provisional continuation; no P001 Bluetooth adapter or Qt 6/QML Settings scaffolding is claimed.
