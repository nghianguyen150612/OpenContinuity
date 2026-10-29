# P007 watchOS foreground client

`authorization: never_evaluated`

## Implementation status

P007 adds source for a native SwiftUI/CoreBluetooth foreground client under `experiments/watchos/`. The screen offers explicit scan, peripheral selection, connection, synthetic test, cancellation and disconnect actions. BLE code retains the central/peripheral delegates, scans only for the P006 service, validates the five UUIDs and their properties, reads version/challenge, serializes `.withResponse` writes at no more than 20 bytes, and verifies a result through the pure `GattWireCodec` module.

The manager uses a tested generation-token gate, expected characteristic, serialized write state and bounded Task deadlines. Every disconnect, timeout and failed operation clears the challenge, session ID, queued frames and response expectation. A foreground lifecycle loss requests disconnect and requires a new scan/challenge. The UI describes only synthetic transport; it contains no approval or unlock action.

The client expects the 25-byte challenge and full 44-byte/11-byte result in one CoreBluetooth characteristic value. It rejects other lengths. CoreBluetooth exposes no caller-selected read offset here. Full long-read behavior remains **UNKNOWN** until checked on Apple Watch SE 2 with the Linux BlueZ implementation. The client does not combine unrelated callbacks or claim that a simulator proves full long reads.

The source requires a watchOS 10 or later target, Swift 5.9 or later, SwiftUI, CoreBluetooth, Foundation, CryptoKit, and the local Swift package product `OpenContinuityProtocol`. Add `NSBluetoothAlwaysUsageDescription` to the Watch app target Info settings (for example: “Connect to an explicitly selected OpenContinuity research GATT peripheral.”), enable the Watch app target/signing team, and use a paired Watch destination. Open `experiments/watchos/Package.swift` as a Swift package for protocol tests. For an Xcode app target, create a watchOS App target, add `OpenContinuityWatchApp.swift`, `ContentView.swift`, `WatchBluetoothManager.swift`, `GattIdentifiers.swift`, and `GattResearchState.swift`, add the local package dependency, and set the app entry point to `OpenContinuityWatchApp`. Build with `xcodebuild -scheme <WatchAppScheme> -destination 'generic/platform=watchOS' build`; run `swift test --package-path experiments/watchos` for package tests. Exact API availability and compilation are not claimed until those commands run with Xcode/watchOS SDK.

## Security and limits

This is non-authorizing synthetic transport. The challenge is not a Watch authentication nonce; the digest is not a signature, MAC, identity proof, wearer-presence proof, distance bound, or unlock approval. No pairing is triggered, no Apple private APIs or account material are used, and no Linux authorization integration is present. P003/P004 mock Ed25519 signing remains separate.

Compilation, simulator execution, Apple signing, Linux BlueZ hardware, RF advertising, and Apple Watch SE 2 trials are recorded in `docs/P007_VALIDATION.md` and the hardware plan.

## P007A CI coverage

Hosted `macos-15` CI now runs the production Swift package (9 XCTest cases: 4 lifecycle and 5 codec), including CryptoKit verification of the Rust-generated fixture, and an actual unsigned watchOS compile from `experiments/watchos/project.yml`. Initial evidence: [run 40805562115](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40805562115), Xcode 16.4 / watchOS SDK 11.5 / Swift 6.1.2, all checks PASS. This supersedes the earlier Apple-compilation/Swift-test NOT RUN statements above; physical hardware and simulator runtime remain NOT RUN. Long GATT reads on the Watch/BlueZ combination remain UNKNOWN. See P007_VALIDATION for final qualification details.

The manager denies new scans during owned connections, retains the peripheral through disconnect completion, clears synthetic data on every foreground loss, and uses explicit pending operation ownership. The pure lifecycle tests cover scan denial during active operations, background cleanup with and without pending work, disconnect during writes/result reads, old and duplicate tickets, same-peripheral reconnect and rapid repeated cycles. Fake lifecycle events do not establish CoreBluetooth behavior over RF.
