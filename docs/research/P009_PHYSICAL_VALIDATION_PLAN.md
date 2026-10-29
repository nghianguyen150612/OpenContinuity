# P009 physical validation plan

## Current status

The available execution host is Linux without Swift, Xcode, XcodeGen, a BlueZ system bus, a controller, or an Apple Watch. No physical P009 trial has been run. Software framing and cryptographic conformance must not be reported as BLE interoperability.

| Stage | Status | Evidence required for a pass |
| --- | --- | --- |
| Rust transport framing/reassembly | Implemented; local tests pending final gates | Rust tests exercise implementation bytes and failure cases. |
| Swift transport framing/reassembly | Source/test added; NOT RUN locally | `swift test --package-path experiments/watchos` on Apple CI. |
| Rust → Swift → Rust serialized signed exchange | CI workflow added; NOT RUN yet | Rust emits real request frames; Swift reassembles/verifies with CryptoKit, signs a simulated rejection and emits response frames; Rust reassembles and verifies P008. |
| watchOS target unsigned compile | NOT RUN locally | XcodeGen generation and `xcodebuild ... generic/platform=watchOS CODE_SIGNING_ALLOWED=NO build` on Apple CI. |
| BlueZ read-only preflight | NOT RUN | Selected adapter, powered state, manager interfaces, D-Bus owner and capabilities captured without changing configuration. |
| Experimental service registration/cleanup | NOT RUN | P009 BlueZ service source is implemented with owned application/advertisement cleanup; verify only those resources are removed on a host with BlueZ. |
| Linux controller advertisement/RF | NOT RUN | Capture controller model/firmware, adapter selection, advertisement visibility, run duration and cleanup. |
| Watch foreground connection and service discovery | NOT RUN | Signed, authorized test app on Watch SE 2; record watchOS build and CoreBluetooth logs with identifiers redacted. |
| Host request GET/read/ACK with repeated reads | NOT RUN | The CoreBluetooth client path uses one GET/read/ACK at a time; exercise duplicate GET, retry, missing ACK, out-of-order ACK, timeout and disconnect on-device. |
| Watch response sequential write-with-response | NOT RUN | The CoreBluetooth client waits for each write callback; confirm no more than one in-flight write, exact frame reconstruction, completion and disconnect invalidation. |
| Actual LocalAuthentication availability and cancellation | NOT RUN | Record `canEvaluatePolicy(.deviceOwnerAuthentication)` and user-cancel/failure paths on Watch SE 2; no approval may be signed on failure. |
| Independently pinned temporary test enrollment | BLOCKED | A reviewed out-of-band setup and in-memory key injection path are required before a live signed flow. Pairing or advertised key material is not sufficient. |
| End-to-end physical signed rejection/approval | BLOCKED | Requires reviewed safe enrollment and temporary-key injection, software gates, device test, and distinct human consent. Approval requires successful on-device LocalAuthentication. |
| Secure Enclave custody or genuine Watch identity | NOT CLAIMED | Requires independent hardware evidence and a separate design; no code or software test establishes this. |

## Hardware procedure after blockers close

1. Use a dedicated Linux test host and Watch SE 2. Record OS/kernel, BlueZ, controller/firmware, Xcode/watchOS and app versions.
2. Establish and independently verify a temporary test-only host/Watch enrollment out of band. Do not derive trust from the BLE peer, pairing, name, RSSI or service UUID.
3. Confirm selected adapter preflight is read-only and already powered. Run only the explicitly opted-in, bounded-duration process.
4. After a separately reviewed harness or setup supplies test-only pins and the temporary Watch key in memory, start the Watch app in the foreground and explicitly initiate the P009 scan. The current app has no such setup or scan UI. Observe the fixed-fragment GET/read/ACK sequence and the single-in-flight response write flow.
5. Exercise successful transport, repeated GET, dropped read callback, missing/out-of-order ACK, malformed/duplicate frame, wrong transfer ID, timeout, explicit cancel, Watch disconnect, BlueZ owner restart, signed rejection, wrong host key, wrong Watch key, replay and expiration.
6. Attempt approval only after the host signature and bindings verify, an explicit tap, supported LocalAuthentication, successful authentication, and a fresh expiry check. Record only sanitized logs and signature-verification outcomes.
7. Confirm all BlueZ objects are unregistered after completion, failure, timeout and signal termination. Confirm no controller settings, global configuration, PAM, polkit, login manager or desktop session state changed.
8. Delete temporary keys and enrollment records. Report physical and software stages separately, preserving `authorization: never_evaluated`.

No stage above is a PASS until evidence is collected from the named hardware. A physical result remains independent of CI and cannot imply native Apple Auto Unlock, secure proximity, wearer presence or Linux authorization.
