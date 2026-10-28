# P004 BlueZ capability matrix

**Evidence status:** API behavior is **DOCUMENTED**; this environment's host inspection is **VERIFIED** only for the reported missing BlueZ runtime; controller GATT/advertising support is **UNKNOWN**. No GATT service was registered and no radio operation was attempted.

## BlueZ APIs and adapter requirements

| Layer | Requirement / observation | Evidence |
| --- | --- | --- |
| Linux D-Bus | BlueZ owns the `org.bluez` system-bus name and exposes adapter objects such as `/org/bluez/hci0`. | [BlueZ D-Bus API](https://bluez.readthedocs.io/en/latest/) — **DOCUMENTED** |
| Adapter | `org.bluez.Adapter1` exposes adapter state and properties; an `hciN` object alone does not prove peripheral capability. | [Adapter API](https://bluez.readthedocs.io/en/latest/adapter-api/) — **DOCUMENTED** |
| GATT application | `org.bluez.GattManager1.RegisterApplication` registers application-provided objects implementing `GattService1` and `GattCharacteristic1`; unregister is required during cleanup. | [GATT API](https://bluez.readthedocs.io/en/latest/gatt-api/) — **DOCUMENTED** |
| LE advertisement | `org.bluez.LEAdvertisingManager1.RegisterAdvertisement` and `UnregisterAdvertisement` manage an application advertisement object. | [Advertising API](https://bluez.readthedocs.io/en/latest/advertising-api/) — **DOCUMENTED** |
| Controller support | The adapter must expose the manager interfaces and its controller/firmware must support the required LE peripheral and advertising operations. Neither a Linux kernel version nor the presence of `hciN` alone guarantees this. | **DOCUMENTED** API model; local controller is **UNKNOWN** |
| Controller / management | HCI/management controller capabilities and firmware must support LE peripheral advertising; inspect the actual adapter, not the kernel version alone. `btmgmt info` is a BlueZ management diagnostic. | [`btmgmt(1)`](https://manpages.debian.org/testing/bluez/btmgmt.1.en.html) and [Bluetooth Core Specification](https://www.bluetooth.com/specifications/specs/core-specification-5-4/) — **DOCUMENTED** |

The relevant supported feature test is runtime interface availability on the selected BlueZ adapter plus the controller's reported capability. A production adapter must check both. BlueZ API introspection alone cannot establish whether RF advertising succeeds in the current regulatory, firmware, or adapter state.

## What the code does

`opencontinuity research watch-ble --capabilities [--json]` invokes `busctl` read-only to inspect the BlueZ object tree, adapter object interfaces, `Adapter1.Powered`, and BlueZ version when present. It never calls `RegisterApplication`, `RegisterAdvertisement`, `Set`, `StartDiscovery`, `Pair`, or any method that changes system state. Manager-interface presence is reported as verified for the observed D-Bus object; absence is `blocked`; failed inspection is `unknown`. The D-Bus probe does not read controller LE peripheral capability bits; that field stays explicitly `unknown` and a future real adapter must inspect actual controller evidence too.

**No experimental GATT server command is exposed.** This host has neither a live system D-Bus nor BlueZ/controller tooling, so application-object registration, advertisement lifetime, adapter-loss cleanup, and controller compatibility cannot be exercised. A runtime server would additionally need an object-server D-Bus implementation that owns and exports the full object graph and reliably unregisters both manager registrations; this spike stops at the tested capability boundary instead of implying that capability from a mock.

## Environment evidence (2026-10-28)

| Item | Observation | Status |
| --- | --- | --- |
| OS | Debian GNU/Linux 13 (trixie) container; Linux x86_64 kernel `6.18.55` | **VERIFIED** by `/etc/os-release` and `uname -a` |
| BlueZ utilities | `bluetoothctl`, `btmgmt`, and `bluetoothd` not found | **VERIFIED** |
| D-Bus utility | `/usr/bin/busctl` exists | **VERIFIED** |
| System bus | `busctl --system list` reports “Failed to connect to system scope bus via local transport: No such file or directory” | **VERIFIED** |
| BlueZ version | unavailable | **UNKNOWN** |
| Bluetooth adapter/controller/firmware | no system bus and no BlueZ tools; no controller evidence | **UNKNOWN** |
| `GattManager1` / `LEAdvertisingManager1` on host | cannot be inspected without BlueZ service | **UNKNOWN** |
| GATT registration/advertisement/connection | not attempted | **NOT RUN** |

The diagnostic reports these limitations and always sets `authorization: never_evaluated`. It may run in a desktop session where BlueZ is present; its output remains capability discovery, not a claim of successful advertising.

## Reproduction

```sh
opencontinuity research watch-ble --capabilities --json
busctl --system --no-pager tree org.bluez
busctl --system --no-pager introspect org.bluez /org/bluez/hci0
bluetoothd --version
btmgmt info
```

The first command is implemented. The remaining commands are manual host evidence collection and may be unavailable. Do not start or reconfigure BlueZ solely to make the report look positive.
