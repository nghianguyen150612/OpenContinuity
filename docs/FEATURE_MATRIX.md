# Feature Matrix

States describe this P004 branch build. `unsupported` means no implementation is present; `unknown` means the implementation has not established the fact. Research status is separate from protocol support.

| Area | State | Evidence / scope |
| --- | --- | --- |
| Rust portable capability and trust contracts | supported | Implemented and unit-tested in `opencontinuity-core`; this is a project contract, not Apple protocol support. |
| CLI help, version, doctor | supported | Implemented and tested; doctor remains static and performs no host scan. |
| Read-only protocol research fixture report | supported | `opencontinuity research watch --fixture <path> [--json]` validates sanitized offline metadata through a deterministic mock. No Bluetooth access, pairing, authentication, or authorization. |
| Watch endpoint research simulation | supported | `opencontinuity research watch-endpoint --fixture <path> [--json]` runs deterministic Ed25519 mock challenge-response with bounded/versioned messages. Simulation only; report says so and authorization is always `never_evaluated`. |
| BlueZ capability discovery | experimental | `opencontinuity research watch-ble --capabilities [--json]` read-only inspects BlueZ D-Bus object/interface availability when the system bus and BlueZ exist. Does not register, advertise, pair, or connect. |
| Native Bluetooth/BlueZ GATT peripheral | unsupported | No GATT object server or advertisement registration implementation; no controller present/validated in this environment. |
| Mock enrollment verification | supported | Separate in-memory `MockEnrollmentRecord` binds host/account/Watch identity and algorithm to the expected key. Endpoint key substitution, unknown/revoked enrollment and binding/algorithm mismatch fail closed in Rust tests. Not production enrollment/storage. |
| watchOS companion app | blocked | No Apple SDK/Xcode/signing target in this Linux environment; no app source is claimed buildable and no Apple compilation or device run occurred. |
| Foreground daemon lifecycle | supported | Safe idle process; nonprivileged and no integrations. |
| Apple Watch SE 2 Auto Unlock, native protocol | unsupported | P002 research found public third-party enrollment/STS support blocked. Trust stages are unknown; no OS unlock behavior. |
| AirDrop via native AWDL | unsupported | No AWDL or AirDrop implementation. |
| AirPods Pro 3 AAP/AACP and Settings | unsupported | No device protocol or UI implementation. |
| AirPods automatic switching | unsupported | Not implemented. |
| AirPlay, HomeKit, iCloud, remaining Continuity | unsupported | Not implemented. |
| Device detection / nearby Bluetooth devices | unknown | No discovery or BLE scanning is performed by this branch. Fixture observations are synthetic/local input. |
| Watch direct BLE endpoint | blocked / unknown | Apple documents Watch central-to-peripheral communication and explicitly blocks Watch `CBPeripheralManager` advertising. Foreground central is a feasible test candidate; background/hands-free listener remains blocked. No physical test. See P003 matrices. |
| Watch–iPhone relay endpoint | experimental | WatchConnectivity supports paired-app messaging with live reachability requirements and queued, non-immediate background delivery. No relay app or Linux transport implemented; phone never substitutes for Watch identity. |
| Foreground Watch confirmation | experimental | Public Local Authentication wrist-detection policy (watchOS 9+) can accompany explicit foreground action. No Apple build or SE 2 test; no third-party biometric or Touch ID claim. |
| Apple Watch Auto Unlock feasibility | blocked / unknown | Native Linux interoperation remains blocked on a public third-party enrollment/STS endpoint; undocumented feasibility and SE 2-specific behavior remain unknown. A separate foreground confirmation protocol is not native Auto Unlock and remains unimplemented. |
| Device identity, cryptographic authentication, freshness, proximity, readiness | unknown | No trust protocol or assessment exists. Nearby signals never imply authorization. |
| Qt 6/QML Settings UI | unsupported | Planned; not present in P001 or this branch. |

Native interoperability, research simulation, experimental feasibility claims, and unimplemented plans are separate claims. No Apple communication or production authentication feature is implemented.
