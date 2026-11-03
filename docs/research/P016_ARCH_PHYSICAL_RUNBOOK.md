# P016 Arch physical qualification runbook

Status: NOT RUN. No owned Arch/controller or native Wayland/Hyprland session is
available in this cloud workspace. P010 Apple Watch SE 2 stays NO-GO.

1. On an owned Arch host, record the revision and locally inspect installed
   BlueZ, D-Bus, CLI and Qt versions. Run the read-only P015 preflight above.
   Verify system D-Bus and org.bluez availability and an adapter with powered=true
   and the Transport filter. An unavailable property is unknown. Do not change
   power, trust, pairing, permissions, policy or services to manufacture a pass.
2. Obtain consent for a controlled BLE peripheral and an independent receiver.
   Keep raw logs private (umask 077). Avoid recording addresses, names, serials,
   D-Bus paths or captures in published evidence. Note other discovery clients:
   Discovering is shared and cannot prove this client's ownership or cleanup.
3. Only by explicit operator action, run the existing bounded scan:
   `opencontinuity devices discover --seconds 10 --events-jsonl > private-scan.jsonl`.
   Immediately record the process exit status locally. Never run discovery in CI.
   Check live records, contiguous sequence, one terminal, counts and cleanup.
   Use the validator to produce the sanitized summary; separately attest exit.
4. Repeat with SIGINT and SIGTERM during observations; verify bounded process
   exit, cancelled terminal when deliverable, Stop and filter-clear attempts
   against the owned unique BlueZ peer. Use a locally reviewed diagnostic observer
   without publishing identifiers. Disconnect/Discovering=false alone is not
   proof; other clients may retain discovery. Missing terminal means uncertain
   cleanup to the consumer, even if a diagnostic observer saw a Stop attempt.
5. Independently corroborate the controlled peripheral's advertising and timing
   using a separate receiver and controlled on/off intervals. Record methodology,
   sample counts, discrepancies and consent in a sanitized operator record.
   BlueZ observations or successful JSON parsing alone cannot qualify RF.
6. Run Settings explicitly in the real Qt Wayland session on Hyprland. Verify no
   startup scan, live unverified rows before completion, Cancel, navigation away,
   close, no rescan, and visible stale/uncertain cleanup on child failure. Keep
   this evidence separate from offscreen QML and RF observations.
7. Record each gate independently as PASS, FAIL, BLOCKED or NOT RUN with provenance
   and limitations. If hardware is unavailable leave NOT RUN. No step qualifies
   Apple identity, Watch secure approval, unlock or native Apple interoperability.

Do not pair, trust, enroll, change Bluetooth configuration, enable background
scanning, use PAM/polkit, or run privileged discovery. authorization remains
never_evaluated for every observation.
