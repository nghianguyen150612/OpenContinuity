# P012 validation record

## Software checks

On the implementation head before commit, the following passed: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets` (including the production extraction tests and real CLI subprocess test), `cargo build --workspace`, `python3 -m unittest discover -s scripts/tests -v` (7 tests), and `git diff --check`.

Local Qt CMake/CTest could not run: `cmake` and `ctest` are absent from this Debian cloud image, as are Ninja and Qt 6 development packages/runtime. `swift test --package-path experiments/watchos` could not run because `swift` is absent. Those gates remain required on final-head CI; the PR must remain draft until CI confirms them.

## Host qualification

The implementation environment is a managed Debian Linux cloud container without an owned Arch host or Bluetooth controller. `opencontinuity devices list --json` returned `system_bus_unavailable`, empty adapter/device arrays, and exit code 1. This is an actual cloud-host outcome only. No physical Arch or device evidence is claimed. See [Arch qualification plan](research/P012_ARCH_QUALIFICATION_PLAN.md).

## P010 boundary

P010 Apple Watch ↔ BlueZ physical qualification remains NO-GO. A software fixture, cloud D-Bus result or CI mock does not change it. All research reports retain `authorization: never_evaluated`.
