# P009 validation record

P009 adds `OC-WATCH-AUTH-TRANSPORT/1` framing and a separate opt-in BlueZ GATT service, plus a CoreBluetooth request/response path gated by P008 verification. The Linux CLI accepts only explicit test-only enrollments and signs a fresh, short-lived request. The Watch app has no enrollment/key provisioning UI and ships no test private key, so its authenticated scan and live signed approval remain **BLOCKED**. Every report retains `authorization: never_evaluated`.

## Local validation

| Gate | Status | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | Workspace formatting is clean. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | No warnings. |
| `cargo test --workspace --all-targets` | PASS | 109 passed, 0 failed: CLI unit tests 16, BlueZ mock/integration tests 22, CLI command tests 11, core tests 59, daemon command tests 1. |
| `cargo build --workspace` | PASS | All workspace targets built. |
| `git diff --check` | PASS | No whitespace errors. |
| `swift test --package-path experiments/watchos` | NOT RUN locally | Swift is not installed on this Linux execution host. |
| XcodeGen + unsigned watchOS build | NOT RUN locally | Xcode and XcodeGen are not installed on this Linux execution host. |

The new local Rust coverage includes seven portable P009 framing tests, three BlueZ authenticated-session tests, a serialized-frame CLI test, and a bounded server-duration command test. Existing P007 synthetic GATT and P008 cryptographic suites also pass.

## Hosted CI

Both the push and PR workflows passed at implementation SHA `3febcfca3764d9abc8ac465920f11e0ead1424e0`. The [push run](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40881911861) and [PR run](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40881923256) each passed Rust checks and the macOS job. The full Swift package run executed 20 tests with 0 failures and 1 expected skip (the P009 handoff needs Rust-produced input); the focused Rust → Swift → Rust P009 handoff then passed separately, including tampering, truncation, stale-session and mismatched-key rejection. The existing P008 Swift-to-Rust signature fixture passed. XcodeGen generated the Watch project and the unsigned watchOS build succeeded with Swift 6.1.2, Xcode 16.4 and watchOS SDK 11.5. A documentation-only follow-up will trigger the same checks on its resulting branch head.

The cross-language response uses a deterministic test-only Watch key and a simulated explicit rejection. It does not simulate a physical consent tap or successful LocalAuthentication.

## Runtime and physical gates

| Stage | Status |
| --- | --- |
| Rust transport framing/reassembly | PASS; local tests cover both directions, 218-frame ceiling, malformed/version/session/owner/order/timeout/cancel/completion failures. |
| BlueZ P009 service source and bounded owner state | IMPLEMENTED; host registration requires an already powered selected adapter and uses owned-resource cleanup. Live D-Bus registration is NOT RUN here. |
| CoreBluetooth P009 request/response source | IMPLEMENTED; Apple SDK compile remains pending hosted CI. |
| Rust → Swift → Rust serialized signed exchange | PASS in both hosted runs; exact serialized frames were reassembled and cryptographically verified. Adversarial cases rejected. |
| Linux ↔ Watch RF exchange | NOT RUN |
| Watch SE 2 consent and LocalAuthentication | NOT RUN |
| Safe independent enrollment and temporary Watch-key injection | BLOCKED; no reviewed setup or app provisioning UI exists. |
| Signed live approval | BLOCKED; safe enrollment, Apple build, physical transport and on-device LocalAuthentication evidence are outstanding. |
| P007 synthetic GATT v1 regression | PASS; existing UUIDs, values and semantics are unchanged and workspace tests pass. |
| Linux authorization | NEVER EVALUATED; no PAM/polkit/login/unlock integration exists. |

The execution host has no BlueZ system bus/controller or Apple Watch. CI success is software/build evidence only. `NOT RUN` and `BLOCKED` are not PASS.
