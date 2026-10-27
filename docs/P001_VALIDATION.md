# P001 Validation Record

Recorded locally on 2026-10-27 for branch `feat/p001-bootstrap`. Local results are separate from hosted CI results.

## Local results

Rust toolchain: `rustc 1.85.0 (4d91de4e4 2025-02-17)`, pinned in `rust-toolchain.toml`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace` | PASS: 7 tests passed; no failures |
| `cargo build --workspace` | PASS |
| `opencontinuity --help`, `--version`, `doctor`, `doctor --json` | PASS; JSON parsed and returned schema version 1 |
| `opencontinuityd --help`, `--version` | PASS |
| `opencontinuityd` foreground start and SIGINT shutdown | PASS; exit status 0 |
| `git diff --check` | PASS |

All checks run without Apple devices. Doctor reports protocol support as unsupported and trust stages as unknown. No radio scan or system authentication change was performed.

## Hosted CI

Workflow is configured in `.github/workflows/ci.yml` for push and pull request events. Hosted CI status: **NOT RUN** for this local branch because it has not been pushed. This is not a CI pass. Re-run and record the GitHub Actions result after publishing the branch.
