# P012 BlueZ known-device snapshot schema

Schema v1 is the output of `opencontinuity devices list --json`. It describes one read-only snapshot of BlueZ's already-known objects. It never starts Bluetooth discovery.

## Top-level contract

The JSON object has exactly these fields:

| Field | Contract |
| --- | --- |
| `schema_version` | Integer `1`. |
| `source` | `bluez_known_device_snapshot`. |
| `scan_performed` | Always `false`. |
| `status` | One of the outcomes below. |
| `adapters` | Up to 16 indexed adapters. Empty for every non-`ok` status. |
| `devices` | Up to 128 indexed known devices. Empty for every non-`ok` status. |
| `authorization` | Always `never_evaluated`. |

An adapter has exactly `{ "id": "adapter-001", "powered": true }`. `powered` is a nullable boolean; null means BlueZ did not publish it. A device has exactly `id`, `adapter_id`, `display_name`, `connected`, `paired`, `trusted`, and `services_resolved`. IDs are snapshot indexes, not hardware identifiers. Device booleans are nullable when the property is absent. `display_name` is Alias, then Name, sanitized to 80 Unicode scalar values, or `Unnamed Bluetooth device`.

`status` values: `ok` means a successful snapshot, including a valid empty `devices` array; `no_adapter` means BlueZ answered but no Adapter1 was published; `bluez_unavailable` means the system service was absent; `system_bus_unavailable` means the system bus could not be reached; `access_denied` means the read was rejected; `timeout` means the bounded call did not complete; `malformed_data` means a relevant published property had an unsupported type, invalid device-to-adapter association, or exceeded a record limit; `unsupported_platform` is emitted on non-Linux systems.

CLI exits 0 only for `ok`; every other outcome still prints a complete safe JSON object and exits nonzero. Runtime D-Bus errors are mapped to these classes and their raw messages are never printed. Maximum count is 16 adapters and 128 devices; output is capped at 64 KiB. Names remove control and bidirectional formatting characters. IDs and JSON contain no object paths, MAC addresses, IRKs, UUID lists, advertisement data or credentials.

## Example

```json
{
  "schema_version": 1,
  "source": "bluez_known_device_snapshot",
  "scan_performed": false,
  "status": "ok",
  "adapters": [{"id": "adapter-001", "powered": true}],
  "devices": [{"id": "device-001", "adapter_id": "adapter-001", "display_name": "Headphones", "connected": false, "paired": true, "trusted": null, "services_resolved": null}],
  "authorization": "never_evaluated"
}
```

This contract is independent of DoctorReport v1. `Connected` reports BlueZ's local property, not proximity or active discovery. `Trusted` is local BlueZ state and is not Apple identity authentication.
