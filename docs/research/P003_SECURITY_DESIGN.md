# P003 security design — non-production approval contract

**Status:** contract and deterministic mock only. No real enrollment, Apple key API, transport, PAM hook, or unlock code exists. The prototype is not suitable for production authentication.

## Principals, claims, and trust boundaries

| Principal / signal | May establish | Never establishes by itself |
| --- | --- | --- |
| Linux host | A local challenge and target OS account/session/action | Watch identity or user approval |
| Enrolled Watch signing key | Possession of a separately enrolled Watch endpoint key; signed challenge context | Apple provenance, genuine SE 2 hardware, physical proximity, wearer intent |
| Paired iPhone relay | Message forwarding and transport metadata | Watch identity, Watch approval, wearer presence |
| CoreBluetooth advertisement / discovery | An untrusted radio observation | Identity, authentication, proximity, authorization |
| BLE pairing / encrypted link | Transport peer/link security for that bond | Linux account authorization or Apple Auto Unlock trust |
| `WCSession.isReachable` | Companion app's current ability to send an immediate message | Watch running state as Linux identity or wearer presence |
| Local Authentication wrist/passcode result | OS-reported policy success at evaluation time | Fresh Linux challenge signature unless the app binds that success to key use |
| RSSI / timeout / network route | A heuristic or delivery fact | Distance bound or wormhole resistance |

## Enrollment and account binding

1. An administrator/user explicitly starts enrollment while logged into the intended Linux account. Verify local account identity and require an explicit host label. No system account is silently selected.
2. A foreground Watch app generates a non-exportable signing key using a public, platform-supported hardware key API if available. If the SE 2 runtime does not support the selected access-control policy, enrollment is refused. Do not export or inspect Apple Account keys, Auto Unlock records, IRKs or pairing secrets.
3. Bind the Watch key's public key/fingerprint to a random enrollment ID, a unique Linux host ID and the Linux UID. Confirm the fingerprint through a user-mediated comparison (for example, one-time QR/display confirmation) and verify by a fresh Watch-signed enrollment transcript. The paired phone may relay enrollment bytes but cannot confirm or rewrite them.
4. Store the host enrollment record with restrictive permissions and integrity protection in a per-user credential store where available. Store public Watch material on Linux; private Watch key stays in Watch key custody. Avoid a shared secret copied to the phone. Backups must not export private key material.
5. Revocation removes the enrollment and invalidates pending requests. Recovery is explicit local re-enrollment after host access by an independent method; there is no silent new Watch or phone substitution. Loss of the Watch requires revocation.

This ceremony is a design proposal, not implemented. Secure Enclave signing availability/access controls on Apple Watch SE 2 and an acceptable in-person enrollment UX remain **UNKNOWN** pending Apple SDK and device tests.

The host must also be authenticated to the Watch during enrollment and for each request, for example by pinning a host verification key during the user-mediated enrollment and signing the challenge. The current mock omits that host-to-Watch authentication step; it models only Watch response verification. A Linux endpoint presented by an untrusted network peer must not be allowed to prompt/sign as an enrolled host.

## Session challenge and approval message contract

The Rust prototype implements a bounded simulated schema, version 1:

- **Challenge:** exact protocol version; 32-byte nonce; host ID; local Linux UID/account reference; current login-session ID; requested action; issue and expiry times.
- **Approval:** exact challenge version and every challenge field; enrolled Watch identity ID; approve/deny; explicit-presence marker; response time; 64-byte Ed25519 signature over a domain-separated, length-prefixed canonical transcript.
- **Acceptance requirements:** exact enrolled identity and public key; signature verifies; version exactly supported; host/account/session/action/nonce/expiry match; `issued_at <= response_at <= now < expiry`; explicit Watch confirmation is present; approval is affirmative; nonce is unused. Any missing, duplicate, stale, malformed, unexpected, disconnected, unknown or unsupported message denies.
- **Bounds:** JSON message maximum 4096 bytes; identifiers 1–128 ASCII characters; nonce exactly 32 bytes; signature exactly 64 bytes; challenge lifetime at most 30 seconds. Unknown fields and unsupported versions fail closed.
- **Nonce:** simulation uses SHA-256 over a fixed test domain/counter to obtain deterministic, unique test nonces. A production implementation must use an OS CSPRNG with failure treated as denial; deterministic nonce generation is expressly forbidden for real authentication.
- **Timeout clock:** the mock's timestamps are test inputs only. A real host must enforce the deadline from its own monotonic clock at message receipt and must not trust a Watch-supplied timestamp to extend the request window. Cross-device wall-clock skew is not a freshness signal.
- **Version negotiation:** v1 is exact-match in the mock; there is no fallback. A future negotiable protocol must authenticate the supported-version set and chosen version, and reject downgrade. Unknown versions cannot be guessed or coerced to v1.
- **Authorization:** the report's only state is `never_evaluated`. Signature verification in the mock models endpoint authentication only; it does not map approval to a Linux unlock decision.

The prototype uses `ed25519-dalek` Ed25519 signing and `sha2` for deterministic mock nonces. It has a fixed test-only mock signing seed, no production secret. It does **not** implement public-key enrollment, an actual OS CSPRNG, a host-signed challenge, Watch secure storage, link encryption, account-store persistence, hardware attestation, or any real transport. Do not reuse its mock key, ID values, or nonce source.

## Threat responses

| Threat | Contract response and residual risk |
| --- | --- |
| Spoofed discovery / RSSI manipulation | Discovery does not move trust state; RSSI is never an authorization input. Only enrolled public-key signature can prove key possession. |
| Replay / duplicate response | Fresh challenge nonce, session-bound signed transcript, expiry and one-time nonce cache. Duplicate, late, or wrong-session responses are rejected. Cache persistence across process restart is a required future design item; the current mock cache is in-memory only. |
| Protocol downgrade / confusion | Exact version match, strict schema, domain separation, bounded lengths, reject unknown fields/actions and no legacy fallback. Future negotiation must be signed/bound. |
| Compromised iPhone | Relay may have its own relay identity for logging/diagnostics, but Linux must never treat that identity as the Watch key. It can observe metadata, deny service, replay/delay/reorder/redirect messages. It has no Watch signing key, cannot modify a valid signature, and cannot authorize as Watch. Challenge binds host/account/session/action. E2E signature does not prevent wormholing an interactive valid Watch exchange. |
| BLE/IP wormhole | No claim of physical proximity from RSSI, response time, Bluetooth link or route. A remote Watch can be tunneled. Explicit user action lowers unattended risk but does not supply cryptographic distance bounding. Residual risk blocks hands-free unlock. |
| Stolen Watch / owner absent | Requiring the app action plus passcode/wrist Local Authentication is stronger than a background presence bit. A stolen device on-wrist or attacker with its passcode remains a risk; revoke immediately and require independent host access to re-enroll. |
| Off-wrist / locked Watch | Wrist detection policy should fail after removal and an approval requires an active foreground action. Any unavailable/false result denies. Physical behavior must be tested, including delay before auto-lock. |
| Compromised phone identity substitution | Phone cannot enroll or substitute Watch key after enrollment. Linux verifies its own pinned Watch public key, not phone assertions or `WCSession` sender fields. |
| Compromised Linux host | Host can misuse account sessions, alter verifier, leak data or deny service. A Watch signature cannot make a compromised endpoint trustworthy. No P003 host privilege or auth-system change is allowed. |
| Key extraction / backup / logs | Watch key should be non-exportable where supported; public key is revocable. No secret/raw capture logging. Linux stores only public key and local binding; production key-store choice is unresolved. |
| Timeout / queue delay / dropped transport | Short expiry; no extension or background queue fallback for active auth. Any transport fault fails closed and user uses normal Linux login. |
| User denial, absent presence, invalid signature | Deny; no fallback to address, pairing, phone approval, or advertisement. |

## Non-negotiable invariants

1. Discovery never implies authorization.
2. RSSI never implies authenticated proximity.
3. WatchConnectivity reachability never implies user presence.
4. Bluetooth pairing alone never grants Linux authorization.
5. The relay never signs or independently asserts Watch approval.
6. Expired, duplicate, malformed, wrong-session, unknown-identity, wrong-version and unexpected messages fail closed.
7. Missing or unknown hardware capabilities remain `UNKNOWN` or `BLOCKED`.
8. No automatic PAM, login-manager, root-authentication, or session-unlock integration in P003.

## Residual questions before any future unlock milestone

- Can an SE 2 Watch app generate and use a hardware-backed signing key with access bound to the exact foreground Local Authentication approval?
- Does the Watch interface provide a clear host/account/action confirmation UX and a deny/cancel action under realistic lock/screen/wrist conditions?
- Can BLE or LAN availability be reliable enough for interactive use without weakening timeouts?
- Is a residual relay/wormhole risk acceptable for an explicitly user-initiated login approval? No public anti-relay guarantee was found for this custom protocol.
- What Linux secret store, privilege boundary, PAM caller/session binding, revocation journal, and crash-safe replay database would a separately reviewed future milestone use?
- What threat review and independent protocol review are required before any privilege boundary changes?

## References

- [Apple Local Authentication wrist-detection policy](https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthenticationwithwristdetection), [Apple Secure Enclave CryptoKit](https://developer.apple.com/documentation/cryptokit/secureenclave), and [P002 Apple Auto Unlock analysis](P002_WATCH_AUTO_UNLOCK.md).
- [Ed25519 RFC 8032](https://www.rfc-editor.org/rfc/rfc8032), [`ed25519-dalek` crate](https://crates.io/crates/ed25519-dalek), and [`sha2` crate](https://crates.io/crates/sha2).
