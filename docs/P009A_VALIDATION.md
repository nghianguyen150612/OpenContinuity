# P009A validation record

P009A continues P009 on the existing authenticated Watch transport branch. It does not start P010, import P007B, or change P007 GATT v1. The experiment remains test-only and every report uses `authorization: never_evaluated`.

## Local Rust validation

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS: 128 tests, 0 failures (CLI unit 30, BlueZ mock/integration 22, CLI commands 12, core 63, daemon commands 1). |
| `cargo build --workspace` | PASS |
| `git diff --check` | PASS |

Coverage includes controlled monotonic request/response deadlines, response completion after a consent decision at the end of its deadline, request and response timeout polling during BLE silence, setup readiness and restart freshness, bounded BlueZ setup, request connection loss, signed expiry during consent, wrong owner/session, cancel, malformed or incomplete transfers, failed signatures, signed rejection, enrollment mismatch/expiry, secure host-key file creation, and explicit fingerprint confirmations. Existing P007 tests pass.

## Apple validation

Swift and Xcode are unavailable in the Linux work environment. The current-head macOS GitHub Actions run passed Swift codec/lifecycle tests, the serialized Rust → Swift → Rust P009 exchange (including CryptoKit verification in both directions), and unsigned watchOS application compilation:

- [Run 40990995735](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40990995735) on `9b335aa55ca1c2ae7d3c7bf1d6894df2ce855fdc`: PASS.

Earlier hosted runs exposed a duplicate Swift test declaration, a missing protocol-module import, and watchOS-unavailable file import/text selection APIs. Those issues were fixed before this passing run.

## Hardware status

Linux ↔ Watch BLE, BlueZ controller registration, physical consent and LocalAuthentication have NOT been run. See [the P009A physical test plan](research/P009A_PHYSICAL_TEST_PLAN.md). A green build does not close these hardware gates.
