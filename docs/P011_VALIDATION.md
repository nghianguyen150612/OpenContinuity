# P011 validation record

P011 is a provisional native Settings foundation, not a claim about the unavailable
original P001–P060 roadmap. Branch: `feat/p011-qt6-settings-foundation`, starting
SHA `7e6ed97e1c40b83a9012cef3b8075c568d9e5e24`, base main
`e3afed25e9d8b8b63896c9117254cc86611855d2`.

`authorization: never_evaluated`. P010 physical qualification remains **NO-GO**.
No Linux unlock, PAM/polkit, pairing, scans, device writes, key generation, daemon
activation, background monitoring or research server startup is added. P012 has
not begun. Nothing is merged automatically.

## Contract and implementation

Reviewed Cargo workspace, core DoctorReport/TrustAssessment definitions, CLI
command dispatch, architecture, matrix, roadmap, threat model, P010 validation,
CI, README and mandatory P011 scope before UI implementation. Doctor v1 has five
root fields, three fixed capability IDs and six trust fields. Native Watch Auto
Unlock, AirDrop and AirPods settings remain unsupported; trust remains unknown.
No Rust core or Cargo dependency changes are made.

The actual executable `opencontinuity-settings` compiles from C++20 and embedded
Qt Quick/QML resources. It has eight navigable pages: Overview, Devices, Apple
Watch, AirDrop, AirPods, Continuity, Research Lab / Diagnostics and About.
The C++ backend owns two QAbstractListModels and a bounded asynchronous QProcess
bridge. See [architecture](research/P011_SETTINGS_ARCHITECTURE.md),
[design](research/P011_DESKTOP_DESIGN_SYSTEM.md),
[contract/security](research/P011_DOCTOR_BRIDGE.md) and
[build/install guide](research/P011_DESKTOP_BUILD_AND_TEST.md).

## Actual local execution

Debian 13 x86_64, GCC 14.2, CMake 3.31.6, Ninja 1.12.1, Qt 6.8.2,
Rust 1.85.0. Development packages were downloaded/extracted into a workspace
prefix because this executor cannot install system packages. CMake used that
prefix; runtime paths selected its SVG plugin plus existing system QML/platform
plugins. No privileged host configuration was changed. Standard distro install
commands are documented separately; the CI minimum is Ubuntu 24.04 Qt 6.4.2.

| Gate actually run | Result |
| --- | --- |
| CMake Debug configure and Ninja build; Release with `BUILD_TESTING=OFF` | PASS; actual `opencontinuity-settings`, `settings-tests`, `doctor-fixture`, `settings-ui-smoke` targets built |
| `doctor-backend` CTest | PASS; QtTest 62 passed, 0 failed, 0 skipped, including init/cleanup and real Rust CLI contract |
| `qml-window-navigation` CTest | PASS; QtTest 5 passed, 0 failed, including light/dark/missing-CLI data cases and init/cleanup |
| Overall `ctest --test-dir build/settings --output-on-failure --verbose` | PASS; 2/2 CTest entries |
| CMake local-prefix install | PASS; executable, desktop launcher and scalable icon installed |
| Installed executable launch | PASS; remained open for two seconds with no QML runtime error, using installed sibling Rust CLI |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS; 130 tests across CLI, private-bus transport integration, portable core and daemon |
| `cargo build --workspace` | PASS |
| `python3 -m unittest discover -s scripts/tests -v` | PASS; 7 tests |
| `git diff --check` | PASS before scoped commits |

The native backend tests cover valid parsing, every capability enum (including
unsupported/unknown), wrong schemas/types/project, every missing root/trust field,
unknown/duplicate IDs, unknown capability/trust enums, duplicate JSON members,
extra fields, excessive/control strings, malformed/truncated/trailing/oversized
JSON, process stdout/stderr caps, unexpected stderr, timeout, nonzero exit, crash,
missing/non-executable/relative/failed-start executable, concurrent refresh,
cancellation/stale callback isolation, model reset/update, stale retention,
recovery, report aging and the actual Rust report. The controlled native peer
accepts only `doctor --json`, without shell or Bluetooth.

Window tests instantiate the actual top-level ApplicationWindow resources, use
native keyboard input to select every page, exercise Refresh, mouse selection followed by adjacent-page
shortcuts, resize to 1280×720, 2560×1440 and 760×560, and fail on QML warnings.
A rendered pixel assertion verifies that the injected dark palette actually
renders dark. Local light/dark captures were visually inspected. These are real
Qt windows using offscreen/software rendering, not HTML or syntax-only checks.

## Failures found and resolved

- System `apt-get` failed for permission reasons. User-owned package extraction
  enabled actual local compilation; this is not a remaining build blocker.
- First native-window smoke failed on the missing Debian `qt6-svg-plugins` image
  decoder. Installing it into the workspace plugin prefix resolved the error.
- The added palette smoke initially missed a `QPalette` include; the compilation
  error was corrected. Visual inspection also found default Fusion palette
  inheritance stayed light; explicit SystemPalette bindings plus a rendered-color
  assertion now verify dark output.
- The baseline main [CI run 41047695886](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/41047695886)
  had a pre-existing Rust test failure in
  `expired_or_mismatched_test_enrollment_fails_before_request_initialization`:
  two wall-clock reads raced a one-millisecond expiry/overlong margin. A scoped
  test-only commit uses injected activation time and an unambiguously overlong
  expiry. Runtime authentication behavior is unchanged; the full Rust gates pass.
- Final navigation review removed a mouse-selection assignment that could break
  the sidebar/shortcut binding. Selection now has one source of truth; the smoke
  test clicks Overview then uses Alt+Right and verifies the selection. Its first
  delegate lookup used QObject ownership and failed; visual-tree lookup now
  matches QML ListView ownership, and all three UI cases pass. Explicit focus
  outlines were added for keyboard navigation.
- Formatting of that test needed `cargo fmt`; the required formatting check
  subsequently passes.

## Hosted CI and remaining manual gates

A separate `Qt Settings (Linux)` job preserves the existing Rust/Python and
`Swift tests and watchOS compile` jobs. It installs Qt development/runtime
packages, builds the real Rust CLI and desktop app, runs both CTest suites and
checks local-prefix installation and the installed native executable launch.
Apple CI retains Swift package tests, both signed Rust/Swift interoperability
paths, actual unsigned watchOS SDK compilation and development ZIP packaging.

Hosted final-head checks are pending at this documentation commit. Read
[PR #12 checks](https://github.com/nghianguyen150612/OpenContinuity/pull/12/checks) for the final SHA; prior green runs do not establish
final-head success. The PR remains Draft until Qt and regression gates are
verified. Local Debian cannot run Apple SDK/Swift/watchOS checks.

Real Wayland/X11/Hyprland sessions, fractional scaling, screen-reader output,
Arch installation, physical Watch/BlueZ, LocalAuthentication and radio/MTU
behavior are **NOT RUN**. Qt's normal Wayland/X11 plugins are supported by design,
not qualified by an offscreen result. P010 hardware stays NO-GO regardless of
desktop/Apple software CI. All native Apple protocols remain unsupported.
