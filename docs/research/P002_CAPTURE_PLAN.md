# P002 read-only capture plan

## Current capability

P002 ships only an **offline mock-fixture** path. It is deterministic and requires no Bluetooth hardware or privileges:

```sh
cargo run -p opencontinuity-cli -- research watch \
  --fixture tests/fixtures/watch-discovery.json --json
```

The command opens the named JSON file, validates the schema, and emits a report whose authorization state is always `never_evaluated`. It does not invoke BlueZ, `bluetoothctl`, `btmon`, HCI, a Wi-Fi interface, pairing, or authentication. Invalid fields, unknown JSON fields, unsupported schema versions, out-of-range RSSI, and backend errors fail closed.

## Hardware study objective

The future hardware study should determine what metadata is observable while a user-owned Apple Watch SE 2 and a user-owned Mac perform Apple's supported Auto Unlock flow. It must not claim that Linux can reproduce Apple's protocol based on observing advertisements. The study distinguishes:

- Host-visible LE advertisements and their changing addresses.
- Apple Watch state changes that appear correlated with the built-in feature.
- On-air exchanges only when captured with a suitable BLE sniffer and in a controlled environment.
- macOS-side logs/diagnostics, which may show high-level events but are not wire-format proof.

## Preconditions and consent

Before any future real capture implementation or experiment:

1. Use only devices owned or explicitly authorized by the researcher: one Apple Watch SE 2, paired iPhone, and Mac that already supports Auto Unlock.
2. Record device model identifiers, watchOS/iOS/macOS versions, Bluetooth adapter/controller and firmware, kernel and BlueZ versions, capture tool versions, UTC time, room layout, and test steps. Do not record Apple Account identifiers.
3. Tell nearby participants and remove unrelated BLE devices from the controlled RF area where practical. Do not record other people's radios or identifiers.
4. Capture only the minimum period needed. Do not extract, display, log, publish, or commit IRKs, Apple Account data, keychain records, long-term keys, decrypted application data, or unredacted addresses.
5. Keep raw capture artifacts outside the repository, with restrictive local permissions, and delete them after extracting the approved sanitized metadata. Commit only a synthetic or reviewed redacted fixture.
6. Do not disable security features, alter PAM/polkit/login configuration, pair Linux with the Watch, write to the Watch, or attempt active protocol injection.

## Reproducible experiment sequence

This sequence is a plan, not a claim that capture has been run. Active scanning is opt-in and can affect a shared controller; use a dedicated adapter when available. Passive over-air capture requires a dedicated BLE sniffer and does not require pairing with the Apple devices.

### A. Establish a baseline

1. Record versions and adapter inventory using read-only commands:

   ```sh
   uname -a
   bluetoothctl --version
   btmgmt info
   bluetoothctl list
   ```

2. With the Watch unlocked/on-wrist and Auto Unlock enabled on the Mac, perform one ordinary password login, then record no more than the time window and visible high-level result.
3. Repeat with the Watch locked, off wrist, and farther away as separate controlled conditions. Never infer protocol semantics from a single correlated change.

### B. Optional LE discovery (active scan, explicitly opt-in)

Only after confirming no other BlueZ client is using the dedicated adapter, start an LE scan manually:

```text
bluetoothctl
select <dedicated-controller>
menu scan
transport le
scan on
```

Stop with `scan off` immediately after the bounded observation window. Record BlueZ version, selected controller, start/stop UTC timestamps, and whether another client shared discovery. BlueZ shares discovery sessions and merges discovery filters; `StopDiscovery` can release only the caller's session, so do not assume the radio stopped while another client remains. [BlueZ Adapter API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst)

Active scanning is non-pairing but not passive: the local adapter performs discovery and may request scan responses. It can alter RF activity and reveal the Linux host to other nearby radios. Do not run as a privileged production service.

### C. Optional passive HCI or over-air capture

For a future HCI trace of the Linux controller's own discovery, use BlueZ `btmon` only after the explicit scan authorization above and only on the dedicated adapter:

```sh
btmon -i <controller-index> --write /tmp/p002-owned-device.btsnoop
```

Check `btmon --help` for the installed version; options vary. Depending on host permissions, monitoring may require elevated privileges or a narrowly scoped capability. If that is unavailable, stop; do not add root privileges to OpenContinuity merely to satisfy this plan. HCI traces show host/controller events, not necessarily all over-air packets or decrypted STS application traffic.

For link-layer capture, use a BLE sniffer supported by its vendor tooling, clocked and placed with only the owned Watch/Mac in the RF test area. Document sniffer model, firmware, channel-follow behavior, antenna position and limitations. Do not inject traffic or attempt to decrypt Apple secrets. An RF sniffer may miss channel hops or encrypted exchanges, so absence from a trace is not proof that an exchange did not occur.

### D. Sanitize and review

Before any result is shared:

- Drop Bluetooth addresses, IRKs, raw advertising payloads, Apple Account names, Wi-Fi identifiers, key material, and unrelated devices.
- Retain only time-relative event labels, protocol transport metadata, sanitized field lengths/IDs where safe, RSSI sample ranges with explicit limitations, tool versions, and repeat counts.
- Hash raw files only if a chain-of-custody record is needed; do not publish raw captures or hashes that could identify private recordings.
- Check whether a field interpretation is independently specified. If it comes only from a reverse-engineering project, label it DOCUMENTED or HYPOTHESIS; if the project's own field definition says uncertain, label UNKNOWN.
- Never convert a proximity or advertisement observation into authorization.

## Fixture format

`tests/fixtures/watch-discovery.json` shows the current version. Each item has a synthetic unique label, optional untrusted name, optional RSSI in the bounded range `-127..=20`, and a list of company IDs. Raw payloads, addresses and keys are intentionally absent. The fixture is test input, not a capture, and must not be cited as hardware evidence.

## Exit criteria for a future real capture adapter

Separate approval and review are needed before adding any radio code. The adapter should default to disabled, require an explicit `--capture-ble` flag and bounded duration, use BlueZ D-Bus discovery without pairing/connect/write, explain shared-scan effects, record only redacted fields, stop on every error/interrupt, and never emit an unlock or identity result. Hardware validation must name the actual controller and capture setup; mock fixture passes must not be reported as hardware passes.
