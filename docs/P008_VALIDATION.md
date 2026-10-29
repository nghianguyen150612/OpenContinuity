# P008 validation record

P008 is provisional because the source P001–P060 roadmap is still unavailable.
This record separates portable software tests from Apple SDK compilation and
physical Watch qualification.

## Local Rust environment

| Item | Result |
| --- | --- |
| Starting branch/SHA | `feat/p008-watch-authentication-prototype` / `2833d6b2d6993830269a1ea57d212236d796245b` |
| Host | Linux x86_64 |
| Rust toolchain | `rustc 1.85.0 (4d91de4e4 2025-02-17)` |
| Cargo | `cargo 1.85.0 (d73d2caf9 2024-12-31)` |
| rustfmt / Clippy | `rustfmt 1.8.0-stable` / `clippy 0.1.85` |
| P-256 crate | `p256 0.13.2`, Apache-2.0 OR MIT |
| Rust workspace suite | PASS, 97 tests across all targets |

Local commands passed on Rust 1.85.0:

```text
cargo fmt --all -- --check                   PASS
cargo clippy --workspace --all-targets -- -D warnings  PASS
cargo test --workspace --all-targets          PASS (97 tests)
cargo build --workspace                      PASS
git diff --check                              PASS
```

The Rust unit tests include canonical-byte matching for the fixture that Swift
verifies, and an offline fixture report test that asserts
`authorization: never_evaluated`. The macOS workflow runs Swift tests with an
output path, then runs the Rust CLI against the CryptoKit-signed response
written by those tests. That step is the direct Swift-to-Rust signature check.

## Apple and Swift environment

`swift` and `xcodebuild` are unavailable in this Linux workspace. Local Swift
tests and local watchOS target compilation are therefore **NOT RUN**. Hosted
macOS CI records Swift, Xcode and watchOS SDK versions, runs package tests,
verifies the CryptoKit response in Rust, generates the XcodeGen project, and
builds the unsigned watchOS target. Hosted results and run URLs are recorded
after the P008 branch is pushed.

## Hardware qualification

| Trial | Status | Evidence |
| --- | --- | --- |
| Rust software signatures and fixture verifier | PASS | Local Rust unit and workspace tests, including fixture verification |
| Rust request accepted by CryptoKit | Pending hosted CI | Shared request vector in Swift package tests |
| CryptoKit response accepted by Rust | Pending hosted CI | Swift test output passed to fixture CLI in macOS job |
| watchOS SDK compile | Pending hosted CI | XcodeGen plus unsigned generic watchOS build |
| LocalAuthentication on Watch SE 2 | NOT RUN | Requires physical Watch and on-device interaction |
| Secure Enclave key custody on Watch SE 2 | UNVERIFIED | No secure-element key is created or qualified |
| BlueZ ↔ Apple Watch BLE exchange | NOT RUN | P008 is not integrated with GATT v1 and no physical trial was performed |
| Signed device installation | NOT RUN | No physical signing/install trial |

Even passing hosted software checks only establishes interoperability of the
implemented signatures and codecs. It does not establish genuine Watch
identity, wearer presence, secure proximity, secure key custody, production
enrollment, persistent replay protection or Linux authorization. All
experimental reports retain `authorization: never_evaluated`.
