# OpenContinuity Watch experiment

This is an experimental foreground watchOS BLE client for the non-authorizing P006 GATT transport, plus isolated P008 signed approval and P009 authenticated research transport. The GATT UI offers explicit scan, connect, synthetic trial, cancel and disconnect controls. A separate **Authenticated Research** screen creates an ephemeral CryptoKit P-256 Watch key, imports and independently verifies a Linux host public pin, exports the public Watch enrollment, requires explicit confirmation of both fingerprints, and then enables P009 scan/connect. A verified signed request is shown in `ResearchConsentView`; approval requires an explicit action and successful LocalAuthentication. No unverified request can present consent. Neither path contains a Linux unlock action.

## Source layout

- `OpenContinuityWatchApp.swift`, `ContentView.swift`: SwiftUI entry point and interface.
- `WatchBluetoothManager.swift`: retained CoreBluetooth central/peripheral delegates, separate filtered P006/P009 scans, characteristic validation, sequential acknowledged writes, P009 GET/read/ACK delivery, P008 verification/consent gating, and bounded deadlines.
- `GattIdentifiers.swift`, `GattResearchState.swift`: exact UUIDs and research flow states.
- `GattWireCodec.swift`: Foundation/CryptoKit-only P006 codec.
- `WatchAuthProtocol.swift`: isolated P-256/SHA-256 canonical transcripts, independent test enrollment verification, raw signatures and replay/expiry checks for P008.
- `WatchAuthTransport.swift`: P009-only signed-envelope framing, strict transfer assembly, owner/session checks, explicit completion and GET/read/ACK cursor used by the CoreBluetooth manager.
- `P009ResearchEnrollment.swift`: strict test-only host pin validation, public Watch enrollment export schema and SHA-256 fingerprint helpers.
- `AuthenticatedResearchView.swift`: explicit test enrollment, dual fingerprint confirmation, host selection, cancellation and cleanup UI.
- `ResearchConsentView.swift`: explicit foreground approve/reject view for an already verified P008 request. Approval checks LocalAuthentication at runtime and is never presented automatically.
- `Tests/OpenContinuityWatchTests/`: deterministic codec tests and shared JSON vector.

## Build setup

The portable package manifest at this directory root exposes `OpenContinuityProtocol`. Run `swift test --package-path experiments/watchos` on a machine with Swift 5.9+ and CryptoKit. Generate the watchOS app with XcodeGen 2.46.0: `xcodegen generate --spec experiments/watchos/project.yml`. The specification sets target membership, watchOS 10 minimum, the local protocol package, SwiftUI entry point, CoreBluetooth, LocalAuthentication and the Bluetooth usage description. Build with `xcodebuild -project experiments/watchos/OpenContinuityWatch.xcodeproj -scheme OpenContinuityWatch -configuration Debug -destination 'generic/platform=watchOS' CODE_SIGNING_ALLOWED=NO build`. Open that generated project and enable a development team/signing for physical installation. Set `NSBluetoothAlwaysUsageDescription` to explain the explicit research peripheral connection, select a development team, then build for `generic/platform=watchOS` or a paired Watch destination. The Mac must have an Xcode version with a matching watchOS SDK. No project metadata or build result is fabricated here.

## Protocol boundary

At most 20 bytes are written per frame, including its 15-byte header. The client waits for each `.withResponse` completion. It accepts only complete 25-byte challenge and 44-byte success or 11-byte error values. CoreBluetooth long-read delivery is UNKNOWN until verified on an actual Watch SE 2 and Linux BlueZ host. Partial values fail closed.

A successful digest proves only that the experimental server returned a hash of the submitted synthetic bytes. It does not identify the host or Watch, prove proximity or wearer presence, or authorize or unlock anything. No pairing, Apple-private API, Apple Account material, PAM, polkit, or login-manager code is used.

## P008 signed approval research

`WatchAuthProtocol.swift` uses CryptoKit P-256 signatures over the shared `OC-WATCH-AUTH-RESEARCH/1` canonical messages. Host keys must come from an independently pinned test enrollment before `VerifiedHostRequest` can be created. The P009 manager presents `ResearchConsentView` only after a user starts the authenticated flow and the signed request verifies. It uses public `.deviceOwnerAuthentication` policy support checking, shows **Research confirmation — does not unlock Linux.**, and produces no signature on cancellation, denial, unsupported policy or expiry. The ephemeral Watch signing key stays in app memory for the active test and is cleared after terminal cleanup or app backgrounding. Only public key material is exported; no private key is embedded or persisted by the app. Real device behavior remains unverified.

## P009 transport research

`WatchAuthTransport.swift` mirrors the Rust 15-byte header: `OA`, version, 8-byte transfer ID, sequence, flags, and little-endian total length on frame zero. The envelope is canonical P008 message bytes plus the 64-byte raw detached signature. At ATT MTU 23, frame payload is five bytes and 1,088 bytes fit in 218 frames. Host delivery uses explicit GET, a small read, then ACK; CoreBluetooth reads never implicitly choose or advance an offset. The Watch manager waits for each response write with response before sending the next. Request transfer is bounded to 20 seconds, consent to 25 seconds, response delivery to 15 seconds, and each callback/inactivity interval to 8 seconds, all clamped by signed expiry. CI uses test-only enrollment and simulates a signed rejection; that software choice does not represent physical LocalAuthentication. See [P009 design](../../docs/research/P009_AUTH_TRANSPORT_DESIGN.md), [temporary enrollment](../../docs/research/P009A_TEMPORARY_ENROLLMENT.md), [security review](../../docs/research/P009A_SECURITY_REVIEW.md), and [validation](../../docs/P009A_VALIDATION.md).

## P007A CI and lifecycle

`CI / Swift tests and watchOS compile` runs independently of Linux Rust checks on `macos-15`. It inventories Swift/Xcode/SDKs, runs the CryptoKit protocol, transport and enrollment test suite, generates the project and compiles the Watch application without signing. Physical radio, simulator runtime and Watch installation are separate gates.

Start Scan is disabled and rejected in the manager while any connection is owned. Disconnect retains that peripheral until a terminal callback; suspension performs the same cleanup even after Completed/Cancelled. `GattLifecycle` is the tested pure ownership gate. Operations are serialized and characteristic identity is checked. Connection-specific delegate generations and central replacement reject old connection callbacks.
