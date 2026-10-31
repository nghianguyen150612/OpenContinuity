# P013 validation — bounded Bluetooth LE discovery

Date: 2026-10-31. Branch: `feat/p013-opt-in-bluetooth-discovery`.
Starting commit: `4f1b26a9715525e15ce74578c5c035db4cc18abf`.
Base main: `64b98ba7eb6a80e6969a531a2e924a38f33bb391`.
The existing branch is continued without resetting prior commits. No P014 work or
automatic merge is included.

## Inherited P012 CI

Inspected final P012 head `3dc2f3459e826ef1a0bb9c84cae35e69f517fa3c`, not merely
its merge commit. [CI run 41161372066](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/41161372066)
completed **successfully**: Rust checks, Qt Settings, Swift tests, both Rust/Swift
interoperability paths, unsigned watchOS build and development artifact upload.
No inherited regression was found in those P012 results. A later P013 duplicate
push run exposed the inherited Doctor crash-fixture race: `failure(crash)`
reported `timed_out` instead of `process_error` under its 100 ms deadline. The
fixture now uses a non-core-dumping fatal signal on Unix; only the dedicated
timeout scenario keeps a 100 ms deadline. Other error-classification scenarios
allow normal process startup/reaping time without accepting a timeout as success.
Production Doctor behavior is unchanged. Final CI is checked again after this fix.

## Local software validation

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, Rust 1.85.0 |
| `cargo test --workspace --all-targets` | PASS, 166 tests |
| `cargo build --workspace` | PASS |
| `python3 -m unittest discover -s scripts/tests -v` | PASS, 7 tests including P010 evidence separation |
| `git diff --check` | PASS |
| Ubuntu 24.04 CMake/Ninja, C++20, Qt 6.4.2 | PASS, backend warnings treated as errors |
| CTest with actual CLI Doctor regression | PASS, all 4 suites |
| Local install/version/native launch smoke | PASS, offscreen software rendering |
| Local Swift / Xcode / watchOS | NOT RUN, Apple toolchain unavailable on this Linux host; preserved in final-head CI |

QtTest outcomes (including data rows and init/cleanup): Doctor backend 62 passed,
inventory backend 27 passed, discovery backend 44 passed, real QML 9 passed.
No skipped tests in the run with `OC_REAL_CLI` set. Seven real QML scenarios cover
light/dark, empty observations, unavailable BlueZ, cleanup warning, powered-off
adapter and missing CLI, plus all eight existing pages. Scan/Cancel and navigation
cancellation are actual UI interactions against the native fixture executable.

Discovery software coverage includes argument opt-in/bounds, cached snapshots
excluded, sender/interface/adapter filtering, InterfacesAdded and PropertiesChanged,
type validation, duplicate events, random per-session IDs, name/privacy limits,
128-device and 4,096-event caps, no adapter/power/LE support, denied start,
SIGINT/SIGTERM, cancellation during a pending start, method timeout, bus loss,
BlueZ owner replacement, adapter removal/power loss, cleanup failures and another
client's discovery session surviving our StopDiscovery. Cooperative exclusion is
tested both for concurrent scans and an active P007 GATT research server.

Qt coverage includes strict independent parser/state/exit validation, duplicate
members (including escaped keys), malformed and excessive output, unsafe labels,
models, one active QProcess, no automatic start, graceful cancellation, stale
results, bounded force termination, recovery without stale callbacks and shutdown.
P012 inventory command/module/schema and DoctorReport capability states remain
unchanged; GATT changes are limited to cooperative adapter exclusion.

## Final-head GitHub Actions

The existing workflow retains all Rust/Python, Qt, Swift codec/lifecycle,
Rust→Swift→Rust, CryptoKit→Rust, unsigned watchOS compilation and artifact
packaging steps. Discovery tests run without Bluetooth hardware.

The exact checked head, final run URL and conclusions are recorded in
[Draft PR #14](https://github.com/nghianguyen150612/OpenContinuity/pull/14) and the
delivery report. Consult its current Checks for the final commit; a green P012
baseline or earlier P013 head does not establish final-head success.
[Branch runs](https://github.com/nghianguyen150612/OpenContinuity/actions?query=branch%3Afeat%2Fp013-opt-in-bluetooth-discovery).

## Physical evidence and merge boundary

**Arch Bluetooth discovery: NOT RUN / BLOCKED.** No real controller or Arch
session is accessible here; no radio trial was initiated. Private dbus-daemon
fixtures exercise real zbus API calls, not RF. Offscreen Qt rendering is not
Hyprland/Wayland, X11 or hardware background-lifecycle qualification. See the
[physical plan](research/P013_ARCH_PHYSICAL_TEST_PLAN.md).

Bluetooth discovery is not authentication. Apple Watch Auto Unlock, AWDL/AirDrop,
AirPods configuration/switching and Linux unlock remain unsupported. No pairing,
connection, trust/forget, PAM, polkit or account integration is added.
`authorization: never_evaluated` remains fixed. P010 Apple Watch SE 2 physical
qualification remains **NO-GO**.

Residual risks: BlueZ merges filters and events may reflect cached/other-client
activity; lost replies or forced death make explicit cleanup uncertain; normal
transport buffers are not a process-memory sandbox; cooperative exclusion is not
a universal adapter lock. Software review and final-head CI are required before
merging the Draft PR. Physical acceptance remains a separate unpassed gate.
