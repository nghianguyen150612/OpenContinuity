# P016 adversarial stream test matrix

All Rust cases below spawn the production CLI on isolated private fake BlueZ
D-Bus instances. No test is physical RF evidence. See P016_VALIDATION for results.

| Condition | Deterministic control and assertions |
| --- | --- |
| Continuous slow reader | 4 KiB pipe, 512-byte reads with 10 ms pauses; 200, 512 and 513 inputs; exact labels, sequence, change, session and terminal counts |
| Record budget | 100 new observations plus updates through 512 accepted events; 513th rejected; 514 total records at cap, no sequence holes |
| SIGINT/SIGTERM pending output | 200 records exceed unread 4 KiB pipe, then signal; resume drain for cancelled terminal or retain block for bounded nonzero exit |
| Owner loss under pressure | Release well-known name with queued records; cleanup targets pinned unique owner; bluez_unavailable, no redirected calls |
| Bus failure under pressure | Kill private daemon with records pending; nonzero exit, ordered accepted output, incomplete cleanup |
| Close before terminal | Fake filter-clear method signals entry then delays reply; close stdout after Stop and during clear; nonzero exit |
| Partial terminal after cleanup | Child-only RLIMIT_FSIZE=450 and ignored SIGXFSZ on regular stdout file; complete session followed by invalid partial terminal; nonzero exit and exact cleanup log |
| Permanent block | Existing 4 KiB unread pipe test plus blocked cancellation; finite process lifetime and no successful terminal |
| Other bounds/failures | Existing 4096 duplicate-signal flood, 128-device cap, uncertain Start, cleanup failures, malformed activity, closed stdout |

New waits use a 12-second kill-and-reap watchdog. Each private daemon is reaped
on drop. Slow draining is about 50 KiB/s and exceeds two seconds overall at full
budget, while continually acknowledging records within the stall deadline.
These tests do not claim that arbitrary infinitesimal byte progress keeps a
record alive: only a fully written and flushed record resets the deadline.
Qt separately enforces a 22-second process deadline.

Audit: 1 session + at most 512 accepted device records + 1 terminal = 514.
The bounded channel accommodates all records even if none are consumed. Sequence,
byte and device-event counters advance only after successful enqueue. A single
FIFO writer acknowledges after write_all and flush. Serialized payload storage
is bounded by 512 KiB total (plus bounded metadata, one in-flight line and
serialization temporaries). The nonterminal byte ceiling reserves 2 KiB for the
terminal. Cleanup precedes terminal creation and terminal acknowledgement
precedes successful process return. The detached writer is never joined when
blocked; process exit ends it. No production writer rewrite was warranted. A production cleanup defect was
found: retaining a full zbus signal subscription after collection can stall the
connection reader and starve Stop/filter-clear replies. Cleanup now consumes the
session, drops signal subscriptions before calls, and retains the pinned owner
and cooperative guard through bounded connection close. The deterministic
signals_arriving_during_stop_cannot_starve_cleanup_replies test fails against
starting HEAD (incomplete cleanup, 4.25 s) and passes with the fix (0.43 s). Its
fake Stop delays 300 ms while 200 post-cancel signals fill the old 64-slot queue;
none of those post-cancel signals are accepted as observations.

The pipe terminal is below PIPE_BUF, so regular-file truncation is used to prove
partial OS writes without adding a production fault-injection switch. Physical
BlueZ cleanup is still NOT RUN. Byte/line limits are defense in depth; sanitized
80-character names mean ordinary accepted records do not reach 2 KiB, and 514
ordinary records do not necessarily reach the aggregate ceiling.
