# P005 security review

## Scope and preserved trust boundaries

P005 adds a Linux-only experimental BlueZ GATT server and bounded synthetic transport. It does not alter P004’s `MockEnrollmentRecord`, enrolled verification-key selection, revocation checks, or P003 mock signature behavior. No GATT message enters the mock enrollment verifier, and no endpoint-supplied key can become a trust anchor.

BLE discovery is not identity. A GATT connection is not authentication. Bluetooth pairing is not Linux account authorization. RSSI and timing do not prove proximity, and a signed message alone would not prevent a relay. A future Watch app must verify an independently enrolled Linux host identity before showing any approval prompt. P005 sends no approval prompt and never calls PAM, polkit, a login manager, or a privileged helper. Reports always say `authorization: never_evaluated`.

## Controls implemented

- Explicit `--experimental`, required adapter selection, duration from 1 to 300 seconds, and no automatic server startup.
- No adapter power/discoverability changes, pairing, discovery scan, persistent service, or system configuration writes.
- OS `/dev/urandom` generates per-process object-path tokens and per-device session/challenge identifiers. Deterministic P003 Ed25519 test seeds remain test-only.
- Per-Device object-path state; 1024-byte message cap; ATT MTU checks; strict version, magic, session, sequence, flags and length validation; duplicate/reorder rejection; partial timeout; disconnect/idle cleanup.
- Only synthetic exchange bytes are buffered in memory. Response digest is a transport acknowledgment, not a signature or authentication result.
- D-Bus registrations are bound to this process’s bus connection and unique object paths. Teardown is idempotent and unregisters advertisement before GATT application. BlueZ policy errors are surfaced without authorization bypass.
- Controller LE peripheral support is left `unknown`; manager interface presence is not treated as evidence of controller radio operation.

## Residual risks and limits

The D-Bus server has not been exercised against a real BlueZ service. Hardware-specific permission behavior, advertising slot exhaustion, controller removal races and RF behavior remain unverified. A malicious connected client can cause its own session to fail or reconnect; BlueZ scheduling and kernel ATT resource limits remain outside application control. Challenge reads expose random values to the connected peer and nearby BLE metadata remains observable. `GetManagedObjects` polling is used for adapter/client state and can only establish disconnect state as reported by BlueZ.

There is no Watch-side implementation, host authentication, production enrollment/revocation store, durable replay guard, Secure Enclave operation, anti-relay mechanism, proximity proof, or Linux authorization. No Apple IRK, Keychain data, account credential, or private Auto Unlock material is accessed. Never enable this server as an unlock factor.
