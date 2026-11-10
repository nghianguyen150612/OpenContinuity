# P018 validation report

**Status: INCOMPLETE; Draft PR #19 remains draft.** This report records the current implementation state, not physical qualification.

## Executed locally in Codex Cloud

- Runtime: Debian 13, x86_64; Rust 1.85.0 installed in user space; no Swift/Xcode watchOS SDK, Qt 6.4.2, CMake or Ninja was available on PATH when checked.
- P018 host CLI Rust unit suite: 50 tests passed after the production P006 trace path was added.
- Private BlueZ D-Bus integration suite: 23 tests passed on the prior targeted run; this suite is regression coverage for registration/cleanup and does not prove P018 trace fields on live BlueZ hardware.
- Python suite before paired raw-export cases: 47 tests passed; paired-export focused tests were subsequently added and passed separately (10 P018 tests). A final full run is pending.
- `cargo fmt`, workspace Clippy and full workspace tests require rerun after the last edits. Workspace build, final full test counts, and `git diff --check` are pending.

## Hosted checks

The push and PR workflows for each final commit must both complete. The CI checks run Rust, Qt 6.4.2/CMake/Ninja/CTest/QML, Swift XCTest/CryptoKit, Rust–Swift–Rust transport interoperability, unsigned watchOS compilation and packaging. See PR #19 Actions; no final-head conclusion is recorded yet.

## Physical evidence

- Arch BLE/RF and independent receiver: NOT RUN.
- Real BlueZ controller registration and RF cleanup: NOT RUN.
- Native Wayland/Hyprland: NOT RUN.
- Signed Watch SE 2 installation and W0–W6: NOT RUN/BLOCKED.
- Native Apple Auto Unlock: NO-GO.
- P010 Watch SE 2 physical qualification: NO-GO.

Software tests do not alter those statuses. Every diagnostic and validator report preserves `authorization: never_evaluated`.
