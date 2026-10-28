# P003 — Watch endpoint feasibility

**Status:** Research and portable mock prototype complete; Apple build and hardware testing blocked/not run. Evidence snapshot: 2026-10-28.

**Branch base:** `main` at `365444eea4a776b92dc2dfa3315275929e09ef63`; P003 branch began at `6147c931565988184a31dca4022cbf5bd157b08e`.

**Target:** Apple Watch SE (2nd generation), optional paired iPhone, Linux host.

> Roadmap reconciliation: the original P001–P060 planning document is still unavailable in the checkout. This is a provisional continuation of P002, not a verified transcription of the original P003 intent. Existing P001 docs establish a CLI and idle daemon but no Bluetooth/BlueZ adapter scaffold and no Qt 6/QML Settings scaffolding. Both gaps remain open.

## Decision summary

| Path | Decision | Exact blocker / evidence needed |
| --- | --- | --- |
| Direct Watch ↔ Linux BLE | **CONDITIONAL GO for foreground experiment; NO-GO for unattended endpoint.** watchOS publicly supports the central role, so a foreground Watch app may connect to a Linux BLE peripheral. A Watch GATT peripheral/server advertising the service is blocked by Apple's `CBPeripheralManager` documentation. | Build with a supported macOS/Xcode/watchOS SDK, implement only a foreground central app, and test Watch SE 2 ↔ BlueZ GATT on owned hardware. Background reconnect is task-driven, not a continuously available server. |
| Direct Watch ↔ Linux IP | **UNKNOWN / not a P003 implementation target yet.** watchOS supports `URLSession` networking and may route traffic through iPhone, Wi-Fi, or cellular. Apple does not establish this project's direct local Linux endpoint, discovery, or delivery behavior. | An Apple SDK build and physical test must show Watch → host local-IP request under paired-iPhone, Wi-Fi-only, and cellular states, plus local network policy and timeout behavior. |
| Watch ↔ iPhone ↔ Linux | **GO as an optional, user-mediated relay experiment; NO-GO as an independent Watch-authentication claim.** `WCSession` offers immediate messaging when reachable and queued background transfer with non-immediate delivery. iPhone-to-Linux BLE/LAN is technically possible as a separately implemented link, but no such adapter exists here. | Physical iPhone and Watch tests, iOS background behavior, bounded latency, relay outage recovery, and a Watch signature verified end-to-end by Linux. The iPhone cannot create or modify a Watch approval. |
| Foreground Watch confirmation | **CONDITIONAL GO for a separate open-protocol prototype.** User-initiated app interaction plus Local Authentication with wrist detection (watchOS 9+) can provide a stronger presence condition than RSSI or reachability. | Validate actual prompt/approval UX, Secure Enclave/key API availability on SE 2, and whether a public API can bind key use to the same approval action on physical hardware. No Apple SDK or device test occurred. |
| Background / hands-free authorization | **NO-GO on current public evidence.** watchOS has bounded feature-specific sessions and background tasks, not a general always-running third-party listener. CoreBluetooth background behavior does not promise a 24/7 response. | This decision changes only if Apple documents a general continuous execution/notification entitlement or repeatable physical testing demonstrates a supported continuous service within published APIs and policies. |
| Future Linux unlock integration | **BLOCKED and out of scope for P003.** A separately enrolled foreground approval protocol is technically plausible, but it is not Apple Auto Unlock and has no trusted hardware endpoint demonstrated. Native Apple Auto Unlock remains blocked by P002's absent public third-party STS/enrollment interface. | Secure watch endpoint/key custody, tested user presence, host/account binding, relay/wormhole threat analysis, independent protocol review, and a separate authorization milestone. No PAM, root service, or automatic unlock was added. |

## Research findings

### A. Direct Watch ↔ Linux

- **DOCUMENTED:** Core Bluetooth lets apps manage central connections to remote peripherals. Apple's watchOS guidance describes direct Watch connections to Bluetooth accessories. Thus the plausible direct BLE topology is *Linux BlueZ peripheral / Watch central*, initiated in Watch app foreground. [A1] [A2]
- **BLOCKED:** Apple's `CBPeripheralManager` reference states service advertising through that API is unavailable on watchOS. A third-party Watch GATT server that Linux scans for is not a supported topology in the reviewed public API. This does not preclude Apple system-owned BLE roles; it specifically constrains third-party app APIs. [A3]
- **DOCUMENTED with limits:** watchOS 8+ can reconnect to Bluetooth peripherals during a background refresh task, and watchOS 9+ can retain a peripheral connection while the app is suspended. Apple still describes this work in terms of specific tasks, sessions, connections and budgeted work. This does not establish an always-on background listener or arbitrary wake-on-Linux-challenge service. [A4]
- **DOCUMENTED:** Watch apps use `URLSession` for network requests. Apple's guide says the request may route via the paired iPhone, a known Wi-Fi network or cellular. Network framework and low-level networking have watchOS-specific constraints. **UNKNOWN:** direct arbitrary local-LAN host connectivity and discovery for an authentication exchange on SE 2 have not been tested. Do not infer direct LAN support from internet requests. [A5] [A6]
- **DOCUMENTED:** Apple's Local Network Privacy technote lists watchOS as not implementing that privacy prompt/control. This does not prove that all local networking APIs work on Watch; transport/API support is a separate issue. [A7]
- **HARDWARE:** Apple lists SE 2 Bluetooth 5.3, Wi-Fi 4 and S8 SiP. No UWB sensor appears in the published SE 2 specs. These product facts do not establish a supported proximity-ranging interface or Auto Unlock endpoint. [A8]

### B. Apple Watch ↔ iPhone ↔ Linux

- **DOCUMENTED:** `WatchConnectivity` connects an iOS app and its paired WatchKit app. `sendMessage` requires `isReachable`; Apple describes the Watch extension as running in the foreground or certain high-priority background contexts. A false or changing reachable state causes errors. [B1] [B2]
- **DOCUMENTED:** `transferUserInfo` queues work for background delivery and persists across app suspension/termination, but Apple says delivery is opportunistic, can be delayed, and is not instantaneous. It is unsuitable as a bounded login challenge-response delivery guarantee. [B1] [B3]
- **DOCUMENTED:** an active Watch extension can wake its iOS counterpart when sending an immediate message; the reverse direction does not wake the Watch extension. This asymmetry makes phone-originated silent prompt delivery unreliable. A Watch-initiated foreground action is better aligned with the API. [B4]
- **HYPOTHESIS:** iPhone can separately communicate with Linux over BLE central/peripheral APIs or IP networking, but iOS background scheduling and device policy will affect reachability and latency. P003 has no iOS companion, transport adapter or timing measurements.
- **Security condition:** Linux trusts the enrolled Watch public key and a signature over the complete fresh challenge, account/session/action/version/expiry and explicit approval. iPhone only forwards bytes. A compromised relay can suppress, reorder, delay, replay or redirect messages, but cannot forge a correctly bound Watch signature. Expired and replayed responses fail closed. The phone is not the Watch identity, and `WCSession.isReachable` never means presence.

### C. Explicit Watch confirmation

- **DOCUMENTED:** `LAPolicy.deviceOwnerAuthenticationWithWristDetection` is available on watchOS 9+; Apple says evaluation fails if no Watch passcode is set/entered or the Watch has detected removal from the wrist. This is passcode/wrist-state authentication, not Touch ID or a third-party biometric API. [C1]
- **SECURITY DESIGN:** show the host, local account, and requested action in a foreground Watch screen; require an explicit approve/deny gesture; then require successful wrist-detection policy evaluation before signing one challenge. Denial, cancel, policy failure, notification-only delivery, screen lock, or timeout produces no approval. Wrist detection and prior passcode entry support device-owner presence but do not prove intent by themselves; the explicit app action supplies intent.
- **DOCUMENTED but untested:** CryptoKit exposes public key cryptography and Secure Enclave P-256 signing APIs. API documentation is not evidence that this app can create the required access-controlled key or bind a signature to this consent flow on the SE 2. Validate `SecureEnclave.isAvailable`, key accessibility, and Watch keychain behavior in an Apple build and on physical hardware. [C2] [C3]
- Apple's Watch SE 2 specifications list an optical heart sensor, not Touch ID. No Touch ID or third-party biometric/watch-unlock API is claimed. Apple's Local Authentication interface is the documented user-authentication mechanism examined here. [A8] [C1]

## Evidence labels

- **VERIFIED:** repository behavior reproduced by code inspection or local automated tests. It does not mean Apple has verified a protocol.
- **DOCUMENTED:** stated in linked Apple or BlueZ primary documentation.
- **HYPOTHESIS:** plausible architecture that has not been built or measured here.
- **UNKNOWN:** evidence reviewed does not establish the behavior or fact.
- **BLOCKED:** a concrete public API, toolchain, device or evidence dependency prevents the proposed path.

## P003 portable prototype

`opencontinuity research watch-endpoint --fixture tests/fixtures/watch-endpoint.json --json` processes a bounded, versioned offline scenario. The portable core has endpoint/transport traits, explicit capability and trust/evidence fields, a mock Watch signer, a mock transport, fixed-size challenge nonce validation, a 30-second maximum challenge lifetime, Ed25519 message signatures via `ed25519-dalek`, transcript binding, explicit-presence gating, and replay rejection. `sha2` derives deterministic mock nonces only.

The fixture uses a public, hard-coded **mock-only** Ed25519 seed and all identifiers are synthetic. This makes output reproducible, not secure. The report says `offline_mock_simulation_only`, reports simulation evidence, and sets authorization to `never_evaluated`. It never uses Apple APIs, Bluetooth, LAN, an iPhone or an Apple Watch. The code is not a deployed authentication protocol; it lacks a production CSPRNG, secure enrollment, authenticated host-to-Watch setup, hardware key custody, and a security review. The CLI and daemon do not gain an unlock path.

## Preserved P002 conclusions

P002 found that Apple Auto Unlock documents authenticated STS, protected key material, rotating secrets, and peer-to-peer Wi-Fi proximity assessment. Public third-party Linux enrollment/STS interoperability is still blocked. BLE advertisements, IRK matches, RSSI, Apple Account pairing, WatchConnectivity reachability and a mock signature are not native Apple Auto Unlock and do not authorize Linux unlock. [P002 research]

## Sources

- [A1] [Apple Core Bluetooth](https://developer.apple.com/documentation/corebluetooth) — BLE central/peripheral framework.
- [A2] [Apple WWDC21: Connect Bluetooth devices to Apple Watch](https://developer.apple.com/videos/play/wwdc2021/10005/) — direct Watch-to-accessory CoreBluetooth and foreground/background refresh model.
- [A3] [Apple `CBPeripheralManager`](https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager) — watchOS cannot advertise services with this API.
- [A4] [Apple WatchKit: Using background tasks](https://developer.apple.com/documentation/watchkit/using-background-tasks) and [watchOS background execution](https://developer.apple.com/documentation/watchkit/background-execution).
- [A5] [Apple: Keeping your watchOS content up to date](https://developer.apple.com/documentation/watchos-apps/keeping-your-watchos-app-s-content-up-to-date).
- [A6] [Apple TN3135: Low-level networking on watchOS](https://developer.apple.com/documentation/technotes/tn3135-low-level-networking-on-watchos).
- [A7] [Apple TN3179: Understanding local network privacy](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy).
- [A8] [Apple Watch SE (2nd generation) technical specifications](https://support.apple.com/en-us/111853).
- [B1] [Apple Watch Connectivity / `WCSession`](https://developer.apple.com/documentation/watchconnectivity/wcsession).
- [B2] [Apple `WCSession.isReachable`](https://developer.apple.com/documentation/watchconnectivity/wcsession/isreachable).
- [B3] [Apple `transferUserInfo(_:)`](https://developer.apple.com/documentation/watchconnectivity/wcsession/transferuserinfo(_:)).
- [B4] [Apple `sendMessage(_:replyHandler:errorHandler:)`](https://developer.apple.com/documentation/watchconnectivity/wcsession/sendmessage(_:replyhandler:errorhandler:)).
- [C1] [Apple `deviceOwnerAuthenticationWithWristDetection`](https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthenticationwithwristdetection).
- [C2] [Apple CryptoKit Secure Enclave](https://developer.apple.com/documentation/cryptokit/secureenclave).
- [C3] [Apple Secure Enclave P-256 signing key](https://developer.apple.com/documentation/cryptokit/secureenclave/p256/signing/privatekey).
- [P002 research] [P002 Auto Unlock analysis](P002_WATCH_AUTO_UNLOCK.md), [protocol matrix](P002_PROTOCOL_MATRIX.md), and [security analysis](P002_SECURITY_ANALYSIS.md).

No external hardware/build result is asserted by this report. Physical Watch SE 2, paired iPhone and BlueZ validation is **NOT RUN**; macOS/Xcode/watchOS build is **BLOCKED** because this Linux environment has no Apple toolchain.
