# P018 Watch SE 2 physical trial runbook

**Cloud execution status: W0–W6 NOT RUN/BLOCKED.** No real Watch, Linux controller, independent RF receiver or native Wayland compositor was available. P010 remains NO-GO. Keep the topology **Linux BlueZ GATT peripheral/server ↔ Apple Watch foreground CoreBluetooth central/client**. The Watch does not advertise.

Each gate is recorded independently. A later gate cannot turn an earlier failure into a pass. Use `PASS`, `FAIL`, `BLOCKED`, or `NOT RUN`; attach source and provenance. Software fixtures and operator statements never establish physical hardware success.

| Gate | Required evidence | Cloud status |
| --- | --- | --- |
| W0 signed installation | Signed development installation and explicit app launch on an owned Watch SE 2 | NOT RUN / BLOCKED |
| W1 Linux service and advertisement | Owned P006 GATT application and LE advertisement registration, replies, cleanup, plus independent RF observation | NOT RUN / BLOCKED |
| W2 foreground discovery | Explicit Watch scan, controlled Linux peripheral discovery, connection, service and characteristic validation | NOT RUN / BLOCKED |
| W3 P006 exchange | Complete challenge and response, sequential acknowledged writes, verified synthetic digest, bounded cleanup | NOT RUN / BLOCKED |
| W4 P009 rejection | Test-only signed request validation and explicitly signed rejection returned to Linux | NOT RUN / BLOCKED |
| W5 P009 approval | Fresh test-only request, explicit foreground consent, actual LocalAuthentication outcome, signed approval verified by Linux | NOT RUN / BLOCKED |
| W6 cancellation and cleanup | Separate cancellation, disconnect and expiry cases; owned unregister replies; independent RF cessation | NOT RUN / BLOCKED |

1. **W0:** On an owned Watch SE 2 and paired iPhone, use a compatible Mac/Xcode, the operator's development team and the existing signed development-install procedure in [P007B](P007B_APPLE_SIGNED_INSTALL.md). Launch the app explicitly. Record only sanitized operator facts. Hosted unsigned builds do not pass W0.
2. **W1:** Complete the read-only P017 host preflight. With no overlapping discovery session on the same adapter, inspect and enable the Watch diagnostics toggle to obtain its local label, then start P006 with `opencontinuity research watch-gatt serve --adapter hci0 --duration-seconds 120 --experimental --diagnostics --trial-id <Watch-label> --json`. Capture stderr privately. An independent RF observer records its own evidence. A matching label is not a link-layer identity.
3. **W2:** Keep the Watch app foreground; press Scan explicitly, select the controlled Linux service, connect, and verify service and characteristic discovery. Record callback errors only as fixed categories. Do not infer identity from the displayed peripheral name.
4. **W3:** Run the P006 synthetic exchange. Check complete challenge/result lengths, frame acknowledgements, digest result and owned cleanup. Repeat missing callback, cancel, disconnect and timeout as separate runs. Missing/delayed long reads or ACKs are failure/unknown, never PASS.
5. **W4:** Stop P006 and confirm owned cleanup. Use the existing test-only P009 enrollment and dual pin confirmation from [P009A](P009A_TEMPORARY_ENROLLMENT.md), then run `research watch-auth-gatt serve` with its exact required options plus `--experimental --diagnostics --trial-id <Watch-label>`. Explicitly reject the signed research request and verify that signed rejection on Linux.
6. **W5:** Start a fresh bounded server session. On the foreground Watch, require verified request display, explicit consent and actual successful LocalAuthentication before approval. Unsupported or denied LA fails closed. Confirm the Linux research decision independently. Do not infer Linux authorization.
7. **W6:** Run cancellation, disconnect, expiry, server termination and cleanup as distinct cases. Distinguish unregister attempted from reply confirmed. Independently observe RF cessation; process exit alone is not proof.

For pairing, export the Watch diagnostic JSON and retain the host stderr line. Run `python3 scripts/p018_watch_trial_evidence.py --host host.json --watch watch.json`. The validator's result is a sanitized structural summary only. Record W0 physical installation and independent RF observations separately through owner review. Do not change pairing/trust, system policy or login state, export private keys, enroll production keys, or run background scanning. Keep `authorization: never_evaluated` throughout.
