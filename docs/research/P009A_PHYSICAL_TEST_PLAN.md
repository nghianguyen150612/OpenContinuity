# P009A physical test plan

No physical P009 BLE or LocalAuthentication trial has been performed. Hosted software tests and unsigned watchOS compilation are not radio evidence.

## Equipment and setup

- Dedicated Linux test host with an already powered BlueZ LE peripheral adapter.
- Supported foreground watchOS device with the unsigned research app installed using a development signing setup.
- A separate independently verified channel for comparing the host and Watch public-key fingerprints.
- Test-only host key and short-lived enrollment created using [the P009A temporary enrollment workflow](P009A_TEMPORARY_ENROLLMENT.md).

Record OS/kernel, BlueZ version, controller and firmware, Xcode/watchOS versions, Watch model, app build, adapter name, test start/end and sanitized logs. Do not log private keys, full signed envelopes, account data or unnecessary BLE identifiers.

## Test sequence

1. Generate a new Linux host test key and export its public pin. Independently compare the displayed host fingerprint with the Watch import screen.
2. Create a new ephemeral Watch key, transfer only its public enrollment, and compare its fingerprint on the Watch and Linux operator views. Confirm the enrollment is bound to the intended host identity and Linux UID.
3. Start Linux with both exact fingerprint confirmations. Verify adapter preflight is read-only, setup completes inside 30 seconds, the P009 service appears only after registration, and the P008 issue time is after readiness.
4. Start Authenticated Research on the Watch, select the intended host, and capture request status, repeated GET/read/ACK behavior and explicit request completion.
5. Verify the request signature, host identity, UID, Watch enrollment, nonce, session, action, expiry and replay checks before the consent screen appears.
6. Test explicit rejection and approval separately. For approval, record successful foreground LocalAuthentication. Confirm neither result is presented as a Linux unlock.
7. Confirm one response frame is outstanding at a time, each write-with-response callback precedes the next frame, and Linux verifies the complete response signature and signed decision.
8. Repeat failures: wrong host fingerprint, wrong Watch fingerprint, changed enrollment binding, revoked and expired pin, wrong key, expired P008 request, LA that completes after expiry, duplicate/stale callback after reconnect, wrong owner/transfer ID/sequence, partial/oversized envelope, lost connection during request GET/read/ACK, cancel in every phase, BlueZ owner restart, background transition and rapid repeated actions.
9. Confirm every failure is terminal, no stale request or response is accepted, a retry uses a new request/nonce/session, and Linux unregisters only its own advertisement and GATT application.
10. Delete the host key and enrollment files. Confirm Watch research state is cleared after completion and backgrounding.

## Evidence table

| Evidence | Status |
| --- | --- |
| Rust protocol, deadline, enrollment and mock BlueZ tests | PASS locally |
| Swift protocol/enrollment tests | Pending hosted macOS CI |
| Unsigned watchOS target build | Pending hosted macOS CI |
| Linux ↔ Watch physical BLE exchange | NOT RUN |
| Watch LocalAuthentication approval and late-expiry behavior | NOT RUN |
| BlueZ cleanup on physical controller | NOT RUN |

A physical PASS requires captured evidence for the relevant row on the named devices. Do not substitute a mock, simulator, or CI result for physical BLE evidence.
