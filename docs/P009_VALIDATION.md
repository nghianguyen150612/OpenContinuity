# P009 validation history

This document records the original P009 validation. It is superseded for current implementation status by [P009A validation](P009A_VALIDATION.md), which records the deadline lifecycle, temporary enrollment and new Apple CI run.

The original P009 implementation at SHA `3febcfca3764d9abc8ac465920f11e0ead1424e0` passed the [push workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40881911861) and [PR workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40881923256). A follow-up at SHA `e1992ea1e17b036b656d0fb0136a56920605e12a` passed the [push workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40882447260) and [PR workflow](https://github.com/nghianguyen150612/OpenContinuity/actions/runs/40882455512). Those runs exercised the original Rust checks, Swift protocol tests, serialized Rust → Swift → Rust transport exchange and unsigned watchOS build.

The original report's hardware status remains applicable: Linux ↔ Watch RF transport, Watch LocalAuthentication and BlueZ controller behavior were not run. Those stages remain NOT RUN until physical evidence is collected. P009A software validation must not be treated as physical BLE evidence or Linux authorization.
