# Feature Matrix

States describe the P009 provisional branch build. `unsupported` means no implementation is present; `unknown` means the implementation has not established the fact. Research status is separate from protocol support.

| Area | State | Evidence / scope |
| --- | --- | --- |
| Rust portable capability and trust contracts | supported | Implemented and unit-tested in `opencontinuity-core`; this is a project contract, not Apple protocol support. |
| CLI help, version, doctor | supported | Implemented and tested; doctor remains static and performs no host scan. |
| Read-only protocol research fixture report | supported | `opencontinuity research watch --fixture <path> [--json]` validates sanitized offline metadata through a deterministic mock. No Bluetooth access, pairing, authentication, or authorization. |
| Watch endpoint research simulation | supported | `opencontinuity research watch-endpoint --fixture <path> [--json]` runs deterministic Ed25519 mock challenge-response with bounded/versioned messages. Simulation only; report says so and authorization is always `never_evaluated`. |
| BlueZ capability discovery | experimental | `opencontinuity research watch-ble --capabilities [--json]` remains read-only. Runtime manager interface absence is unsupported, D-Bus denial is blocked, missing evidence is unknown, and controller radio capability is not tested. |
| Experimental Linux BlueZ GATT transport | experimental | Opt-in, 1–300 second CLI server exports real zbus object interfaces, registers an application and advertisement, handles bounded synthetic exchange, and cleans up owned registrations. Compiled and unit/mock tested; no live BlueZ registration or radio validation here. |
| P006 session/host qualification | experimental | Bounded per-client synthetic sessions, private D-Bus cleanup tests and read-only inventory; actual controller and Watch interoperability NOT RUN. |
| Production Bluetooth adapter | unsupported | P001 adapter scaffolding remains incomplete; this isolated research CLI server does not complete it. |
| Mock enrollment verification | supported | Separate in-memory `MockEnrollmentRecord` binds host/account/Watch identity and algorithm to the expected key. Endpoint key substitution, unknown/revoked enrollment and binding/algorithm mismatch fail closed in Rust tests. Not production enrollment/storage. |
| P008 signed Watch approval research protocol | experimental | Isolated `OC-WATCH-AUTH-RESEARCH/1` transcript with P-256/SHA-256, strict canonical encoding, role separation, bounded timestamps and test-only independent enrollment pins. Rust unit tests pass; hosted Rust/Swift signature interchange is pending CI. No Linux authorization. |
| P008 fixture verifier CLI | experimental | `opencontinuity research watch-auth --fixture <path> [--json]` verifies signed test-only request and response fixtures with no system side effects. Every report says `authorization: never_evaluated`. |
| Swift P008 CryptoKit codec and signing | experimental source | `WatchAuthProtocol.swift` matches the Rust transcript and shared public-key/signature formats; Rust-signed request fixture is tested by Swift and CryptoKit-generated response is sent to Rust in hosted CI. Local Apple toolchain is unavailable in this environment. |
| Foreground P008 consent component | experimental source | `ResearchConsentView` requires a verified request and explicit choice; approval checks public `.deviceOwnerAuthentication` support and fails closed. It is not connected to GATT v1. Apple compilation and SE 2 LocalAuthentication behavior remain pending/unverified. |
| P009 signed research transport codec | experimental | `watch_auth_transport.rs` and `WatchAuthTransport.swift` implement separate versioned framing, bounded 1,088-byte envelopes, strict sequencing/owner/session checks, explicit completion and GET/read/ACK request delivery. Portable Rust tests pass locally; Swift and hosted integration await Apple CI. |
| P009 Rust → Swift → Rust serialized exchange | experimental source | Workflow consumes real serialized Rust request frames in Swift, verifies P008 with CryptoKit, signs a simulated denial and returns response frames for Rust reassembly/P008 verification. This is test-fixture integration, not a live Bluetooth exchange or physical consent. |
| P009 BlueZ/CoreBluetooth runtime transport | experimental source | The Linux CLI registers a distinct bounded P009 service and verifies a fresh signed response; the Watch manager implements explicit GET/read/ACK, verified-request gating, consent presentation and sequential response writes. Apple SDK compile, live D-Bus operation and RF remain NOT RUN. |
| P009 live signed approval | blocked pending hardware validation | P009A includes manual test-only enrollment and in-memory ephemeral Watch key handling. Apple SDK CI and physical BlueZ/Watch/LocalAuthentication evidence are separate gates. No private test key is shipped. |
| watchOS foreground research client | experimental source | SwiftUI/CoreBluetooth app and pure wire codec source exist under `experiments/watchos/`; no Apple compilation, signing, simulator, or physical Watch test is claimed. |
| Foreground daemon lifecycle | supported | Safe idle process; nonprivileged and no integrations. |
| Apple Watch SE 2 Auto Unlock, native protocol | unsupported | P002 research found public third-party enrollment/STS support blocked. Trust stages are unknown; no OS unlock behavior. |
| AirDrop via native AWDL | unsupported | No AWDL or AirDrop implementation. |
| AirPods Pro 3 AAP/AACP and Settings | unsupported | No device protocol or UI implementation. |
| AirPods automatic switching | unsupported | Not implemented. |
| AirPlay, HomeKit, iCloud, remaining Continuity | unsupported | Not implemented. |
| Foreground Watch BLE discovery | experimental source | Scan is explicit and filtered to the P006 service; no real peripheral was available for validation. |
| Watch direct BLE endpoint | blocked / unknown | Apple documents Watch central-to-peripheral communication and explicitly blocks Watch `CBPeripheralManager` advertising. Foreground central is a feasible test candidate; background/hands-free listener remains blocked. No physical test. See P003 matrices. |
| Watch–iPhone relay endpoint | experimental | WatchConnectivity supports paired-app messaging with live reachability requirements and queued, non-immediate background delivery. No relay app or Linux transport implemented; phone never substitutes for Watch identity. |
| Foreground Watch confirmation | experimental | Public `.deviceOwnerAuthentication` is listed for watchOS; P008 checks runtime support after explicit user action. No Apple build or SE 2 test; no third-party biometric or Touch ID claim. |
| Apple Watch Auto Unlock feasibility | blocked / unknown | Native Linux interoperation remains blocked on a public third-party enrollment/STS endpoint; undocumented feasibility and SE 2-specific behavior remain unknown. P008 is a separate custom research protocol, not native Auto Unlock. |
| Production device identity, secure key custody, persistent replay state, proximity, readiness | unknown | P008 only demonstrates software signatures under test-only pins. Genuine Watch identity, hardware-backed custody, durable replay state and proximity are not established. |
| Qt 6/QML Settings UI | unsupported | P001 Settings framework remains absent; a GATT research command does not complete the UI. |

Native interoperability, research simulation, experimental feasibility claims, and unimplemented plans are separate claims. No native Apple protocol or production authentication feature is implemented.
