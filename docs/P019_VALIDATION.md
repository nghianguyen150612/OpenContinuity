# P019 validation

P019 is software work only. This report records the current branch's tested
state separately from physical qualification. Update the local and hosted
results below from the exact final commit before the Draft PR is considered
ready for review.

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
descriptor coverage, passed all 225 tests.
This is recorded rather than hidden; the final-head hosted workflow is still
required to assess repeatability.

## Hosted CI

Populate this section only after the P019 Draft PR exists and both the branch
push and PR workflows have completed on the exact final SHA. Hosted Qt/QML and
Swift/watchOS results are separate from local validation and physical tests.

- Final P019 SHA: pending
- Push workflow: pending
- Draft PR and workflow: pending
- Rust / Qt / Swift job result: pending

## Physical qualification

Arch/RTL8852BE radio and real BlueZ/AWDL cleanup: NOT RUN. iPhone 13 receiver
listing and file transfer: NOT RUN. RTL8852BE support on the target laptop:
UNKNOWN. Native Wayland/Hyprland: NOT RUN. P010 Watch SE 2 W0–W6: NOT
RUN/BLOCKED and NO-GO. Native Apple Watch Auto Unlock: NO-GO.

Every software report keeps `authorization: never_evaluated` and
`physicalQualification: not_qualified`. No hosted test may promote physical
qualification.
