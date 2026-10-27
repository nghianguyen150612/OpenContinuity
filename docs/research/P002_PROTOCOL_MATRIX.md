# P002 protocol capability matrix

Research snapshot: 2026-10-27. Status labels follow [P002 Watch research](P002_WATCH_AUTO_UNLOCK.md). “Blocked” refers to the supported native Linux implementation path using public interfaces; it does not assert cryptographic impossibility.

| Area | Status | Evidence and confidence | Open point / boundary |
| --- | --- | --- | --- |
| Mac/Watch feature prerequisites | DOCUMENTED | Apple requires BLE and Wi-Fi on, same Apple Account with 2FA, Watch passcode, and explicit enablement on the Mac. [A2] | No physical SE 2 test was performed. |
| Auto Unlock rendezvous | DOCUMENTED | BLE connection used to prepare and initiate unlock. [A1] | GATT service/characteristics and exact BLE packets are not public in reviewed docs. |
| Nearby advertisement metadata | DOCUMENTED | Reverse-engineering sources document Apple's manufacturer data and Nearby Info message; flag meanings are explicitly marked uncertain by the format source. [U3] | Watch flag semantics/version coverage and SE 2 emissions are UNKNOWN. |
| Apple Watch detection by IRK | VERIFIED as code behavior; DOCUMENTED as Bluetooth mechanism | Two upstream projects resolve randomized addresses using an IRK; Bluetooth SIG defines IRK-based RPA resolution. [U1, U2, B3] | IRK availability, custody, revocation, and Apple Watch SE 2 behavior are not independently verified. Resolution is not session authentication. |
| RSSI proximity | DOCUMENTED | BlueZ reports RSSI and accepts RSSI filters. [B1, B2] | RSSI is not a defensible distance or anti-relay measurement. No authentication claim follows. |
| Service discovery | DOCUMENTED | BlueZ has LE discovery and Device1 service resolution APIs; APIs do not specify Apple's Auto Unlock GATT. [B1, B2] | Required proprietary services/characteristics unknown. P002 fixture mode does no Bluetooth discovery. |
| Standard Bluetooth pairing | DOCUMENTED as BlueZ capability; UNKNOWN as Auto Unlock mechanism | BlueZ `Pair()` is a standard host API; Apple public Auto Unlock description specifies STS trust, not whether normal SMP bonding is part of it. [B2, A1] | Do not initiate pairing to probe this question. |
| Auto Unlock enrollment/trust | DOCUMENTED for Apple-to-Apple | Same Apple Account, explicit relationship enablement, long-term keys exchanged at feature enablement. [A1, A2] | Public third-party enrollment interface: UNKNOWN; Linux enrollment path: BLOCKED on documented support. |
| STS mutual authentication | DOCUMENTED | Apple says STS is mutually authenticated, has long-term keys, per-request ephemeral session keys, and terminates inside Secure Enclaves (or kernel on specified Macs). [A1] | Public message format, cipher suite details, peer enrollment and Linux endpoint: UNKNOWN/BLOCKED. |
| Challenge-response / freshness | DOCUMENTED in Apple architecture | Unique ephemeral session keys and changing target-generated 32-byte unlock secrets are described. [A1] | Exact request nonce, transcript binding and replay proofs are not public; no Linux implementation exists. |
| Replay resistance | DOCUMENTED in part | Apple secret replacement/rotation means the documented successful-use secret is replaced; per-request ephemeral session keys are used. [A1] | Independent protocol audit and replay test vectors unavailable. Exact protections beyond this description are UNKNOWN. |
| Proximity assurance | DOCUMENTED | Apple says peer-to-peer Wi-Fi securely approximates distance; Mac/Watch condition is 2–3 m or less. [A1] | Exact ranging scheme, tolerances, relay bounds and role of AWDL: UNKNOWN. RSSI is not a substitute. |
| AWDL / Wi-Fi transport | DOCUMENTED only as peer-to-peer Wi-Fi | Apple names peer-to-peer Wi-Fi in its public unlock description. [A1] | AWDL use for Auto Unlock not established. Do not extrapolate from AirDrop. |
| Secure Enclave / Watch records | DOCUMENTED | Apple says STS crypto material stays in Secure Enclaves; target unlock record uses a protected PDK/secret lifecycle. [A1] | Which Apple Watch SE 2 internals provide each operation and exact record layout are not public. Linux access path: BLOCKED. |
| Apple-private entitlements / APIs | UNKNOWN | No public entitlement or developer API for third-party Auto Unlock enrollment found in sources reviewed. | Absence of public evidence is not proof that a particular entitlement exists. Treat unsupported until Apple documents otherwise. |
| Apple Watch SE 2 compatibility with Apple Mac | DOCUMENTED generically; SE 2-specific test UNKNOWN | Apple supports the Watch feature generically, lists Watch Series 2 and earlier support for Auto Unlock, and documents SE 2 hardware. [A2, A3, A4] | No SE 2/macOS hardware validation; current OS combinations still need direct test. |
| Apple Watch SE 2 as Linux peer (native protocol) | BLOCKED on available supported interfaces; overall feasibility UNKNOWN | No third-party enrollment/STS interface located; Linux has no Apple Secure Enclave, and advertisement parsing does not speak the documented authenticated protocol. [A1, B1, B2] | New Apple documentation or a supported Watch-side endpoint could alter assessment. |
| Open separately enrolled protocol | BLOCKED pending watch-side endpoint and security design | The repository has no watch application or public third-party Watch service capable of a documented Linux challenge-response. | Watch app APIs and user-presence semantics require targeted feasibility research; do not equate a custom protocol with Apple Auto Unlock. |
| P002 implementation capability | VERIFIED | Core types validate offline sanitized fixtures and emit `authorization: never_evaluated`; mock backend is deterministic and errors produce no report. | No hardware adapter, radio access, capture, pairing, identity resolution, or authentication is implemented. |

## Citation key

- [A1] [Apple Platform Security — Automatically unlock Apple devices](https://support.apple.com/en-ca/guide/security/sec6ab47ebfc/web)
- [A2] [Apple Support — Unlock your Mac with Apple Watch](https://support.apple.com/en-us/102442)
- [A3] [Apple Support — Apple Watch SE (2nd generation) technical specifications](https://support.apple.com/en-us/111853)
- [A4] [Apple Support — Continuity features and requirements](https://support.apple.com/en-us/108046)
- [B1] [BlueZ Adapter1 API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst)
- [B2] [BlueZ Device1 API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Device.rst)
- [B3] [Bluetooth SIG Core Specification 5.4, architecture and LE privacy](https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/Core-54/out/en/architecture%2C-mixing%2C-and-conventions/architecture.html)
- [U1] [watch-unlock-rs at `360aafcaca6394359646a14fb2e771a56d5b6ef0`](https://github.com/KatelynHaworth/watch-unlock-rs/tree/360aafcaca6394359646a14fb2e771a56d5b6ef0)
- [U2] [watch-unlock-linux at `9f5e8c695459089e20bcf3898aa8a46acd629c9f`](https://github.com/DavidSt49/watch-unlock-linux/tree/9f5e8c695459089e20bcf3898aa8a46acd629c9f)
- [U3] [continuity at `d9fa98bddafbfa3baafc84e37a417d78a0598546`](https://github.com/furiousMAC/continuity/tree/d9fa98bddafbfa3baafc84e37a417d78a0598546)
