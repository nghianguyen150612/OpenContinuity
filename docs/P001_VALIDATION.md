# P001 Validation Record

Recorded locally on 2026-10-27 for source commit `7f51ca52b25d27fed03bda81b8b0c396dafaf752` on branch `feat/p001-bootstrap`. Local results are separate from hosted CI results.

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

Workflow is configured in `.github/workflows/ci.yml` for push and pull request events. Before publishing the branch, hosted CI was **NOT RUN**. After the branch push, GitHub Actions run [40477843676](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40477843676) was created for source commit `7f51ca52b25d27fed03bda81b8b0c396dafaf752` (push event). At 2026-10-27 09:05 UTC its status was **QUEUED**, with no runner assigned and no conclusion. This is neither a pass nor a failure; check the run for its final result.
