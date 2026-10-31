# P012 BlueZ inventory architecture

`apps/opencontinuity-cli/src/device_inventory.rs` is a Linux-specific, read-only adapter. A manual CLI invocation connects to the system bus and calls `org.freedesktop.DBus.ObjectManager.GetManagedObjects` once on `org.bluez` root. It retains only `org.bluez.Adapter1` and `org.bluez.Device1` and only the properties specified in [schema v1](P012_DEVICE_SNAPSHOT_SCHEMA.md). The synchronous D-Bus operation runs on a detached worker and the CLI waits at most three seconds.

There is no scan, polling, adapter power write, pairing, connect, trust, remove, application registration or property mutation in this path. It uses no shell helper, privilege escalation, policy update, daemon or privileged service. The GATT research server and the existing `research watch-ble` capability probe remain separate modules and commands.

The extraction function is shared by production conversion and deterministic injected-map tests. It sorts object paths in process memory solely to assign adapter/device indexes, associates each Device1 with its published adapter prefix, and drops paths before serialization. Paths are never logged or included in errors. Missing optional values stay null. Incorrect types and orphan devices fail the snapshot as `malformed_data`; record limits fail closed. BlueZ's known Device1 objects may represent previously paired devices and do not prove that the device is nearby.

The Settings application consumes only the JSON CLI boundary. `DeviceInventoryProvider` starts fixed arguments asynchronously, bounds stdout/stderr and runtime, parses schema v1 independently, rejects stale callbacks, and owns a dedicated `QAbstractListModel`. It does not link Rust into C++, modify DoctorProvider, or give QML direct D-Bus access. Refresh is explicit; the app does not inventory on startup.

BlueZ facts do not feed DoctorReport and cannot change the unsupported Apple Watch, AirDrop or AirPods states. Research reports continue to say `authorization: never_evaluated`. Qt's system platform integration is used without compositor-specific assumptions.
