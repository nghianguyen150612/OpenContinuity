# P019 validation

## P019A recovery

The historical results below do not validate P019A. See
[receive publication hardening](research/P019A_RECEIVE_PUBLICATION_HARDENING.md).
Receive now reports writer_containment_unavailable before backend startup.
Atomic bundle storage and strict pinned doctor parsing have deterministic
software regressions. Descendant containment and graceful process-group
escalation are unsupported; no receive publication is enabled.

P019A local validation: workspace tests passed 235 tests (95 CLI unit,
25 BlueZ D-Bus, 15 CLI command, 3 discovery CLI, 1 inventory CLI,
29 discovery D-Bus, 66 core, 1 daemon CLI); Python passed 55 tests.
Formatting, Clippy with warnings denied, build and whitespace gates are
rerun before delivery. Final-head hosted results are recorded with delivery
evidence; older green workflows are not evidence for this recovery.
Qt/QML, Swift/CryptoKit and watchOS gates are unchanged. Physical qualification
remains not_qualified and authorization remains never_evaluated.


P019 is software work only. This report records the current branch's tested
state separately from physical qualification. The local and hosted results
below are tied to implementation commit
`939d4978d0fa33b9dfc8e27f382be646fa92f501`; the validation-record commit is
documentation-only.

## Implemented paths

- Rust v1 AirDrop status contracts and receive lifecycle.
- Read-only Linux sysfs/procfs and fixed-path `iw`/Owly probes.
- Fixed-version Owly 0.4.2 receive-process supervision, terminal consent,
  deadlines, signals, process groups and JSON terminal result.
- Private staging, path/type/link/size checks, no-overwrite rename, private
  file permissions and temporary runtime identity state.
- Qt strict-schema status provider and Settings page with manual read-only
  refresh and disabled GUI Receive action.
- Rust and Qt fixture tests. Fixtures do not emulate AWDL, Wi-Fi drivers, an
  iPhone or AirDrop RF behavior.

## Local validation

Environment: Codex Cloud Linux workspace on Debian, Rust/Cargo 1.85.0,
Python 3.12.14. The checkout is based on P018 merge
`a3448cc428ff1e8afe7c393ea3be109eefdea5dc`. Results below were run against
the final P019 source state, including the nonblocking terminal relay and
executable trust checks.

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS, 225 tests (85 CLI unit, 25 BlueZ D-Bus, 15 CLI command, 3 discovery CLI, 1 inventory CLI, 29 discovery D-Bus, 66 core, 1 daemon CLI) |
| `cargo build --workspace` | PASS |
| `python3 -m unittest discover -s scripts/tests -v` | PASS, 55 tests |
| `git diff --check` | PASS |
| Qt 6.4.2 CMake/Ninja/CTest and QML smoke | Not available locally (`cmake`, `ninja`, `qmake6`, and `qtpaths6` absent); hosted CI required |
| Swift, CryptoKit, watchOS compile and packaging | Hosted macOS CI required |

The first parallel Rust workspace run had two transient private BlueZ test-bus
timeouts. Subsequent full workspace runs passed without test changes. The
latest run, after adding executable trust-mode and nonblocking terminal
descriptor coverage, passed all 225 tests. This is recorded rather than
hidden.

## Hosted CI

Both final-head workflows completed successfully for implementation commit
`939d4978d0fa33b9dfc8e27f382be646fa92f501`:

- [Branch push workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/43000306906): Rust checks, Qt Settings, and Swift/watchOS all passed.
- [Draft PR #20 workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/43000318545): Rust checks, Qt Settings, and Swift/watchOS all passed.
- Qt 6.4.2/Ninja ran all 5 CTest targets successfully, totaling 162 QtTest cases; the run includes the real QML window, navigation, and manual AirDrop refresh smoke test.
- Swift XCTest suites passed. Rust-to-Swift-to-Rust signed transport and CryptoKit verification passed; unsigned watchOS compilation and development artifact packaging also passed.

The first Qt run on `e172da6` found that clicking AirDrop's manual refresh
changed focus before the next keyboard navigation assertion. The UI test now
explicitly returns focus to the navigation control after refresh; the corrected
QML smoke test passes in both workflows above. The final validation-record
commit is documentation-only; its own push and PR checks are verified
separately from these implementation-source results.

## Physical qualification

Arch/RTL8852BE radio and real BlueZ/AWDL cleanup: NOT RUN. iPhone 13 receiver
listing and file transfer: NOT RUN. RTL8852BE support on the target laptop:
UNKNOWN. Native Wayland/Hyprland: NOT RUN. P010 Watch SE 2 W0–W6: NOT
RUN/BLOCKED and NO-GO. Native Apple Watch Auto Unlock: NO-GO.

Every software report keeps `authorization: never_evaluated` and
`physicalQualification: not_qualified`. No hosted test may promote physical
qualification.
