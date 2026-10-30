# P011 Doctor bridge

The authoritative v1 definitions are `crates/opencontinuity-core/src/lib.rs` and
the `doctor --json` arm in `apps/opencontinuity-cli/src/main.rs`. The command is
static, read-only and performs no scan. Current native capabilities are all
`unsupported`, and all six Watch trust stages are `unknown`.

Accepted JSON has exactly these root keys:

- `schema_version`: numeric 1 (strings, booleans, other versions rejected).
- `project`: exactly `OpenContinuity`.
- `version`: nonempty string, at most 64 UTF-16 code units.
- `capabilities`: exactly three objects, each with only `id`, `state`, `summary`.
  IDs are exactly `apple_watch_auto_unlock`, `airdrop`, `airpods_settings`, each
  once. Order may vary. Summary is nonempty, at most 512 UTF-16 code units.
- `watch_auto_unlock_trust`: exactly `device_detection`, `trusted_identity`,
  `cryptographic_authentication`, `session_freshness`, `proximity_assurance`,
  `operational_readiness`, each with a valid state string.

Every state is one of `supported`, `experimental`, `unsupported`, `blocked`,
`unknown`. Unknown IDs/enums/fields and missing/type-invalid fields fail closed.
Malformed, truncated, trailing or oversized JSON is rejected. A lexical duplicate
object-member pass supplements QJsonDocument (which otherwise normalizes duplicate
keys); escaped equivalent keys collide. Strings reject control/format characters.
External strings are rendered as plain text, never rich text or URLs.

Only one active refresh runs. The production executable is a sibling CLI or an
explicit absolute `--cli` path checked for executable regular-file access.
QProcess receives a program and the fixed two arguments separately. There is no
shell, command parsing, PATH lookup, polling, research command or daemon startup.
The operator must trust the installed executable: this is not a sandbox for an
arbitrary malicious program. Symlinked packaged executables are allowed; path
validation does not authenticate binaries or eliminate filesystem replacement.

A single-shot timer bounds each run at 3000 ms. Stdout is capped at 16384 bytes;
stderr at 4096 bytes, with one sentinel byte to detect overflow. Channels are
read incrementally; oversized output kills the run. Successful exit must be normal,
zero, with empty stderr and a fully valid stdout report. Neither raw stderr nor
parser-controlled error text reaches QML. Error messages are fixed local strings.
QProcess/OS pipe buffers can transiently contain additional bytes before an event
is serviced; retained application buffers and accepted JSON remain capped.

Every process callback checks both process identity and generation. Cancellation
retires and disconnects callbacks, kills the child asynchronously, then deletes it
on completion. Timers stop on retirement. Reentrant Refresh is ignored while busy;
a cancelled refresh may be followed immediately by a new generation. No synchronous
process waiting is performed on the GUI thread.

State is `loading`, `loaded`, `cli_unavailable`, `invalid_report`, `timed_out`,
`process_error` or `cancelled`. If a prior valid report exists, any refresh failure
sets `reportState=stale` and preserves the concrete cause in `failureState`.
`schemaVersion`, `reportVersion` and `lastUpdated` describe the last valid report.
The five-minute age timer also marks it stale without launching another process.
Old rows stay visibly labelled as previous data even while loading. There is no
state conversion from a bridge error into `supported`.

Native Qt tests use a separately compiled controlled peer, which itself rejects
arguments other than `doctor --json`. Modes are test-peer environment variables,
not production command overrides. Tests cover parser rejection, all enums, models,
process output bounds, timeout, crash/nonzero/start errors, missing/relative paths,
concurrency, cancellation, retired callbacks, stale retention, recovery and age.
`OC_REAL_CLI` optionally connects tests to the real Rust binary; CI requires it.
No shell or Bluetooth is used by these tests.
