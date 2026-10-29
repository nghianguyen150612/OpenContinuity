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

The macOS workflow runs the full Swift package suite, asks the Rust CLI to serialize a test-only signed host request into P009 frames, runs a focused Swift reassembly/CryptoKit verification/signing/adversarial test over those exact frames, and asks Rust to reassemble and verify the resulting signed response. Existing P008 Swift-to-Rust fixture verification remains enabled. XcodeGen generates the Watch project and Xcode builds the unsigned watchOS target. Final SHA, run URLs, test counts and conclusions will be recorded after the branch is pushed and both push and PR workflow runs finish.

The cross-language response uses a deterministic test-only Watch key and a simulated explicit rejection. It does not simulate a physical consent tap or successful LocalAuthentication.

## Runtime and physical gates

| Stage | Status |
| --- | --- |
| Rust transport framing/reassembly | PASS; local tests cover both directions, 218-frame ceiling, malformed/version/session/owner/order/timeout/cancel/completion failures. |
| BlueZ P009 service source and bounded owner state | IMPLEMENTED; host registration requires an already powered selected adapter and uses owned-resource cleanup. Live D-Bus registration is NOT RUN here. |
| CoreBluetooth P009 request/response source | IMPLEMENTED; Apple SDK compile remains pending hosted CI. |
| Rust → Swift → Rust serialized signed exchange | CI PENDING; fixture test includes tampering, truncation, stale session, and mismatched-key rejection. |
| Linux ↔ Watch RF exchange | NOT RUN |
| Watch SE 2 consent and LocalAuthentication | NOT RUN |
| Safe independent enrollment and temporary Watch-key injection | BLOCKED; no reviewed setup or app provisioning UI exists. |
| Signed live approval | BLOCKED; safe enrollment, Apple build, physical transport and on-device LocalAuthentication evidence are outstanding. |
| P007 synthetic GATT v1 regression | PASS; existing UUIDs, values and semantics are unchanged and workspace tests pass. |
| Linux authorization | NEVER EVALUATED; no PAM/polkit/login/unlock integration exists. |

The execution host has no BlueZ system bus/controller or Apple Watch. CI success is software/build evidence only. `NOT RUN` and `BLOCKED` are not PASS.
