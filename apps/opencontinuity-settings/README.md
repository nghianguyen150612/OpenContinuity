# OpenContinuity Settings

Native Linux Qt Quick Settings with a read-only Rust CLI bridge. Requires C++20,
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
one trusted absolute executable path; it always receives only `doctor --json`.
A missing CLI still leaves every page navigable with an explanatory error.
Refresh is manual (Ctrl+R), asynchronous, limited to three seconds, and never
performs Bluetooth discovery. Reports become visibly stale after five minutes.

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
not implemented. P007–P010 research is informational only;
`authorization: never_evaluated`. P010 hardware qualification remains NO-GO.
