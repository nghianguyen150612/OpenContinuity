# OpenContinuity Watch experiment

This is a source-only foreground watchOS BLE client for the non-authorizing P006 GATT transport. The UI offers explicit scan, connect, synthetic trial, cancel and disconnect controls. It contains no Linux unlock action.

## Source layout

- `OpenContinuityWatchApp.swift`, `ContentView.swift`: SwiftUI entry point and interface.
- `WatchBluetoothManager.swift`: retained CoreBluetooth central/peripheral delegates, explicit filtered scan, characteristic validation, sequential acknowledged writes and bounded deadlines.
- `GattIdentifiers.swift`, `GattResearchState.swift`: exact UUIDs and research flow states.
- `GattWireCodec.swift`: Foundation/CryptoKit-only P006 codec.
- `Tests/OpenContinuityWatchTests/`: deterministic codec tests and shared JSON vector.

## Build setup

The portable package manifest at this directory root exposes `OpenContinuityProtocol`. Run `swift test --package-path experiments/watchos` on a machine with Swift 5.9+ and CryptoKit. To create the watch app target, use Xcode to create a watchOS App target (minimum watchOS 10), add the five app source files above, add this local package as a dependency, and set `OpenContinuityWatchApp` as the app entry point. Set `NSBluetoothAlwaysUsageDescription` to explain the explicit research peripheral connection, select a development team, then build for `generic/platform=watchOS` or a paired Watch destination. The Mac must have an Xcode version with a matching watchOS SDK. No project metadata or build result is fabricated here.

## Protocol boundary

At most 20 bytes are written per frame, including its 15-byte header. The client waits for each `.withResponse` completion. It accepts only complete 25-byte challenge and 44-byte success or 11-byte error values. CoreBluetooth long-read delivery is UNKNOWN until verified on an actual Watch SE 2 and Linux BlueZ host. Partial values fail closed.

A successful digest proves only that the experimental server returned a hash of the submitted synthetic bytes. It does not identify the host or Watch, prove proximity or wearer presence, or authorize or unlock anything. No pairing, Apple-private API, Apple Account material, PAM, polkit, or login-manager code is used.
