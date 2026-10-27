# Feature Matrix

States describe this P002 branch build. `unsupported` means no implementation is present; `unknown` means the implementation has not established the fact. Research status is separate from protocol support.

| Area | State | Evidence / scope |
| --- | --- | --- |
| Rust portable capability and trust contracts | supported | Implemented and unit-tested in `opencontinuity-core`; this is a project contract, not Apple protocol support. |
| CLI help, version, doctor | supported | Implemented and tested; doctor remains static and performs no host scan. |
| Read-only protocol research fixture report | supported | `opencontinuity research watch --fixture <path> [--json]` validates sanitized offline metadata through a deterministic mock. No Bluetooth access, pairing, authentication, or authorization. |
| Native Bluetooth/BlueZ adapter | unsupported | No platform adapter; the research `DiscoveryAdapter` is only an interface with a mock implementation. |
| Foreground daemon lifecycle | supported | Safe idle process; nonprivileged and no integrations. |
| Apple Watch SE 2 Auto Unlock, native protocol | unsupported | Research/implementation not present. Trust stages are unknown; no OS unlock behavior. |
| AirDrop via native AWDL | unsupported | No AWDL or AirDrop implementation. |
| AirPods Pro 3 AAP/AACP and Settings | unsupported | No device protocol or UI implementation. |
| AirPods automatic switching | unsupported | Not implemented. |
| AirPlay, HomeKit, iCloud, remaining Continuity | unsupported | Not implemented. |
| Device detection / nearby Bluetooth devices | unknown | No discovery or BLE scanning is performed by this branch. Fixture observations are synthetic/local input. |
| Apple Watch Auto Unlock feasibility | blocked / unknown | Native Linux interoperation is blocked on a documented third-party enrollment/STS endpoint; undocumented feasibility and SE 2 specific behavior remain unknown. See P002 research decision. |
| Device identity, cryptographic authentication, freshness, proximity, readiness | unknown | No trust protocol or assessment exists. Nearby signals never imply authorization. |
| Qt 6/QML Settings UI | unsupported | Planned; not present in P001 or this branch. |

Native interoperability, experimental support, and unimplemented plans are separate claims. No feature is marked experimental because P001 contains no protocol prototype.
