# P019 upstream and license audit

The following public default-branch snapshots were fetched and inspected for
README claims, manifests, license files, architecture, receiver paths, and
hardware limits. These hashes pin what was reviewed; they are not approvals or
security endorsements.

| Repository | Inspected commit | License evidence | Architecture and relevant behavior | P019 decision |
| --- | --- | --- | --- | --- |
| [t4t5/owly](https://github.com/t4t5/owly) | `6f2e8ea99aed0687558d78a1dd49154ab3a5fa76` | Workspace and `owly-airdrop` / `owly-radio` manifests state MIT. The nested `crates/omdrop-owl/omdrop-awdl/LICENSE` is GPL-2.0-only for kernel-derived helper sources. Do not infer every subcomponent license from the repository root. | Rust 2024 workspace, version 0.4.2, declared Rust 1.88. `owly-airdrop` supplies a Rust receiver core and Tokio/rustls driver, mDNS announcement, explicit request events/accept/decline, bounded archive extraction, and staging. `owly receive` uses an existing `awdl0`, advertises over mDNS, prompts for each `/Ask`, and serves `/Discover`, `/Ask`, `/Upload`. BLE wake advertising is used in sender commands, not this receive command. Shared mode may preserve Wi-Fi only under supported driver/channel combinations; Exclusive/Active-monitor modes may turn Wi-Fi off. RTL8852BE Shared is listed for a specific CachyOS/kernel/device setup only. | Selected only as an optional, fixed-version external CLI process. No source or binaries are vendored. Do not use `--accept`. The public CLI lacks configurable total resource caps and structured readiness/progress output; therefore Settings does not start receiving and P019 documents the process boundary. The Rust library is a possible later integration only after an explicit MSRV and API/ownership decision. |
| [seemoo-lab/owl](https://github.com/seemoo-lab/owl) | `0ee3263f944ea721c355f826aa90d636d012a512` | GPL-3.0 project license. | C/C++ Linux AWDL userspace stack with mac80211 monitor interface, packet injection, synchronization/election, bridge/TAP interface and radio-specific setup. It is a radio layer, not the same as Owly's current Rust service architecture. | Research only. No GPL source or linked dependency is added to the MIT project. |
| [seemoo-lab/opendrop](https://github.com/seemoo-lab/opendrop) | `11fe7ba7861093b302bc0637e8cb10adf2d29337` | GPLv3 `LICENSE`; Python package metadata also declares GPLv3. | Python AirDrop client/receiver layered on macOS AWDL or Linux OWL, with TLS/HTTP, mDNS and archive handling. Its README documents Contacts Only dependence on Apple credentials, Everyone mode, automatic receive behavior in the historical CLI, and BLE wake limitations. | Protocol/research reference only. No code copied or run; its automatic accept behavior is not reused. |
| [ayourtch-llm/opendrop-rs](https://github.com/ayourtch-llm/opendrop-rs) | `dccc798e244363eb92d35e3c52e9a913188dda91` | Workspace manifest declares `GPL-3.0-only`; `LICENSE` is GPLv3. | Two-binary Linux design: `filin` AWDL link creation with monitor mode/injection and `luftlift` mDNS/TLS/HTTP/`/Discover`/`/Ask`/`/Upload`. README reports real Apple-device transfers but also says the AWDL process self-configures the radio, requires root/capabilities and that BLE wake is absent. This report is an upstream claim, not independently reproduced here. | Research only; no GPL source copied or linked. The radio-mutating architecture conflicts with P019's no-automatic-radio-change boundary. |
| [BenItBuhner/AirDrop-for-Everything](https://github.com/BenItBuhner/AirDrop-for-Everything) | `34bdc8dfdd9c3eebd5b5bd8588b2fad2243a8e9c` | Root `LICENSE` and workspace manifests state MIT. `NOTICE` and README describe a clean-room implementation and identify inspirations and separately licensed upstreams; individual files/dependencies still require review before reuse. Workspace uses Rust 2021; the manifest has no `rust-version` field while the README badge claims 1.75+. | Cross-platform Rust protocol core and thin shells, with mDNS, TLS, binary plists, DVZip/CPIO, plus platform-dependent AWDL and BLE pieces. README distinguishes LAN interoperability from AWDL and BLE wake, and describes per-request ask and receive safety. | Monitor as a possible later library/process candidate. No dependency selected: actively changing, broad target/dependency surface, and this branch needs a Linux AWDL lifecycle with explicit ownership first. Its self-described clean-room status is not a substitute for source and dependency audit. |

## Architecture comparison

OWL is a lower-level Linux radio implementation that creates an AWDL link and
requires hardware/driver-specific monitor-mode and injection behavior. OpenDrop
adds AirDrop application protocols over that link. `opendrop-rs` combines the
same layers into two GPLv3 Linux executables. AirDrop-for-Everything separates
an MIT-described protocol core from link layers and platform shells but its
current architecture spans several operating systems and has no declared
manifest MSRV. Owly separates a radio service (`owlyd` / `owly-radio`) from
AirDrop application code (`owly-airdrop`), and its user CLI can consume an
already-prepared `awdl0` without asking OpenContinuity to activate the radio.

All reviewed receiver implementations use mDNS/DNS-SD and the AirDrop HTTP/TLS
endpoints. Owly explicitly implements `/Discover`, `/Ask`, `/Upload`; its
receive path requires a running AWDL interface and does not send a BLE wake
advertisement. Everyone-mode TLS is not Apple identity verification; Contacts
Only depends on Apple-signed identity material that OpenContinuity does not
have. We do not implement or claim Apple identity, contacts matching, Apple
Account integration or authenticated AirDrop sender identity.

## Dependency and distribution policy

The P019 source adds no upstream dependency and vendors no upstream source.
At runtime it can invoke `owly` only from `/usr/bin/owly` or
`/usr/local/bin/owly`, requires version 0.4.2, passes a fixed argument vector,
and does not search `PATH`. Users install and update Owly independently under
its license and trust model. A separate executable boundary is not a blanket
legal safe harbor; if OpenContinuity later distributes the process or links
libraries, maintainers must review the exact dependency tree, notices, source
and distribution terms. The GPLv3 OWL/OpenDrop/opendrop-rs code is not copied
or linked into the MIT workspace.
