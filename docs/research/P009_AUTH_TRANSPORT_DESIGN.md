# P009 authenticated research transport design

## Scope and status

P009 defines `OC-WATCH-AUTH-TRANSPORT/1`, a separate transport for P008's P-256 signed research request and response. It is separate from P007's synthetic GATT v1 service and does not turn a transport acknowledgment into an authentication decision.

The portable Rust state machine, pure Swift counterpart, a distinct opt-in BlueZ GATT service, a CoreBluetooth client path, and a foreground **Authenticated Research** enrollment screen are implemented. The Watch creates an ephemeral CryptoKit P-256 test key in memory, imports and independently verifies the Linux host public pin, exports only the Watch public enrollment, and requires confirmation of both fingerprints. Linux requires both explicit fingerprint values before registration. This is a test-only manual exchange, not genuine Watch identity or production enrollment. CI also hands actual serialized frames Rust → Swift → Rust. This is software integration evidence, not BLE interoperability or physical end-to-end evidence.

## GATT allocation

The proposed isolated service UUID and characteristics are:

| UUID | Role | Properties |
| --- | --- | --- |
| `9a7f0001-6f4e-4c4f-9b4f-4f70656e436f` | Authenticated research service | Primary |
| `9a7f0002-6f4e-4c4f-9b4f-4f70656e436f` | Host request frame | Read |
| `9a7f0003-6f4e-4c4f-9b4f-4f70656e436f` | Host request GET/ACK/control | Write with response |
| `9a7f0004-6f4e-4c4f-9b4f-4f70656e436f` | Watch response frame | Write with response |
| `9a7f0005-6f4e-4c4f-9b4f-4f70656e436f` | Watch response completion/cancel | Write with response |
| `9a7f0006-6f4e-4c4f-9b4f-4f70656e436f` | Version/session/length status | Read |

These UUIDs are P009-only. P007 v1 UUIDs, request/response bytes and semantics remain unchanged. No fallback or downgrade is defined.

## Runtime integration and opt-in boundaries

On Linux, `opencontinuity research watch-auth-gatt serve` requires `--experimental`, an explicit adapter, a runtime of 1–60 seconds, schema-2 `test_only` enrollment pins, a separate mode-0600 host-key file, and exact host/Watch SHA-256 confirmation flags. The Watch record must be active and bound to the selected host identity, enrollment and UID. P009A adds `generate-host-key`, `export-host-pin` and `check-enrollment` commands. Linux performs a read-only adapter preflight, never powers/configures the adapter, binds one BlueZ device owner, and unregisters only its own GATT application and advertisement.

P009A bounds preflight and registration to 30 seconds total and each D-Bus method call to 2 seconds. No P008 request is signed until both the GATT application and advertisement are registered. A fresh request is then signed with the P008 60-second maximum lifetime. The request transfer, consent wait and response transfer each have separate monotonic deadlines; see [P009A deadline model](P009A_DEADLINE_MODEL.md). The Watch UI requires manual host-key pin verification, exports only its public enrollment, and waits for the Linux operator's Watch-fingerprint confirmation before starting a scan. Pairing, advertised names, service discovery and peripheral identity remain untrusted.

## Envelope and frames

The signed envelope is the exact P008 canonical message immediately followed by its 64-byte raw detached signature. Its length is `canonical_length + 64`; no JSON, key, or transport ACK is included in the signature envelope. The canonical message is at most 1,024 bytes, so the envelope ceiling is 1,088 bytes.

Each frame header is 15 bytes:

| Offset | Size | Encoding | Meaning |
| ---: | ---: | --- | --- |
| 0 | 2 | ASCII `OA` | P009 framing magic |
| 2 | 1 | unsigned | Transport version, exactly `1` |
| 3 | 8 | bytes | Random transfer ID |
| 11 | 1 | unsigned | Sequence, starting at 0 |
| 12 | 1 | bit field | `0x01` start, `0x02` end; all other bits rejected |
| 13 | 2 | little-endian | Total envelope bytes on start frame, zero otherwise |
| 15 | 1–499 | bytes | Ordered envelope fragment |

The GATT characteristic identifies direction: request frames are host-to-Watch, and response frames are Watch-to-host. A decoder is configured for one direction and rejects use on the other stream. At ATT MTU 23, writes carry at most 20 value bytes; after the header, 5 bytes remain. `ceil(1088 / 5) = 218` frames, within the 8-bit sequence space. At ATT MTU 517, the maximum payload is 499 bytes (`517 - 3 - 15`). The implementation caps envelope length at 1,088 bytes, frame length at `MTU - 3`, and conservative frame count at 218.

The Rust and Swift codecs share these field positions, constants, flags, bounds, and little-endian total length. `SignedEnvelope`/`WatchSignedEnvelope` also parse the canonical message for the selected direction before a completed envelope can be consumed. Cryptographic verification remains a separate P008 step.

## Host-to-Watch chunk and acknowledgment design

CoreBluetooth does not let a caller select an arbitrary GATT read offset. The Watch therefore writes a 13-byte control value before each read:

```text
"OA" | version:u8 | opcode:u8 | transfer_id:8 bytes | frame_index:u8
```

`GET` is opcode 1, `ACK` is opcode 2, `COMPLETE` is opcode 3, and `CANCEL` is opcode 4. `GET(index)` selects one fixed-size frame. Reading the request characteristic returns that same frame until the Watch writes `ACK(index)` with response. A repeated GET for the currently outstanding index returns the same bytes; it does not advance. Only the explicit, ordered ACK advances. An ACK for an unrequested or different index, a skipped index, a different owner, or a different transfer ID is rejected. Completion succeeds once, and only after every frame has been ACKed.

The status read returns exactly 13 bytes: `OA | version:u8 | transfer_id:8 | envelope_length:u16le`. The length determines the fixed frame count (`ceil(length / 5)` at the conservative MTU 23). The request frame characteristic is unavailable until a matching GET is accepted. `CANCEL` is accepted on either control characteristic; `COMPLETE` is accepted after all request frames are ACKed or after a fully reassembled response.

This uses public CoreBluetooth operations (`writeValue(_:for:type: .withResponse)` and `readValue(for:)`) and never assumes a multi-kilobyte characteristic read. The Linux mailbox and Watch CoreBluetooth manager implement the host GET/ACK rules. Each control write completes with response before the selected request frame is read or acknowledged. This code has not been exercised over a BlueZ/CoreBluetooth radio link.

## Watch-to-host transfer

The Watch writes one response frame at a time with response. The next write may begin only after `didWriteValueFor` succeeds. The host response assembler rejects any transfer ID, sequence, version, total, or owner mismatch. A separate 13-byte control write completes or cancels the transfer. The complete command is accepted once, after exact-length reassembly; a duplicate completion or incomplete response is rejected. A valid transport envelope is then passed to P008 response verification. A signed rejection remains a rejection.

The Rust server and Watch CoreBluetooth manager implement this sequence for the separate P009 service. The manager writes one response frame with response at a time, then sends one completion command. A response is not accepted on Linux until P008 validates its signature and bindings. Apple SDK compilation and physical callback behavior remain pending hosted CI and device validation.

## Ownership, deadlines, and cleanup

The core assembler is constructed with an explicit owner identifier, transfer ID, direction and monotonic phase start. Host request transfer is limited to 20 seconds; response transfer is limited to 15 seconds from its first response frame. Both have an 8-second inactivity limit and are clamped by the signed expiry and consent deadline. Disconnect/cancel clears session bytes; owner changes cannot resume an old transfer. A process restart creates a new owner/session and invalidates callbacks at the app lifecycle layer. The current protocol is single-session in-memory and intentionally does not persist replay state.

The BlueZ integration uses the P006A ordered disconnect monitor, binds one active GATT client to one fresh signed P008 request, invalidates it at disconnect/timeout/BlueZ owner change, and rejects another client's access. It registers only its own D-Bus objects and unregisters owned advertisement/application resources on shutdown. The CoreBluetooth manager binds callbacks to one connection generation and discards session bytes on disconnect, timeout, cancellation, or app suspension.

## Relationship to P007 and P008

P007 provides synthetic transport framing, challenge bytes and a SHA-256 test digest. It is not authenticated. P008 provides canonical P-256 signed messages, independent test-only enrollment pins, replay/expiry/binding validation and a gated consent view, but has no transport. P009 adds a separately versioned framing and state machine plus a Rust/Swift serialized software round trip. Neither P007 nor P008 wire formats are modified.

P009A supplies an explicitly user-initiated temporary test enrollment flow. Apple SDK build evidence and physical BLE validation remain separate gates. Keep physical signed approval claims blocked until those gates close. P009 never authorizes Linux or claims genuine Watch identity, secure proximity, wearer presence, Secure Enclave custody, or Auto Unlock. See [temporary enrollment](P009A_TEMPORARY_ENROLLMENT.md) and [security review](P009A_SECURITY_REVIEW.md).
