# P002 security analysis

## Scope and security decision

This analysis covers research and any hypothetical future Watch unlock path. The current P002 code reads sanitized local JSON fixtures only. It has no BlueZ connection, BLE scan, pairing, authentication, authorization, PAM, polkit, login-manager, privileged service, key storage, or device-write path. The report format can represent only `authorization: never_evaluated`.

BLE presence is an untrusted observation. Device names, company IDs, manufacturer data, addresses, RSSI, an IRK-resolved private address, and reverse-engineered lock/autounlock flags do not authorize a session. Apple documents a distinct mutual STS protocol, per-request ephemeral keys, rotating unlock secrets, protected device key material, and peer-to-peer Wi-Fi distance approximation for its own ecosystem. [Apple Platform Security](https://support.apple.com/en-ca/guide/security/sec6ab47ebfc/web)

## Threat assessment

| Threat | Attack path | P002 disposition | Requirements before any unlock prototype |
| --- | --- | --- | --- |
| Advertisement spoofing | Attacker advertises an Apple-like name, company ID, or copied Nearby Info bytes. | Fixture and any capture remain untrusted; no identity claim or authorization path. | Authenticate an enrolled key cryptographically; reject label/address/flag-only matches. |
| Address spoofing / IRK theft | Attacker copies a static address or obtains an IRK and generates matching RPAs. | No keys are accepted or stored. Address resolution is only a BLE privacy function. | Do not expose or extract Apple pairing secrets; use an enrollment secret designed for the new protocol, protected at rest and revocable. |
| Replay | Recorded advertisement or prior response is replayed. | No unlock token or acceptance logic exists. | Fresh unpredictable challenge; authenticated transcript binding both peers, host, user/session, protocol version and purpose; one-time response and bounded lifetime. |
| Relay / wormhole | Nearby radio endpoint relays traffic to a distant enrolled Watch, making RSSI at the host misleading. | RSSI is not treated as distance or authority. | Defensible distance bounding or explicit user action with documented limitations; test relay setup. Ordinary BLE RSSI and timeouts are insufficient. |
| RSSI manipulation / multipath | TX power, body attenuation, antenna placement, reflections or interference alter measured RSSI. | RSSI is represented as untrusted observation only. | Never use RSSI as authentication or sole proximity gate. If used for UX, label it heuristic and independent of auth. |
| Downgrade / protocol confusion | Attacker forces legacy advertisement-only behavior or weaker protocol version. | No negotiation or protocol fallback is implemented. | Pin allowed protocol versions and algorithms in authenticated transcript; fail closed on unsupported, ambiguous or downgraded capability. |
| Malicious nearby device / parser exploit | Crafted malformed BLE frames or fixture data targets parsers. | P002 parses bounded JSON fields and rejects unknown fields; it does not parse raw BLE packets. | Add fuzzing, length limits, memory-safe parsing, isolated capture fixtures, and independent parser review before a radio adapter. |
| Compromised Linux host | Malware reads enrollment keys, injects authorization, alters policy, or logs secrets. | No secret or privileged operation exists. | Minimize exposure; isolate auth broker, bind to PAM caller/session, hardware-backed storage where available, secure boot/integrity policy, audit and revoke; recognize host compromise remains a critical residual risk. |
| Stolen Watch | A stolen Watch is unlocked and remains worn or is removed before auto-lock timeout. | No watch state is accepted as authority. Apple itself requires Watch unlocked and not in Bedtime mode and a prior Mac password unlock. [Apple Platform Security](https://support.apple.com/en-ca/guide/security/sec6ab47ebfc/web) | Require explicit local policy, rapid revocation, short enrollment lifetime/re-auth, and define stolen-device lock behavior. A wearable-presence signal cannot prove the enrolled user is wearing it. |
| Unattended unlock | Watch is left next to the machine, relayed, or used while owner is absent. | No unlock function. | Define user-presence model, explicit opt-in per account, relock behavior, timeout and fallback; require user action if presence cannot be defended. |
| Key extraction / database leak | IRK or new protocol enrollment key is copied from config, logs, backups or crash dumps. | No credentials or secrets accepted by P002. | No plaintext logs; OS secret service/hardware key where practical; authenticated, integrity-protected storage, least privilege and secure deletion/re-enrollment. |
| Capture privacy | BLE capture reveals persistent identifiers or nearby third-party device metadata. | The repository stores only a synthetic fixture with no address or payload. | Capture only in controlled space with owned devices; minimize duration; redact addresses and payload fields before sharing; delete raw captures after review. |

## Trust model for future design

A future implementation must maintain separate decisions for:

1. **Discovery:** a radio observation exists. Untrusted.
2. **Identity:** peer proves possession of a secret enrolled through an explicit ceremony. An IRK match is not enough.
3. **Authentication:** a fresh cryptographic challenge-response verifies an approved peer and binds a transcript to the intended host and account.
4. **Freshness and session binding:** nonces/counters, expiry and replay cache prevent reuse; authentication includes session/user/purpose.
5. **Proximity and user presence:** independent evidence has a documented relay threat model. RSSI does not fill this role.
6. **Authorization:** explicit local policy maps an authenticated peer and user action to a narrowly scoped unlock decision.
7. **Operational readiness:** secure storage, lock state, daemon health and OS integration are separately established.

Missing or ambiguous evidence, protocol version mismatch, timeout, adapter error, multiple candidate identities, revoked enrollment, stale response or malformed frame must deny authorization. There must be no automatic enrollment and no silent fallback to RSSI/advertisement logic.

## Native Apple interoperability risks

Apple's documented STS and unlock-secret system keeps key operations in Apple device trust domains. Replacing those checks with an IRK and flags would remove the documented mutual authentication, session freshness, secret rotation and Wi-Fi distance checks. P002 rejects this substitution. Exact Apple private entitlements, pairing record contents and Watch-side implementation details remain **UNKNOWN**, not asserted facts.

## Separate open protocol risks

A custom protocol cannot automatically treat an Apple Watch as an authenticator. The project has no demonstrated Watch app endpoint, key provisioning capability, trustworthy user-presence signal, or secure way to conduct an enrollment with that device. A Linux-only credential or a shared static secret embedded in an app would create a new protocol and threat model. No such design or implementation is included in P002.
