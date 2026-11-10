# P018 validation report

**Status: P018 software implementation complete; Draft PR #19 remains open.**
This report separates software checks from physical qualification. The hosted
results below validate implementation commit `c0c14d5a35cf1163cb6df0dc13d18c2816f15d6e`;
the later validation-document update does not change implementation code.

## Local Codex Cloud checks

Environment: Debian 13, x86_64, Rust 1.85.0 and Python 3.12.14. Local Swift/Xcode,
watchOS SDK, CMake, Ninja and Qt 6.4.2 were unavailable. Hosted workflows were
used for the platform gates rather than treating missing local tools as passes.

All available requested local gates passed on the implementation commit:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets -- --quiet`: **191 passed** across the
  workspace (CLI 56, private BlueZ D-Bus 25, CLI command tests 13, discovery
  CLI 3, inventory CLI 1, private discovery BlueZ D-Bus 29, core 63, daemon
  command 1).
- `cargo build --workspace`
- `python3 -m unittest discover -s scripts/tests -v`: **54 passed**.
- `git diff --check`

The private BlueZ fixture tests exercise the production registration and
cleanup paths. They do not establish real controller, RF, or Watch behavior.

## Hosted GitHub Actions

Both final implementation-commit workflows passed all three jobs:

- [Final-head push workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/42805082655)
- [Final-head pull-request workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/42805092259)

Each workflow passed Rust checks, Qt Settings (Linux), and Swift tests/watchOS
compile. Hosted details:

- **Rust:** formatting, Clippy, all **191 tests**, build, and whitespace checks
  passed on both events.
- **Qt 6.4.2:** CMake/Ninja configuration, build, installation, and **4/4 CTest
  targets passed**. The four QtTest executables reported 62, 27, 42 and 12
  assertions respectively (**143 total**); the 12-assertion QML navigation test
  exercised real Scan/Cancel/navigation smoke behavior.
- **Swift/watchOS:** **36 XCTest cases**, zero failures; one suite-gated Rust/Swift
  interoperability case was explicitly selected and passed in the dedicated
  round-trip step. CryptoKit request/response verification, Rust → Swift → Rust
  signed transport, unsigned watchOS SDK compilation and unsigned development
  artifact packaging/upload all passed. The artifact is unsigned and requires
  local signing for installation.

These jobs establish software/build behavior only. They do not claim a signed
Watch installation or CoreBluetooth exchange on physical hardware.

## Physical qualification

- Arch BLE/RF and independent receiver: **NOT RUN**.
- Real BlueZ controller registration and RF cleanup: **NOT RUN**.
- Native Wayland/Hyprland: **NOT RUN**.
- Watch SE 2 signed installation: **NOT RUN / BLOCKED**.
- W0–W6 Watch SE 2 gates: **NOT RUN / BLOCKED individually**.
- P010 Watch SE 2 physical qualification: **NO-GO**.
- Native Apple Auto Unlock: **NO-GO**.

Imported host, Watch, operator, observer and fixture records remain unverified;
the offline validator never qualifies physical hardware. Every diagnostic and
validator report preserves `authorization: never_evaluated`.
