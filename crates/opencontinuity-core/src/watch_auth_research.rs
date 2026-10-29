//! P-256 signed, fixture-only Watch approval protocol research.
//!
//! This module is intentionally separate from `gatt_wire` and the P003/P004
//! Ed25519 mocks. It performs no operating-system, Bluetooth, key-store, or
//! authorization I/O. All enrollment records accepted by this prototype are
//! explicitly test-only and in-memory.

use p256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt};

pub const PROTOCOL_NAME: &str = "OC-WATCH-AUTH-RESEARCH/1";
pub const PROTOCOL_VERSION: u8 = 1;
pub const ALGORITHM_ID: u8 = 1;
pub const ALGORITHM_NAME: &str = "p256_ecdsa_sha256";
pub const MAX_CANONICAL_MESSAGE_BYTES: usize = 1024;
pub const NONCE_BYTES: usize = 32;
pub const SESSION_ID_BYTES: usize = 16;
pub const PUBLIC_KEY_BYTES: usize = 65;
pub const SIGNATURE_BYTES: usize = 64;
pub const MAX_ID_BYTES: usize = 128;
pub const MAX_REQUEST_LIFETIME_MS: u64 = 60_000;
pub const MAX_REPLAY_ENTRIES: usize = 4096;
pub const ACTION_RESEARCH_CONFIRMATION: &str = "research_confirmation";

const MAGIC: [u8; 4] = *b"OCWA";
const REQUEST_KIND: u8 = 1;
const RESPONSE_KIND: u8 = 2;
const REQUEST_DOMAIN: &[u8] = b"OC-WATCH-AUTH-RESEARCH/1\0HOST_REQUEST\0";
const RESPONSE_DOMAIN: &[u8] = b"OC-WATCH-AUTH-RESEARCH/1\0WATCH_RESPONSE\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    Malformed,
    Oversized,
    InvalidUtf8,
    InvalidIdentifier,
    NonCanonical,
    UnsupportedVersion,
    UnsupportedAlgorithm,
    WrongMessageType,
    InvalidPublicKey,
    InvalidSignature,
    UnknownEnrollment,
    EnrollmentRevoked,
    EnrollmentInactive,
    NonTestEnrollment,
    BindingMismatch,
    WrongAction,
    InvalidTimestamp,
    NotYetValid,
    Expired,
    LifetimeTooLong,
    Replay,
    ReplayStoreFull,
    InvalidDecision,
    ConsentNotCompleted,
    EntropyUnavailable,
    InvalidFixture,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self {
            Self::Malformed => "malformed protocol message",
            Self::Oversized => "protocol message exceeds its size limit",
            Self::InvalidUtf8 => "identifier is not valid UTF-8",
            Self::InvalidIdentifier => "identifier has an invalid or noncanonical form",
            Self::NonCanonical => "message is not canonically encoded",
            Self::UnsupportedVersion => "unsupported protocol version",
            Self::UnsupportedAlgorithm => "unsupported signature algorithm",
            Self::WrongMessageType => "unexpected message type",
            Self::InvalidPublicKey => "invalid P-256 public key encoding",
            Self::InvalidSignature => "signature verification failed",
            Self::UnknownEnrollment => "enrollment is unknown",
            Self::EnrollmentRevoked => "enrollment is revoked",
            Self::EnrollmentInactive => "enrollment is inactive",
            Self::NonTestEnrollment => "enrollment is not marked test-only",
            Self::BindingMismatch => "message does not match its enrollment bindings",
            Self::WrongAction => "unsupported action",
            Self::InvalidTimestamp => "invalid timestamp interval",
            Self::NotYetValid => "request was issued in the future",
            Self::Expired => "request or response has expired",
            Self::LifetimeTooLong => "request lifetime exceeds the protocol limit",
            Self::Replay => "nonce or session has already been used",
            Self::ReplayStoreFull => "in-memory replay guard is full",
            Self::InvalidDecision => "invalid approval decision",
            Self::ConsentNotCompleted => "explicit foreground consent was not completed",
            Self::EntropyUnavailable => "operating-system random source failed",
            Self::InvalidFixture => "invalid or incomplete research fixture",
        };
        f.write_str(description)
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentState {
    Active,
    Suspended,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAlgorithm {
    P256EcdsaSha256,
    Ed25519,
    P384EcdsaSha384,
}

/// Independently supplied host trust anchor. No wire message can create one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostEnrollment {
    pub enrollment_id: String,
    pub host_identity: String,
    pub linux_uid: u32,
    pub public_key_sec1_hex: String,
    pub key_algorithm: KeyAlgorithm,
    pub key_version: u8,
    pub state: EnrollmentState,
    pub test_only: bool,
}

/// Independently supplied Watch trust anchor, pinned to one host/account pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchEnrollment {
    pub enrollment_id: String,
    pub watch_identity: String,
    pub bound_host_enrollment_id: String,
    pub bound_host_identity: String,
    pub bound_linux_uid: u32,
    pub public_key_sec1_hex: String,
    pub key_algorithm: KeyAlgorithm,
    pub key_version: u8,
    pub state: EnrollmentState,
    pub test_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAuthRequest {
    pub host_enrollment_id: String,
    pub host_identity: String,
    pub linux_uid: u32,
    pub target_watch_enrollment_id: String,
    pub session_id: [u8; SESSION_ID_BYTES],
    pub nonce: [u8; NONCE_BYTES],
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchAuthResponse {
    pub request_digest: [u8; 32],
    pub nonce: [u8; NONCE_BYTES],
    pub host_enrollment_id: String,
    pub host_identity: String,
    pub linux_uid: u32,
    pub target_watch_enrollment_id: String,
    pub watch_identity: String,
    pub session_id: [u8; SESSION_ID_BYTES],
    pub action: String,
    pub decision: Decision,
    pub response_at_ms: u64,
    pub consent_result: ConsentResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentResult {
    ExplicitUserDecision,
    Cancelled,
    Unsupported,
    LocalAuthenticationDenied,
}

/// Only constructed by a successful host verifier. It binds later user consent
/// to the exact authenticated request bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedHostRequest {
    request: HostAuthRequest,
    canonical_bytes: Vec<u8>,
}

impl VerifiedHostRequest {
    pub fn request(&self) -> &HostAuthRequest {
        &self.request
    }

    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(&self.canonical_bytes).into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedHostRequest {
    pub request: HostAuthRequest,
    pub signature_raw: [u8; SIGNATURE_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedWatchResponse {
    pub response: WatchAuthResponse,
    pub signature_raw: [u8; SIGNATURE_BYTES],
}

#[derive(Debug, Default)]
pub struct ReplayGuard {
    nonces: HashSet<[u8; NONCE_BYTES]>,
    sessions: HashSet<[u8; SESSION_ID_BYTES]>,
}

impl ReplayGuard {
    fn remember(
        &mut self,
        nonce: [u8; NONCE_BYTES],
        session_id: [u8; SESSION_ID_BYTES],
    ) -> Result<(), ProtocolError> {
        if self.nonces.contains(&nonce) || self.sessions.contains(&session_id) {
            return Err(ProtocolError::Replay);
        }
        if self.nonces.len() >= MAX_REPLAY_ENTRIES || self.sessions.len() >= MAX_REPLAY_ENTRIES {
            return Err(ProtocolError::ReplayStoreFull);
        }
        self.nonces.insert(nonce);
        self.sessions.insert(session_id);
        Ok(())
    }
}

/// Generate fresh protocol identifiers from the operating-system CSPRNG.
/// This does not generate or store a signing key.
pub fn prepare_host_request(
    enrollment: &HostEnrollment,
    target_watch_enrollment_id: impl Into<String>,
    issued_at_ms: u64,
    lifetime_ms: u64,
) -> Result<HostAuthRequest, ProtocolError> {
    validate_host_enrollment(enrollment)?;
    if !(1..=MAX_REQUEST_LIFETIME_MS).contains(&lifetime_ms) {
        return Err(ProtocolError::LifetimeTooLong);
    }
    let expires_at_ms = issued_at_ms
        .checked_add(lifetime_ms)
        .ok_or(ProtocolError::InvalidTimestamp)?;
    let target_watch_enrollment_id = target_watch_enrollment_id.into();
    validate_id(&target_watch_enrollment_id)?;
    let mut session_id = [0; SESSION_ID_BYTES];
    let mut nonce = [0; NONCE_BYTES];
    getrandom::getrandom(&mut session_id).map_err(|_| ProtocolError::EntropyUnavailable)?;
    getrandom::getrandom(&mut nonce).map_err(|_| ProtocolError::EntropyUnavailable)?;
    Ok(HostAuthRequest {
        host_enrollment_id: enrollment.enrollment_id.clone(),
        host_identity: enrollment.host_identity.clone(),
        linux_uid: enrollment.linux_uid,
        target_watch_enrollment_id,
        session_id,
        nonce,
        issued_at_ms,
        expires_at_ms,
        action: ACTION_RESEARCH_CONFIRMATION.into(),
    })
}

fn sign_host_request(
    request: HostAuthRequest,
    signing_key: &SigningKey,
) -> Result<SignedHostRequest, ProtocolError> {
    let canonical = encode_host_request(&request)?;
    let signature_raw = sign(REQUEST_DOMAIN, &canonical, signing_key);
    Ok(SignedHostRequest {
        request,
        signature_raw,
    })
}

/// Prepare and sign a request with the private key that matches the
/// independently enrolled host public key.
pub fn build_host_request(
    enrollment: &HostEnrollment,
    signing_key: &SigningKey,
    target_watch_enrollment_id: impl Into<String>,
    issued_at_ms: u64,
    lifetime_ms: u64,
) -> Result<SignedHostRequest, ProtocolError> {
    validate_host_enrollment(enrollment)?;
    if signing_key
        .verifying_key()
        .to_encoded_point(false)
        .as_bytes()
        != enrollment.public_key()?.to_encoded_point(false).as_bytes()
    {
        return Err(ProtocolError::BindingMismatch);
    }
    sign_host_request(
        prepare_host_request(
            enrollment,
            target_watch_enrollment_id,
            issued_at_ms,
            lifetime_ms,
        )?,
        signing_key,
    )
}

/// Verify the host signature and all Watch-side request bindings before a UI
/// can obtain a `VerifiedHostRequest` token.
pub fn verify_host_request(
    enrollment: Option<&HostEnrollment>,
    request: HostAuthRequest,
    signature_raw: &[u8],
    expected_watch_enrollment_id: &str,
    now_ms: u64,
    replay_guard: &mut ReplayGuard,
) -> Result<VerifiedHostRequest, ProtocolError> {
    let enrollment = enrollment.ok_or(ProtocolError::UnknownEnrollment)?;
    validate_host_enrollment(enrollment)?;
    let canonical_bytes = encode_host_request(&request)?;
    validate_request_time(&request, now_ms)?;
    if request.action != ACTION_RESEARCH_CONFIRMATION {
        return Err(ProtocolError::WrongAction);
    }
    if request.host_enrollment_id != enrollment.enrollment_id
        || request.host_identity != enrollment.host_identity
        || request.linux_uid != enrollment.linux_uid
        || request.target_watch_enrollment_id != expected_watch_enrollment_id
    {
        return Err(ProtocolError::BindingMismatch);
    }
    verify(
        REQUEST_DOMAIN,
        &canonical_bytes,
        signature_raw,
        &enrollment.public_key()?,
    )?;
    replay_guard.remember(request.nonce, request.session_id)?;
    Ok(VerifiedHostRequest {
        request,
        canonical_bytes,
    })
}

/// Build a response after a caller records an explicit foreground choice.
/// The verified token is consumed so one request cannot be approved twice by
/// this API. Cancellation or failed/unsupported LocalAuthentication must not
/// call this function.
pub fn create_watch_response(
    verified_request: VerifiedHostRequest,
    enrollment: &WatchEnrollment,
    signing_key: &SigningKey,
    decision: ManualDecision,
    consent_result: ConsentResult,
    response_at_ms: u64,
) -> Result<SignedWatchResponse, ProtocolError> {
    validate_watch_enrollment(enrollment)?;
    if consent_result != ConsentResult::ExplicitUserDecision {
        return Err(ProtocolError::ConsentNotCompleted);
    }
    let request_digest = verified_request.digest();
    let request = verified_request.request;
    if response_at_ms < request.issued_at_ms {
        return Err(ProtocolError::NotYetValid);
    }
    if response_at_ms >= request.expires_at_ms {
        return Err(ProtocolError::Expired);
    }
    if request.target_watch_enrollment_id != enrollment.enrollment_id
        || enrollment.bound_host_enrollment_id != request.host_enrollment_id
        || enrollment.bound_host_identity != request.host_identity
        || enrollment.bound_linux_uid != request.linux_uid
    {
        return Err(ProtocolError::BindingMismatch);
    }
    if signing_key
        .verifying_key()
        .to_encoded_point(false)
        .as_bytes()
        != enrollment.public_key()?.to_encoded_point(false).as_bytes()
    {
        return Err(ProtocolError::BindingMismatch);
    }
    let response = WatchAuthResponse {
        request_digest,
        nonce: request.nonce,
        host_enrollment_id: request.host_enrollment_id,
        host_identity: request.host_identity,
        linux_uid: request.linux_uid,
        target_watch_enrollment_id: request.target_watch_enrollment_id,
        watch_identity: enrollment.watch_identity.clone(),
        session_id: request.session_id,
        action: request.action,
        decision: match decision {
            ManualDecision::Approve => Decision::Approved,
            ManualDecision::Reject => Decision::Rejected,
        },
        response_at_ms,
        consent_result,
    };
    let canonical = encode_watch_response(&response)?;
    let signature_raw = sign(RESPONSE_DOMAIN, &canonical, signing_key);
    Ok(SignedWatchResponse {
        response,
        signature_raw,
    })
}

/// Verify the response using the independently pinned Watch key on the host.
/// This returns a research signature result only; it has no authorization
/// output or side effect.
pub fn verify_watch_response(
    host_enrollment: &HostEnrollment,
    watch_enrollment: Option<&WatchEnrollment>,
    verified_request: &VerifiedHostRequest,
    response: &WatchAuthResponse,
    signature_raw: &[u8],
    now_ms: u64,
    replay_guard: &mut ReplayGuard,
) -> Result<(), ProtocolError> {
    let enrollment = watch_enrollment.ok_or(ProtocolError::UnknownEnrollment)?;
    validate_host_enrollment(host_enrollment)?;
    validate_watch_enrollment(enrollment)?;
    let canonical = encode_watch_response(response)?;
    let request = &verified_request.request;
    if response.consent_result != ConsentResult::ExplicitUserDecision {
        return Err(ProtocolError::ConsentNotCompleted);
    }
    if response.decision != Decision::Approved && response.decision != Decision::Rejected {
        return Err(ProtocolError::InvalidDecision);
    }
    if response.action != ACTION_RESEARCH_CONFIRMATION {
        return Err(ProtocolError::WrongAction);
    }
    if response.response_at_ms < request.issued_at_ms {
        return Err(ProtocolError::NotYetValid);
    }
    if response.response_at_ms >= request.expires_at_ms || now_ms >= request.expires_at_ms {
        return Err(ProtocolError::Expired);
    }
    if response.response_at_ms > now_ms {
        return Err(ProtocolError::InvalidTimestamp);
    }
    if response.request_digest != verified_request.digest()
        || response.nonce != request.nonce
        || response.session_id != request.session_id
        || response.host_enrollment_id != request.host_enrollment_id
        || response.host_identity != request.host_identity
        || response.linux_uid != request.linux_uid
        || response.target_watch_enrollment_id != request.target_watch_enrollment_id
        || response.target_watch_enrollment_id != enrollment.enrollment_id
        || response.watch_identity != enrollment.watch_identity
        || response.action != request.action
        || request.host_enrollment_id != host_enrollment.enrollment_id
        || request.host_identity != host_enrollment.host_identity
        || request.linux_uid != host_enrollment.linux_uid
        || enrollment.bound_host_enrollment_id != host_enrollment.enrollment_id
        || enrollment.bound_host_identity != host_enrollment.host_identity
        || enrollment.bound_linux_uid != host_enrollment.linux_uid
    {
        return Err(ProtocolError::BindingMismatch);
    }
    verify(
        RESPONSE_DOMAIN,
        &canonical,
        signature_raw,
        &enrollment.public_key()?,
    )?;
    replay_guard.remember(response.nonce, response.session_id)?;
    Ok(())
}

impl HostAuthRequest {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        encode_host_request(self)
    }

    pub fn parse_canonical(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_CANONICAL_MESSAGE_BYTES {
            return Err(ProtocolError::Oversized);
        }
        let mut reader = Reader::new(bytes);
        read_header(&mut reader, REQUEST_KIND)?;
        let request = Self {
            host_enrollment_id: reader.read_id()?,
            host_identity: reader.read_id()?,
            linux_uid: reader.read_u32()?,
            target_watch_enrollment_id: reader.read_id()?,
            session_id: reader.read_array()?,
            nonce: reader.read_array()?,
            issued_at_ms: reader.read_u64()?,
            expires_at_ms: reader.read_u64()?,
            action: reader.read_id()?,
        };
        reader.finish()?;
        if encode_host_request(&request)? != bytes {
            return Err(ProtocolError::NonCanonical);
        }
        Ok(request)
    }
}

impl WatchAuthResponse {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        encode_watch_response(self)
    }

    pub fn parse_canonical(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_CANONICAL_MESSAGE_BYTES {
            return Err(ProtocolError::Oversized);
        }
        let mut reader = Reader::new(bytes);
        read_header(&mut reader, RESPONSE_KIND)?;
        let request_digest = reader.read_array()?;
        let nonce = reader.read_array()?;
        let host_enrollment_id = reader.read_id()?;
        let host_identity = reader.read_id()?;
        let linux_uid = reader.read_u32()?;
        let target_watch_enrollment_id = reader.read_id()?;
        let watch_identity = reader.read_id()?;
        let session_id = reader.read_array()?;
        let action = reader.read_id()?;
        let decision = match reader.read_u8()? {
            1 => Decision::Approved,
            2 => Decision::Rejected,
            _ => return Err(ProtocolError::InvalidDecision),
        };
        let response_at_ms = reader.read_u64()?;
        let consent_result = match reader.read_u8()? {
            1 => ConsentResult::ExplicitUserDecision,
            2 => ConsentResult::Cancelled,
            3 => ConsentResult::Unsupported,
            4 => ConsentResult::LocalAuthenticationDenied,
            _ => return Err(ProtocolError::ConsentNotCompleted),
        };
        reader.finish()?;
        let response = Self {
            request_digest,
            nonce,
            host_enrollment_id,
            host_identity,
            linux_uid,
            target_watch_enrollment_id,
            watch_identity,
            session_id,
            action,
            decision,
            response_at_ms,
            consent_result,
        };
        if encode_watch_response(&response)? != bytes {
            return Err(ProtocolError::NonCanonical);
        }
        Ok(response)
    }
}

fn encode_host_request(request: &HostAuthRequest) -> Result<Vec<u8>, ProtocolError> {
    validate_id(&request.host_enrollment_id)?;
    validate_id(&request.host_identity)?;
    validate_id(&request.target_watch_enrollment_id)?;
    validate_id(&request.action)?;
    if request.action != ACTION_RESEARCH_CONFIRMATION {
        return Err(ProtocolError::WrongAction);
    }
    let mut output = header(REQUEST_KIND);
    put_id(&mut output, &request.host_enrollment_id)?;
    put_id(&mut output, &request.host_identity)?;
    output.extend_from_slice(&request.linux_uid.to_be_bytes());
    put_id(&mut output, &request.target_watch_enrollment_id)?;
    output.extend_from_slice(&request.session_id);
    output.extend_from_slice(&request.nonce);
    output.extend_from_slice(&request.issued_at_ms.to_be_bytes());
    output.extend_from_slice(&request.expires_at_ms.to_be_bytes());
    put_id(&mut output, &request.action)?;
    check_message_size(&output)?;
    Ok(output)
}

fn encode_watch_response(response: &WatchAuthResponse) -> Result<Vec<u8>, ProtocolError> {
    validate_id(&response.host_enrollment_id)?;
    validate_id(&response.host_identity)?;
    validate_id(&response.target_watch_enrollment_id)?;
    validate_id(&response.watch_identity)?;
    validate_id(&response.action)?;
    if response.action != ACTION_RESEARCH_CONFIRMATION {
        return Err(ProtocolError::WrongAction);
    }
    let mut output = header(RESPONSE_KIND);
    output.extend_from_slice(&response.request_digest);
    output.extend_from_slice(&response.nonce);
    put_id(&mut output, &response.host_enrollment_id)?;
    put_id(&mut output, &response.host_identity)?;
    output.extend_from_slice(&response.linux_uid.to_be_bytes());
    put_id(&mut output, &response.target_watch_enrollment_id)?;
    put_id(&mut output, &response.watch_identity)?;
    output.extend_from_slice(&response.session_id);
    put_id(&mut output, &response.action)?;
    output.push(match response.decision {
        Decision::Approved => 1,
        Decision::Rejected => 2,
    });
    output.extend_from_slice(&response.response_at_ms.to_be_bytes());
    output.push(match response.consent_result {
        ConsentResult::ExplicitUserDecision => 1,
        ConsentResult::Cancelled => 2,
        ConsentResult::Unsupported => 3,
        ConsentResult::LocalAuthenticationDenied => 4,
    });
    check_message_size(&output)?;
    Ok(output)
}

fn header(kind: u8) -> Vec<u8> {
    let mut output = Vec::with_capacity(256);
    output.extend_from_slice(&MAGIC);
    output.extend_from_slice(&[PROTOCOL_VERSION, kind, ALGORITHM_ID]);
    output
}

fn read_header(reader: &mut Reader<'_>, expected_kind: u8) -> Result<(), ProtocolError> {
    if reader.read_array::<4>()? != MAGIC {
        return Err(ProtocolError::Malformed);
    }
    if reader.read_u8()? != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if reader.read_u8()? != expected_kind {
        return Err(ProtocolError::WrongMessageType);
    }
    if reader.read_u8()? != ALGORITHM_ID {
        return Err(ProtocolError::UnsupportedAlgorithm);
    }
    Ok(())
}

fn check_message_size(bytes: &[u8]) -> Result<(), ProtocolError> {
    if bytes.len() > MAX_CANONICAL_MESSAGE_BYTES {
        Err(ProtocolError::Oversized)
    } else {
        Ok(())
    }
}

fn put_id(output: &mut Vec<u8>, value: &str) -> Result<(), ProtocolError> {
    validate_id(value)?;
    let bytes = value.as_bytes();
    output.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn validate_id(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        return Err(ProtocolError::InvalidIdentifier);
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ProtocolError::Malformed)?;
        if end > self.bytes.len() {
            return Err(ProtocolError::Malformed);
        }
        let result = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(result)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProtocolError::Malformed)
    }

    fn read_u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_be_bytes(self.read_array()?))
    }

    fn read_id(&mut self) -> Result<String, ProtocolError> {
        let len = u16::from_be_bytes(self.read_array()?) as usize;
        if len == 0 || len > MAX_ID_BYTES {
            return Err(ProtocolError::InvalidIdentifier);
        }
        let value = std::str::from_utf8(self.take(len)?).map_err(|_| ProtocolError::InvalidUtf8)?;
        validate_id(value)?;
        Ok(value.to_owned())
    }

    fn finish(&self) -> Result<(), ProtocolError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProtocolError::NonCanonical)
        }
    }
}

fn validate_request_time(request: &HostAuthRequest, now_ms: u64) -> Result<(), ProtocolError> {
    if request.expires_at_ms <= request.issued_at_ms {
        return Err(ProtocolError::InvalidTimestamp);
    }
    if request.expires_at_ms - request.issued_at_ms > MAX_REQUEST_LIFETIME_MS {
        return Err(ProtocolError::LifetimeTooLong);
    }
    if request.issued_at_ms > now_ms {
        return Err(ProtocolError::NotYetValid);
    }
    if now_ms >= request.expires_at_ms {
        return Err(ProtocolError::Expired);
    }
    Ok(())
}

fn validate_host_enrollment(enrollment: &HostEnrollment) -> Result<(), ProtocolError> {
    validate_id(&enrollment.enrollment_id)?;
    validate_id(&enrollment.host_identity)?;
    validate_record(
        enrollment.key_algorithm,
        enrollment.key_version,
        enrollment.state,
        enrollment.test_only,
    )?;
    enrollment.public_key()?;
    Ok(())
}

fn validate_watch_enrollment(enrollment: &WatchEnrollment) -> Result<(), ProtocolError> {
    validate_id(&enrollment.enrollment_id)?;
    validate_id(&enrollment.watch_identity)?;
    validate_id(&enrollment.bound_host_enrollment_id)?;
    validate_id(&enrollment.bound_host_identity)?;
    validate_record(
        enrollment.key_algorithm,
        enrollment.key_version,
        enrollment.state,
        enrollment.test_only,
    )?;
    enrollment.public_key()?;
    Ok(())
}

fn validate_record(
    algorithm: KeyAlgorithm,
    key_version: u8,
    state: EnrollmentState,
    test_only: bool,
) -> Result<(), ProtocolError> {
    if algorithm != KeyAlgorithm::P256EcdsaSha256 || key_version != 1 {
        return Err(ProtocolError::UnsupportedAlgorithm);
    }
    if !test_only {
        return Err(ProtocolError::NonTestEnrollment);
    }
    match state {
        EnrollmentState::Active => Ok(()),
        EnrollmentState::Suspended => Err(ProtocolError::EnrollmentInactive),
        EnrollmentState::Revoked => Err(ProtocolError::EnrollmentRevoked),
    }
}

impl HostEnrollment {
    fn public_key(&self) -> Result<VerifyingKey, ProtocolError> {
        let bytes = decode_hex::<PUBLIC_KEY_BYTES>(&self.public_key_sec1_hex)?;
        if bytes[0] != 0x04 {
            return Err(ProtocolError::InvalidPublicKey);
        }
        VerifyingKey::from_sec1_bytes(&bytes).map_err(|_| ProtocolError::InvalidPublicKey)
    }
}

impl WatchEnrollment {
    fn public_key(&self) -> Result<VerifyingKey, ProtocolError> {
        let bytes = decode_hex::<PUBLIC_KEY_BYTES>(&self.public_key_sec1_hex)?;
        if bytes[0] != 0x04 {
            return Err(ProtocolError::InvalidPublicKey);
        }
        VerifyingKey::from_sec1_bytes(&bytes).map_err(|_| ProtocolError::InvalidPublicKey)
    }
}

fn sign(domain: &[u8], canonical: &[u8], key: &SigningKey) -> [u8; SIGNATURE_BYTES] {
    let mut transcript = Vec::with_capacity(domain.len() + canonical.len());
    transcript.extend_from_slice(domain);
    transcript.extend_from_slice(canonical);
    let signature: Signature = key.sign(&transcript);
    signature.to_bytes().into()
}

fn verify(
    domain: &[u8],
    canonical: &[u8],
    signature_raw: &[u8],
    key: &VerifyingKey,
) -> Result<(), ProtocolError> {
    let signature =
        Signature::from_slice(signature_raw).map_err(|_| ProtocolError::InvalidSignature)?;
    let mut transcript = Vec::with_capacity(domain.len() + canonical.len());
    transcript.extend_from_slice(domain);
    transcript.extend_from_slice(canonical);
    key.verify(&transcript, &signature)
        .map_err(|_| ProtocolError::InvalidSignature)
}

fn decode_hex<const N: usize>(text: &str) -> Result<[u8; N], ProtocolError> {
    if text.len() != N * 2 {
        return Err(ProtocolError::InvalidFixture);
    }
    let mut output = [0; N];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or(ProtocolError::NonCanonical)?;
        let low = hex_nibble(pair[1]).ok_or(ProtocolError::NonCanonical)?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedMessageFixture {
    pub canonical_hex: String,
    pub signature_raw_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchAuthFixture {
    pub schema_version: u8,
    pub now_ms: u64,
    pub host_enrollment: HostEnrollment,
    pub watch_enrollment: WatchEnrollment,
    pub request: SignedMessageFixture,
    pub response: Option<SignedMessageFixture>,
}

impl WatchAuthFixture {
    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        if input.len() > 16 * 1024 {
            return Err(ProtocolError::Oversized);
        }
        let fixture: Self =
            serde_json::from_str(input).map_err(|_| ProtocolError::InvalidFixture)?;
        if fixture.schema_version != 1 {
            return Err(ProtocolError::UnsupportedVersion);
        }
        Ok(fixture)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WatchAuthFixtureReport {
    pub schema_version: u8,
    pub protocol: &'static str,
    pub host_request_signature_verified: bool,
    pub watch_response_signature_verified: bool,
    pub decision: FixtureDecision,
    pub result: &'static str,
    pub authorization: &'static str,
    pub source: &'static str,
}

/// Offline-only validation of both independently enrolled directions.
pub fn verify_fixture(fixture: &WatchAuthFixture) -> Result<WatchAuthFixtureReport, ProtocolError> {
    if !fixture.host_enrollment.test_only || !fixture.watch_enrollment.test_only {
        return Err(ProtocolError::NonTestEnrollment);
    }
    let request_bytes = decode_variable_hex(&fixture.request.canonical_hex)?;
    let request = HostAuthRequest::parse_canonical(&request_bytes)?;
    let request_signature = decode_hex::<SIGNATURE_BYTES>(&fixture.request.signature_raw_hex)?;
    let mut watch_replay = ReplayGuard::default();
    let verified_request = verify_host_request(
        Some(&fixture.host_enrollment),
        request,
        &request_signature,
        &fixture.watch_enrollment.enrollment_id,
        fixture.now_ms,
        &mut watch_replay,
    )?;

    let signed_response = fixture
        .response
        .as_ref()
        .ok_or(ProtocolError::ConsentNotCompleted)?;
    let response_bytes = decode_variable_hex(&signed_response.canonical_hex)?;
    let response = WatchAuthResponse::parse_canonical(&response_bytes)?;
    let response_signature = decode_hex::<SIGNATURE_BYTES>(&signed_response.signature_raw_hex)?;
    let mut host_replay = ReplayGuard::default();
    verify_watch_response(
        &fixture.host_enrollment,
        Some(&fixture.watch_enrollment),
        &verified_request,
        &response,
        &response_signature,
        fixture.now_ms,
        &mut host_replay,
    )?;
    let decision = match response.decision {
        Decision::Approved => FixtureDecision::Approved,
        Decision::Rejected => FixtureDecision::Rejected,
    };
    Ok(WatchAuthFixtureReport {
        schema_version: 1,
        protocol: PROTOCOL_NAME,
        host_request_signature_verified: true,
        watch_response_signature_verified: true,
        result: match response.decision {
            Decision::Approved => "signature_verified",
            Decision::Rejected => "signature_verified_rejected",
        },
        decision,
        authorization: "never_evaluated",
        source: "offline_test_fixture",
    })
}

fn decode_variable_hex(text: &str) -> Result<Vec<u8>, ProtocolError> {
    if text.is_empty() || text.len() % 2 != 0 || text.len() > MAX_CANONICAL_MESSAGE_BYTES * 2 {
        return Err(ProtocolError::InvalidFixture);
    }
    let mut output = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0]).ok_or(ProtocolError::NonCanonical)?;
        let low = hex_nibble(pair[1]).ok_or(ProtocolError::NonCanonical)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

/// Convert bytes to lower-case fixture hex. Public for cross-language test
/// tooling; never call this for private key material.
pub fn fixture_hex(bytes: &[u8]) -> String {
    hex_encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::FieldBytes;

    const NOW: u64 = 1_800_000_000_600;
    const ISSUED: u64 = 1_800_000_000_000;

    fn host_key() -> SigningKey {
        // TEST-ONLY scalar; never used by runtime code or shipped in fixtures.
        SigningKey::from_bytes(&FieldBytes::from([1u8; 32])).unwrap()
    }

    fn watch_key() -> SigningKey {
        // TEST-ONLY scalar; never used by runtime code or shipped in fixtures.
        SigningKey::from_bytes(&FieldBytes::from([2u8; 32])).unwrap()
    }

    fn enrollment_hex(key: &SigningKey) -> String {
        hex_encode(key.verifying_key().to_encoded_point(false).as_bytes())
    }

    fn host_enrollment() -> HostEnrollment {
        HostEnrollment {
            enrollment_id: "host-enrollment-test-01".into(),
            host_identity: "linux-host-test-01".into(),
            linux_uid: 1000,
            public_key_sec1_hex: enrollment_hex(&host_key()),
            key_algorithm: KeyAlgorithm::P256EcdsaSha256,
            key_version: 1,
            state: EnrollmentState::Active,
            test_only: true,
        }
    }

    fn watch_enrollment() -> WatchEnrollment {
        WatchEnrollment {
            enrollment_id: "watch-enrollment-test-01".into(),
            watch_identity: "watch-se2-test-01".into(),
            bound_host_enrollment_id: "host-enrollment-test-01".into(),
            bound_host_identity: "linux-host-test-01".into(),
            bound_linux_uid: 1000,
            public_key_sec1_hex: enrollment_hex(&watch_key()),
            key_algorithm: KeyAlgorithm::P256EcdsaSha256,
            key_version: 1,
            state: EnrollmentState::Active,
            test_only: true,
        }
    }

    fn request() -> HostAuthRequest {
        HostAuthRequest {
            host_enrollment_id: "host-enrollment-test-01".into(),
            host_identity: "linux-host-test-01".into(),
            linux_uid: 1000,
            target_watch_enrollment_id: "watch-enrollment-test-01".into(),
            session_id: [0x31; SESSION_ID_BYTES],
            nonce: [0x52; NONCE_BYTES],
            issued_at_ms: ISSUED,
            expires_at_ms: ISSUED + 60_000,
            action: ACTION_RESEARCH_CONFIRMATION.into(),
        }
    }

    fn signed_request(req: HostAuthRequest) -> SignedHostRequest {
        sign_host_request(req, &host_key()).unwrap()
    }

    fn verified_request() -> VerifiedHostRequest {
        let signed = signed_request(request());
        verify_host_request(
            Some(&host_enrollment()),
            signed.request,
            &signed.signature_raw,
            "watch-enrollment-test-01",
            NOW,
            &mut ReplayGuard::default(),
        )
        .unwrap()
    }

    fn signed_response(decision: ManualDecision) -> SignedWatchResponse {
        create_watch_response(
            verified_request(),
            &watch_enrollment(),
            &watch_key(),
            decision,
            ConsentResult::ExplicitUserDecision,
            NOW,
        )
        .unwrap()
    }

    fn verify_response(
        response: &WatchAuthResponse,
        signature: &[u8],
        watch: Option<&WatchEnrollment>,
        now: u64,
        guard: &mut ReplayGuard,
    ) -> Result<(), ProtocolError> {
        verify_watch_response(
            &host_enrollment(),
            watch,
            &verified_request(),
            response,
            signature,
            now,
            guard,
        )
    }

    #[test]
    fn host_request_roundtrips_and_is_signed_with_domain_separation() {
        let signed = signed_request(request());
        let bytes = signed.request.canonical_bytes().unwrap();
        assert_eq!(
            HostAuthRequest::parse_canonical(&bytes).unwrap(),
            signed.request
        );
        verify_host_request(
            Some(&host_enrollment()),
            signed.request.clone(),
            &signed.signature_raw,
            "watch-enrollment-test-01",
            NOW,
            &mut ReplayGuard::default(),
        )
        .unwrap();
        let mut wrong_domain = RESPONSE_DOMAIN.to_vec();
        wrong_domain.extend_from_slice(&bytes);
        host_enrollment()
            .public_key()
            .unwrap()
            .verify(
                &wrong_domain,
                &Signature::from_slice(&signed.signature_raw).unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            bytes[0..7],
            [b'O', b'C', b'W', b'A', 1, REQUEST_KIND, ALGORITHM_ID]
        );
    }

    #[test]
    fn p256_public_key_and_signature_encodings_are_sec1_and_raw() {
        let signed = signed_request(request());
        assert_eq!(
            host_enrollment()
                .public_key()
                .unwrap()
                .to_encoded_point(false)
                .as_bytes()
                .len(),
            65
        );
        assert_eq!(signed.signature_raw.len(), 64);
        assert_eq!(signed.request.canonical_bytes().unwrap()[..4], MAGIC);
    }

    #[test]
    fn watch_response_binds_exact_request_and_is_independently_verified() {
        let signed = signed_response(ManualDecision::Approve);
        assert_eq!(
            signed.response.request_digest,
            Sha256::digest(request().canonical_bytes().unwrap()).as_slice()
        );
        verify_response(
            &signed.response,
            &signed.signature_raw,
            Some(&watch_enrollment()),
            NOW,
            &mut ReplayGuard::default(),
        )
        .unwrap();
    }

    #[test]
    fn explicit_rejection_is_signed_but_never_authorizes() {
        let signed = signed_response(ManualDecision::Reject);
        verify_response(
            &signed.response,
            &signed.signature_raw,
            Some(&watch_enrollment()),
            NOW,
            &mut ReplayGuard::default(),
        )
        .unwrap();
        assert_eq!(signed.response.decision, Decision::Rejected);
    }

    #[test]
    fn request_replay_nonce_and_session_are_rejected() {
        let signed = signed_request(request());
        let mut guard = ReplayGuard::default();
        verify_host_request(
            Some(&host_enrollment()),
            signed.request.clone(),
            &signed.signature_raw,
            "watch-enrollment-test-01",
            NOW,
            &mut guard,
        )
        .unwrap();
        assert_eq!(
            verify_host_request(
                Some(&host_enrollment()),
                signed.request,
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut guard,
            ),
            Err(ProtocolError::Replay)
        );
    }

    #[test]
    fn response_replay_is_rejected_after_signature_verification() {
        let signed = signed_response(ManualDecision::Approve);
        let mut guard = ReplayGuard::default();
        verify_response(
            &signed.response,
            &signed.signature_raw,
            Some(&watch_enrollment()),
            NOW,
            &mut guard,
        )
        .unwrap();
        assert_eq!(
            verify_response(
                &signed.response,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut guard
            ),
            Err(ProtocolError::Replay)
        );
    }

    #[test]
    fn unknown_revoked_suspended_and_non_test_enrollments_fail_closed() {
        let signed = signed_request(request());
        assert_eq!(
            verify_host_request(
                None,
                signed.request.clone(),
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::UnknownEnrollment)
        );
        let mut revoked = host_enrollment();
        revoked.state = EnrollmentState::Revoked;
        assert_eq!(
            verify_host_request(
                Some(&revoked),
                signed.request.clone(),
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::EnrollmentRevoked)
        );
        let mut suspended = host_enrollment();
        suspended.state = EnrollmentState::Suspended;
        assert_eq!(
            verify_host_request(
                Some(&suspended),
                signed.request.clone(),
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::EnrollmentInactive)
        );
        let mut production = host_enrollment();
        production.test_only = false;
        assert_eq!(
            verify_host_request(
                Some(&production),
                signed.request,
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::NonTestEnrollment)
        );
    }

    #[test]
    fn host_and_watch_key_substitution_and_wrong_role_are_rejected() {
        let signed = signed_request(request());
        let mut replaced = host_enrollment();
        replaced.public_key_sec1_hex = enrollment_hex(&watch_key());
        assert_eq!(
            verify_host_request(
                Some(&replaced),
                signed.request.clone(),
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::InvalidSignature)
        );
        assert_eq!(
            verify_host_request(
                Some(&host_enrollment()),
                signed.request.clone(),
                &signed.signature_raw,
                "other-watch",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::BindingMismatch)
        );
        let signed_watch = signed_response(ManualDecision::Approve);
        let mut changed_watch = watch_enrollment();
        changed_watch.public_key_sec1_hex = enrollment_hex(&host_key());
        assert_eq!(
            verify_response(
                &signed_watch.response,
                &signed_watch.signature_raw,
                Some(&changed_watch),
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::InvalidSignature)
        );
    }

    #[test]
    fn account_identity_action_and_session_substitution_are_rejected() {
        let signed = signed_request(request());
        for (field, expected) in [
            ("uid", ProtocolError::BindingMismatch),
            ("host", ProtocolError::BindingMismatch),
            ("action", ProtocolError::WrongAction),
            ("session", ProtocolError::InvalidSignature),
        ] {
            let mut altered = signed.request.clone();
            match field {
                "uid" => altered.linux_uid += 1,
                "host" => altered.host_identity = "other-host".into(),
                "action" => altered.action = "unlock".into(),
                "session" => altered.session_id[0] ^= 1,
                _ => unreachable!(),
            }
            let result = verify_host_request(
                Some(&host_enrollment()),
                altered,
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default(),
            );
            assert_eq!(result.unwrap_err(), expected, "{field}");
        }
    }

    #[test]
    fn request_expiry_future_issue_and_lifetime_are_checked() {
        let signed = signed_request(request());
        let check = |now| {
            verify_host_request(
                Some(&host_enrollment()),
                signed.request.clone(),
                &signed.signature_raw,
                "watch-enrollment-test-01",
                now,
                &mut ReplayGuard::default(),
            )
        };
        assert_eq!(
            check(signed.request.expires_at_ms).unwrap_err(),
            ProtocolError::Expired
        );
        assert_eq!(
            check(signed.request.expires_at_ms + 1).unwrap_err(),
            ProtocolError::Expired
        );
        assert_eq!(
            check(signed.request.issued_at_ms - 1).unwrap_err(),
            ProtocolError::NotYetValid
        );
        let mut too_long = signed.request.clone();
        too_long.expires_at_ms = too_long.issued_at_ms + MAX_REQUEST_LIFETIME_MS + 1;
        let too_long = sign_host_request(too_long, &host_key()).unwrap();
        assert_eq!(
            verify_host_request(
                Some(&host_enrollment()),
                too_long.request,
                &too_long.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            )
            .unwrap_err(),
            ProtocolError::LifetimeTooLong
        );
    }

    #[test]
    fn response_tampering_unknown_revoked_and_bad_consent_fail_closed() {
        let signed = signed_response(ManualDecision::Approve);
        let mut changed = signed.response.clone();
        changed.request_digest[0] ^= 1;
        assert_eq!(
            verify_response(
                &changed,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::BindingMismatch)
        );
        assert_eq!(
            verify_response(
                &signed.response,
                &signed.signature_raw,
                None,
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::UnknownEnrollment)
        );
        let mut revoked = watch_enrollment();
        revoked.state = EnrollmentState::Revoked;
        assert_eq!(
            verify_response(
                &signed.response,
                &signed.signature_raw,
                Some(&revoked),
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::EnrollmentRevoked)
        );
        let mut cancelled = signed.response.clone();
        cancelled.consent_result = ConsentResult::Cancelled;
        assert_eq!(
            verify_response(
                &cancelled,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::ConsentNotCompleted)
        );
        let mut unsupported = signed.response.clone();
        unsupported.consent_result = ConsentResult::Unsupported;
        assert_eq!(
            verify_response(
                &unsupported,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default(),
            ),
            Err(ProtocolError::ConsentNotCompleted)
        );
        let mut bad_signature = signed.signature_raw;
        bad_signature[0] ^= 0xff;
        assert_eq!(
            verify_response(
                &signed.response,
                &bad_signature,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::InvalidSignature)
        );
    }

    #[test]
    fn response_builder_requires_explicit_manual_choice_and_matching_private_key() {
        let verified = verified_request();
        assert_eq!(
            create_watch_response(
                verified,
                &watch_enrollment(),
                &watch_key(),
                ManualDecision::Approve,
                ConsentResult::Cancelled,
                NOW
            ),
            Err(ProtocolError::ConsentNotCompleted)
        );
        let verified = verified_request();
        assert_eq!(
            create_watch_response(
                verified,
                &watch_enrollment(),
                &host_key(),
                ManualDecision::Approve,
                ConsentResult::ExplicitUserDecision,
                NOW
            ),
            Err(ProtocolError::BindingMismatch)
        );
        assert_eq!(
            create_watch_response(
                verified_request(),
                &watch_enrollment(),
                &watch_key(),
                ManualDecision::Approve,
                ConsentResult::ExplicitUserDecision,
                request().expires_at_ms,
            ),
            Err(ProtocolError::Expired)
        );
    }

    #[test]
    fn response_host_account_session_action_expiry_and_absent_consent_fail_closed() {
        let signed = signed_response(ManualDecision::Approve);
        let mut wrong_uid = signed.response.clone();
        wrong_uid.linux_uid += 1;
        assert_eq!(
            verify_response(
                &wrong_uid,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default(),
            ),
            Err(ProtocolError::BindingMismatch)
        );
        let mut wrong_session = signed.response.clone();
        wrong_session.session_id[0] ^= 1;
        assert_eq!(
            verify_response(
                &wrong_session,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default(),
            ),
            Err(ProtocolError::BindingMismatch)
        );
        let mut wrong_host = signed.response.clone();
        wrong_host.host_identity = "other-host".into();
        assert_eq!(
            verify_response(
                &wrong_host,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default(),
            ),
            Err(ProtocolError::BindingMismatch)
        );
        let mut wrong_action = signed.response.clone();
        wrong_action.action = "unlock".into();
        assert_eq!(
            verify_response(
                &wrong_action,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                NOW,
                &mut ReplayGuard::default(),
            ),
            Err(ProtocolError::WrongAction)
        );
        assert_eq!(
            verify_response(
                &signed.response,
                &signed.signature_raw,
                Some(&watch_enrollment()),
                request().expires_at_ms + 1,
                &mut ReplayGuard::default(),
            ),
            Err(ProtocolError::Expired)
        );
        let fixture = WatchAuthFixture {
            schema_version: 1,
            now_ms: NOW,
            host_enrollment: host_enrollment(),
            watch_enrollment: watch_enrollment(),
            request: SignedMessageFixture {
                canonical_hex: hex_encode(&request().canonical_bytes().unwrap()),
                signature_raw_hex: hex_encode(&signed_request(request()).signature_raw),
            },
            response: None,
        };
        assert_eq!(
            verify_fixture(&fixture),
            Err(ProtocolError::ConsentNotCompleted)
        );
    }

    #[test]
    fn in_memory_replay_guard_is_bounded_and_fails_closed_when_full() {
        let mut guard = ReplayGuard::default();
        for value in 0..MAX_REPLAY_ENTRIES {
            let mut nonce = [0; NONCE_BYTES];
            nonce[..8].copy_from_slice(&(value as u64).to_be_bytes());
            let mut session_id = [0; SESSION_ID_BYTES];
            session_id[..8].copy_from_slice(&(value as u64).to_be_bytes());
            guard.remember(nonce, session_id).unwrap();
        }
        let mut nonce = [0; NONCE_BYTES];
        nonce[..8].copy_from_slice(&(MAX_REPLAY_ENTRIES as u64).to_be_bytes());
        let mut session_id = [0; SESSION_ID_BYTES];
        session_id[..8].copy_from_slice(&(MAX_REPLAY_ENTRIES as u64).to_be_bytes());
        assert_eq!(
            guard.remember(nonce, session_id),
            Err(ProtocolError::ReplayStoreFull)
        );
    }

    #[test]
    fn parser_rejects_downgrade_wrong_curve_malformed_and_trailing_bytes() {
        let bytes = request().canonical_bytes().unwrap();
        let mut downgraded = bytes.clone();
        downgraded[4] = 0;
        assert_eq!(
            HostAuthRequest::parse_canonical(&downgraded),
            Err(ProtocolError::UnsupportedVersion)
        );
        let mut wrong_algorithm = bytes.clone();
        wrong_algorithm[6] = 2;
        assert_eq!(
            HostAuthRequest::parse_canonical(&wrong_algorithm),
            Err(ProtocolError::UnsupportedAlgorithm)
        );
        let mut wrong_type = bytes.clone();
        wrong_type[5] = RESPONSE_KIND;
        assert_eq!(
            HostAuthRequest::parse_canonical(&wrong_type),
            Err(ProtocolError::WrongMessageType)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            HostAuthRequest::parse_canonical(&trailing),
            Err(ProtocolError::NonCanonical)
        );
        assert_eq!(
            HostAuthRequest::parse_canonical(&bytes[..bytes.len() - 1]),
            Err(ProtocolError::Malformed)
        );
        let mut wrong_curve = host_enrollment();
        wrong_curve.key_algorithm = KeyAlgorithm::P384EcdsaSha384;
        let signed = signed_request(request());
        assert_eq!(
            verify_host_request(
                Some(&wrong_curve),
                signed.request,
                &signed.signature_raw,
                "watch-enrollment-test-01",
                NOW,
                &mut ReplayGuard::default()
            ),
            Err(ProtocolError::UnsupportedAlgorithm)
        );
    }

    #[test]
    fn invalid_utf8_and_noncanonical_ids_are_rejected() {
        let mut bytes = request().canonical_bytes().unwrap();
        bytes[9] = 0xff;
        assert_eq!(
            HostAuthRequest::parse_canonical(&bytes),
            Err(ProtocolError::InvalidUtf8)
        );
        let mut bad = request();
        bad.host_identity = "host with spaces".into();
        assert_eq!(bad.canonical_bytes(), Err(ProtocolError::InvalidIdentifier));
    }

    #[test]
    fn prepare_host_request_uses_os_random_and_bounded_lifetime() {
        let one = prepare_host_request(&host_enrollment(), "watch-enrollment-test-01", NOW, 30_000)
            .unwrap();
        let two = prepare_host_request(&host_enrollment(), "watch-enrollment-test-01", NOW, 30_000)
            .unwrap();
        assert_ne!(one.session_id, two.session_id);
        assert_ne!(one.nonce, two.nonce);
        let signed = build_host_request(
            &host_enrollment(),
            &host_key(),
            "watch-enrollment-test-01",
            NOW,
            30_000,
        )
        .unwrap();
        verify_host_request(
            Some(&host_enrollment()),
            signed.request,
            &signed.signature_raw,
            "watch-enrollment-test-01",
            NOW,
            &mut ReplayGuard::default(),
        )
        .unwrap();
        assert_eq!(
            build_host_request(
                &host_enrollment(),
                &watch_key(),
                "watch-enrollment-test-01",
                NOW,
                30_000,
            ),
            Err(ProtocolError::BindingMismatch)
        );
        assert_eq!(
            prepare_host_request(&host_enrollment(), "watch-enrollment-test-01", NOW, 0),
            Err(ProtocolError::LifetimeTooLong)
        );
    }

    #[test]
    fn fixed_host_request_matches_the_shared_fixture_when_present() {
        let fixture: WatchAuthFixture = serde_json::from_str(include_str!("../../../experiments/watchos/Tests/OpenContinuityWatchTests/Fixtures/watch-auth-request-v1.json")).unwrap();
        let canonical = request().canonical_bytes().unwrap();
        assert_eq!(hex_encode(&canonical), fixture.request.canonical_hex);
        let signature = decode_hex::<SIGNATURE_BYTES>(&fixture.request.signature_raw_hex).unwrap();
        verify_host_request(
            Some(&fixture.host_enrollment),
            request(),
            &signature,
            &fixture.watch_enrollment.enrollment_id,
            fixture.now_ms,
            &mut ReplayGuard::default(),
        )
        .unwrap();
    }

    #[test]
    fn offline_fixture_report_never_evaluates_linux_authorization() {
        let signed_host = signed_request(request());
        let signed_watch = signed_response(ManualDecision::Approve);
        let fixture = WatchAuthFixture {
            schema_version: 1,
            now_ms: NOW,
            host_enrollment: host_enrollment(),
            watch_enrollment: watch_enrollment(),
            request: SignedMessageFixture {
                canonical_hex: hex_encode(&signed_host.request.canonical_bytes().unwrap()),
                signature_raw_hex: hex_encode(&signed_host.signature_raw),
            },
            response: Some(SignedMessageFixture {
                canonical_hex: hex_encode(&signed_watch.response.canonical_bytes().unwrap()),
                signature_raw_hex: hex_encode(&signed_watch.signature_raw),
            }),
        };
        let report = verify_fixture(&fixture).unwrap();
        assert_eq!(report.result, "signature_verified");
        assert_eq!(report.authorization, "never_evaluated");
        assert!(report.host_request_signature_verified);
        assert!(report.watch_response_signature_verified);
    }

    #[test]
    fn signatures_from_separate_protocol_roles_do_not_cross_verify() {
        let signed_host = signed_request(request());
        let response = signed_response(ManualDecision::Approve);
        let host_bytes = signed_host.request.canonical_bytes().unwrap();
        let mut host_domain = REQUEST_DOMAIN.to_vec();
        host_domain.extend_from_slice(&host_bytes);
        let mut response_domain = RESPONSE_DOMAIN.to_vec();
        response_domain.extend_from_slice(&host_bytes);
        let host_sig = Signature::from_slice(&signed_host.signature_raw).unwrap();
        assert!(
            host_key()
                .verifying_key()
                .verify(&host_domain, &host_sig)
                .is_ok()
        );
        assert!(
            host_key()
                .verifying_key()
                .verify(&response_domain, &host_sig)
                .is_err()
        );
        let response_bytes = response.response.canonical_bytes().unwrap();
        let mut request_domain = REQUEST_DOMAIN.to_vec();
        request_domain.extend_from_slice(&response_bytes);
        let response_sig = Signature::from_slice(&response.signature_raw).unwrap();
        assert!(
            watch_key()
                .verifying_key()
                .verify(&request_domain, &response_sig)
                .is_err()
        );
    }

    #[test]
    fn protocol_errors_have_stable_nonsecret_display_text() {
        assert_eq!(
            ProtocolError::InvalidSignature.to_string(),
            "signature verification failed"
        );
    }
}
