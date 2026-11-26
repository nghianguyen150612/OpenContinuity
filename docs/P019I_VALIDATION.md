# P019I validation

## Historical extraction evidence

Initial extraction records reported local Rust checks and a macOS cross-target
check; those historical claims were not reproduced in the R1 starting environment.
At baseline `68ece30df1a488d4e353ef5223e79fc15ce6ddd5`, hosted
[push](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/45514379053)
and [PR](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/45514390144)
CI passed all six jobs: Rust 1.85 fmt/Clippy/tests/build, Qt/QML, and
Swift/CryptoKit interoperability plus unsigned watchOS compilation. Rust had
209 passing tests, Python 55, Qt five CTest targets (20 AirDrop cases).
These are hosted results, not local R1 execution.

## R1 current environment

The starting environment has no Cargo, rustc, Clippy, CMake, CTest or Qt 6
development runtime. A temporary Rust 1.85 toolchain under `/tmp/p019i-tools`
is used for available local Rust checks without changing host configuration.
Locally executed with the temporary Rust 1.85 toolchain: formatting, Clippy
with warnings denied, 213 workspace tests, and workspace build passed.
Python unittest discovery: 55 passed. Diff whitespace check: passed.
Qt/CMake/CTest, Swift/CryptoKit and watchOS checks are hosted only; the local
`qmllint` wrapper cannot supply Qt 6 validation.
Final source SHA, local Rust counts, and exact hosted push/PR results are
recorded in PR #21's description after completion, avoiding a self-referential
commit SHA in this file. The full workflows include Swift/CryptoKit and
unsigned watchOS compilation; neither establishes physical qualification.

The historical no-wireless/no-Owly host result is runner-specific and is not
an observation of the target RTL8852BE hardware. R1 disables live Owly probing
because D-Bus activation can start radio journal recovery; see the scope review.

`authorization: never_evaluated`

`physicalQualification: not_qualified`

RTL8852BE AWDL / iPhone 13 AirDrop: NOT RUN.
Watch SE 2 W0–W6: NOT RUN/BLOCKED. Native Apple Auto Unlock: NO-GO.
