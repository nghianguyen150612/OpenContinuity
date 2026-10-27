# P002 — Apple Watch Auto Unlock protocol research

Status: RESEARCH / NOT IMPLEMENTED
Base: main at 5763b7adc8d4099bcfb72a17ff48cdac7cf60904

## Objective
Assess whether an Apple Watch SE 2 can participate in a secure Linux desktop unlock flow without claiming compatibility with Apple's proprietary Auto Unlock protocol. AirDrop and AirPods Pro 3 are out of scope.

## Trust boundaries
- BLE advertisement, RSSI, watch proximity and device name are discovery signals, never authorization.
- An authenticated enrollment binding must exist before any unlock decision.
- Challenge/response must be fresh, replay-resistant and bound to the host, enrolled watch and session.
- Device possession, user intent and OS login authorization are distinct decisions.
- Failure, timeout, ambiguous identity, disconnected device or unsupported crypto must deny unlock.
- No PAM, polkit, login manager or privileged daemon integration during this research milestone.

## Research questions
1. Which protocol exchanges can be observed legally on owned devices, and which require Apple-private keys, entitlements or hardware?
2. Can the Watch SE 2 authenticate a third-party Linux host, or is the pairing trust restricted to Apple's ecosystem?
3. What cryptographic evidence would prove peer identity, freshness, user intent and resistance to relay?
4. Which APIs are available on Linux for BLE discovery and capture, and what are their limits?
5. What minimum evidence would justify a later opt-in, privilege-separated unlock prototype?

## Evidence protocol
Record hardware/OS versions, capture method, timestamp, reproducible commands, sanitized packet metadata, confidence level and source licensing for each observation. Never collect secrets or publish private pairing material. Separate observed facts, source claims and hypotheses.

## Acceptance gates
- Protocol capability matrix with evidence references and explicit UNKNOWN/BLOCKED states.
- Threat analysis covering replay, relay, spoofing, downgrade and unattended unlock.
- Read-only reproducible capture plan; no changes to authentication.
- Go/no-go decision for native Apple Auto Unlock interoperability vs a separately enrolled open protocol.
- CI and review on the P002 PR; no automatic merge.

## Carry-over from P001 audit
The original planning scope mentioned Bluetooth adapter scaffolding and Qt6/QML Settings framework. PR #1 merged the Rust foundation but did not demonstrate those components. Track these as unresolved P001 scope reconciliation items, not silently as completed P002 functionality.
