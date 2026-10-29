# OpenContinuity Watch experiment

This is an experimental foreground watchOS BLE client for the non-authorizing P006 GATT transport, plus an isolated P008 signed approval research component. The GATT UI offers explicit scan, connect, synthetic trial, cancel and disconnect controls. The P008 consent view accepts only a separately verified request and is not connected to GATT v1. Neither path contains a Linux unlock action.

## Source layout

- `OpenContinuityWatchApp.swift`, `ContentView.swift`: SwiftUI entry point and interface.
- `WatchBluetoothManager.swift`: retained CoreBluetooth central/peripheral delegates, explicit filtered scan, characteristic validation, sequential acknowledged writes and bounded deadlines.
- `GattIdentifiers.swift`, `GattResearchState.swift`: exact UUIDs and research flow states.
- `GattWireCodec.swift`: Foundation/CryptoKit-only P006 codec.
- `WatchAuthProtocol.swift`: isolated P-256/SHA-256 canonical transcripts, independent test enrollment verification, raw signatures and replay/expiry checks for P008.
- `ResearchConsentView.swift`: explicit foreground approve/reject view for an already verified P008 request. Approval checks LocalAuthentication at runtime; it is not presented automatically and does not store its in-memory research signing key.
- `Tests/OpenContinuityWatchTests/`: deterministic codec tests and shared JSON vector.

## Build setup

The portable package manifest at this directory root exposes `OpenContinuityProtocol`. Run `swift test --package-path experiments/watchos` on a machine with Swift 5.9+ and CryptoKit. Generate the watchOS app with XcodeGen 2.46.0: `xcodegen generate --spec experiments/watchos/project.yml`. The specification sets target membership, watchOS 10 minimum, the local protocol package, SwiftUI entry point, CoreBluetooth, LocalAuthentication and the Bluetooth usage description. Build with `xcodebuild -project experiments/watchos/OpenContinuityWatch.xcodeproj -scheme OpenContinuityWatch -configuration Debug -destination 'generic/platform=watchOS' CODE_SIGNING_ALLOWED=NO build`. Open that generated project and enable a development team/signing for physical installation. Set `NSBluetoothAlwaysUsageDescription` to explain the explicit research peripheral connection, select a development team, then build for `generic/platform=watchOS` or a paired Watch destination. The Mac must have an Xcode version with a matching watchOS SDK. No project metadata or build result is fabricated here.

## Protocol boundary

At most 20 bytes are written per frame, including its 15-byte header. The client waits for each `.withResponse` completion. It accepts only complete 25-byte challenge and 44-byte success or 11-byte error values. CoreBluetooth long-read delivery is UNKNOWN until verified on an actual Watch SE 2 and Linux BlueZ host. Partial values fail closed.

A successful digest proves only that the experimental server returned a hash of the submitted synthetic bytes. It does not identify the host or Watch, prove proximity or wearer presence, or authorize or unlock anything. No pairing, Apple-private API, Apple Account material, PAM, polkit, or login-manager code is used.

## P008 signed approval research

`WatchAuthProtocol.swift` uses CryptoKit P-256 signatures over the shared `OC-WATCH-AUTH-RESEARCH/1` canonical messages. Host keys must come from an independently pinned test enrollment before `VerifiedHostRequest` can be created. `ResearchConsentView` is an app component for a parent that presents it only after a user starts a test and verifies the signed request. It uses public `.deviceOwnerAuthentication` policy support checking, shows **Research confirmation — does not unlock Linux.**, and produces no signature on cancellation, denial, unsupported policy or expiry. The test signing key is supplied in memory and is not persisted. This component is not connected to the P006/P007 GATT v1 transport. See the P008 protocol and consent research documents; real device behavior remains unverified.

## P007A CI and lifecycle

`CI / Swift tests and watchOS compile` runs independently of Linux Rust checks on `macos-15`. It inventories Swift/Xcode/SDKs, runs the 9 package tests with CryptoKit, generates the project and compiles the Watch application without signing. Initial qualification passed with Xcode 16.4 / watchOS 11.5 / Swift 6.1.2. Physical radio, simulator runtime and Watch installation are separate gates.

Start Scan is disabled and rejected in the manager while any connection is owned. Disconnect retains that peripheral until a terminal callback; suspension performs the same cleanup even after Completed/Cancelled. `GattLifecycle` is the tested pure ownership gate. Operations are serialized and characteristic identity is checked. Connection-specific delegate generations and central replacement reject old connection callbacks.
