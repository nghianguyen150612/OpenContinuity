# P004 hardware validation record and plan

**Physical-device execution: NOT RUN.** Rust unit/integration tests and a read-only capability diagnostic do not constitute BLE interoperability evidence.

## Environment record for this implementation

| Component | Recorded value | Status |
| --- | --- | --- |
| Linux environment | Debian 13 (trixie), x86_64, kernel `6.18.55` container | **VERIFIED** |
| Reference Arch/Hyprland/Caelestia laptop | Not attached | **NOT RUN** |
| BlueZ version | Unknown; `bluetoothd` unavailable | **UNKNOWN** |
| Controller model / firmware / HCI version | No adapter data available | **UNKNOWN** |
| Adapter LE peripheral/advertising capability | No system bus or controller tools | **UNKNOWN** |
| GATT registration / advertisement / connection | Not attempted | **NOT RUN** |
| Apple Watch SE 2 model identifier / watchOS | Not attached | **NOT RUN** |
| Paired iPhone / iOS | Not attached | **NOT RUN** |
| Xcode / watchOS SDK / signing | Not available on Linux | **BLOCKED** |
| Direct Watch BLE, background, locked, off-wrist | No app or hardware | **NOT RUN** |

## Reproducible Linux test plan

Use a non-production, unprivileged test user on an owned Arch Linux host. Record UTC date, distribution/kernel, `bluetoothd --version`, `btmgmt info`, controller model/firmware/HCI version, adapter path, BlueZ `Adapter1.Powered`, and observed `GattManager1` / `LEAdvertisingManager1` interfaces. Save sanitized command output; remove Bluetooth addresses and nearby-device names before sharing.

1. Run `opencontinuity research watch-ble --capabilities --json`; verify it performs no state changes and reports the actual BlueZ interfaces.
2. Only in a later reviewed GATT-server milestone, register an application and advertisement with explicit opt-in and a short timeout, then record service visibility and one Watch connection.
3. Exercise normal disconnect, adapter power-off/removal, manager registration error, timeout, SIGINT and SIGTERM. Confirm both advertisement and application registrations are removed, and unrelated BlueZ clients remain usable.
4. Restore the adapter after detach and verify capability discovery again. Do not modify pairing databases, daemon configuration, system services, or global radio settings for this test.

Every execution needs its own result row with PASS/FAIL/BLOCKED/NOT RUN and logs. The plan itself is not an executed result.

## Reproducible Apple Watch SE 2 test plan

On a supported Mac, record macOS, Xcode, watchOS SDK, app bundle/build, signing identity and provisioning; on devices record the Watch model identifier and watchOS build plus paired iPhone model and iOS build. Build/install the foreground central app only after reviewing its public API usage and signing profile.

| Test | Required observation | Current result |
| --- | --- | --- |
| Foreground discovery | Scan for the configured service and display candidate host for confirmation | **NOT RUN** |
| GATT connection/disconnect | Connect, disconnect manually, recover from link loss | **NOT RUN** |
| Challenge receive | Version, bounds, host identity and timeout handling | **NOT RUN** |
| Approve / Deny | Explicit UI action; Deny never returns approval | **NOT RUN** |
| Signature | App key creation/storage, selected algorithm, output encoding, Rust verification | **NOT RUN** |
| Foreground lifecycle | App foreground throughout scan and exchange | **NOT RUN** |
| Background/suspension | Observe callbacks and timeout; make no wake guarantee | **NOT RUN** |
| Screen-off / locked / off-wrist | Record exact API and system policy behavior; no inference from foreground baseline | **NOT RUN** |
| Timeout/reconnect | Bounded failure, no stale approval, fresh user-initiated retry | **NOT RUN** |

No test may alter Apple Auto Unlock credentials, Apple Account state, pairing records, IRKs, or Linux authentication configuration. Physical success can establish only the tested versions and conditions; it cannot establish native Auto Unlock, secure distance, or session authorization.
