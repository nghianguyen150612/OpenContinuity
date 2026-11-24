# P019I validation

## Local validation

- Rust 1.85 formatting: PASS (`cargo fmt --all -- --check`).
- Clippy: PASS (`cargo clippy --workspace --all-targets -- -D warnings`).
- Workspace tests: PASS, 209 passed, 0 failed, 0 ignored (`cargo test --workspace --all-targets`).
- Workspace build: PASS (`cargo build --workspace`).
- macOS Rust cross-target check: PASS (`cargo check --workspace --all-targets --target x86_64-apple-darwin`); this checks the explicit unsupported-platform report path, not a macOS runtime.
- Python unittest discovery: PASS, 55 passed.
- `git diff --check`: PASS.
- Local QML/C++ execution was unavailable because this runner has no Qt 6 development/runtime installation; the `qmllint` wrapper points to a missing Qt 5 binary. The hosted Qt 6 CI job is the validation gate for this layer.

The final hosted push and Draft PR workflows both pass on the extraction head. The Qt 6 job passed all five CTest targets, including 20 AirDrop status backend cases and the real QML navigation smoke test. Its build compiles the AirDrop report parser and verifies rejection of an unapproved Owly version.

The local host report observed no wireless interface and no fixed-path Owly installation. It returned `radio: unsupported`, `awdlInterfaceState: blocked`, `backend: blocked`, `receiverAvailable: false`, and `physicalQualification: not_qualified`. This is an observation about this runner only.

Mock tests establish strict schema rejection, missing/unsupported state mapping, bounded subprocess execution, no Receive backend launch/staging/publication, and disabled Qt Receive behavior. They do not establish hardware support or physical AirDrop interoperability. Hosted push and Draft PR workflow results are linked from the final PR checks for its exact head SHA.

`authorization: never_evaluated`

`physicalQualification: not_qualified`

RTL8852BE AWDL / iPhone 13 AirDrop: NOT RUN. Watch SE 2 W0–W6: NOT RUN/BLOCKED.
Native Apple Auto Unlock: NO-GO.
