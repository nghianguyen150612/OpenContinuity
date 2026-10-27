# P003 — Watch-side endpoint feasibility and transport boundaries

**Status:** IN PROGRESS — initial research scope only; no watchOS app or unlock implementation.
**Base:** `main` at `365444eea4a776b92dc2dfa3315275929e09ef63` (P002 squash merge).
**Target:** Apple Watch SE (2nd generation), paired iPhone, and a Linux OpenContinuity host.

> Roadmap reconciliation: The original full P001–P060 planning document is not available in the checked-in repository. Earlier planning grouped P001–P006 as foundation/settings and P007–P018 as Watch work. Accordingly, this P003 Watch-specific topic is a **provisional continuation of the implemented P002 research branch**, not a verified transcription of the original P003 title or acceptance criteria. Reconcile before declaring the milestone complete. P001's BlueZ-adapter scaffolding and Qt6/QML Settings gaps remain open.

## Objective

Determine whether **public, supported third-party watchOS APIs** can expose a cryptographically authenticated, opt-in endpoint suitable for a *separately enrolled* Linux companion. Determine whether hands-free interaction is technically achievable in foreground and in typical watchOS background execution states.

This is **not** a reproduction of Apple's private Mac Auto Unlock protocol. P002's native Apple STS interoperability decision remains NO-GO for documented public interfaces; unknown proprietary interoperability must not be described as implemented.

## Paths to evaluate independently

1. **Watch ↔ Linux direct:** Can a third-party watchOS app initiate or accept BLE or network traffic with a Linux host using documented public APIs? Verify advertising/peripheral role vs central role, reachable state, permission prompts, session lifetimes, background behavior and OS-version support. No assumption of a persistent GATT server.
2. **Watch ↔ paired iPhone ↔ Linux relay:** A companion iOS app can use Apple's `WatchConnectivity` framework to exchange data with its own paired watchOS app, but this framework does not itself describe a direct Linux peer. Research a separate authenticated iOS-to-Linux transport and whether latency/background delivery supports the intended UX.
3. **Explicit watch-app confirmation:** Investigate a user-initiated Watch action as a fallback when background reachability is insufficient. Label this as a different interaction model from Apple's automatic unlock.
4. **No viable Watch endpoint:** Record a defensible NO-GO or UNKNOWN if public APIs or actual tests cannot establish a secure and usable endpoint. Do not relabel iPhone proximity as Watch authentication.

## Verified documentation leads (to validate against targeted OS versions)

- Apple **Watch Connectivity**: two-way data transfer between an iOS app and its paired watchOS app; live communication is conditional on both endpoints' runtime state. https://developer.apple.com/documentation/watchconnectivity
- Apple `WCSession.isReachable`: immediate messaging requires a reachable active counterpart; an active foreground Watch app or certain high-priority background states are relevant. https://developer.apple.com/documentation/watchconnectivity/wcsession/isreachable
- Apple **watchOS background tasks**: execution and Bluetooth work are constrained; Apple's documentation describes a limited background Bluetooth scan/alert budget. Verify the exact behavior on the target watchOS and do not design an always-listening service from simulator-only evidence. https://developer.apple.com/documentation/watchkit/using-background-tasks
- Apple **watchOS background content**: background wakeups are limited and not guaranteed. https://developer.apple.com/documentation/watchos-apps/keeping-your-watchos-app-s-content-up-to-date
- Apple **Core Bluetooth** framework: research public APIs and watchOS platform-specific availability rather than assuming iOS API parity. https://developer.apple.com/documentation/corebluetooth

These references support **research questions**, not evidence that a Watch SE 2 can act as a 24/7 Linux authenticator.

## Security requirements

- Discovery, app reachability, Watch-to-iPhone pairing and Apple Account membership are **not** Linux unlock authorization.
- Require explicit local account/host enrollment and revocation; document device ownership, user consent and recovery.
- Define fresh challenge–response and bind it to protocol version, host identity, account, session, action and expiry; reject replay/downgrade.
- Analyze a compromised iPhone relay, stolen Watch, unlocked Watch without wearer intent, BLE relays and network MITM.
- Never use BLE RSSI, manufacturer data, IRK, app notification or `WCSession.isReachable` as a sufficient unlock signal.
- Verify whether user-presence/authentication APIs provide the desired assurances; do not assume hardware-backed attestation or Watch biometric capability.
- Fail closed on suspended endpoints, late background delivery, untrusted relays, stale challenges, key failures and ambiguous sessions.
- **No PAM/polkit/login-manager integration, root service, automatic pairing or real unlock** in this milestone.

## Scoped deliverables

1. A capability matrix comparing direct Watch–Linux, Watch–iPhone–Linux, and explicit user-confirmation paths, with documented/verified/unknown/blocked labels.
2. An API-availability matrix for the minimum supported watchOS/iOS version; availability must be checked from SDK docs or builds, not guessed.
3. A threat model for endpoint enrollment, key custody, compromised relays, freshness and user presence.
4. A reproducible hardware test plan for an owned Watch SE 2, paired iPhone and Linux host, including foreground/background/suspended modes and latency/failure measurements.
5. If tooling permits, an isolated, **non-authorizing** proof of concept and deterministic host-side mock tests, without changing production login policy. Do not claim Watch hardware validation from mocks or simulator-only runs.
6. A written go/no-go decision about **each** transport and interaction model, listing blockers and what exact evidence would unblock them.
7. Review/CI evidence and a PR against `main`; do not merge automatically.

## Acceptance gates

- [ ] Public APIs and their OS-version/background constraints are evidenced with URLs and dates.
- [ ] At least direct and iPhone-relay alternatives receive an independent feasibility assessment.
- [ ] Cryptographic trust, freshness, user presence and relay threats are explicitly addressed.
- [ ] No unauthenticated presence signal flows into authorization.
- [ ] Hardware claims reflect actual owned-device testing; otherwise marked NOT TESTED.
- [ ] Any changed Rust/Swift code passes applicable local checks and hosted CI, or blockers are stated plainly.
- [ ] Original P003 roadmap ownership and carried-over P001 work are reconciled when the authoritative source becomes accessible.
- [ ] Reviewer approves go/no-go and PR merge explicitly.

## Repository continuity

Preserve canonical project name **OpenContinuity**, CLI **`opencontinuity`**, daemon **`opencontinuityd`**, existing Rust 2024 workspace and P001/P002 doctor/research behavior. Defer AirDrop and AirPods Pro 3 until the current Watch research decision is sufficiently grounded.
