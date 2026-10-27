# Roadmap

The requested planning source `OpenContinuity-Roadmap-P001-P060.md` was not present in the repository or accessible workspace during P001. This document records the P001 request's priorities and does not invent plans for missing milestones.

## P001 — Project Bootstrap & Protocol Baseline

Establish the Rust 2024 workspace, typed capability and trust states, safe CLI and foreground daemon, upstream evidence manifest, security and licensing policy, CI gates, and architecture docs. No real Apple protocol support or host integration is included.

## P002 — Apple Watch Auto Unlock research

Research outcome on branch `research/p002-watch-auto-unlock`: Apple's native Watch-to-Mac flow is documented as BLE rendezvous, Secure-Enclave-terminated mutually authenticated STS, rotating unlock secrets and peer-to-peer Wi-Fi distance approximation. Native Linux implementation is a NO-GO on the public interfaces reviewed; Apple does not document a third-party enrollment/STS endpoint. Undocumented native feasibility and Apple Watch SE 2-specific behavior remain UNKNOWN. See the P002 research documents for evidence and limitations. P002 adds offline fixture research only; it does not modify system authentication policy.

## P001 scope reconciliation

The P001 files in this repository establish a Rust workspace, portable trust contracts, CLI, idle daemon, CI, and documentation. They do not include a real Bluetooth/BlueZ adapter scaffold or Qt 6/QML Settings framework. The P002 mock research trait is not the missing platform adapter. These items remain unresolved against the unavailable P001–P060 source roadmap; no acceptance criteria are inferred for them.

## Priority order from the project brief

1. Apple Watch SE 2 native Auto Unlock protocol research.
2. Native AWDL/AirDrop interoperability with iPhone 13.
3. AirPods Pro 3 AAP/AACP, complete Settings and switching.
4. Remaining Continuity, AirPlay, HomeKit and iCloud features.

Future platform order begins with Linux (reference: Arch Linux, Hyprland, Caelestia), then other Linux distributions, FreeBSD/Unix-like, and Windows. No schedule or completion claims are made for P002–P060 until the supplied planning source is available.
