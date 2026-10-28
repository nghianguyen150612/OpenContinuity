# P005 Linux BlueZ GATT server

Status: experimental implementation present; real BlueZ/controller and Watch validation NOT RUN in this environment.

The Linux CLI exports a temporary GATT object tree over the system D-Bus using zbus 5.12.0 (MSRV 1.77; built with pinned Rust 1.85.0). BlueZ’s published [Adapter API](https://bluez.readthedocs.io/en/latest/adapter-api/), [GATT API](https://bluez.readthedocs.io/en/latest/gatt-api/) and [LE Advertising API](https://bluez.readthedocs.io/en/latest/advertising-api/) define the methods/properties used. The upstream `test/example-gatt-server` and `test/example-advertisement` examples were also inspected for server-side `GetManagedObjects`, `GetAll`, `ReadValue`, `WriteValue`, and `Release` behavior.

## D-Bus contract used

| Interface | Methods / properties used |
| --- | --- |
| `org.bluez.Adapter1` | `Powered` (read from `GetManagedObjects` properties); the server refuses an unpowered adapter and never calls `Set` or turns it on. |
| `org.bluez.GattManager1` | `RegisterApplication(object, dict)` and `UnregisterApplication(object)`. |
| `org.bluez.GattService1` | `UUID`, `Primary`, and characteristic object paths beneath the service path. |
| `org.bluez.GattCharacteristic1` | `UUID`, `Service`, `Flags`; `ReadValue(a{sv}) → ay` and `WriteValue(ay, a{sv})`. BlueZ supplies `device`, `mtu`, and `offset` options where applicable. |
| `org.bluez.LEAdvertisingManager1` | `RegisterAdvertisement(object, dict)` and `UnregisterAdvertisement(object)`; `SupportedInstances` and `ActiveInstances` are inspected if BlueZ provides them. |
| `org.bluez.LEAdvertisement1` | `Type="peripheral"`, `ServiceUUIDs`, `LocalName`, and `Release()`. |
| `org.freedesktop.DBus.ObjectManager` | zbus exports `GetManagedObjects()` at the application root; BlueZ requires this for the registered GATT hierarchy. |
| `org.freedesktop.DBus.Properties` | zbus supplies standard property `Get`/`GetAll` behavior for exported interfaces and the ObjectManager snapshot. |

BlueZ returns `InvalidArguments` / `AlreadyExists` for GATT registration errors. Advertisement registration can return `InvalidArguments`, `AlreadyExists`, `InvalidLength`, and `NotPermitted` (including exhausted advertising instances). D-Bus access denial and policy rejection are returned to the caller with the stage and original BlueZ error; the implementation does not relax policy. Advertisement registration failure rolls back only this process’s GATT application. Cleanup always attempts advertisement removal before application removal. Dropping the process’s bus connection also lets BlueZ discard resources owned by that connection.

## Running the experiment

```sh
opencontinuity research watch-gatt serve \
  --adapter hci0 --duration-seconds 60 --experimental --json
```

The adapter name is required and must be an `hciN` name. `--experimental` is mandatory. Runtime is restricted to 1–300 seconds. The command installs SIGINT/SIGTERM cleanup, checks adapter presence and power state, monitors adapter presence/power and connected clients, stops at the deadline, then unregisters owned resources. It does not pair, scan, power on, set discoverability, modify BlueZ configuration, start a daemon, or add privileged services. An adapter with missing GATT/advertising interfaces is `unsupported`; blocked D-Bus inspection/registration is reported separately. The controller’s LE peripheral capability stays `unknown` unless actual registration and radio evidence later establish more; manager presence alone never implies radio support.

## Temporary service

Service UUID: `8e6f0001-6f4e-4c4f-9b4f-4f70656e436f`.

The five characteristics are listed in [P005_WIRE_CONTRACT.md](P005_WIRE_CONTRACT.md). The advertisement contains the service UUID and the short local name `OC Research`. No device address, host identity, user name, approval, or secret is included. BLE advertisements are visible to nearby observers; run only during an intentional foreground experiment.

No Watch app or central-client build is included. The intended topology is Apple Watch SE 2 foreground CoreBluetooth central → Linux BlueZ peripheral. A later watch app needs Xcode, a supported watchOS SDK, an Apple Developer signing identity, a paired iPhone/Watch, and user-granted Bluetooth permission. Apple compilation/signing and physical Watch validation are **NOT RUN**.

This server only exchanges synthetic bytes and returns a synthetic acknowledgment. It does not authenticate a Watch, perform enrollment, approve an action, or grant Linux account/session authorization. Every report carries `authorization: never_evaluated`.
