# Roadmap

The requested planning source `OpenContinuity-Roadmap-P001-P060.md` was not present in the repository or accessible workspace during P001. This document records the P001 request's priorities and does not invent plans for missing milestones.

## P001 — Project Bootstrap & Protocol Baseline

Establish the Rust 2024 workspace, typed capability and trust states, safe CLI and foreground daemon, upstream evidence manifest, security and licensing policy, CI gates, and architecture docs. No real Apple protocol support or host integration is included.

## P002 — Apple Watch Auto Unlock research

Research outcome on branch `research/p002-watch-auto-unlock`: Apple's native Watch-to-Mac flow is documented as BLE rendezvous, Secure-Enclave-terminated mutually authenticated STS, rotating unlock secrets and peer-to-peer Wi-Fi distance approximation. Native Linux implementation is a NO-GO on the public interfaces reviewed; Apple does not document a third-party enrollment/STS endpoint. Undocumented native feasibility and Apple Watch SE 2-specific behavior remain UNKNOWN. See the P002 research documents for evidence and limitations. P002 adds offline fixture research only; it does not modify system authentication policy.

## P001 scope reconciliation

The P001 files in this repository establish a Rust workspace, portable trust contracts, CLI, idle daemon, CI, and documentation. They do not include a real Bluetooth/BlueZ adapter scaffold or Qt 6/QML Settings framework. The P002 mock research trait is not the missing platform adapter. These items remain unresolved against the unavailable P001–P060 source roadmap; no acceptance criteria are inferred for them.

## P003 — Watch endpoint feasibility (provisional)

The authoritative P001–P060 roadmap remains unavailable, so this P003 entry is a provisional continuation of P002. Public API review found a foreground direct BLE topology (Linux GATT peripheral, Watch CoreBluetooth central), but no third-party Watch peripheral advertising, no guaranteed always-running listener, and no physical SE 2 validation. WatchConnectivity can relay paired-app messages but does not provide a delivery deadline or user-presence proof. A foreground explicit Watch approval with Local Authentication wrist/passcode policy is plausible but awaits Apple SDK/device validation. The portable CLI mock exercises strict signed challenge-response and fail-closed cases only; it is neither Apple communication nor unlock. See P003 matrices/security/hardware plan. No PAM/root/session-unlock integration is in scope.

## P004 — Foreground Watch ↔ Linux BLE GATT feasibility (provisional)

The P004 working scope continues the available P003 branch and has not been reconciled with the missing P001–P060 planning source. It adds a separate mock enrollment trust record, signer-substitution tests, and read-only BlueZ D-Bus capability inspection via `opencontinuity research watch-ble --capabilities [--json]`. It does not register a GATT application or advertise because the implementation environment has no live system bus, BlueZ daemon, adapter or controller. Apple SDK, signing, Watch and iPhone are unavailable; no Swift target or hardware result is claimed. Ed25519 remains test-only; a P-256/SHA-256 interop shape is documented but not implemented. Host-signed challenge verification and production enrollment storage do not exist. Reports always retain `authorization: never_evaluated`. See P004 validation and research records.

The original Qt6/QML Settings framework and production Bluetooth adapter scaffolding remain absent and unresolved. P004's read-only capability probe is not either deliverable.

## Priority order from the project brief

1. Apple Watch SE 2 native Auto Unlock protocol research.
2. Native AWDL/AirDrop interoperability with iPhone 13.
3. AirPods Pro 3 AAP/AACP, complete Settings and switching.
4. Remaining Continuity, AirPlay, HomeKit and iCloud features.

Future platform order begins with Linux (reference: Arch Linux, Hyprland, Caelestia), then other Linux distributions, FreeBSD/Unix-like, and Windows. No schedule or completion claims are made for P002–P060 until the supplied planning source is available.
