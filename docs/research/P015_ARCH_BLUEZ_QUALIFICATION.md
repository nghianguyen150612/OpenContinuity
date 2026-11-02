# Arch BlueZ qualification

Run `python3 scripts/p015_arch_preflight.py` on the owned host. It uses read-only system bus queries, never starts discovery or changes power or policy. Adapter indices are snapshot-local. An unavailable query is unknown, not a pass; `Transport` filter availability alone does not establish working LE radio. Settings version is unknown when installed, because the GUI is not launched by preflight.

A separate, operator-approved active test may run `opencontinuity devices discover --seconds 10 --events-jsonl` locally. Treat its output as private: names and session identifiers can identify people or devices. Do not upload raw output. Record only reviewed aggregate results, exit status, terminal status and cleanup status, with a timestamp and software versions. Use an independently operated known peripheral/scanner to corroborate RF, with separate consent. Check GUI behavior on native Wayland/Hyprland separately. A stream observation is not proof of identity or proximity.

Record each stage as qualified, blocked, not_run, or failed with provenance: environment preflight; BlueZ discovery session; stream delivery; cleanup confirmation; independent RF; native GUI; Apple Watch hardware. No hardware stages have been run here. P010 Watch SE 2 remains NO-GO.
