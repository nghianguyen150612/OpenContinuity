# OpenContinuity Settings

Native Linux Qt Quick Settings with a Rust CLI process bridge. Requires C++20,
CMake 3.22+, Ninja and Qt 6.4+. Tested locally with Qt 6.8.2; Ubuntu 24.04 CI uses
its Qt 6.4.2 packages. Qt 6.5+ is preferred where packaged. No Qt dependency is
added to Cargo or the portable core.

From the repository root:

```sh
cargo build --workspace
cmake -S apps/opencontinuity-settings -B build/settings -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build/settings
OC_REAL_CLI="$PWD/target/debug/opencontinuity" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  ctest --test-dir build/settings --output-on-failure
build/settings/opencontinuity-settings --cli "$PWD/target/debug/opencontinuity"
```

The last command opens a desktop window. In a headless environment, use the
smoke tests instead. CLI lookup defaults to `opencontinuity` in the Settings
executable's own directory, never the current directory or PATH. `--cli` accepts
one trusted absolute executable path; it receives only fixed `doctor --json` or
`devices list --json`, or explicit `devices discover --seconds 10 --json` arguments.
A missing CLI still leaves every page navigable with an explanatory error.
Doctor refresh is manual (Ctrl+R), asynchronous, limited to three seconds, and never
performs Bluetooth discovery. Device snapshots are refreshed only by the
Devices button and stale rows are labeled.

Install to a user-owned prefix:

```sh
cmake --install build/settings --prefix "$HOME/.local"
install -m 755 target/debug/opencontinuity "$HOME/.local/bin/opencontinuity"
```

The launcher and original SVG icon install under `share/applications` and
`share/icons/hicolor/scalable/apps`. Put `$HOME/.local/bin` on your desktop's PATH.
No services, authentication settings or compositor configuration are installed.

See [build and troubleshooting](../../docs/research/P011_DESKTOP_BUILD_AND_TEST.md)
for Arch/Hyprland, Ubuntu, Wayland/X11 plugins and manual checks. Native Watch
Auto Unlock, AirDrop and AirPods protocols are unsupported. Device discovery is
explicit and bounded as described below. P007–P010 research is informational only;
`authorization: never_evaluated`. P010 hardware qualification remains NO-GO.

The known-device section lists BlueZ's already-known objects and says explicitly that
no Bluetooth scan was performed by that inventory request. Empty, unavailable, denied and stale outcomes are
distinct. BlueZ `Trusted` does not authenticate an Apple device. No connect,
pair, forget, AirDrop, AirPods or Watch unlock controls are provided. See the
[P012 schema](../../docs/research/P012_DEVICE_SNAPSHOT_SCHEMA.md),
[UI design](../../docs/research/P012_SETTINGS_DEVICES_UI.md), and
[validation](../../docs/P012_VALIDATION.md).

## P013 active discovery

Devices now has a separate **Scan Bluetooth LE for 10 seconds** action with an
explicit radio disclosure, asynchronous progress, Cancel, sanitized observations,
no-results/errors, stale previous results and cleanup warnings. It invokes only
`devices discover --seconds 10 --json` through a trusted absolute CLI. Startup,
Doctor refresh and navigation never start discovery. Navigation away, loss of
window activity and close request graceful cancellation, with a bounded fallback.
`Refresh known devices` stays independent and read-only. No Pair/Connect/Trust/
Forget or Apple protocol action is added. Bluetooth discovery is not authentication.

See [UI behavior](../../docs/research/P013_SETTINGS_SCAN_UI.md),
[schema](../../docs/research/P013_DISCOVERY_SCHEMA.md) and
[validation](../../docs/P013_VALIDATION.md). CTest includes a private native CLI
fixture for discovery parser/process/model tests and real QML Scan/Cancel clicks.
Tests never require Bluetooth hardware. Physical Arch testing remains NOT RUN.
