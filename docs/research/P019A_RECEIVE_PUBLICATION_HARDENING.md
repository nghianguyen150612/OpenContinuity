# P019A receive publication hardening

## Finding and containment decision

At checkpoint c6746644, receive finalized staging after cancellation, expiry,
nonzero backend exit and relay failure. Only output-limit failure discarded it.
Thus unsuccessful sessions could publish files, and multiple top-level renames
could expose a prefix before a later failure. Neither behavior is acceptable.

Publication eligibility now requires successful backend exit, no cancellation,
no expired deadline, no output-limit violation, successful output relays,
verified termination of every owned writer, and successful staging validation.
All failure results report zero published files/bytes. With verified quiescence,
failed staging is discarded; with unknown quiescence it is retained privately
and cleanup reports incomplete. Runtime identity state is likewise retained
after backend launch when quiescence cannot be established.

**Receive activation is currently blocked before staging/backend creation.**
The explicit terminal end reason is `writer_containment_unavailable`, state
`failed`, backend process `blocked`, cleanup `not_started`, exit status 1.
This is the requested fail-closed alternative, not completed descendant
termination support. The former receive capability is deliberately unavailable.

## Assurance and limitations

A reaped leader does not prove that its process group is empty. Even an empty
group cannot prove the absence of escaped descendants using setsid/setpgid.
A pidfd gives stable identity for one process, not containment of descendants.
A delegated cgroup plus independently enforced prevention of migration, or a
reviewed sandbox/controlled process model, would be needed before enabling
publication. This change neither requires root nor changes host cgroups.

No global process scan or negative-PGID signalling is used. The dormant
supervisor can kill/reap only its direct owned child, within a bounded polling
deadline; it does not claim group cleanup or graceful Owly shutdown. Output
reader joins have deadlines, including read-only probes whose descendants may
retain pipe descriptors. Timed-out reader threads detach with bounded buffers;
this is a resource-lifetime limitation, not verified descendant cleanup.

Known-writer, failed-session, cancellation, expiry, nonzero-exit, relay-failure
and output-limit regressions exercise production helpers using software
fixtures. They do not establish escaped-descendant termination. Graceful
group shutdown, SIGINT-to-SIGKILL group escalation, and escaped-writer
quiescence remain unsupported, rather than being inferred from leader status.
No content is published under this limitation.

## Atomic bundle primitive

After quiescence is independently established, storage finalization validates
all staging entries, including interrupted upload trees, before removing Owly
temporary directories. It rejects unsafe names, symlinks, multiply linked files,
special files and count/depth/size violations. Directories become 0700 and
regular files 0600. UTF-8 names are checked by byte length.

The destination directory is opened and pinned by descriptor. Staging operations
use that descriptor through Linux procfs, and publication uses descriptor-relative
renameat2(RENAME_NOREPLACE). Renaming/replacing the destination pathname cannot
redirect publication. Procfs is required; failure is closed. The operator and
same-UID local processes are trusted; this is not a sandbox against that UID
or root, and validation assumes verified writer quiescence.

One successful rename exposes the entire bundle:

```text
destination/
  AirDrop Receive <128-bit-random-id>/
    photo.jpg
    document.pdf
    folder/file.txt
```

Up to eight generated names are attempted on collisions. Existing entries are
never overwritten. There are no individual top-level publication renames.
Validation or rename failure exposes no prefix. Successful bundles persist
after the staging object's destructor; unpublished staging is removed only
when safe. Atomic namespace visibility does not imply crash durability:
there is no file/directory fsync protocol or persistence guarantee.

JSON counts are exact regular-file counts and bytes in a successfully published
bundle; zero-byte files count as files. Counts are zero on failure. No private
paths or untrusted names are included in JSON. Transfer outcome, advertisement
and physical interoperability remain unknown.

## Doctor contract

The contract was checked against t4t5/owly commit
6f2e8ea99aed0687558d78a1dd49154ab3a5fa76, version 0.4.2:
crates/owly-cli/src/doctor.rs and owly-radio/src/{radio,probe,quirks}.rs.
Required nullable station/driver/device/status/mode/channel/card fields,
hardware boolean, missing objects (id/say), issues strings, card booleans,
regdom and channel records are checked. Unknown status/mode/missing categories
are rejected. Unknown extra fields are tolerated. Empty missing alone is
insufficient: hardware must be true. Invalid JSON, nonzero exit, timeout and
either output truncation fail closed. Exact binary version, local AWDL
interface readiness and link-local address remain mandatory.
Doctor readiness never means live advertisement or iPhone interoperability.

## Unchanged limits and physical status

The aggregate staging scanner remains approximate, not a hard filesystem
quota. Its disk-exhaustion risk is unresolved. Qt Receive stays disabled.
P006/P009, Bluetooth radio guards, fixed-version executable trust, explicit
consent, isolated state and singleton locking are unchanged.

Real RTL8852BE AWDL: NOT RUN. iPhone 13 discovery/transfer: NOT RUN.
Native Wayland/Hyprland: NOT RUN. Watch SE 2 W0–W6: NOT RUN/BLOCKED.
Native Apple Watch Auto Unlock: NO-GO.

Every report preserves `authorization: never_evaluated` and
`physicalQualification: not_qualified`. Software tests do not establish
native AirDrop compatibility or physical qualification.

Final review also separated the staging-retention flag from successful
publication: retaining an unknown-writer directory must never cause exhausted
bundle-name collisions to be reported as successful publication.
