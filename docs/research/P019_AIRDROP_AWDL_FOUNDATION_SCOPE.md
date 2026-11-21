# P019 — AirDrop/AWDL Foundation and Safe Receive Integration

P019 is a provisional milestone because the original P001–P060 roadmap source
is unavailable. It begins the AirDrop interoperability priority with the Linux
host as an AirDrop receiver and an iPhone 13 as the primary physical sender.
Arch Linux is the first target. The user's Realtek RTL8852BE is an unverified
test candidate; no current laptop configuration is inferred from historic
observations.

## Implemented in this branch

- `opencontinuity airdrop doctor [--json]` and `airdrop backend status
  [--json]` report a local, read-only backend and radio snapshot in schema v1.
  The detector does not start discovery, activate AWDL, register services,
  alter the radio or change firewall/NetworkManager state.
- The optional receiver path supervises an installed, exact-version Owly 0.4.2
  CLI at a fixed system path. It requires the operator to select the existing
  destination, declare that radio setup is already prepared, pass explicit
  consent, and use a terminal. It never invokes `owly radio up`, `--accept`, a
  shell, a package manager or an arbitrary executable from `PATH`.
- The receiver uses a same-user lock, a bounded lifetime, a private staging
  directory, a temporary XDG state root for Owly's generated TLS identity,
  process-group cancellation, an individual file-size resource limit, and
  post-transfer file-tree validation. Accepted files are moved without
  overwriting and receive owner-only permissions.
- Settings has a manual, read-only status refresh. Its Receive control is
  intentionally disabled because the upstream consent prompt is terminal
  based; the CLI is the explicit interactive entry point.

## Runtime states and evidence

`unsupported`, `unknown`, `available`, `blocked`, `ready`, `active` and
`failed` describe local capability or process state. A ready report means only
that the local preflight observed the selected conditions. Control-plane,
advertisement and data-plane states are independent. Owly's CLI does not
provide a structured advertisement-ready or byte-progress event, so those
fields remain unknown. A returned file is evidence of local filesystem output,
not a verified Apple identity or independently observed RF exchange.

Every report retains `authorization: never_evaluated` and
`physicalQualification: not_qualified`. No status value grants Linux access.

## Explicit non-goals

P019 does not implement an AWDL radio driver, install or start Owly/owlyd,
switch channels, configure monitor mode, detach NetworkManager, edit firewall
rules, install capabilities, elevate privileges, wake iPhones over BLE, send
AirDrop, implement Contacts Only authentication, or provide Apple Account or
Apple-signed identity. It does not claim universal chipset support or zero
Wi-Fi disruption. P010 Watch physical qualification stays NO-GO; W0–W6 remain
NOT RUN/BLOCKED; native Apple Watch Auto Unlock remains NO-GO.

## Dependencies and boundary

Owly's inspected `owly-airdrop` API has receiver events and configurable
resource limits, but the inspected workspace requires Rust 1.88 while
OpenContinuity's toolchain is Rust 1.85. The implementation therefore invokes
the exact-version upstream `owly` CLI as a separate local process instead of
silently raising the project MSRV or copying a second receiver. The upstream
CLI prompts before each transfer and safely extracts archives, but its CLI
does not expose total-resource-limit configuration or machine-readable
advertisement/progress events. OpenContinuity polls and stops on its staging
budgets; this leaves a bounded detection interval and possible transient
staging overshoot. That limitation is documented and keeps the UI action
disabled. A future direct API adapter should preserve Rust 1.85 in the main
workspace or make any toolchain change explicit and separately validated.

## Qualification

Hosted fixtures and unit tests validate local schemas, process bounds,
cleanup, and storage behavior only. They do not prove that an iPhone lists or
transfers to this Linux host. RTL8852BE qualification and the iPhone 13
physical trial require the separate procedure in
[P019 RTL8852BE physical qualification](P019_RTL8852BE_PHYSICAL_QUALIFICATION.md).
