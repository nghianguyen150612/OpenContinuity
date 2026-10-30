# P011 Settings architecture

P011 is provisional because the original P001–P060 roadmap was unavailable.
It closes the native Settings foundation gap without implementing a production
Bluetooth adapter or beginning P012.

`apps/opencontinuity-settings` is an independent CMake/C++20 project. The
`opencontinuity-settings` target links shared Qt Core, Gui, Qml, Quick,
QuickControls2 and Svg. Resources embed `Main.qml`, `SettingsPage.qml`, `Card.qml`
and the original icon; startup needs no network, fonts download or web engine.
Qt 6.4 is the minimum for Ubuntu 24.04 distro packages; Qt 6.8.2 was used locally.
No Qt crate, dependency, C ABI or platform service enters the Rust core/workspace.

The graph is:

```text
ApplicationWindow / navigation / SettingsPage / Card (QML resources)
    → DoctorProvider Q_PROPERTY + two StatusModel QAbstractListModels (C++)
    → QProcess: absolute executable + [doctor, --json]
    → opencontinuity CLI → portable DoctorReport v1 (Rust/serde)
```

The CLI remains the capability authority. A process boundary avoids a second
Rust ABI/build integration and limits the bridge to an already tested public
read-only command. No daemon, bus, radio or auth adapter is linked into the UI.
The executable defaults to a sibling `opencontinuity`; an explicit trusted
absolute `--cli` path supports separate build trees. There is no PATH search,
working-directory fallback, user command field or environment command override.
Distributors must install the CLI separately into the same `bin` directory.

`DoctorReport::parse` accepts only strict v1; `StatusModel` exposes identifier,
title, state and summary roles. `DoctorProvider` owns asynchronous lifecycle,
last-success time, version/schema, loading, failure and stale state. QML owns
presentation, navigation and scrolling. A failed refresh preserves the last
valid rows with a visible stale warning. Before a valid report there are no rows.
All external text uses `Text.PlainText`; process diagnostics are never displayed.
See [bridge contract](P011_DOCTOR_BRIDGE.md).

Overview, Devices, Apple Watch, AirDrop, AirPods, Continuity, Research Lab /
Diagnostics and About are actual navigable content in the same native window.
There are no enabled protocol toggles. Devices is an adapter-missing empty state.
Watch trust is separate from capability support and from documented research.
Research status is baseline documentation, not live CI evidence or hardware
observation. `authorization: never_evaluated`; no Linux unlock is available.
Physical P010 qualification remains NO-GO.

Wayland and X11 use Qt's normal platform plugins. No Hyprland API or compositor
settings are required. Offscreen tests prove QML/window/navigation behavior only;
real compositor, accessibility reader and physical device testing remain separate.
