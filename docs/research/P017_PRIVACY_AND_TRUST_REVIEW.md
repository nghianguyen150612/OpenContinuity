# P017 privacy and trust review

Software tooling is reviewed here; physical Arch, Wayland and Watch trials are
NOT RUN. All output preserves `authorization: never_evaluated`.

P017 adds a Python operator wrapper, deterministic fixture tests and runbooks. It
reuses P015 read-only host probes and P016 strict evidence validation. It leaves
the Rust BlueZ ownership path, bounded writer, Qt protocol/parser/model and
P006/P009 wire formats unchanged. Inventory still performs one read-only
GetManagedObjects request. No scan occurs on startup, default preflight, evidence
validation, documentation generation or CI hardware access.

The active trust boundary requires an owned Arch test host, explicit local radio
consent, a trusted executable and ready prerequisites. Fixed argv, absolute
canonical path checks, parent ownership/mode checks, no shell, nonblocking capped
stdout, discarded stderr, process groups, finite deadlines and bounded TERM/KILL
fallback prevent argument injection and unbounded child output/lifetime. These
checks assume a trusted local account/runtime/environment and do not authenticate
installed software. Same-account replacement races and OS scheduling failures
remain outside that boundary. A force-killed process leaves cleanup uncertain.

Raw CLI transcripts can contain personal display names even after production
sanitization. Default operation retains only bounded data in memory and exports
P016 fixed classifications/counts. Optional storage requires a canonical existing
0700 directory and creates a new 0600 file with O_NOFOLLOW/O_EXCL relative to its
directory descriptor. No overwrite, telemetry or upload exists. Explicit local
input is bounded and regular-file-only; error messages do not echo paths, values,
exception details or malformed decoder excerpts. Host kernel suffixes, arbitrary
OS/version labels, device addresses/paths and personal names are not exported.
Manual screenshots and D-Bus traces require private operator review/redaction.

Operator declarations use exact fields/enums and opaque proof names. Passing a
schema check does not establish honesty, authenticity, proof contents or a
signature on evidence. Operator-reported passes remain separately labeled and
never promote physical gates. Actual RF requires a separate receiver, actual
cleanup requires an independent observer and actual GUI requires Wayland/Hyprland.
Shared BlueZ Discovering is not owned-session evidence. Physical Watch results
require signed installation and independent per-stage observations.

The following remain prohibited: automatic/background discovery; pairing/trust
writes; persistent device identity inference; Apple-private protocol impersonation;
Apple Account integration; native Auto Unlock/STS; PAM, polkit or login-manager
changes; production enrollment; metadata telemetry/upload. Existing P009 public
pins are manual test-only keys, not genuine Apple identity. LocalAuthentication
is a research-consent result, not secure proximity or authorized Linux unlock.

Remaining technical risks: controller firmware/LE filter behavior, advertisement
packing/capacity, system-bus registration permissions, shared-filter semantics,
physical long-read/MTU behavior, Watch callback order and foreground suspension,
Xcode provisioning and real compositor/plugin behavior. Mock D-Bus, offscreen Qt,
CryptoKit and unsigned SDK checks reduce software risk but close none of these
hardware gates. Keep P010 Watch qualification and native Apple Auto Unlock NO-GO.
