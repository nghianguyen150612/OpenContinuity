# P004 watchOS app feasibility

**Status:** public API feasibility is **DOCUMENTED**; app compilation and device behavior are **BLOCKED / NOT RUN**. No Swift source is represented as buildable in this checkout.

## Role and app design

The candidate architecture is a foreground watchOS app using `CBCentralManager` to scan for a configured Linux peripheral, show the user a host identity for confirmation, connect to a selected GATT service, receive a bounded versioned challenge, then offer explicit Approve and Deny actions. The app would sign only after user action and return a bounded response. Manual disconnect and visible timeout/error states are required.

Apple documents Core Bluetooth central manager and GATT client APIs for watchOS. Peripheral advertising with `CBPeripheralManager` is not the Watch role selected for this design. A foreground central API is not evidence for continuous discovery, background execution, or wake-on-request. The user must launch and foreground the app for the proposed experiment; background, suspension, screen-off, locked, and off-wrist cases need independent physical testing.

References:

- [Core Bluetooth](https://developer.apple.com/documentation/corebluetooth) — **DOCUMENTED**.
- [`CBCentralManager`](https://developer.apple.com/documentation/corebluetooth/cbcentralmanager) — **DOCUMENTED** central role.
- [`CBPeripheral`](https://developer.apple.com/documentation/corebluetooth/cbperipheral) and [Core Bluetooth background processing](https://developer.apple.com/documentation/corebluetooth) — **DOCUMENTED** APIs with lifecycle/background constraints; not an always-on listener guarantee.
- [`CBPeripheralManager`](https://developer.apple.com/documentation/corebluetooth/cbperipheralmanager) — peripheral role is not assumed for Apple Watch.
- [LocalAuthentication](https://developer.apple.com/documentation/localauthentication) and [`deviceOwnerAuthenticationWithWristDetection`](https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthenticationwithwristdetection) — **DOCUMENTED** API surface. Availability and actual SE 2 behavior must be confirmed against the selected SDK and hardware.

## Apple tooling and hardware results

| Check | Status | Evidence |
| --- | --- | --- |
| Xcode / `xcodebuild` | **BLOCKED** | `xcodebuild` is not installed in this Linux environment. |
| watchOS SDK | **BLOCKED** | No Apple SDK is present. |
| Swift compiler | **BLOCKED** | No `swift` executable is present. |
| Apple code signing | **NOT RUN** | No Apple development identity or provisioning environment is available here. |
| Apple Watch SE 2 build/runtime | **NOT RUN** | No Watch or paired iPhone is attached. |
| Foreground scan/connect/GATT | **NOT RUN** | No app or Watch hardware. |
| Background/screen-off/locked/off-wrist behavior | **NOT RUN** | No app or Watch hardware. |

No Swift app target was added because it could not be checked against an installed watchOS SDK. Exact Apple-side prerequisites are a Mac capable of running a supported Xcode, the matching watchOS SDK, an Apple Developer signing setup for installing on the owned Watch SE 2, a paired iPhone, and the physical Watch. Record Xcode/macOS/SDK versions, Watch model identifier, watchOS build, iPhone/iOS version, app signing identity and build number for each test. Simulator success would not establish Bluetooth radio, Secure Enclave, wrist policy, or lifecycle behavior on a physical Watch.

## Proposed key API compatibility

CryptoKit documents P-256 ECDSA signing and Secure Enclave-backed P-256 key types. **DOCUMENTED:** [`SecureEnclave.P256.Signing.PrivateKey`](https://developer.apple.com/documentation/cryptokit/secureenclave/p256/signing/privatekey), [`P256.Signing.PrivateKey`](https://developer.apple.com/documentation/cryptokit/p256/signing/privatekey), and [`P256.Signing.ECDSASignature`](https://developer.apple.com/documentation/cryptokit/p256/signing/ecdsasignature). The target Watch SE 2's exact key creation, persistence, access-control and policy behavior remains **UNKNOWN** until built and run on hardware.

If the Watch experiment proceeds, use one explicit cross-language contract: ECDSA over P-256 with SHA-256; 65-byte uncompressed SEC1 public key; 64-byte fixed-width `r || s` signature from CryptoKit's raw representation; Rust verifies using the audited `p256` crate's ECDSA verifier after validating the SEC1 point. Do not pass DER in one direction and raw representation in the other. This P004 Rust mock remains Ed25519 and does not implement that P-256 contract.

No background or hands-free behavior, wearer proof, distance assurance, native Apple Auto Unlock, or Linux authorization is claimed.
