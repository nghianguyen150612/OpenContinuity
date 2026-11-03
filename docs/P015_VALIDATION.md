# P015 validation

P015 remains provisional. Physical Arch/BlueZ RF, native Wayland/Hyprland, and Apple Watch SE 2 qualification were not performed; Watch qualification remains NO-GO. See the P015 research documents for the bounded writer design, limitations, and operator preflight. Hosted final-head and GUI validation must be independently verified before merge.

Local Debian 13 validation: Rust 1.85.0 formatting, Clippy with warnings denied, workspace tests (174 passed) and workspace build passed. Python unittest discovery passed 9 tests. The private BlueZ D-Bus suite includes a 100-event delayed-reader burst, permanent blockage and closed pipe (24 tests total). `git diff --check` passed. Local Qt/CMake/Ninja and Apple SDKs were unavailable; hosted results must be checked on the final commit. Further coverage for slow continuous readers, owner loss or cancellation specifically during queue pressure, terminal-write failure, and additional Qt adversarial cases remains outstanding. Do not infer physical qualification from software fixtures.

P016 adds adversarial production stream and Qt lifecycle tests plus a local,
privacy-safe evidence validator. See [P016 validation](P016_VALIDATION.md). Software fixtures
do not qualify physical RF, native Wayland/Hyprland or Apple Watch SE 2; those
hardware gates remain NOT RUN and Watch remains NO-GO.
