# P002 — Apple Watch Auto Unlock research

**Status:** Research complete; no unlock implementation. Research snapshot: 2026-10-27.
**Branch base:** `main` at `5763b7adc8d4099bcfb72a17ff48cdac7cf60904`.
**Scope:** Apple Watch SE (2nd generation) and macOS Auto Unlock. AirDrop and AirPods Pro 3 remain future milestones.

## Decision

**NO-GO for native Apple Auto Unlock implementation on Linux using currently documented public interfaces.** Apple documents a mutual STS protocol whose key material is exchanged at enablement and whose tunnel terminates in the devices' Secure Enclaves (with a stated kernel exception for Macs without a Secure Enclave). Apple also requires an enabled device relationship under the same Apple Account. The public material reviewed does not describe a third-party Linux enrollment/API or a public implementation of Apple's STS endpoint. BLE observation, Continuity advertisement parsing, IRK resolution, and BlueZ discovery do not implement this protocol.

The conclusion is **BLOCKED for a supported native implementation on available public interfaces**, while the broader question of whether undocumented Apple behavior could be replicated remains **UNKNOWN**. This is not evidence that Apple Watch SE 2 cannot unlock supported Macs: Apple's own feature is documented for Apple Watch, and its public requirements do not single out SE 2 as incompatible. Linux interoperability is the open issue. A separately enrolled open protocol is a different product and is also **BLOCKED pending evidence of a safe, usable watch-side endpoint and user-presence interaction**. P002 does not design that protocol or claim that an Apple Watch can run it.

## Evidence labels

- **VERIFIED** — reproducible from this repository's code/tests or an identified upstream source at a fixed revision. This label does not mean Apple has verified an inferred proprietary protocol claim.
- **DOCUMENTED** — stated by Apple, the Bluetooth SIG, BlueZ, or a cited upstream project.
- **HYPOTHESIS** — a plausible explanation that has not been independently established.
- **UNKNOWN** — evidence reviewed does not resolve the question.
- **BLOCKED** — a concrete dependency is missing for the proposed implementation path.

## Established architecture

1. **DOCUMENTED — BLE is a setup and rendezvous transport.** Apple's Platform Security guide says the devices establish a BLE connection to prepare for automatic unlock. It does not publish the Auto Unlock GATT service, characteristics, or wire messages. [A1]
2. **DOCUMENTED — Auto Unlock uses mutual STS authentication and long-term trust.** Apple says feature enablement exchanges long-term keys, each request negotiates unique ephemeral session keys, and the STS tunnel is negotiated directly between the Secure Enclaves. For Macs without a Secure Enclave, Apple says the kernel terminates that tunnel. [A1]
3. **DOCUMENTED — unlock secrets rotate.** Apple describes a target-generated 32-byte random unlock secret, an unlock record protected by that secret, a fresh secret returned after use, and discarding the used secret and PDK from memory. The public page does not disclose STS message framing or enrollment key derivation. [A1]
4. **DOCUMENTED — proximity is not RSSI-only in Apple's flow.** Apple says the devices make a new BLE connection, then use peer-to-peer Wi-Fi to securely approximate distance, with Mac/Watch distance required to be 2–3 m or less. Exact ranging messages, calibrated error bounds, and relay-attack properties are not published in the reviewed source. [A1]
5. **DOCUMENTED — Apple Account and explicit feature enablement are prerequisites.** Apple's setup guide requires Bluetooth and Wi-Fi, the same Apple Account with two-factor authentication, a Watch passcode, and enabling the Watch on the Mac. After restart/log out, the first Mac login requires the password. [A2]
6. **DOCUMENTED — policy depends on Watch state.** Apple lists that the Watch must be unlocked and not in Bedtime mode, and the Mac must have been unlocked by another method at least once after the Watch was put on wrist and unlocked. [A1]
7. **UNKNOWN — transport details beyond the published BLE and peer-to-peer Wi-Fi roles.** No evidence reviewed establishes whether AWDL is used for this specific Auto Unlock exchange, which Wi-Fi frame types are involved, or whether another proprietary transport participates. Do not label it AWDL without capture or direct Apple documentation.
8. **UNKNOWN — ordinary Bluetooth SMP pairing's role.** Apple's public Auto Unlock account describes mutual STS and a BLE connection, but does not say the trust handshake is a normal BLE bond or expose its pairing records. BlueZ `Pair()` is a distinct standard API that starts Bluetooth pairing; invoking it is out of scope for the P002 tool.

## BLE discovery, identity, and BlueZ

- **DOCUMENTED:** BlueZ `org.bluez.Adapter1` offers discovery start/stop and filters for LE transport, service UUID, RSSI, manufacturer/service data and duplicates. Discovery is shared among clients; stopping one session does not necessarily stop the controller scan. BlueZ can expose RSSI and advertisement metadata on device objects. [B1, B2]
- **DOCUMENTED:** the Bluetooth Core Specification defines resolvable private addresses and IRKs for address privacy and resolution. Resolving an RPA says that the address matches an IRK; it does not prove a fresh authenticated Auto Unlock session, wearer intent, physical proximity, or exclusive possession of the key. [B3]
- **VERIFIED (upstream source behavior):** `watch-unlock-rs` at `360aafcaca6394359646a14fb2e771a56d5b6ef0` performs LE discovery, resolves an address using a configured IRK, reads RSSI, then parses Apple's manufacturer data. It has no STS, session challenge, replay state, peer-to-peer Wi-Fi ranging, or session binding in the inspected code. [U1]
- **VERIFIED (upstream source behavior):** `watch-unlock-linux` at `9f5e8c695459089e20bcf3898aa8a46acd629c9f` parses HCI/Continuity advertising, checks an IRK match and Nearby Info flags, compares RSSI thresholds, and calls `loginctl unlock-session`. The README instructs extracting an IRK from macOS Keychain and running a privileged system service. This is not evidence of native Auto Unlock protocol interoperability. [U2]
- **DOCUMENTED, semantics uncertain:** the `continuity` project's Nearby Info table lists the Watch-locked and Auto Unlock bits but explicitly marks the flag meanings as “Not sure about this flag”. The watch-unlock projects use those bits as operational inputs. Therefore their interpretation is not promoted to VERIFIED here. [U3, U1, U2]
- **DOCUMENTED:** BlueZ can perform service discovery after connection, expose `ServicesResolved`, and provide `Pair`, `Connect` and related methods. BlueZ does not provide Apple's private STS, Apple Account relationship setup, Secure Enclave access, or the Apple Watch's private unlock records. [B2]

### Distinct concepts

| Signal/mechanism | What evidence supports | What it does not establish |
| --- | --- | --- |
| Name / manufacturer company ID | An advertisement contained a label/company field | Device identity or authenticity |
| Address / IRK match | Address resolution under a particular IRK, if the key is correct | Fresh authentication, user presence, or proximity |
| Nearby Info flags | A reverse-engineered advertisement field with uncertain semantics in the upstream format document | Unlock authorization or Apple STS success |
| RSSI | Received signal level at one receiver at one time | Reliable distance; secure proximity; relay resistance |
| BlueZ `Pair()` / `Connect()` | Bluetooth pairing or link establishment as exposed by BlueZ | Apple Auto Unlock enrollment or authentication |
| Apple STS + rotating secret + peer-to-peer Wi-Fi range | Apple-documented Auto Unlock architecture for supported Apple devices | A public Linux implementation path |

## Apple Watch SE 2 compatibility

- **DOCUMENTED:** Apple identifies the second-generation SE hardware and its S8 SiP in its technical specifications. [A3]
- **DOCUMENTED:** Apple describes Watch-to-Mac Auto Unlock generically and lists Mac/OS prerequisites; the reviewed support page does not name SE 2 as excluded or provide a model-by-model Watch compatibility table. Apple also documents Series 2 and earlier working with compatible Macs, indicating that this feature is not limited to recent watch generations. [A2, A4]
- **UNKNOWN:** This research did not conduct a physical SE 2/macOS unlock test, record an SE 2 advertisement, or verify current SE 2 watchOS behavior. No P002 hardware pass is claimed.
- **UNKNOWN:** Whether SE 2 has any model-specific Auto Unlock cryptographic or radio behavior. No reviewed Apple source provides this implementation detail.

## Apple-private dependencies and interoperability barrier

- **DOCUMENTED:** Apple's description places STS key material and the Auto Unlock tunnel in Secure Enclaves and stores the target-side unlock record/key material in protected device domains. [A1]
- **DOCUMENTED:** Apple setup binds the relationship to the same Apple Account and a user-enabled setting. [A2]
- **UNKNOWN:** Exact entitlements, private APIs, provisioning records, and account services used internally. The public sources reviewed do not disclose them. Do not state that a specific entitlement is required without evidence.
- **BLOCKED:** A Linux process cannot implement the published endpoint merely by scanning BLE. Linux has no Apple Secure Enclave and no documented API to enroll a Linux host into Apple's trust relationship or access Watch-held unlock records. No supported Apple integration path was found in the reviewed public documentation.
- **HYPOTHESIS:** Reverse engineering advertisement behavior alone cannot reconstruct the complete unlock mechanism because Apple's documented security channel and secret lifecycle are outside those advertisements. The exact undocumented implementation may differ; the published architecture is the basis for the go/no-go.

## Implementation in this branch

The Rust core adds evidence-state and sanitized observation types, a platform-independent discovery trait, and a deterministic mock fixture backend. `opencontinuity research watch --fixture <path> [--json]` only reads JSON fixtures. It does not access BlueZ or a radio and cannot represent an authorized outcome. Unknown fixture fields are rejected; names and RSSI remain untrusted. The existing `doctor` output remains static and unchanged in meaning.

## Carry-over from P001 audit

The supplied P001–P060 roadmap was not present in the repository or workspace at inspection; P001 scope is not inferred beyond the checked-in docs and PR #1. The P001 commit does not contain Bluetooth adapter scaffolding or a Qt 6/QML Settings framework. The new discovery trait is a small, mock-backed **P002 research abstraction**, not a completed BlueZ adapter scaffold. Qt 6/QML Settings remains absent and unsupported. These are recorded as unresolved scope reconciliation items, not completed features.

## Source index

- [A1] Apple Platform Security, [Automatically unlock Apple devices](https://support.apple.com/en-ca/guide/security/sec6ab47ebfc/web).
- [A2] Apple Support, [Unlock your Mac with Apple Watch](https://support.apple.com/en-us/102442).
- [A3] Apple Support, [Apple Watch SE (2nd generation) technical specifications](https://support.apple.com/en-us/111853).
- [A4] Apple Support, [Continuity features and requirements](https://support.apple.com/en-us/108046).
- [B1] BlueZ, [`org.bluez.Adapter1` API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst); pinned source reviewed at [BlueZ commit `291cff0680095a2bd4dd35ec0ccb5c42cf9c347c`](https://kernel.googlesource.com/pub/scm/bluetooth/bluez/+/291cff0680095a2bd4dd35ec0ccb5c42cf9c347c/doc/adapter-api.txt).
- [B2] BlueZ, [`org.bluez.Device1` API](https://github.com/bluez/bluez/blob/master/doc/org.bluez.Device.rst).
- [B3] Bluetooth SIG, [Core Specification 5.4 architecture and LE privacy](https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/Core-54/out/en/architecture%2C-mixing%2C-and-conventions/architecture.html).
- [U1] [watch-unlock-rs](https://github.com/KatelynHaworth/watch-unlock-rs/tree/360aafcaca6394359646a14fb2e771a56d5b6ef0), MIT, commit `360aafcaca6394359646a14fb2e771a56d5b6ef0`.
- [U2] [watch-unlock-linux](https://github.com/DavidSt49/watch-unlock-linux/tree/9f5e8c695459089e20bcf3898aa8a46acd629c9f), no license file or GitHub license classification at reviewed revision, commit `9f5e8c695459089e20bcf3898aa8a46acd629c9f`.
- [U3] [continuity](https://github.com/furiousMAC/continuity/tree/d9fa98bddafbfa3baafc84e37a417d78a0598546), GPL-2.0, commit `d9fa98bddafbfa3baafc84e37a417d78a0598546`, especially `messages/nearby_info.md`.
- [U4] [owly](https://github.com/t4t5/owly/tree/9a676e11d011c34ea32798a2541f2b1eeb66e731), MIT repository license; its bundled `omdrop-awdl` component includes GPL-2.0-only material; commit `9a676e11d011c34ea32798a2541f2b1eeb66e731`. AirDrop/AWDL only; no Watch unlock coverage inspected.
