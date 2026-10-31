# P013 discovery security review

## Authorized mutation and trust boundary

Only an explicit discovery CLI command or Settings Scan click starts radio use.
No startup, Doctor, inventory, daemon, page-load or timer path starts discovery.
The dedicated connection calls only GetManagedObjects/GetNameOwner,
GetDiscoveryFilters, SetDiscoveryFilter, StartDiscovery and owned cleanup calls.
The per-client filter requests LE transport; it never changes adapter power,
pairing, connections, trust or global Bluetooth settings.

Start/Stop ownership is tracked independently from shared Discovering state.
Negative starts never send StopDiscovery. Confirmed starts get one StopDiscovery
attempt followed by filter removal, including on cancellation and errors. Lost
replies remain uncertain and disconnect is the fallback. Cleanup stays pinned
to the original BlueZ unique owner, so a replacement cannot inherit it. Software
private-bus tests verify another client's session remains active.

## Untrusted input and privacy

Signals must come from the pinned sender and selected adapter. Only explicit
Device1 activity fields can create records; snapshots and name-only changes are
not sightings. Duplicate paths retain a single scan-scoped index. Timers, queue,
message, object, event, record and name caps limit work. The independent Qt
parser rejects duplicate/unknown fields, wrong types, unsupported schemas and
inconsistent state/exit codes. Process callbacks check identity and generation.

Public records contain random scan-scoped IDs, sanitized local display names
and truthful event provenance. MAC-shaped labels and BlueZ paths are suppressed;
P012's control/format stripping is reused. No address/path/RSSI/advertisement,
IRK, pairing secret, research transcript or key is serialized. Labels can still
contain personal information or attacker-selected text; sanitized does not mean
anonymous or authentic. No telemetry, upload or persistent discovery log exists.

## Limits and residual risks

- BlueZ merges discovery filters. A requested LE scan does not attest the source
  of every event. Cached updates and another client's work may cause signals.
  No event is asserted to be an independent RF sighting or proximity proof.
- D-Bus disconnect cleanup depends on BlueZ and OS behavior. SIGKILL, daemon loss,
  wedged services and lost replies cannot guarantee explicit StopDiscovery. The
  report/UI disclose uncertainty; repeated StopDiscovery is never used to force
  global radio shutdown.
- The cooperative abstract-socket guard covers this build's same-UID GATT and
  discovery processes on the same configured bus address. It is not a global
  Bluetooth lock, and does not stop or alter another application.
- zbus and QProcess allocate transport buffers before application size checks.
  The caps are not a strict process-memory sandbox. The local CLI, BlueZ, Qt,
  plugins, kernel and user's account remain trusted local components.
- Malicious labels can resemble product names or include personal text. Neither
  a name, UUID, manufacturer field nor signal strength authenticates Apple
  hardware, the wearer or a Linux user. There is no authorization consumer.
- Physical cleanup, RF behavior, controller compatibility, real compositor
  background behavior and interference require separate Arch hardware trials.

Bluetooth discovery is not authentication. DoctorReport capability states and
all P007–P010 wire/security boundaries remain unchanged. Apple Watch Auto Unlock,
AWDL/AirDrop, AirPods configuration/switching and Linux unlocking remain
unsupported. Every discovery result has `authorization: never_evaluated`.
P010 Apple Watch SE 2 physical qualification remains **NO-GO**.
