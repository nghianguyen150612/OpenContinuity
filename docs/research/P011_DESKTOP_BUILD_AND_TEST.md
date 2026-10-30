# P011 desktop build and test

Run commands from the repository root. Rust remains pinned to 1.85.0. Settings is
an independent C++20 CMake project with Qt 6.4+ (6.5+ preferred). Ubuntu 24.04 CI
uses packaged Qt 6.4.2; the Debian 13 executor uses Qt 6.8.2. Do not add Qt to Cargo.

## Dependencies

Ubuntu 24.04 (the separate Qt CI job installs these):

```sh
sudo apt-get update
sudo apt-get install -y g++ cmake ninja-build qt6-base-dev qt6-declarative-dev qt6-svg-dev \
  qt6-qpa-plugins qt6-wayland qml6-module-qtquick qml6-module-qtquick-controls \
  qml6-module-qtquick-layouts qml6-module-qtquick-window \
  qml6-module-qtqml-workerscript qml6-module-qtquick-templates
```

Debian 13 additionally splits the SVG image plugin into `qt6-svg-plugins`. Its
absence causes `Unsupported image format` when loading the original icon. QtTest
comes from qt6-base-dev; QML runtime modules are needed in addition to headers.

Arch/CachyOS package names:

```sh
sudo pacman -S --needed base-devel cmake ninja qt6-base qt6-declarative qt6-svg qt6-wayland
```

Hyprland/Caelestia use normal Qt Wayland integration; neither is an app dependency.
Arch package names are documented distribution packages; no Arch hardware or
compositor session was available for local verification. This app needs no BlueZ
package, system bus, privileged service or compositor configuration change.

## Configure, build, test, launch

```sh
cargo build --workspace
cmake -S apps/opencontinuity-settings -B build/settings -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build/settings
OC_REAL_CLI="$PWD/target/debug/opencontinuity" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  ctest --test-dir build/settings --output-on-failure --verbose
build/settings/opencontinuity-settings --cli "$PWD/target/debug/opencontinuity"
```

Targets: `opencontinuity-settings`, `doctor-fixture`, `settings-tests`,
`settings-ui-smoke`. CTest entries: `doctor-backend`, `qml-window-navigation`.
Tests create the real ApplicationWindow and navigate every page, rather than
only parsing QML. The backend suite includes a real Rust contract check when
`OC_REAL_CLI` is set; without it that one test is explicitly skipped. No hardware
is scanned. `-DBUILD_TESTING=OFF` omits QtTest and test executables.

Qt selects the desktop platform normally. For a manual compositor check:

```sh
QT_QPA_PLATFORM=wayland build/settings/opencontinuity-settings --cli "$PWD/target/debug/opencontinuity"
QT_QPA_PLATFORM=xcb build/settings/opencontinuity-settings --cli "$PWD/target/debug/opencontinuity"
```

These need an actual Wayland or X11 session and corresponding plugins. Offscreen
success is not evidence of compositor, RF or Apple device interoperability.

## Install without privileged services

```sh
cmake --install build/settings --prefix "$HOME/.local"
install -m 755 target/debug/opencontinuity "$HOME/.local/bin/opencontinuity"
```

Use the same prefix for both binaries. The installed desktop launcher uses
`opencontinuity-settings`; ensure the prefix's bin directory is on the desktop's
PATH. Launcher and scalable icon install under `share/`. CMake installs only the
Settings application; Rust CLI installation is a separate explicit command.
No daemon, root helper, PAM/polkit, Bluetooth or session configuration is installed.

CI also installs into a temporary prefix, checks desktop/icon files, runs
`--version`, and launches the installed executable for two seconds using
`tests/launch_smoke.py`. It must remain open and have no QML runtime errors.

## Troubleshooting and manual matrix

Missing CLI: put the trusted CLI beside Settings or use an absolute `--cli` path.
Relative paths and PATH lookup are intentionally rejected. Invalid report: confirm
that this CLI emits DoctorReport v1 with `opencontinuity doctor --json`. Timeout:
retry Refresh; no old report is treated as current. Missing QML modules/platform
plugins: install the runtime packages above, then inspect Qt's `QT_DEBUG_PLUGINS=1`
output locally. Do not paste credential-bearing environment dumps into reports.

On a real Linux desktop, check 1280×720 and 2560×1440, fractional scaling, system
light/dark transitions, Tab and arrow navigation, mouse scrolling, missing CLI,
Refresh/Cancel and a screen reader. P011 automation covers palettes/window sizes
and native input offscreen; physical Wayland/Hyprland/X11 and assistive technology
remain manual/unrun. No Watch, AirDrop or AirPods operation can be qualified here.
