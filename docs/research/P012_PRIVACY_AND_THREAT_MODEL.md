# P012 privacy and threat model

BlueZ object paths and device addresses are sensitive identifiers. They remain inside the one-shot adapter and are removed before serialization. Public IDs are positional indexes scoped to a snapshot. Alias and Name can identify a person; they are shown only in the local Settings UI and are not logged, uploaded or persisted by OpenContinuity. Names are length capped and control/bidirectional-format characters are removed. No RSSI, service UUIDs, advertisement bytes, IRKs, credentials or cryptographic material are read into the public schema.

The process accepts BlueZ data as untrusted. It applies limits of 16 adapters, 128 devices, 80 Unicode scalar values per display name, 64 KiB serialized output, a three-second command deadline, a 64 KiB Qt stdout cap and a 4 KiB stderr cap. Rust and Qt validate the schema and supported status values independently. Unknown optional properties are null; malformed relevant property types or invalid associations fail closed. D-Bus error messages are never exposed.

Threats include malicious or buggy D-Bus services, hostile names, oversized maps, process hangs, duplicate or malformed JSON, stale process responses and spoofed Bluetooth names. These controls reduce parser and privacy exposure but do not authenticate BlueZ, devices, Apple identity, the wearer, or proximity. BlueZ `Trusted` is not a cryptographic trust decision by OpenContinuity. A successful inventory grants no authorization; every related report retains `authorization: never_evaluated`.

No Bluetooth mutations, scan calls, elevated privileges, policy changes, network requests, logs of names, or telemetry are added. A compromised local BlueZ service, kernel, CLI binary, Qt runtime or account remains outside the application boundary.
