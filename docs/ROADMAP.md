# Roadmap

The requested planning source `OpenContinuity-Roadmap-P001-P060.md` was not present in the repository or accessible workspace during P001. This document records the P001 request's priorities and does not invent plans for missing milestones.

## P001 — Project Bootstrap & Protocol Baseline

Establish the Rust 2024 workspace, typed capability and trust states, safe CLI and foreground daemon, upstream evidence manifest, security and licensing policy, CI gates, and architecture docs. No real Apple protocol support or host integration is included.

## P002 — Apple Watch Auto Unlock research

Proposed next step: build an evidence-backed protocol research and threat model for Apple Watch SE 2. Identify message flows, trust provisioning, cryptographic authentication, freshness, replay resistance, proximity assurance and supported hardware limits before writing any unlock integration. Keep any prototype isolated and non-authorizing until secure design and independent tests exist. Do not modify system authentication policy.

## Priority order from the project brief

1. Apple Watch SE 2 native Auto Unlock protocol research.
2. Native AWDL/AirDrop interoperability with iPhone 13.
3. AirPods Pro 3 AAP/AACP, complete Settings and switching.
4. Remaining Continuity, AirPlay, HomeKit and iCloud features.

Future platform order begins with Linux (reference: Arch Linux, Hyprland, Caelestia), then other Linux distributions, FreeBSD/Unix-like, and Windows. No schedule or completion claims are made for P002–P060 until the supplied planning source is available.
