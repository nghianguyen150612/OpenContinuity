# Contributing

Contributions should advance genuine, evidence-backed Apple interoperability while keeping the portable core independent of platform services.

## Development

Use the pinned Rust 1.85.0 toolchain (edition 2024 minimum) or a compatible newer stable toolchain. Before proposing Rust changes, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep protocol code, OS adapters, UI integration and domain contracts in separate modules. Do not introduce placeholder protocol implementations. Add tests that run without Apple devices wherever feasible. Record local validation separately from CI results.

## Protocol claims and security

Document the source and confidence for protocol behavior. Use `experimental` when behavior is incomplete or insufficiently validated. Do not report native support based on a custom replacement. Do not treat BLE presence, RSSI, an IRK match, or an unlocked-state advertisement as sufficient authorization for OS unlock. Sensitive flows must fail closed and keep device detection, identity, cryptographic authentication, session freshness, proximity assurance and readiness distinct.

Never include personal pairing keys, IRKs, Apple Account tokens or unredacted packet captures in code, issues, logs, tests or commits. Do not change PAM, sudoers, system authentication, global Bluetooth identity, Wi-Fi drivers, kernel modules or lock-screen security as part of development setup. No telemetry by default.

## Licensing

The project is MIT-licensed. Do not copy or vendor upstream code without recording the exact source revision, license, notices and required attribution. In particular, GPLv3 and GPL-2.0 sources require deliberate compatibility review before linking, copying or distributing a combined work. Do not use repositories whose license is unverified. Follow per-file and bundled-component notices even where a repository has a top-level license. See [the upstream audit](docs/UPSTREAM_MANIFEST.md).

## Changes

Keep pull requests scoped to a milestone and include user-visible documentation for changed support states. Never state a feature is supported without reproducible evidence and a testable implementation.
