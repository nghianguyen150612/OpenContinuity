# P009A validation record

P009A continues P009 on the existing authenticated Watch transport branch. It does not start P010, import P007B, or change P007 GATT v1. The experiment remains test-only and every report uses `authorization: never_evaluated`.

## Local Rust validation

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS: 125 tests, 0 failures (CLI unit 28, BlueZ mock/integration 22, CLI commands 12, core 62, daemon commands 1). |
| `cargo build --workspace` | PASS |
| `git diff --check` | PASS |

Coverage includes controlled monotonic request/response deadlines, response completion after a consent decision at the end of its deadline, setup readiness and restart freshness, bounded BlueZ setup, request connection loss, signed expiry during consent, wrong owner/session, cancel, malformed or incomplete transfers, failed signatures, signed rejection, enrollment mismatch/expiry, secure host-key file creation, and explicit fingerprint confirmations. Existing P007 tests pass.

## Apple validation

Swift and Xcode are unavailable in the Linux work environment. The first hosted run found and failed on a duplicate Swift deadline-test declaration; the duplicate was removed, and the follow-up macOS run will validate Swift tests, the serialized Rust → Swift → Rust P009 response exchange, and unsigned watchOS compilation. Hosted URLs and final results will be recorded after that run completes.

## Hardware status

Linux ↔ Watch BLE, BlueZ controller registration, physical consent and LocalAuthentication have NOT been run. See [the P009A physical test plan](research/P009A_PHYSICAL_TEST_PLAN.md). A green build does not close these hardware gates.
