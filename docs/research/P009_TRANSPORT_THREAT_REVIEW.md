# P009 transport threat review

## Security claim

P009 transports P008 research messages without changing their canonical signed bytes. The transport can establish bounded, ordered reassembly for one in-memory owner and transfer ID. Only P008 signature/binding/freshness/replay validation can establish that an enrolled test key signed a request or response. Nothing in this milestone authorizes Linux.

## Wire and state controls

| Threat | Implemented control |
| --- | --- |
| Version downgrade or P007 confusion | Separate `OA` framing magic, explicit transport version 1, distinct P009 UUID family and no fallback. P007's `OC` service remains unchanged. |
| Oversized values / memory exhaustion | Envelope cap 1,088 bytes, canonical cap 1,024 bytes, ATT value check at `MTU - 3`, fixed 15-byte header, 218-frame ceiling. |
| Request metadata exposure | P009 provides integrity/authentication through P008 signatures but no confidentiality. A connected nearby peer may read host identity, Linux UID, Watch enrollment ID, nonce, action and session metadata. Do not include secrets; no Bluetooth pairing is treated as trust. |
| Frame tamper, missing, duplicate or reordered data | Sequence starts at zero, exact transfer total in start frame, zero total on continuations, exact end/length match, strict next-sequence check. |
| Cross-client frame injection | Assemblers and mailboxes bind to one owner string. The Linux adapter maps that owner to the BlueZ Device object path and P006A disconnect monitor; the Watch client separately binds callbacks and buffers to one CoreBluetooth connection generation. |
| Cross-session substitution | Every frame/control names an eight-byte transfer ID. P008 response additionally binds the signed response to request digest, nonce, P008 session and enrollment pair. |
| Ambiguous host-request read | GET selects one frame, GET retry returns the same frame, the read does not advance, and only explicit ordered ACK advances. The CoreBluetooth manager sends GET and ACK with response and reads one selected small value between them. |
| Incomplete or repeated completion | Explicit completion after exact byte transfer/ACK of all request chunks; completion is single-use. |
| Disconnect, cancellation or callback restart | The Rust server and CoreBluetooth manager invalidate owner-bound state on disconnect, cancellation, timeout, owner restart, or app suspension. Connection delegate generations reject stale callbacks. |
| Replay | P008 in-memory replay guards validate nonce/session on both endpoints. They are bounded and do not survive process restart. |
| Forged peer / wrong key | P008 independently pinned enrollment and P-256 signature validation; advertised BLE keys are never used. |
| Setup delay consuming request freshness | No P008 request is signed until bounded 30-second BlueZ readiness and both registrations succeed; each D-Bus method call is capped at 2 seconds. |
| Delayed consent consuming response-transfer time | Host request, consent wait and response transfer have independent monotonic deadlines, all clamped to signed P008 expiry. |
| Unverified test enrollment | Both peers display and require explicit manually verified SHA-256 pins; Linux `serve` requires exact host and Watch fingerprint flags before D-Bus setup. |

## Remaining attacks and limits

- The Rust and Swift software tests do not establish RF interoperability, real BlueZ behavior, CoreBluetooth callback ordering, Watch long-read behavior, or on-device LocalAuthentication policy support.
- The temporary test-enrollment workflow is implemented, but manual fingerprint exchange remains dependent on operator procedure. This is not a production enrollment ceremony or persistent trust store.
- Apple SDK build and physical radio validation must be checked separately. Software integration does not establish actual BLE callback behavior or RF interoperability.
- In-memory replay protection resets on process restart. An attacker able to restart either app can present a previously valid message within its lifetime unless a new nonce/session is issued and old state is otherwise controlled.
- An attacker can relay genuine signed research traffic between endpoints. Transfer IDs and signatures do not prove distance or proximity.
- Nearby peers can still deny service through connection attempts, radio interference, disconnects, or BlueZ resource pressure. The server limits this service to one owner and uses P006A fail-closed disconnect handling, but it cannot prevent RF interference or all resource-exhaustion attempts.
- P008 signatures do not encrypt the request or response. BLE payload metadata can be observed by a connected or otherwise capable radio peer; this experiment carries no credentials or secrets.
- A compromised Linux host, Watch app, test key, or authenticated provisioning channel can forge research data.

## Review conclusion

Portable framing, deadline enforcement, temporary test enrollment and Rust/Swift cryptographic fixture integration are suitable for software review once the current CI passes. Keep physical signed approval claims blocked until Apple build and hardware gates close. No physical or production trust claim is supported. `authorization: never_evaluated` is mandatory for every result.
