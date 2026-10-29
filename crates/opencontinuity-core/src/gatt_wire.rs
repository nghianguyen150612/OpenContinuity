//! Versioned, bounded framing for the non-authorizing BlueZ transport experiment.
//!
//! This module contains no Bluetooth or operating-system I/O. The frame format
//! is intentionally independent of MTU: callers supply the negotiated ATT MTU
//! and each frame is checked against its `MTU - 3` attribute payload limit.

use std::time::{Duration, Instant};

pub const WIRE_VERSION: u8 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1024;
pub const MIN_ATT_MTU: usize = 23;
pub const MAX_ATT_MTU: usize = 517;
pub const ATT_WRITE_OVERHEAD: usize = 3;
pub const FRAME_HEADER_BYTES: usize = 15;
pub const PARTIAL_MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);
const MAGIC: [u8; 2] = *b"OC";
const FLAG_START: u8 = 0x01;
const FLAG_END: u8 = 0x02;
const ALLOWED_FLAGS: u8 = FLAG_START | FLAG_END;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    InvalidAttMtu,
    FrameTooLarge,
    Malformed,
    VersionMismatch,
    SessionMismatch,
    DuplicateOrOutOfOrder,
    StartRequired,
    MessageInProgress,
    MessageTooLarge,
    LengthMismatch,
    PartialMessageExpired,
    AlreadyComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    Complete,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameProgress {
    Partial { received: usize, total: usize },
    Complete(Vec<u8>),
}

/// One per connected BLE device. Callers must not share an assembler between
/// clients, even if two clients send the same frame sequence.
#[derive(Debug, Clone)]
pub struct FrameAssembler {
    session_id: [u8; 8],
    total_len: Option<usize>,
    next_sequence: u8,
    message: Vec<u8>,
    last_frame_at: Option<Instant>,
    complete: bool,
}

impl FrameAssembler {
    pub fn new(session_id: [u8; 8]) -> Self {
        Self {
            session_id,
            total_len: None,
            next_sequence: 0,
            message: Vec::new(),
            last_frame_at: None,
            complete: false,
        }
    }

    pub fn session_id(&self) -> [u8; 8] {
        self.session_id
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn completed_payload(&self) -> Option<&[u8]> {
        self.complete.then_some(self.message.as_slice())
    }

    pub fn reset(&mut self, session_id: [u8; 8]) {
        *self = Self::new(session_id);
    }

    pub fn push(
        &mut self,
        frame: &[u8],
        att_mtu: usize,
        now: Instant,
    ) -> Result<FrameProgress, WireError> {
        if !(MIN_ATT_MTU..=MAX_ATT_MTU).contains(&att_mtu) {
            return Err(WireError::InvalidAttMtu);
        }
        if frame.len() > att_mtu - ATT_WRITE_OVERHEAD {
            return Err(WireError::FrameTooLarge);
        }
        if frame.len() < FRAME_HEADER_BYTES || frame[..2] != MAGIC {
            return Err(WireError::Malformed);
        }
        if frame[2] != WIRE_VERSION {
            return Err(WireError::VersionMismatch);
        }
        if frame[3..11] != self.session_id {
            return Err(WireError::SessionMismatch);
        }
        if self.complete {
            return Err(WireError::AlreadyComplete);
        }
        let sequence = frame[11];
        let flags = frame[12];
        let start = flags & FLAG_START != 0;
        let end = flags & FLAG_END != 0;
        let total_field = u16::from_le_bytes([frame[13], frame[14]]) as usize;
        let payload = &frame[FRAME_HEADER_BYTES..];

        if flags & !ALLOWED_FLAGS != 0 || payload.is_empty() {
            return Err(WireError::Malformed);
        }
        if let Some(last) = self.last_frame_at {
            if now.saturating_duration_since(last) > PARTIAL_MESSAGE_TIMEOUT {
                return Err(WireError::PartialMessageExpired);
            }
        }

        if start {
            if self.total_len.is_some() {
                return Err(WireError::MessageInProgress);
            }
            if sequence != 0 {
                return Err(WireError::DuplicateOrOutOfOrder);
            }
            if !(1..=MAX_MESSAGE_BYTES).contains(&total_field) {
                return Err(WireError::MessageTooLarge);
            }
            if payload.len() > total_field {
                return Err(WireError::LengthMismatch);
            }
            self.total_len = Some(total_field);
        } else {
            if self.total_len.is_none() {
                return Err(WireError::StartRequired);
            }
            if total_field != 0 {
                return Err(WireError::Malformed);
            }
            if sequence != self.next_sequence {
                return Err(WireError::DuplicateOrOutOfOrder);
            }
        }

        let total = self.total_len.expect("start validated total length");
        if self.message.len().saturating_add(payload.len()) > total {
            return Err(WireError::LengthMismatch);
        }
        self.message.extend_from_slice(payload);
        self.last_frame_at = Some(now);
        if end {
            if self.message.len() != total {
                return Err(WireError::LengthMismatch);
            }
            self.complete = true;
            return Ok(FrameProgress::Complete(self.message.clone()));
        }
        if self.message.len() == total {
            return Err(WireError::LengthMismatch);
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(WireError::MessageTooLarge)?;
        Ok(FrameProgress::Partial {
            received: self.message.len(),
            total,
        })
    }

    pub fn partial_expired(&self, now: Instant) -> bool {
        !self.complete
            && self
                .last_frame_at
                .is_some_and(|last| now.saturating_duration_since(last) > PARTIAL_MESSAGE_TIMEOUT)
    }

    pub fn check_control(&self, value: &[u8]) -> Result<ControlCommand, WireError> {
        if value.len() != 12 || value[..2] != MAGIC {
            return Err(WireError::Malformed);
        }
        if value[2] != WIRE_VERSION {
            return Err(WireError::VersionMismatch);
        }
        if value[3..11] != self.session_id {
            return Err(WireError::SessionMismatch);
        }
        match value[11] {
            1 if self.complete => Ok(ControlCommand::Complete),
            1 => Err(WireError::StartRequired),
            2 => Ok(ControlCommand::Cancel),
            _ => Err(WireError::Malformed),
        }
    }
}

pub fn encode_control(session_id: [u8; 8], command: ControlCommand) -> [u8; 12] {
    let mut value = [0; 12];
    value[..2].copy_from_slice(&MAGIC);
    value[2] = WIRE_VERSION;
    value[3..11].copy_from_slice(&session_id);
    value[11] = match command {
        ControlCommand::Complete => 1,
        ControlCommand::Cancel => 2,
    };
    value
}

/// Build one wire frame. Intended for protocol tests and clients; `att_mtu`
/// is enforced by `FrameAssembler::push` at the receiver.
pub fn encode_frame(
    session_id: [u8; 8],
    sequence: u8,
    start: bool,
    end: bool,
    total_len: usize,
    payload: &[u8],
) -> Result<Vec<u8>, WireError> {
    if payload.is_empty()
        || payload.len() > u16::MAX as usize
        || (start && (sequence != 0 || !(1..=u16::MAX as usize).contains(&total_len)))
        || (!start && total_len != 0)
    {
        return Err(WireError::Malformed);
    }
    let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(WIRE_VERSION);
    frame.extend_from_slice(&session_id);
    frame.push(sequence);
    frame.push((u8::from(start) * FLAG_START) | (u8::from(end) * FLAG_END));
    frame.extend_from_slice(&(total_len as u16).to_le_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    fn test_frame(
        id: [u8; 8],
        seq: u8,
        start: bool,
        end: bool,
        total: usize,
        payload: &[u8],
    ) -> Vec<u8> {
        encode_frame(id, seq, start, end, total, payload).unwrap()
    }

    #[test]
    fn mtu_and_sequence_limits_are_checked() {
        let id = *b"session1";
        let now = Instant::now();
        let frame = test_frame(id, 0, true, true, 1, b"x");
        assert_eq!(
            FrameAssembler::new(id).push(&frame, 22, now),
            Err(WireError::InvalidAttMtu)
        );
        assert_eq!(
            FrameAssembler::new(id).push(&frame, 518, now),
            Err(WireError::InvalidAttMtu)
        );
        let mut assembler = FrameAssembler::new(id);
        for sequence in 0..=255u8 {
            let frame = test_frame(
                id,
                sequence,
                sequence == 0,
                sequence == 255,
                if sequence == 0 { 256 } else { 0 },
                b"x",
            );
            let result = assembler.push(&frame, 23, now);
            if sequence == 255 {
                assert!(matches!(result, Ok(FrameProgress::Complete(_))));
            } else {
                assert!(matches!(result, Ok(FrameProgress::Partial { .. })));
            }
        }
        let mut assembler = FrameAssembler::new(id);
        for sequence in 0..=255u8 {
            let frame = test_frame(
                id,
                sequence,
                sequence == 0,
                false,
                if sequence == 0 { 257 } else { 0 },
                b"x",
            );
            let result = assembler.push(&frame, 23, now);
            if sequence == 255 {
                assert_eq!(result, Err(WireError::MessageTooLarge));
            } else {
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn supports_minimum_mtu_and_reassembles_ordered_fragments() {
        let id = *b"session1";
        let now = Instant::now();
        let mut assembler = FrameAssembler::new(id);
        let first = test_frame(id, 0, true, false, 10, b"hello");
        assert_eq!(first.len(), 20);
        assert_eq!(
            assembler.push(&first, MIN_ATT_MTU, now),
            Ok(FrameProgress::Partial {
                received: 5,
                total: 10
            })
        );
        let second = test_frame(id, 1, false, true, 0, b" rust");
        assert_eq!(second.len(), 20);
        assert_eq!(
            assembler.push(&second, MIN_ATT_MTU, now + Duration::from_secs(1)),
            Ok(FrameProgress::Complete(b"hello rust".to_vec()))
        );
    }

    #[test]
    fn rejects_bad_order_duplicates_sessions_versions_and_malformed_frames() {
        let id = *b"session1";
        let now = Instant::now();
        let mut assembler = FrameAssembler::new(id);
        assert_eq!(
            assembler.push(&[0; FRAME_HEADER_BYTES], 23, now),
            Err(WireError::Malformed)
        );
        let wrong_version = {
            let mut frame = test_frame(id, 0, true, true, 1, b"a");
            frame[2] = 2;
            frame
        };
        assert_eq!(
            assembler.push(&wrong_version, 23, now),
            Err(WireError::VersionMismatch)
        );
        let wrong_session = test_frame(*b"session2", 0, true, true, 1, b"a");
        assert_eq!(
            assembler.push(&wrong_session, 23, now),
            Err(WireError::SessionMismatch)
        );
        let out_of_order = test_frame(id, 1, false, true, 0, b"a");
        assert_eq!(
            assembler.push(&out_of_order, 23, now),
            Err(WireError::StartRequired)
        );
        let first = test_frame(id, 0, true, false, 6, b"abc");
        assert!(matches!(
            assembler.push(&first, 23, now),
            Ok(FrameProgress::Partial { .. })
        ));
        assert_eq!(
            assembler.push(&first, 23, now),
            Err(WireError::MessageInProgress)
        );
        let duplicate = test_frame(id, 0, false, false, 0, b"d");
        assert_eq!(
            assembler.push(&duplicate, 23, now),
            Err(WireError::DuplicateOrOutOfOrder)
        );
    }

    #[test]
    fn enforces_total_length_mtu_timeout_and_completion_control() {
        let id = *b"session1";
        let now = Instant::now();
        let oversized = test_frame(id, 0, true, false, 1, b"012345678901");
        assert_eq!(
            FrameAssembler::new(id).push(&oversized, 23, now),
            Err(WireError::FrameTooLarge)
        );
        let huge = test_frame(id, 0, true, false, 1025, b"a");
        assert_eq!(
            FrameAssembler::new(id).push(&huge, 247, now),
            Err(WireError::MessageTooLarge)
        );

        let mut assembler = FrameAssembler::new(id);
        let first = test_frame(id, 0, true, false, 6, b"abc");
        assert!(matches!(
            assembler.push(&first, 23, now),
            Ok(FrameProgress::Partial { .. })
        ));
        let second = test_frame(id, 1, false, true, 0, b"def");
        assert_eq!(
            assembler.push(
                &second,
                23,
                now + PARTIAL_MESSAGE_TIMEOUT + Duration::from_millis(1)
            ),
            Err(WireError::PartialMessageExpired)
        );
        assert_eq!(
            assembler.push(&second, 23, now + Duration::from_secs(1)),
            Ok(FrameProgress::Complete(b"abcdef".to_vec()))
        );

        assembler.reset(id);
        let final_frame = test_frame(id, 0, true, true, 3, b"abc");
        assert_eq!(
            assembler.push(&final_frame, 23, now),
            Ok(FrameProgress::Complete(b"abc".to_vec()))
        );
        assert_eq!(
            assembler.check_control(&encode_control(id, ControlCommand::Complete)),
            Ok(ControlCommand::Complete)
        );
        assert_eq!(
            assembler.check_control(&encode_control(id, ControlCommand::Cancel)),
            Ok(ControlCommand::Cancel)
        );
        assert_eq!(
            assembler.push(&final_frame, 23, now),
            Err(WireError::AlreadyComplete)
        );
    }

    #[test]
    fn p007_shared_swift_fixture_matches_rust_wire_encoders() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../experiments/watchos/Tests/OpenContinuityWatchTests/Fixtures/gatt-v1.json"
        ))
        .unwrap();
        let from_hex = |value: &str| -> Vec<u8> {
            value
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    let text = std::str::from_utf8(pair).unwrap();
                    u8::from_str_radix(text, 16).unwrap()
                })
                .collect()
        };
        let id: [u8; 8] = from_hex(fixture["session_id"].as_str().unwrap())
            .try_into()
            .unwrap();
        let payload = from_hex(fixture["payload"].as_str().unwrap());
        let expected_frames: Vec<Vec<u8>> = fixture["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| from_hex(value.as_str().unwrap()))
            .collect();
        assert_eq!(
            encode_frame(id, 0, true, false, payload.len(), &payload[..5]).unwrap(),
            expected_frames[0]
        );
        assert_eq!(
            encode_frame(id, 1, false, true, 0, &payload[5..]).unwrap(),
            expected_frames[1]
        );
        assert_eq!(
            encode_control(id, ControlCommand::Complete).to_vec(),
            from_hex(fixture["complete"].as_str().unwrap())
        );
        assert_eq!(
            encode_control(id, ControlCommand::Cancel).to_vec(),
            from_hex(fixture["cancel"].as_str().unwrap())
        );
        assert_eq!(
            [
                vec![WIRE_VERSION, 0],
                id.to_vec(),
                (payload.len() as u16).to_le_bytes().to_vec(),
                sha2::Sha256::digest(&payload).to_vec()
            ]
            .concat(),
            from_hex(fixture["success"].as_str().unwrap())
        );
        assert_eq!(
            [
                vec![WIRE_VERSION, 2],
                id.to_vec(),
                (9u16).to_le_bytes().to_vec(),
                sha2::Sha256::digest(b"cancelled").to_vec()
            ]
            .concat(),
            from_hex(fixture["cancel_response"].as_str().unwrap())
        );
        assert_eq!(
            [vec![WIRE_VERSION, 0x80], id.to_vec(), vec![1]].concat(),
            from_hex(fixture["error"].as_str().unwrap())
        );
        assert_eq!(from_hex(fixture["challenge"].as_str().unwrap()).len(), 25);
    }
}
