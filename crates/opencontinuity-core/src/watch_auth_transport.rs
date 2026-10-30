//! Versioned framing and ownership for the opt-in signed Watch research path.
//!
//! This wire contract is deliberately separate from `gatt_wire` (synthetic
//! P007). It carries P008's canonical message followed by its detached raw
//! P-256 signature. It has no Bluetooth, key storage, or authorization I/O.

use std::time::{Duration, Instant};

pub const PROTOCOL_NAME: &str = "OC-WATCH-AUTH-TRANSPORT/1";
pub const PROTOCOL_VERSION: u8 = 1;
pub const SERVICE_UUID: &str = "9a7f0001-6f4e-4c4f-9b4f-4f70656e436f";
pub const REQUEST_DATA_UUID: &str = "9a7f0002-6f4e-4c4f-9b4f-4f70656e436f";
pub const REQUEST_CONTROL_UUID: &str = "9a7f0003-6f4e-4c4f-9b4f-4f70656e436f";
pub const RESPONSE_DATA_UUID: &str = "9a7f0004-6f4e-4c4f-9b4f-4f70656e436f";
pub const RESPONSE_CONTROL_UUID: &str = "9a7f0005-6f4e-4c4f-9b4f-4f70656e436f";
pub const STATUS_UUID: &str = "9a7f0006-6f4e-4c4f-9b4f-4f70656e436f";

pub const MAX_CANONICAL_BYTES: usize = 1024;
pub const SIGNATURE_BYTES: usize = 64;
pub const MAX_ENVELOPE_BYTES: usize = MAX_CANONICAL_BYTES + SIGNATURE_BYTES;
pub const FRAME_HEADER_BYTES: usize = 15;
pub const MIN_ATT_MTU: usize = 23;
pub const MAX_ATT_MTU: usize = 517;
pub const ATT_WRITE_OVERHEAD: usize = 3;
pub const CONSERVATIVE_ATT_VALUE_BYTES: usize = 20;
pub const CONSERVATIVE_FRAME_PAYLOAD_BYTES: usize =
    CONSERVATIVE_ATT_VALUE_BYTES - FRAME_HEADER_BYTES;
pub const MAX_FRAME_COUNT: usize = MAX_ENVELOPE_BYTES.div_ceil(CONSERVATIVE_FRAME_PAYLOAD_BYTES);
/// Maximum wall-independent time to retrieve the signed host request after
/// the first connected client claims the ready session.
pub const REQUEST_TRANSFER_TIMEOUT: Duration = Duration::from_secs(20);
/// Maximum time for a signed response transfer after its first frame.
pub const RESPONSE_TRANSFER_TIMEOUT: Duration = Duration::from_secs(15);
/// A stalled BLE transfer also fails after this much inactivity, even when its
/// absolute phase deadline has more time remaining.
pub const BLE_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(8);
/// The Watch must return a response or cancellation within this interval after
/// the host receives the explicit request-delivery COMPLETE command.
pub const WAITING_FOR_CONSENT_TIMEOUT: Duration = Duration::from_secs(25);
/// A ready research session that receives no client is retired after this
/// interval. Signed P008 expiry remains the outer bound for all phases.
pub const WAITING_FOR_CLIENT_TIMEOUT: Duration = Duration::from_secs(45);
/// Compatibility alias for earlier users of the transport helpers.
pub const TRANSFER_TIMEOUT: Duration = RESPONSE_TRANSFER_TIMEOUT;

const MAGIC: [u8; 2] = *b"OA";
const FLAG_START: u8 = 0x01;
const FLAG_END: u8 = 0x02;
const ALLOWED_FLAGS: u8 = FLAG_START | FLAG_END;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    HostToWatch,
    WatchToHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    InvalidAttMtu,
    FrameTooLarge,
    Malformed,
    VersionMismatch,
    DirectionMismatch,
    SessionMismatch,
    OwnerMismatch,
    DuplicateOrOutOfOrder,
    StartRequired,
    MessageInProgress,
    MessageTooLarge,
    InvalidEnvelope,
    LengthMismatch,
    TransferExpired,
    InactivityExpired,
    IncompleteTransfer,
    AlreadyCompleted,
    DuplicateCompletion,
    Cancelled,
    InvalidChunkCommand,
}

/// A bounded monotonic phase deadline. Activity may move the idle deadline,
/// but never moves the absolute deadline.
#[derive(Debug, Clone, Copy)]
pub struct PhaseDeadline {
    absolute: Instant,
    idle_timeout: Option<Duration>,
    last_activity: Instant,
}

impl PhaseDeadline {
    pub fn new(
        started_at: Instant,
        absolute_timeout: Duration,
        idle_timeout: Option<Duration>,
    ) -> Self {
        Self {
            absolute: started_at + absolute_timeout,
            idle_timeout,
            last_activity: started_at,
        }
    }

    pub fn check(&self, now: Instant) -> Result<(), TransportError> {
        if now >= self.absolute {
            return Err(TransportError::TransferExpired);
        }
        if self
            .idle_timeout
            .is_some_and(|timeout| now.saturating_duration_since(self.last_activity) >= timeout)
        {
            return Err(TransportError::InactivityExpired);
        }
        Ok(())
    }

    pub fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub fn absolute(&self) -> Instant {
        self.absolute
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEnvelope {
    direction: Direction,
    canonical: Vec<u8>,
    signature: [u8; SIGNATURE_BYTES],
}

impl SignedEnvelope {
    pub fn new(
        direction: Direction,
        canonical: &[u8],
        signature: &[u8],
    ) -> Result<Self, TransportError> {
        if canonical.is_empty() || canonical.len() > MAX_CANONICAL_BYTES {
            return Err(TransportError::MessageTooLarge);
        }
        if signature.len() != SIGNATURE_BYTES {
            return Err(TransportError::InvalidEnvelope);
        }
        match direction {
            Direction::HostToWatch => {
                crate::watch_auth_research::HostAuthRequest::parse_canonical(canonical).map(|_| ())
            }
            Direction::WatchToHost => {
                crate::watch_auth_research::WatchAuthResponse::parse_canonical(canonical)
                    .map(|_| ())
            }
        }
        .map_err(|_| TransportError::InvalidEnvelope)?;
        let mut raw_signature = [0; SIGNATURE_BYTES];
        raw_signature.copy_from_slice(signature);
        Ok(Self {
            direction,
            canonical: canonical.to_vec(),
            signature: raw_signature,
        })
    }

    pub fn from_bytes(direction: Direction, bytes: &[u8]) -> Result<Self, TransportError> {
        if bytes.len() <= SIGNATURE_BYTES || bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(TransportError::InvalidEnvelope);
        }
        let split = bytes.len() - SIGNATURE_BYTES;
        Self::new(direction, &bytes[..split], &bytes[split..])
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.canonical.len() + SIGNATURE_BYTES);
        output.extend_from_slice(&self.canonical);
        output.extend_from_slice(&self.signature);
        output
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }

    pub fn signature(&self) -> &[u8; SIGNATURE_BYTES] {
        &self.signature
    }
}

/// Encode a signed envelope into ATT value-sized frames. Direction is bound
/// by the dedicated request/response characteristic, not inferred from data.
pub fn encode_frames(
    direction: Direction,
    transfer_id: [u8; 8],
    envelope: &SignedEnvelope,
    att_mtu: usize,
) -> Result<Vec<Vec<u8>>, TransportError> {
    if envelope.direction != direction {
        return Err(TransportError::DirectionMismatch);
    }
    encode_payload_frames(&envelope.as_bytes(), transfer_id, att_mtu)
}

fn encode_payload_frames(
    bytes: &[u8],
    transfer_id: [u8; 8],
    att_mtu: usize,
) -> Result<Vec<Vec<u8>>, TransportError> {
    if !(MIN_ATT_MTU..=MAX_ATT_MTU).contains(&att_mtu) {
        return Err(TransportError::InvalidAttMtu);
    }
    let value_limit = att_mtu - ATT_WRITE_OVERHEAD;
    if value_limit <= FRAME_HEADER_BYTES {
        return Err(TransportError::FrameTooLarge);
    }
    if !(SIGNATURE_BYTES + 1..=MAX_ENVELOPE_BYTES).contains(&bytes.len()) {
        return Err(TransportError::MessageTooLarge);
    }
    let payload_limit = value_limit - FRAME_HEADER_BYTES;
    let frame_count = bytes.len().div_ceil(payload_limit);
    if frame_count > MAX_FRAME_COUNT || frame_count > (u8::MAX as usize + 1) {
        return Err(TransportError::MessageTooLarge);
    }
    let total = u16::try_from(bytes.len()).map_err(|_| TransportError::MessageTooLarge)?;
    Ok(bytes
        .chunks(payload_limit)
        .enumerate()
        .map(|(index, payload)| {
            let start = index == 0;
            let end = index + 1 == frame_count;
            let flags = (if start { FLAG_START } else { 0 }) | (if end { FLAG_END } else { 0 });
            let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
            frame.extend_from_slice(&MAGIC);
            frame.push(PROTOCOL_VERSION);
            frame.extend_from_slice(&transfer_id);
            frame.push(index as u8);
            frame.push(flags);
            frame.extend_from_slice(&(if start { total } else { 0 }).to_le_bytes());
            frame.extend_from_slice(payload);
            frame
        })
        .collect())
}

/// Reassembly bound to one BlueZ client path, one direction and one transfer
/// id. The last frame only marks byte completeness; `complete()` is a separate
/// explicit commit command.
#[derive(Debug, Clone)]
pub struct TransferAssembler {
    transfer_id: [u8; 8],
    direction: Direction,
    owner: String,
    total_len: Option<usize>,
    next_sequence: u16,
    bytes: Vec<u8>,
    deadline: PhaseDeadline,
    byte_complete: bool,
    committed: bool,
    cancelled: bool,
}

impl TransferAssembler {
    pub fn new(
        transfer_id: [u8; 8],
        direction: Direction,
        owner: impl Into<String>,
        created_at: Instant,
    ) -> Self {
        Self::new_with_timeout(
            transfer_id,
            direction,
            owner,
            created_at,
            RESPONSE_TRANSFER_TIMEOUT,
            Some(BLE_INACTIVITY_TIMEOUT),
        )
    }

    pub fn new_with_timeout(
        transfer_id: [u8; 8],
        direction: Direction,
        owner: impl Into<String>,
        started_at: Instant,
        absolute_timeout: Duration,
        idle_timeout: Option<Duration>,
    ) -> Self {
        Self {
            transfer_id,
            direction,
            owner: owner.into(),
            total_len: None,
            next_sequence: 0,
            bytes: Vec::new(),
            deadline: PhaseDeadline::new(started_at, absolute_timeout, idle_timeout),
            byte_complete: false,
            committed: false,
            cancelled: false,
        }
    }

    pub fn push(
        &mut self,
        owner: &str,
        direction: Direction,
        frame: &[u8],
        att_mtu: usize,
        now: Instant,
    ) -> Result<(), TransportError> {
        self.check_owner(owner)?;
        if direction != self.direction {
            return Err(TransportError::DirectionMismatch);
        }
        self.check_live(now)?;
        if !(MIN_ATT_MTU..=MAX_ATT_MTU).contains(&att_mtu) {
            return Err(TransportError::InvalidAttMtu);
        }
        if frame.len() > att_mtu - ATT_WRITE_OVERHEAD {
            return Err(TransportError::FrameTooLarge);
        }
        if frame.len() <= FRAME_HEADER_BYTES || frame[..2] != MAGIC {
            return Err(TransportError::Malformed);
        }
        if frame[2] != PROTOCOL_VERSION {
            return Err(TransportError::VersionMismatch);
        }
        if frame[3..11] != self.transfer_id {
            return Err(TransportError::SessionMismatch);
        }
        if self.byte_complete || self.committed {
            return Err(TransportError::AlreadyCompleted);
        }
        let sequence = frame[11];
        let flags = frame[12];
        let start = flags & FLAG_START != 0;
        let end = flags & FLAG_END != 0;
        let total_field = u16::from_le_bytes([frame[13], frame[14]]) as usize;
        let payload = &frame[FRAME_HEADER_BYTES..];
        if flags & !ALLOWED_FLAGS != 0 || payload.is_empty() {
            return Err(TransportError::Malformed);
        }
        if start {
            if self.total_len.is_some() {
                if (sequence as u16) < self.next_sequence {
                    return Err(TransportError::DuplicateOrOutOfOrder);
                }
                return Err(TransportError::MessageInProgress);
            }
            if sequence != 0 {
                return Err(TransportError::DuplicateOrOutOfOrder);
            }
            if !(SIGNATURE_BYTES + 1..=MAX_ENVELOPE_BYTES).contains(&total_field) {
                return Err(TransportError::MessageTooLarge);
            }
            if payload.len() > total_field {
                return Err(TransportError::LengthMismatch);
            }
            self.total_len = Some(total_field);
        } else {
            if self.total_len.is_none() {
                return Err(TransportError::StartRequired);
            }
            if total_field != 0 {
                return Err(TransportError::Malformed);
            }
        }
        if sequence as u16 != self.next_sequence {
            return Err(TransportError::DuplicateOrOutOfOrder);
        }
        let total = self.total_len.expect("validated start frame has a total");
        if self.bytes.len().saturating_add(payload.len()) > total {
            return Err(TransportError::LengthMismatch);
        }
        let reached_total = self.bytes.len().saturating_add(payload.len()) == total;
        if end != reached_total {
            return Err(TransportError::LengthMismatch);
        }
        self.bytes.extend_from_slice(payload);
        self.next_sequence += 1;
        if reached_total {
            SignedEnvelope::from_bytes(direction, &self.bytes)?;
            self.byte_complete = true;
        }
        self.deadline.touch(now);
        Ok(())
    }

    pub fn complete(
        &mut self,
        owner: &str,
        transfer_id: [u8; 8],
        now: Instant,
    ) -> Result<SignedEnvelope, TransportError> {
        self.check_owner(owner)?;
        self.check_live(now)?;
        if transfer_id != self.transfer_id {
            return Err(TransportError::SessionMismatch);
        }
        if self.committed {
            return Err(TransportError::DuplicateCompletion);
        }
        if !self.byte_complete {
            return Err(TransportError::IncompleteTransfer);
        }
        self.committed = true;
        SignedEnvelope::from_bytes(self.direction, &self.bytes)
    }

    pub fn cancel(&mut self, owner: &str, transfer_id: [u8; 8]) -> Result<(), TransportError> {
        self.check_owner(owner)?;
        if transfer_id != self.transfer_id {
            return Err(TransportError::SessionMismatch);
        }
        if self.committed {
            return Err(TransportError::AlreadyCompleted);
        }
        self.cancelled = true;
        self.bytes.clear();
        Ok(())
    }

    pub fn is_byte_complete(&self) -> bool {
        self.byte_complete
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    fn check_owner(&self, owner: &str) -> Result<(), TransportError> {
        if owner != self.owner {
            Err(TransportError::OwnerMismatch)
        } else {
            Ok(())
        }
    }

    fn check_live(&self, now: Instant) -> Result<(), TransportError> {
        if self.cancelled {
            return Err(TransportError::Cancelled);
        }
        if self.committed {
            return Err(TransportError::AlreadyCompleted);
        }
        self.deadline.check(now)
    }
}

/// A CoreBluetooth-compatible request mailbox. The Watch writes GET(index),
/// reads one fixed-size frame, then explicitly writes ACK(index). Repeated GET
/// for the in-flight index returns identical bytes; reads never advance state.
#[derive(Debug, Clone)]
pub struct RequestChunkMailbox {
    transfer_id: [u8; 8],
    owner: String,
    frames: Vec<Vec<u8>>,
    expected_index: usize,
    requested_index: Option<usize>,
    deadline: PhaseDeadline,
    completed: bool,
    cancelled: bool,
}

impl RequestChunkMailbox {
    pub fn new(
        transfer_id: [u8; 8],
        owner: impl Into<String>,
        envelope: &SignedEnvelope,
        att_mtu: usize,
        now: Instant,
    ) -> Result<Self, TransportError> {
        Self::new_with_timeout(
            transfer_id,
            owner,
            envelope,
            att_mtu,
            now,
            REQUEST_TRANSFER_TIMEOUT,
            Some(BLE_INACTIVITY_TIMEOUT),
        )
    }

    pub fn new_with_timeout(
        transfer_id: [u8; 8],
        owner: impl Into<String>,
        envelope: &SignedEnvelope,
        att_mtu: usize,
        started_at: Instant,
        absolute_timeout: Duration,
        idle_timeout: Option<Duration>,
    ) -> Result<Self, TransportError> {
        Ok(Self {
            transfer_id,
            owner: owner.into(),
            frames: encode_frames(Direction::HostToWatch, transfer_id, envelope, att_mtu)?,
            expected_index: 0,
            requested_index: None,
            deadline: PhaseDeadline::new(started_at, absolute_timeout, idle_timeout),
            completed: false,
            cancelled: false,
        })
    }

    pub fn get(
        &mut self,
        owner: &str,
        transfer_id: [u8; 8],
        index: usize,
        now: Instant,
    ) -> Result<&[u8], TransportError> {
        self.check(owner, transfer_id, now)?;
        if index != self.expected_index || index >= self.frames.len() {
            return Err(TransportError::DuplicateOrOutOfOrder);
        }
        if self
            .requested_index
            .is_some_and(|requested| requested != index)
        {
            return Err(TransportError::DuplicateOrOutOfOrder);
        }
        self.requested_index = Some(index);
        self.deadline.touch(now);
        Ok(&self.frames[index])
    }

    pub fn acknowledge(
        &mut self,
        owner: &str,
        transfer_id: [u8; 8],
        index: usize,
        now: Instant,
    ) -> Result<(), TransportError> {
        self.check(owner, transfer_id, now)?;
        if self.requested_index != Some(index) || index != self.expected_index {
            return Err(TransportError::DuplicateOrOutOfOrder);
        }
        self.expected_index += 1;
        self.requested_index = None;
        self.deadline.touch(now);
        Ok(())
    }

    pub fn complete(
        &mut self,
        owner: &str,
        transfer_id: [u8; 8],
        now: Instant,
    ) -> Result<(), TransportError> {
        if self.completed {
            return Err(TransportError::DuplicateCompletion);
        }
        self.check(owner, transfer_id, now)?;
        if self.expected_index != self.frames.len() || self.requested_index.is_some() {
            return Err(TransportError::IncompleteTransfer);
        }
        self.completed = true;
        self.deadline.touch(now);
        Ok(())
    }

    pub fn cancel(&mut self, owner: &str, transfer_id: [u8; 8]) -> Result<(), TransportError> {
        if owner != self.owner {
            return Err(TransportError::OwnerMismatch);
        }
        if transfer_id != self.transfer_id {
            return Err(TransportError::SessionMismatch);
        }
        if self.completed {
            return Err(TransportError::AlreadyCompleted);
        }
        self.cancelled = true;
        self.frames.clear();
        Ok(())
    }

    pub fn expected_index(&self) -> usize {
        self.expected_index
    }

    pub fn validate_live(
        &self,
        owner: &str,
        transfer_id: [u8; 8],
        now: Instant,
    ) -> Result<(), TransportError> {
        self.check(owner, transfer_id, now)
    }

    fn check(&self, owner: &str, transfer_id: [u8; 8], now: Instant) -> Result<(), TransportError> {
        if owner != self.owner {
            return Err(TransportError::OwnerMismatch);
        }
        if transfer_id != self.transfer_id {
            return Err(TransportError::SessionMismatch);
        }
        if self.cancelled {
            return Err(TransportError::Cancelled);
        }
        if self.completed {
            return Err(TransportError::AlreadyCompleted);
        }
        self.deadline.check(now)
    }
}

/// Fixed-size control values for request retrieval. The 12-byte GET/ACK values
/// fit every BLE ATT MTU. Completion/cancellation also name the transfer id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestCommand {
    Get { index: u8 },
    Ack { index: u8 },
    Complete,
    Cancel,
}

pub fn encode_request_command(transfer_id: [u8; 8], command: RequestCommand) -> Vec<u8> {
    let (opcode, index) = match command {
        RequestCommand::Get { index } => (1, index),
        RequestCommand::Ack { index } => (2, index),
        RequestCommand::Complete => (3, 0),
        RequestCommand::Cancel => (4, 0),
    };
    [
        vec![b'O', b'A', PROTOCOL_VERSION, opcode],
        transfer_id.to_vec(),
        vec![index],
    ]
    .concat()
}

pub fn parse_request_command(bytes: &[u8]) -> Result<([u8; 8], RequestCommand), TransportError> {
    if bytes.len() != 13 || bytes[..2] != MAGIC || bytes[2] != PROTOCOL_VERSION {
        return Err(TransportError::InvalidChunkCommand);
    }
    let mut transfer_id = [0; 8];
    transfer_id.copy_from_slice(&bytes[4..12]);
    let command = match (bytes[3], bytes[12]) {
        (1, index) => RequestCommand::Get { index },
        (2, index) => RequestCommand::Ack { index },
        (3, 0) => RequestCommand::Complete,
        (4, 0) => RequestCommand::Cancel,
        _ => return Err(TransportError::InvalidChunkCommand),
    };
    Ok((transfer_id, command))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watch_auth_research::{
        ACTION_RESEARCH_CONFIRMATION, ConsentResult, Decision, HostAuthRequest, SESSION_ID_BYTES,
        WatchAuthResponse,
    };

    const NOW: u64 = 1_800_000_000_600;
    const OWNER: &str = "/org/bluez/hci0/dev_01";
    const TRANSFER_ID: [u8; 8] = [0xa5; 8];

    fn request_bytes() -> Vec<u8> {
        HostAuthRequest {
            host_enrollment_id: "host-test".into(),
            host_identity: "linux-test".into(),
            linux_uid: 1000,
            target_watch_enrollment_id: "watch-test".into(),
            session_id: [0x31; SESSION_ID_BYTES],
            nonce: [0x52; 32],
            issued_at_ms: NOW - 100,
            expires_at_ms: NOW + 10_000,
            action: ACTION_RESEARCH_CONFIRMATION.into(),
        }
        .canonical_bytes()
        .unwrap()
    }

    fn response_bytes() -> Vec<u8> {
        WatchAuthResponse {
            request_digest: [0x11; 32],
            nonce: [0x52; 32],
            host_enrollment_id: "host-test".into(),
            host_identity: "linux-test".into(),
            linux_uid: 1000,
            target_watch_enrollment_id: "watch-test".into(),
            watch_identity: "watch-test".into(),
            session_id: [0x31; SESSION_ID_BYTES],
            action: ACTION_RESEARCH_CONFIRMATION.into(),
            decision: Decision::Rejected,
            response_at_ms: NOW,
            consent_result: ConsentResult::ExplicitUserDecision,
        }
        .canonical_bytes()
        .unwrap()
    }

    fn envelope(direction: Direction) -> SignedEnvelope {
        let canonical = match direction {
            Direction::HostToWatch => request_bytes(),
            Direction::WatchToHost => response_bytes(),
        };
        SignedEnvelope::new(direction, &canonical, &[0x77; SIGNATURE_BYTES]).unwrap()
    }

    fn reassemble(direction: Direction, bytes: &[u8]) -> SignedEnvelope {
        let signed = SignedEnvelope::from_bytes(direction, bytes).unwrap();
        let frames = encode_frames(direction, TRANSFER_ID, &signed, MIN_ATT_MTU).unwrap();
        let now = Instant::now();
        let mut assembler = TransferAssembler::new(TRANSFER_ID, direction, OWNER, now);
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame[11] as usize, index);
            assembler
                .push(OWNER, direction, frame, MIN_ATT_MTU, now)
                .unwrap();
        }
        assembler.complete(OWNER, TRANSFER_ID, now).unwrap()
    }

    #[test]
    fn signed_request_and_response_envelopes_round_trip_with_real_frames() {
        for direction in [Direction::HostToWatch, Direction::WatchToHost] {
            let expected = envelope(direction);
            let actual = reassemble(direction, &expected.as_bytes());
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn maximum_envelope_uses_218_conservative_frames() {
        assert_eq!(FRAME_HEADER_BYTES, 15);
        assert_eq!(CONSERVATIVE_FRAME_PAYLOAD_BYTES, 5);
        assert_eq!(MAX_FRAME_COUNT, 218);
        // P008's current canonical schema cannot naturally fill all 1,024
        // bytes. Exercise the shared low-level encoder at the transport cap;
        // these opaque bytes are not interpreted as an authentication message.
        let opaque = vec![0x5a; MAX_ENVELOPE_BYTES];
        let frames = encode_payload_frames(&opaque, TRANSFER_ID, MIN_ATT_MTU).unwrap();
        assert_eq!(frames.len(), 218);
        assert!(frames.iter().all(|frame| frame.len() <= 20));
    }

    #[test]
    fn wrong_version_size_order_duplicates_owner_session_and_direction_fail() {
        let signed = envelope(Direction::HostToWatch);
        let frames =
            encode_frames(Direction::HostToWatch, TRANSFER_ID, &signed, MIN_ATT_MTU).unwrap();
        let now = Instant::now();
        let mut wrong_version = frames[0].clone();
        wrong_version[2] = 2;
        assert_eq!(
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now).push(
                OWNER,
                Direction::HostToWatch,
                &wrong_version,
                MIN_ATT_MTU,
                now
            ),
            Err(TransportError::VersionMismatch)
        );
        assert_eq!(
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now).push(
                OWNER,
                Direction::HostToWatch,
                &[0; 20],
                MIN_ATT_MTU,
                now
            ),
            Err(TransportError::Malformed)
        );
        assert_eq!(
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now).push(
                OWNER,
                Direction::WatchToHost,
                &frames[0],
                MIN_ATT_MTU,
                now
            ),
            Err(TransportError::DirectionMismatch)
        );
        assert_eq!(
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now).push(
                "/org/bluez/hci0/dev_02",
                Direction::HostToWatch,
                &frames[0],
                MIN_ATT_MTU,
                now
            ),
            Err(TransportError::OwnerMismatch)
        );
        let mut wrong_session = frames[0].clone();
        wrong_session[3] ^= 1;
        assert_eq!(
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now).push(
                OWNER,
                Direction::HostToWatch,
                &wrong_session,
                MIN_ATT_MTU,
                now
            ),
            Err(TransportError::SessionMismatch)
        );
        let mut out_of_order =
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now);
        assert_eq!(
            out_of_order.push(OWNER, Direction::HostToWatch, &frames[1], MIN_ATT_MTU, now),
            Err(TransportError::StartRequired)
        );
        let mut duplicate = TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now);
        duplicate
            .push(OWNER, Direction::HostToWatch, &frames[0], MIN_ATT_MTU, now)
            .unwrap();
        assert_eq!(
            duplicate.push(OWNER, Direction::HostToWatch, &frames[0], MIN_ATT_MTU, now),
            Err(TransportError::DuplicateOrOutOfOrder)
        );
    }

    #[test]
    fn missing_bytes_bad_total_timeout_and_incomplete_commit_fail_closed() {
        let signed = envelope(Direction::HostToWatch);
        let frames =
            encode_frames(Direction::HostToWatch, TRANSFER_ID, &signed, MIN_ATT_MTU).unwrap();
        let now = Instant::now();
        let mut assembler = TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now);
        let mut expired_before_first_frame =
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now);
        assert_eq!(
            expired_before_first_frame.push(
                OWNER,
                Direction::HostToWatch,
                &frames[0],
                MIN_ATT_MTU,
                now + TRANSFER_TIMEOUT + Duration::from_millis(1)
            ),
            Err(TransportError::TransferExpired)
        );
        assert_eq!(
            assembler.complete(OWNER, TRANSFER_ID, now),
            Err(TransportError::IncompleteTransfer)
        );
        assembler
            .push(OWNER, Direction::HostToWatch, &frames[0], MIN_ATT_MTU, now)
            .unwrap();
        assert_eq!(
            assembler.complete(OWNER, TRANSFER_ID, now),
            Err(TransportError::IncompleteTransfer)
        );
        assert_eq!(
            assembler.push(
                OWNER,
                Direction::HostToWatch,
                &frames[1],
                MIN_ATT_MTU,
                now + TRANSFER_TIMEOUT + Duration::from_millis(1)
            ),
            Err(TransportError::TransferExpired)
        );
        let mut too_long = frames[0].clone();
        too_long[13..15].copy_from_slice(&((MAX_ENVELOPE_BYTES + 1) as u16).to_le_bytes());
        assert_eq!(
            TransferAssembler::new(TRANSFER_ID, Direction::HostToWatch, OWNER, now).push(
                OWNER,
                Direction::HostToWatch,
                &too_long,
                MIN_ATT_MTU,
                now
            ),
            Err(TransportError::MessageTooLarge)
        );
    }

    #[test]
    fn monotonic_phase_deadlines_bound_idle_and_long_active_transfers() {
        let start = Instant::now();
        let mut deadline = PhaseDeadline::new(
            start,
            REQUEST_TRANSFER_TIMEOUT,
            Some(BLE_INACTIVITY_TIMEOUT),
        );
        assert_eq!(deadline.check(start + Duration::from_secs(7)), Ok(()));
        deadline.touch(start + Duration::from_secs(7));
        assert_eq!(deadline.check(start + Duration::from_secs(14)), Ok(()));
        deadline.touch(start + Duration::from_secs(14));
        assert_eq!(
            deadline.check(start + Duration::from_secs(20)),
            Err(TransportError::TransferExpired)
        );

        let idle = PhaseDeadline::new(
            start,
            REQUEST_TRANSFER_TIMEOUT,
            Some(BLE_INACTIVITY_TIMEOUT),
        );
        assert_eq!(
            idle.check(start + BLE_INACTIVITY_TIMEOUT),
            Err(TransportError::InactivityExpired)
        );
    }

    #[test]
    fn response_transfer_started_late_gets_its_own_bounded_window() {
        let signed = envelope(Direction::WatchToHost);
        let frames =
            encode_frames(Direction::WatchToHost, TRANSFER_ID, &signed, MIN_ATT_MTU).unwrap();
        let ready = Instant::now();
        let response_start = ready + WAITING_FOR_CONSENT_TIMEOUT - Duration::from_millis(1);
        let mut assembler = TransferAssembler::new_with_timeout(
            TRANSFER_ID,
            Direction::WatchToHost,
            OWNER,
            response_start,
            RESPONSE_TRANSFER_TIMEOUT,
            Some(BLE_INACTIVITY_TIMEOUT),
        );
        let mut now = response_start;
        for frame in &frames {
            assembler
                .push(OWNER, Direction::WatchToHost, frame, MIN_ATT_MTU, now)
                .unwrap();
            now += Duration::from_millis(1);
        }
        assert_eq!(assembler.complete(OWNER, TRANSFER_ID, now).unwrap(), signed);
    }

    #[test]
    fn request_get_ack_progress_cannot_extend_the_absolute_transfer_deadline() {
        let signed = envelope(Direction::HostToWatch);
        let start = Instant::now();
        let mut mailbox = RequestChunkMailbox::new_with_timeout(
            TRANSFER_ID,
            OWNER,
            &signed,
            MIN_ATT_MTU,
            start,
            REQUEST_TRANSFER_TIMEOUT,
            Some(BLE_INACTIVITY_TIMEOUT),
        )
        .unwrap();
        for index in 0..2 {
            let at = start + Duration::from_secs(7 * (index as u64 + 1));
            mailbox.get(OWNER, TRANSFER_ID, index, at).unwrap();
            mailbox.acknowledge(OWNER, TRANSFER_ID, index, at).unwrap();
        }
        assert_eq!(
            mailbox.get(OWNER, TRANSFER_ID, 2, start + REQUEST_TRANSFER_TIMEOUT),
            Err(TransportError::TransferExpired)
        );
    }

    #[test]
    fn completion_is_explicit_single_use_and_cancel_discards_bytes() {
        let signed = envelope(Direction::WatchToHost);
        let frames =
            encode_frames(Direction::WatchToHost, TRANSFER_ID, &signed, MIN_ATT_MTU).unwrap();
        let now = Instant::now();
        let mut assembler = TransferAssembler::new(TRANSFER_ID, Direction::WatchToHost, OWNER, now);
        for frame in &frames {
            assembler
                .push(OWNER, Direction::WatchToHost, frame, MIN_ATT_MTU, now)
                .unwrap();
        }
        assert_eq!(assembler.complete(OWNER, TRANSFER_ID, now).unwrap(), signed);
        assert_eq!(
            assembler.complete(OWNER, TRANSFER_ID, now),
            Err(TransportError::AlreadyCompleted)
        );
        let mut cancelled = TransferAssembler::new(TRANSFER_ID, Direction::WatchToHost, OWNER, now);
        cancelled.cancel(OWNER, TRANSFER_ID).unwrap();
        assert_eq!(
            cancelled.push(OWNER, Direction::WatchToHost, &frames[0], MIN_ATT_MTU, now),
            Err(TransportError::Cancelled)
        );
    }

    #[test]
    fn explicit_chunk_get_ack_is_idempotent_until_ack_and_then_ordered() {
        let signed = envelope(Direction::HostToWatch);
        let now = Instant::now();
        let mut mailbox =
            RequestChunkMailbox::new(TRANSFER_ID, OWNER, &signed, MIN_ATT_MTU, now).unwrap();
        let first = mailbox.get(OWNER, TRANSFER_ID, 0, now).unwrap().to_vec();
        assert_eq!(mailbox.get(OWNER, TRANSFER_ID, 0, now).unwrap(), first);
        assert_eq!(
            mailbox.get(OWNER, TRANSFER_ID, 1, now),
            Err(TransportError::DuplicateOrOutOfOrder)
        );
        assert_eq!(
            mailbox.acknowledge(OWNER, TRANSFER_ID, 1, now),
            Err(TransportError::DuplicateOrOutOfOrder)
        );
        mailbox.acknowledge(OWNER, TRANSFER_ID, 0, now).unwrap();
        assert_eq!(
            mailbox.get(OWNER, TRANSFER_ID, 0, now),
            Err(TransportError::DuplicateOrOutOfOrder)
        );
        assert_eq!(
            mailbox.complete(OWNER, TRANSFER_ID, now),
            Err(TransportError::IncompleteTransfer)
        );
        for index in 1..mailbox.frames.len() {
            mailbox.get(OWNER, TRANSFER_ID, index, now).unwrap();
            mailbox.acknowledge(OWNER, TRANSFER_ID, index, now).unwrap();
        }
        mailbox.complete(OWNER, TRANSFER_ID, now).unwrap();
        assert_eq!(
            mailbox.complete(OWNER, TRANSFER_ID, now),
            Err(TransportError::DuplicateCompletion)
        );
    }

    #[test]
    fn fixed_chunk_controls_reject_malformed_and_downgrade_values() {
        let command = RequestCommand::Get { index: 217 };
        let bytes = encode_request_command(TRANSFER_ID, command);
        assert_eq!(bytes.len(), 13);
        assert_eq!(parse_request_command(&bytes), Ok((TRANSFER_ID, command)));
        let mut bad = bytes;
        bad[2] = 0;
        assert_eq!(
            parse_request_command(&bad),
            Err(TransportError::InvalidChunkCommand)
        );
        assert_eq!(
            parse_request_command(&[0; 12]),
            Err(TransportError::InvalidChunkCommand)
        );
    }
}
