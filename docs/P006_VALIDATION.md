# P006 validation

`authorization: never_evaluated`

P006 is a provisional continuation of the implemented P002–P005 BLE research track. Earlier planning grouped P001–P006 as Foundation + Settings; the original P001–P060 roadmap was not recovered verbatim. Neither the production Bluetooth adapter nor the Qt6/QML Settings framework is complete.

## Reproducible local checks

On Debian 13 x86_64, Rust/Cargo 1.85.0, 2026-10-28:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS: 74 tests, 0 failed (13 CLI unit, 22 private D-Bus integration, 8 CLI command, 30 core, 1 daemon command; daemon unit target has 0) |
| `cargo build --workspace` | PASS |
| `git diff --check` | PASS |
| `scripts/qualify-bluez-host.sh` | Executed read-only; system D-Bus unavailable, no controller, BlueZ daemon or btmgmt found. Hardware stages NOT RUN. |
| `opencontinuity research watch-ble --capabilities --json` | `system_bus_unavailable`; no adapters, hardware observations `not_run`, authorization `never_evaluated`. |

Private `dbus-daemon` tests exercise actual zbus object registration and methods against a synthetic BlueZ service, not a real controller. They cover registration rollback including failed rollback, owned cleanup ordering, idempotent already-gone cleanup, explicit adapter selection, runtime timeout, both signals, power loss, BlueZ owner loss, bus loss and bounded registration timeout. P006A's additional private-bus tests emit `Connected=false` then `true` while the snapshot always reports true, so polling cannot explain invalidation. They cover fragmented writes, unread ACKs, old IDs, another client, concurrent calls, `InterfacesRemoved`, adapter removal, owner replacement, malformed-listener failure and fresh-process recovery, ordering-watchdog failure, rejected non-BlueZ callers, and a signal delivered during the initial snapshot. Unit tests cover response/session binding and consumption, cancellation, expiration, client cap, client isolation, reconnect after observed disconnect, malformed options/offsets, and wire ordering, length, timeout and sequence limits. These tests do not prove a real host or Watch works.

## Hardware and platform evidence

See [host qualification](research/P006_BLUEZ_HOST_QUALIFICATION.md) for exact commands, classifications and stage-by-stage results. Physical BlueZ/controller registration, RF advertising, service discovery, frame exchange, Arch Linux, Apple SDK build, simulator and physical Watch SE 2 are **NOT RUN**. No Watch compatibility is claimed. The [client contract](research/P006_WATCH_CLIENT_CONTRACT.md) requires confirming long reads and foreground CoreBluetooth behavior on target hardware.

## Remaining limitations

A delivered disconnect event is ordered with GATT calls on one D-Bus connection; a physical disconnect that BlueZ does not report, or reports after a request has already been accepted, cannot be invalidated retroactively. Real BlueZ event and reconnect behavior is NOT RUN. Initial D-Bus connection/handshake and OS/kernel failure behavior are not bounded by the per-method timeout. Cleanup after a broken bus cannot be confirmed and is reported. Non-BlueZ D-Bus senders are rejected for GATT methods; a compromised BlueZ/host, relay, radio exhaustion and secure proximity remain unresolved. See [security review](research/P006_GATT_SESSION_HARDENING.md). These are research-only mitigations, not Watch authentication or Linux authorization. Synthetic ACKs are separate from P003/P004 mock signatures.

Hosted CI is assessed on the final PR commit via the PR's Checks tab; it does not establish hardware behavior.
