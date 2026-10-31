# P012 Arch Linux qualification plan

Run this on the owned Arch Linux / Wayland / Hyprland host from a normal user session. Do not use sudo, change D-Bus policy, alter permissions, or enable/power an adapter for this check. Build the release or debug CLI normally, then run:

```sh
opencontinuity devices list --json
printf 'exit=%s\n' "$?"
```

Record the timestamp, CLI revision, kernel and BlueZ package version, reported `status`, adapter count, known-device count, and exit code. Redact names and do not copy adapter/device IDs as stable identifiers into a public report. `status=ok` with `devices=[]` is a successful empty known-device snapshot. `no_adapter`, `bluez_unavailable`, `system_bus_unavailable`, `access_denied`, and `timeout` are distinct outcomes. Never label a blocked cloud runner or mock fixture as hardware qualification.

Optional read-only cross-checks are `bluetoothctl show` and `busctl --system --timeout=3s call org.bluez / org.freedesktop.DBus.ObjectManager GetManagedObjects`; the latter prints raw object paths and must not be shared without redaction. Do not run `bluetoothctl scan`, `busctl set-property`, or any connect/pair/remove command. No command requires root for routine inventory; if local system policy denies read access, record `access_denied` without changing policy.

This inventory is not P010 physical Watch qualification. It cannot establish Apple identity, proximity, Watch BLE protocol compatibility, authentication, wearer presence or Linux authorization. P010 remains NO-GO until its separate physical evidence is completed.
