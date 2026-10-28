# P006 BlueZ host qualification

`authorization: never_evaluated`

The script `scripts/qualify-bluez-host.sh` is read-only: it reports distribution/kernel, BlueZ version if available, sysfs controller/driver information, `btmgmt info` and experimental capability information, BlueZ interfaces, adapter state, advertising slots and optional reported capabilities. It does not power, pair, scan, register or change configuration. Failed, denied or unavailable commands remain BLOCKED/UNKNOWN; missing optional BlueZ properties do not mean unsupported hardware. Controller firmware version may not be exposed; use the kernel/driver's documented read-only mechanism if available. Outputs can contain hostnames, device paths, Bluetooth addresses and policy information: review and redact before sharing.

On an owned Arch Linux host with Rust, BlueZ, D-Bus, `busctl`, `btmgmt` and a preconfigured LE adapter:

```bash
cargo build --workspace
scripts/qualify-bluez-host.sh | tee p006-host.txt
target/debug/opencontinuity research watch-ble --capabilities --json | tee p006-capabilities.json
```

No elevation is requested by the script. If policy denies `btmgmt` or system D-Bus, record the exact denied command and separately arrange appropriate read-only permissions; do not infer radio support. Ensure the adapter is already powered and available; this project never changes its power, pairing or policy. `supported` means the queried BlueZ interface appears in successful introspection, `unsupported` means that interface was absent in successful introspection, `unknown` means no reliable observation, `blocked` means access was denied, and `not_tested` means no test was attempted. Interface presence is API availability only. `SupportedInstances`/`ActiveInstances` are manager reports, not proof of RF transmission or controller feature support.

Only after an operator explicitly chooses a trial, run for a bounded duration:

```bash
target/debug/opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 60 --experimental --json | tee p006-trial.json
```

Use a second owned BLE central to scan for service `8e6f0001-6f4e-4c4f-9b4f-4f70656e436f`, discover the service and five characteristics, and perform the synthetic exchange specified in [the Watch client contract](P006_WATCH_CLIENT_CONTRACT.md). Record tool/client version, host/controller/firmware, timestamps, UUIDs, received version, result ID match and digest match; avoid publishing addresses, challenges or raw captures. Independently run timeout and SIGINT/SIGTERM trials; controlled power-loss/removal tests require operator action and must not alter unrelated users' devices. Inspect `cleanup_errors` and BlueZ state after each trial. The report's registration-success booleans record D-Bus replies, not over-the-air events. An uncertain registration or failed cleanup requires operator investigation; the tool does not modify the system to repair it.

Record stages separately as PASS, FAIL, BLOCKED or NOT RUN: (1) BlueZ API introspection; (2) controller LE/peripheral feature evidence, with source; (3) GATT registration reply; (4) advertisement registration reply; (5) actual advertisement observed by owned second device; (6) service and characteristics discovered; (7) synthetic framed exchange and digest checked; (8) cleanup after timeout/signals/removal. A pass at one stage does not imply another. Registration does not show actual advertising. Permission failures do not prove unsupported hardware. RSSI, pairing and advertisement are not secure proximity or authorization.

## This environment

2026-10-28: `scripts/qualify-bluez-host.sh` reported Debian 13, kernel `6.18.55` x86_64; no `bluetoothd` or `btmgmt` in PATH, no controllers in `/sys/class/bluetooth`, and system bus connect failed with “No such file or directory”. Therefore BlueZ version, firmware, controller LE support, GATT/advertising APIs and permissions on a real host are **UNKNOWN/BLOCKED**. Stages 3–8, physical radio, an Arch host, and Watch: **NOT RUN**. A private `dbus-daemon` and mock BlueZ integration tests do not count as hardware qualification.
