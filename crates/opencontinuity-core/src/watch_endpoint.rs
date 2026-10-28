//! Portable, non-production Watch endpoint contracts and deterministic mocks.
//!
//! This module has no Apple or Linux I/O. Its Ed25519 exchange is a simulation
//! of a separately enrolled endpoint, not Apple Auto Unlock and not a usable
//! login authenticator. Real deployments need OS-backed CSPRNGs, platform key
//! custody, host authentication, user consent, relay defenses, and review.

use crate::research::EvidenceStatus;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_MESSAGE_BYTES: usize = 4096;
pub const CHALLENGE_NONCE_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const MAX_ID_BYTES: usize = 128;
const MAX_CHALLENGE_LIFETIME_MS: u64 = 30_000;
const MOCK_ONLY_SIGNING_SEED: [u8; 32] = [0x43; 32];
#[cfg(test)]
const MOCK_ATTACKER_SIGNING_SEED: [u8; 32] = [0xa7; 32];

/// Algorithm identifiers are explicit. Ed25519 here is strictly a Rust mock;
/// it is not asserted to be available from watchOS Secure Enclave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentAlgorithm {
    Ed25519MockV1,
    /// Reserved to make unsupported algorithm selection reject explicitly.
    P256EcdsaSha256V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentLifecycle {
    Active,
    Revoked,
    Suspended,
}

/// Test-only trust anchor constructed separately from the endpoint under test.
/// P004 deliberately has no persistent or production enrollment store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MockEnrollmentRecord {
    pub enrollment_id: String,
    pub host_identity: String,
    pub account_identity: String,
    pub watch_endpoint_identity: String,
    pub verification_public_key: Vec<u8>,
    pub key_algorithm: EnrollmentAlgorithm,
    pub key_version: u16,
    pub lifecycle_state: EnrollmentLifecycle,
    pub revoked: bool,
}

impl MockEnrollmentRecord {
    pub fn new_for_test(
        enrollment_id: impl Into<String>,
        host_identity: impl Into<String>,
        account_identity: impl Into<String>,
        watch_endpoint_identity: impl Into<String>,
        verification_public_key: [u8; 32],
    ) -> Result<Self, ProtocolError> {
        let record = Self {
            enrollment_id: enrollment_id.into(),
            host_identity: host_identity.into(),
            account_identity: account_identity.into(),
            watch_endpoint_identity: watch_endpoint_identity.into(),
            verification_public_key: verification_public_key.to_vec(),
            key_algorithm: EnrollmentAlgorithm::Ed25519MockV1,
            key_version: 1,
            lifecycle_state: EnrollmentLifecycle::Active,
            revoked: false,
        };
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_id(&self.enrollment_id)?;
        validate_id(&self.host_identity)?;
        validate_id(&self.account_identity)?;
        validate_id(&self.watch_endpoint_identity)?;
        if self.key_version != 1 {
            return Err(ProtocolError::AlgorithmMismatch);
        }
        if self.verification_public_key.len() != 32 {
            return Err(ProtocolError::Malformed);
        }
        if self.key_algorithm != EnrollmentAlgorithm::Ed25519MockV1 {
            return Err(ProtocolError::AlgorithmMismatch);
        }
        if self.revoked || self.lifecycle_state == EnrollmentLifecycle::Revoked {
            return Err(ProtocolError::EnrollmentRevoked);
        }
        if self.lifecycle_state != EnrollmentLifecycle::Active {
            return Err(ProtocolError::EnrollmentInactive);
        }
        Ok(())
    }
}

/// Research evidence for a capability. These claims describe public API
/// documentation, not an implementation or hardware test in this project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityClaim {
    pub evidence: EvidenceStatus,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointCapabilities {
    pub watch_ble_central: CapabilityClaim,
    pub watch_ble_peripheral_advertising: CapabilityClaim,
    pub watch_direct_local_ip: CapabilityClaim,
    pub watchconnectivity_relay: CapabilityClaim,
    pub continuous_background_listener: CapabilityClaim,
    pub foreground_user_confirmation: CapabilityClaim,
    pub watch_key_storage: CapabilityClaim,
}

impl EndpointCapabilities {
    /// Static public-API research conclusions. `Supported` does not mean this
    /// repository has implemented or tested that capability.
    pub fn researched() -> Self {
        let claim = |evidence, summary: &str| CapabilityClaim {
            evidence,
            summary: summary.into(),
        };
        Self {
            watch_ble_central: claim(
                EvidenceStatus::Documented,
                "Core Bluetooth central APIs are documented for watchOS; physical operation is untested here.",
            ),
            watch_ble_peripheral_advertising: claim(
                EvidenceStatus::Blocked,
                "Apple documents CBPeripheralManager service advertising as unavailable on watchOS.",
            ),
            watch_direct_local_ip: claim(
                EvidenceStatus::Unknown,
                "watchOS documents URLSession networking, but direct Linux local-host interoperability is untested.",
            ),
            watchconnectivity_relay: claim(
                EvidenceStatus::Documented,
                "WatchConnectivity supports paired-app messaging; live delivery requires reachability.",
            ),
            continuous_background_listener: claim(
                EvidenceStatus::Blocked,
                "Public watchOS execution modes do not provide a general always-running third-party listener.",
            ),
            foreground_user_confirmation: claim(
                EvidenceStatus::Documented,
                "A foreground app can present a user action and evaluate Local Authentication policy.",
            ),
            watch_key_storage: claim(
                EvidenceStatus::Documented,
                "CryptoKit and Secure Enclave APIs document hardware-backed P-256 key operations; SE 2 app behavior is untested.",
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    DirectBle,
    DirectIp,
    #[serde(rename = "iphone_relay")]
    IPhoneRelay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportState {
    Connected,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserPresenceEvidence {
    ExplicitConfirmation,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationResult {
    SimulatedVerified,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationState {
    NeverEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchEndpointReport {
    pub schema_version: u8,
    pub source: String,
    pub mock_simulation: bool,
    pub bluez_capability_discovery: String,
    pub experimental_gatt_transport: String,
    pub hardware_watch_observations: String,
    pub capabilities: EndpointCapabilities,
    pub transport: TransportReport,
    pub authentication: AuthenticationReport,
    pub authorization: AuthorizationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportReport {
    pub kind: TransportKind,
    pub state: TransportState,
    pub simulated_response_delay_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticationReport {
    pub result: AuthenticationResult,
    pub evidence: String,
    pub reason: Option<String>,
    pub replay_rejected: bool,
}

/// Versioned and bounded scenario input for the offline mock only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchEndpointFixture {
    pub schema_version: u8,
    pub transport: TransportKind,
    pub transport_connected: bool,
    pub expected_watch_identity: String,
    pub presented_watch_identity: String,
    pub watch_approved: bool,
    pub user_presence: UserPresenceEvidence,
    pub now_ms: u64,
    pub challenge_lifetime_ms: u64,
    pub response_delay_ms: u64,
}

impl WatchEndpointFixture {
    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        if input.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::MessageTooLarge);
        }
        let fixture: Self = serde_json::from_str(input).map_err(|_| ProtocolError::Malformed)?;
        fixture.validate()?;
        Ok(fixture)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != 1 {
            return Err(ProtocolError::UnsupportedVersion);
        }
        validate_id(&self.expected_watch_identity)?;
        validate_id(&self.presented_watch_identity)?;
        if self.challenge_lifetime_ms == 0
            || self.challenge_lifetime_ms > MAX_CHALLENGE_LIFETIME_MS
            || self
                .now_ms
                .checked_add(self.challenge_lifetime_ms)
                .is_none()
            || self.response_delay_ms > 60_000
        {
            return Err(ProtocolError::Malformed);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchChallenge {
    pub version: u16,
    pub nonce: Vec<u8>,
    pub host_id: String,
    pub local_user_id: String,
    pub session_id: String,
    pub action: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

impl WatchChallenge {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if self.nonce.len() != CHALLENGE_NONCE_BYTES
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms > MAX_CHALLENGE_LIFETIME_MS
        {
            return Err(ProtocolError::Malformed);
        }
        validate_id(&self.host_id)?;
        validate_id(&self.local_user_id)?;
        validate_id(&self.session_id)?;
        validate_id(&self.action)
    }

    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        if input.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::MessageTooLarge);
        }
        let message: Self = serde_json::from_str(input).map_err(|_| ProtocolError::Malformed)?;
        message.validate()?;
        Ok(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchApproval {
    pub version: u16,
    pub nonce: Vec<u8>,
    pub host_id: String,
    pub local_user_id: String,
    pub session_id: String,
    pub action: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub identity: String,
    pub approved: bool,
    pub user_presence: UserPresenceEvidence,
    pub responded_at_ms: u64,
    pub signature: Vec<u8>,
}

impl WatchApproval {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if self.nonce.len() != CHALLENGE_NONCE_BYTES || self.signature.len() != SIGNATURE_BYTES {
            return Err(ProtocolError::Malformed);
        }
        validate_id(&self.host_id)?;
        validate_id(&self.local_user_id)?;
        validate_id(&self.session_id)?;
        validate_id(&self.action)?;
        validate_id(&self.identity)
    }

    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        if input.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::MessageTooLarge);
        }
        let message: Self = serde_json::from_str(input).map_err(|_| ProtocolError::Malformed)?;
        message.validate()?;
        Ok(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Malformed,
    MessageTooLarge,
    UnsupportedVersion,
    TransportUnavailable,
    IdentityMismatch,
    InvalidSignature,
    Expired,
    UserRejected,
    MissingPresence,
    Replay,
    UnknownEnrollment,
    EnrollmentRevoked,
    EnrollmentInactive,
    AlgorithmMismatch,
    EnrollmentBindingMismatch,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Malformed => "malformed or invalid message",
            Self::MessageTooLarge => "message exceeds maximum length",
            Self::UnsupportedVersion => "unsupported protocol version",
            Self::TransportUnavailable => "transport is disconnected or unavailable",
            Self::IdentityMismatch => "endpoint identity does not match enrollment",
            Self::InvalidSignature => "approval signature is invalid",
            Self::Expired => "challenge or response is expired or outside its validity window",
            Self::UserRejected => "Watch user rejected the request",
            Self::MissingPresence => "explicit user-presence evidence is missing",
            Self::Replay => "duplicate challenge nonce rejected",
            Self::UnknownEnrollment => "no independently enrolled key exists for this endpoint",
            Self::EnrollmentRevoked => "enrollment is not active",
            Self::EnrollmentInactive => "enrollment is suspended or inactive",
            Self::AlgorithmMismatch => "enrollment key algorithm or version is unsupported",
            Self::EnrollmentBindingMismatch => {
                "challenge does not match the enrolled host and account"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProtocolError {}

/// Platform adapters provide a Watch-side signer only in future OS-specific
/// code. The current implementation is deterministic and mock-only.
pub trait WatchEndpointAdapter {
    fn identity(&self) -> &str;
    fn capabilities(&self) -> EndpointCapabilities;
    fn approve(
        &mut self,
        challenge: &WatchChallenge,
        response_time_ms: u64,
    ) -> Result<WatchApproval, ProtocolError>;
}

pub trait TransportAdapter {
    fn kind(&self) -> TransportKind;
    fn state(&self) -> TransportState;
    fn deliver(
        &mut self,
        endpoint: &mut impl WatchEndpointAdapter,
        challenge: &WatchChallenge,
    ) -> Result<WatchApproval, ProtocolError>;
}

#[derive(Debug, Clone)]
pub struct MockWatchEndpoint {
    identity: String,
    approved: bool,
    user_presence: UserPresenceEvidence,
    signing_key: SigningKey,
}

impl MockWatchEndpoint {
    pub fn new(
        identity: impl Into<String>,
        approved: bool,
        user_presence: UserPresenceEvidence,
    ) -> Result<Self, ProtocolError> {
        let identity = identity.into();
        validate_id(&identity)?;
        Self::with_signing_seed(identity, approved, user_presence, MOCK_ONLY_SIGNING_SEED)
    }

    /// Construct a deterministic endpoint with an explicitly supplied mock
    /// key. Intended for negative substitution tests only.
    pub fn with_signing_seed(
        identity: impl Into<String>,
        approved: bool,
        user_presence: UserPresenceEvidence,
        seed: [u8; 32],
    ) -> Result<Self, ProtocolError> {
        let identity = identity.into();
        validate_id(&identity)?;
        Ok(Self {
            identity,
            approved,
            user_presence,
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

impl WatchEndpointAdapter for MockWatchEndpoint {
    fn identity(&self) -> &str {
        &self.identity
    }

    fn capabilities(&self) -> EndpointCapabilities {
        EndpointCapabilities::researched()
    }

    fn approve(
        &mut self,
        challenge: &WatchChallenge,
        response_time_ms: u64,
    ) -> Result<WatchApproval, ProtocolError> {
        challenge.validate()?;
        let mut approval = WatchApproval {
            version: challenge.version,
            nonce: challenge.nonce.clone(),
            host_id: challenge.host_id.clone(),
            local_user_id: challenge.local_user_id.clone(),
            session_id: challenge.session_id.clone(),
            action: challenge.action.clone(),
            issued_at_ms: challenge.issued_at_ms,
            expires_at_ms: challenge.expires_at_ms,
            identity: self.identity.clone(),
            approved: self.approved,
            user_presence: self.user_presence,
            responded_at_ms: response_time_ms,
            signature: Vec::new(),
        };
        approval.signature = self
            .signing_key
            .sign(&approval_payload(&approval))
            .to_bytes()
            .to_vec();
        Ok(approval)
    }
}

#[derive(Debug, Clone)]
pub struct MockTransportAdapter {
    kind: TransportKind,
    connected: bool,
    response_delay_ms: u64,
}

impl MockTransportAdapter {
    pub fn new(kind: TransportKind, connected: bool, response_delay_ms: u64) -> Self {
        Self {
            kind,
            connected,
            response_delay_ms,
        }
    }
}

impl TransportAdapter for MockTransportAdapter {
    fn kind(&self) -> TransportKind {
        self.kind
    }

    fn state(&self) -> TransportState {
        if self.connected {
            TransportState::Connected
        } else {
            TransportState::Disconnected
        }
    }

    fn deliver(
        &mut self,
        endpoint: &mut impl WatchEndpointAdapter,
        challenge: &WatchChallenge,
    ) -> Result<WatchApproval, ProtocolError> {
        if !self.connected {
            return Err(ProtocolError::TransportUnavailable);
        }
        let response_time = challenge
            .issued_at_ms
            .checked_add(self.response_delay_ms)
            .ok_or(ProtocolError::Malformed)?;
        endpoint.approve(challenge, response_time)
    }
}

#[derive(Debug, Default)]
pub struct ReplayGuard {
    consumed: HashSet<[u8; CHALLENGE_NONCE_BYTES]>,
}

impl ReplayGuard {
    /// Verify against a separately constructed enrollment record. The
    /// endpoint under test never supplies the expected public key.
    pub fn verify_enrolled(
        &mut self,
        enrollment: Option<&MockEnrollmentRecord>,
        challenge: &WatchChallenge,
        approval: &WatchApproval,
        now_ms: u64,
    ) -> Result<(), ProtocolError> {
        let enrollment = enrollment.ok_or(ProtocolError::UnknownEnrollment)?;
        enrollment.validate()?;
        if challenge.host_id != enrollment.host_identity
            || challenge.local_user_id != enrollment.account_identity
        {
            return Err(ProtocolError::EnrollmentBindingMismatch);
        }
        if approval.identity != enrollment.watch_endpoint_identity {
            return Err(ProtocolError::IdentityMismatch);
        }
        let key: [u8; 32] = enrollment
            .verification_public_key
            .as_slice()
            .try_into()
            .map_err(|_| ProtocolError::Malformed)?;
        self.verify(
            challenge,
            approval,
            &enrollment.watch_endpoint_identity,
            &key,
            now_ms,
        )
    }

    pub fn verify(
        &mut self,
        challenge: &WatchChallenge,
        approval: &WatchApproval,
        expected_identity: &str,
        expected_public_key: &[u8; 32],
        now_ms: u64,
    ) -> Result<(), ProtocolError> {
        challenge.validate()?;
        approval.validate()?;
        if now_ms >= challenge.expires_at_ms
            || approval.responded_at_ms < challenge.issued_at_ms
            || approval.responded_at_ms > challenge.expires_at_ms
            || now_ms < approval.responded_at_ms
        {
            return Err(ProtocolError::Expired);
        }
        if approval.version != challenge.version {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if approval.nonce != challenge.nonce
            || approval.host_id != challenge.host_id
            || approval.local_user_id != challenge.local_user_id
            || approval.session_id != challenge.session_id
            || approval.action != challenge.action
            || approval.issued_at_ms != challenge.issued_at_ms
            || approval.expires_at_ms != challenge.expires_at_ms
        {
            return Err(ProtocolError::Malformed);
        }
        if approval.identity != expected_identity {
            return Err(ProtocolError::IdentityMismatch);
        }
        if approval.signature.len() != SIGNATURE_BYTES {
            return Err(ProtocolError::Malformed);
        }
        let public_key = VerifyingKey::from_bytes(expected_public_key)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        let signature = Signature::from_slice(&approval.signature)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        public_key
            .verify(&approval_payload(approval), &signature)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        if !approval.approved {
            return Err(ProtocolError::UserRejected);
        }
        if approval.user_presence != UserPresenceEvidence::ExplicitConfirmation {
            return Err(ProtocolError::MissingPresence);
        }
        let nonce: [u8; CHALLENGE_NONCE_BYTES] = challenge
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| ProtocolError::Malformed)?;
        if !self.consumed.insert(nonce) {
            return Err(ProtocolError::Replay);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicNonceSource {
    next_counter: u64,
}

impl DeterministicNonceSource {
    pub fn new() -> Self {
        Self { next_counter: 0 }
    }

    /// Stable mock nonce only. This is not a cryptographically random nonce
    /// source and must never be used by a real authenticator.
    pub fn next_nonce(&mut self) -> [u8; CHALLENGE_NONCE_BYTES] {
        let mut hash = Sha256::new();
        hash.update(b"OpenContinuity P003 MOCK NONCE v1\0");
        hash.update(self.next_counter.to_be_bytes());
        self.next_counter = self.next_counter.wrapping_add(1);
        hash.finalize().into()
    }
}

impl Default for DeterministicNonceSource {
    fn default() -> Self {
        Self::new()
    }
}

pub fn simulate(fixture: &WatchEndpointFixture) -> Result<WatchEndpointReport, ProtocolError> {
    fixture.validate()?;
    let mut nonce_source = DeterministicNonceSource::new();
    let challenge = WatchChallenge {
        version: PROTOCOL_VERSION,
        nonce: nonce_source.next_nonce().to_vec(),
        host_id: "linux-host-mock".into(),
        local_user_id: "local-user-mock".into(),
        session_id: "session-mock".into(),
        action: "explicit_authentication_request".into(),
        issued_at_ms: fixture.now_ms,
        expires_at_ms: fixture.now_ms + fixture.challenge_lifetime_ms,
    };
    let mut endpoint = MockWatchEndpoint::new(
        &fixture.presented_watch_identity,
        fixture.watch_approved,
        fixture.user_presence,
    )?;
    // Construct the mock trust anchor before and independently of transport
    // endpoint creation. Authentication never reads the endpoint's own key.
    let enrolled_key = SigningKey::from_bytes(&MOCK_ONLY_SIGNING_SEED)
        .verifying_key()
        .to_bytes();
    let enrollment = MockEnrollmentRecord::new_for_test(
        "enrollment-mock-1",
        "linux-host-mock",
        "local-user-mock",
        &fixture.expected_watch_identity,
        enrolled_key,
    )?;
    let mut transport = MockTransportAdapter::new(
        fixture.transport,
        fixture.transport_connected,
        fixture.response_delay_ms,
    );
    let exchange = transport.deliver(&mut endpoint, &challenge);
    let mut replay_guard = ReplayGuard::default();
    let result = exchange.and_then(|approval| {
        replay_guard.verify_enrolled(
            Some(&enrollment),
            &challenge,
            &approval,
            fixture.now_ms + fixture.response_delay_ms,
        )?;
        Ok(approval)
    });
    let (authentication, replay_rejected, reason) = match result {
        Ok(approval) => {
            let replay_rejected = replay_guard.verify_enrolled(
                Some(&enrollment),
                &challenge,
                &approval,
                fixture.now_ms + fixture.response_delay_ms,
            ) == Err(ProtocolError::Replay);
            (
                AuthenticationResult::SimulatedVerified,
                replay_rejected,
                None,
            )
        }
        Err(error) => (
            AuthenticationResult::Rejected,
            false,
            Some(error.to_string()),
        ),
    };
    Ok(WatchEndpointReport {
        schema_version: 1,
        source: "offline_mock_simulation_only".into(),
        mock_simulation: true,
        bluez_capability_discovery: "not_performed".into(),
        experimental_gatt_transport: "not_used_by_offline_mock".into(),
        hardware_watch_observations: "not_run".into(),
        capabilities: endpoint.capabilities(),
        transport: TransportReport {
            kind: transport.kind(),
            state: transport.state(),
            simulated_response_delay_ms: fixture.response_delay_ms,
        },
        authentication: AuthenticationReport {
            result: authentication,
            evidence:
                "deterministic mock exchange; no Apple Watch, iPhone, Bluetooth, or network I/O"
                    .into(),
            reason,
            replay_rejected,
        },
        authorization: AuthorizationState::NeverEvaluated,
    })
}

fn validate_id(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ProtocolError::Malformed);
    }
    Ok(())
}

fn approval_payload(approval: &WatchApproval) -> Vec<u8> {
    let mut payload = b"OpenContinuity P003 simulated Watch approval v1\0".to_vec();
    payload.extend_from_slice(&approval.version.to_be_bytes());
    put_bytes(&mut payload, &approval.nonce);
    put_string(&mut payload, &approval.host_id);
    put_string(&mut payload, &approval.local_user_id);
    put_string(&mut payload, &approval.session_id);
    put_string(&mut payload, &approval.action);
    payload.extend_from_slice(&approval.issued_at_ms.to_be_bytes());
    payload.extend_from_slice(&approval.expires_at_ms.to_be_bytes());
    put_string(&mut payload, &approval.identity);
    payload.push(u8::from(approval.approved));
    payload.push(match approval.user_presence {
        UserPresenceEvidence::ExplicitConfirmation => 1,
        UserPresenceEvidence::None => 0,
    });
    payload.extend_from_slice(&approval.responded_at_ms.to_be_bytes());
    payload
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    put_bytes(out, value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture() -> WatchEndpointFixture {
        WatchEndpointFixture {
            schema_version: 1,
            transport: TransportKind::IPhoneRelay,
            transport_connected: true,
            expected_watch_identity: "watch-se2-mock".into(),
            presented_watch_identity: "watch-se2-mock".into(),
            watch_approved: true,
            user_presence: UserPresenceEvidence::ExplicitConfirmation,
            now_ms: 10_000,
            challenge_lifetime_ms: 10_000,
            response_delay_ms: 50,
        }
    }

    fn challenge() -> WatchChallenge {
        let mut nonce_source = DeterministicNonceSource::new();
        WatchChallenge {
            version: PROTOCOL_VERSION,
            nonce: nonce_source.next_nonce().to_vec(),
            host_id: "host-1".into(),
            local_user_id: "user-1".into(),
            session_id: "session-1".into(),
            action: "login".into(),
            issued_at_ms: 100,
            expires_at_ms: 1_000,
        }
    }

    fn approval_for(challenge: &WatchChallenge, presence: UserPresenceEvidence) -> WatchApproval {
        let mut endpoint = MockWatchEndpoint::new("watch-1", true, presence).unwrap();
        endpoint.approve(challenge, 150).unwrap()
    }

    fn test_enrollment() -> MockEnrollmentRecord {
        let enrolled_key = SigningKey::from_bytes(&MOCK_ONLY_SIGNING_SEED)
            .verifying_key()
            .to_bytes();
        MockEnrollmentRecord::new_for_test("enroll-1", "host-1", "user-1", "watch-1", enrolled_key)
            .unwrap()
    }

    #[test]
    fn independently_enrolled_key_accepts_bound_mock_approval() {
        let challenge = challenge();
        let approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        let enrollment = test_enrollment();
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify_enrolled(Some(&enrollment), &challenge, &approval, 150),
            Ok(())
        );
    }

    #[test]
    fn unknown_revoked_and_inactive_enrollment_fail_closed() {
        let challenge = challenge();
        let approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify_enrolled(None, &challenge, &approval, 150),
            Err(ProtocolError::UnknownEnrollment)
        );
        let mut revoked = test_enrollment();
        revoked.revoked = true;
        assert_eq!(
            guard.verify_enrolled(Some(&revoked), &challenge, &approval, 150),
            Err(ProtocolError::EnrollmentRevoked)
        );
        let mut suspended = test_enrollment();
        suspended.lifecycle_state = EnrollmentLifecycle::Suspended;
        assert_eq!(
            guard.verify_enrolled(Some(&suspended), &challenge, &approval, 150),
            Err(ProtocolError::EnrollmentInactive)
        );
    }

    #[test]
    fn substituted_endpoint_key_cannot_impersonate_enrollment() {
        let challenge = challenge();
        let mut attacker = MockWatchEndpoint::with_signing_seed(
            "watch-1",
            true,
            UserPresenceEvidence::ExplicitConfirmation,
            MOCK_ATTACKER_SIGNING_SEED,
        )
        .unwrap();
        let forged = attacker.approve(&challenge, 150).unwrap();
        let enrollment = test_enrollment();
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify_enrolled(Some(&enrollment), &challenge, &forged, 150),
            Err(ProtocolError::InvalidSignature)
        );
    }

    #[test]
    fn enrolled_algorithm_host_account_and_endpoint_bindings_are_checked() {
        let challenge = challenge();
        let approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        let mut enrollment = test_enrollment();
        enrollment.key_algorithm = EnrollmentAlgorithm::P256EcdsaSha256V1;
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify_enrolled(Some(&enrollment), &challenge, &approval, 150),
            Err(ProtocolError::AlgorithmMismatch)
        );
        enrollment = test_enrollment();
        enrollment.host_identity = "other-host".into();
        assert_eq!(
            guard.verify_enrolled(Some(&enrollment), &challenge, &approval, 150),
            Err(ProtocolError::EnrollmentBindingMismatch)
        );
        enrollment = test_enrollment();
        enrollment.account_identity = "other-user".into();
        assert_eq!(
            guard.verify_enrolled(Some(&enrollment), &challenge, &approval, 150),
            Err(ProtocolError::EnrollmentBindingMismatch)
        );
        enrollment = test_enrollment();
        enrollment.watch_endpoint_identity = "other-watch".into();
        assert_eq!(
            guard.verify_enrolled(Some(&enrollment), &challenge, &approval, 150),
            Err(ProtocolError::IdentityMismatch)
        );
    }

    #[test]
    fn simulated_exchange_is_deterministic_and_never_authorizes() {
        let first = simulate(&fixture()).unwrap();
        let second = simulate(&fixture()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.authentication.result,
            AuthenticationResult::SimulatedVerified
        );
        assert!(first.authentication.replay_rejected);
        assert_eq!(first.authorization, AuthorizationState::NeverEvaluated);
        assert!(first.authentication.evidence.contains("no Apple Watch"));
    }

    #[test]
    fn capabilities_round_trip_with_explicit_evidence() {
        let capabilities = EndpointCapabilities::researched();
        let encoded = serde_json::to_value(&capabilities).unwrap();
        assert_eq!(
            encoded["watch_ble_peripheral_advertising"]["evidence"],
            "blocked"
        );
        assert_eq!(
            serde_json::from_value::<EndpointCapabilities>(encoded).unwrap(),
            capabilities
        );
    }

    #[test]
    fn rejects_unknown_fields_oversize_and_unsupported_versions() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value["untrusted_identity"] = json!(true);
        assert_eq!(
            WatchEndpointFixture::from_json(&value.to_string()),
            Err(ProtocolError::Malformed)
        );
        assert_eq!(
            WatchEndpointFixture::from_json(&" ".repeat(MAX_MESSAGE_BYTES + 1)),
            Err(ProtocolError::MessageTooLarge)
        );
        value = serde_json::to_value(fixture()).unwrap();
        value["schema_version"] = json!(99);
        assert_eq!(
            WatchEndpointFixture::from_json(&value.to_string()),
            Err(ProtocolError::UnsupportedVersion)
        );
    }

    #[test]
    fn challenge_message_is_versioned_bounded_and_strict() {
        let challenge = challenge();
        let encoded = serde_json::to_string(&challenge).unwrap();
        assert_eq!(WatchChallenge::from_json(&encoded), Ok(challenge.clone()));
        assert_eq!(
            WatchChallenge::from_json(&" ".repeat(MAX_MESSAGE_BYTES + 1)),
            Err(ProtocolError::MessageTooLarge)
        );
        let mut value = serde_json::to_value(challenge.clone()).unwrap();
        value["unexpected"] = json!(true);
        assert_eq!(
            WatchChallenge::from_json(&value.to_string()),
            Err(ProtocolError::Malformed)
        );
        value = serde_json::to_value(challenge).unwrap();
        value["version"] = json!(2);
        assert_eq!(
            WatchChallenge::from_json(&value.to_string()),
            Err(ProtocolError::UnsupportedVersion)
        );
    }

    #[test]
    fn nonce_generation_is_unique_and_deterministic() {
        let mut first = DeterministicNonceSource::new();
        let mut second = DeterministicNonceSource::new();
        let first_pair = [first.next_nonce(), first.next_nonce()];
        let second_pair = [second.next_nonce(), second.next_nonce()];
        assert_eq!(first_pair, second_pair);
        assert_ne!(first_pair[0], first_pair[1]);
    }

    #[test]
    fn rejects_duplicate_replay() {
        let challenge = challenge();
        let approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        let endpoint = MockWatchEndpoint::new("watch-1", true, UserPresenceEvidence::None).unwrap();
        let key = endpoint.public_key();
        let mut replay_guard = ReplayGuard::default();
        assert_eq!(
            replay_guard.verify(&challenge, &approval, "watch-1", &key, 150),
            Ok(())
        );
        assert_eq!(
            replay_guard.verify(&challenge, &approval, "watch-1", &key, 150),
            Err(ProtocolError::Replay)
        );
    }

    #[test]
    fn rejects_expired_or_future_responses() {
        let challenge = challenge();
        let approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        let endpoint = MockWatchEndpoint::new("watch-1", true, UserPresenceEvidence::None).unwrap();
        let key = endpoint.public_key();
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify(&challenge, &approval, "watch-1", &key, 1_000),
            Err(ProtocolError::Expired)
        );
        assert_eq!(
            guard.verify(&challenge, &approval, "watch-1", &key, 99),
            Err(ProtocolError::Expired)
        );
    }

    #[test]
    fn rejects_malformed_unexpected_identity_and_downgrade() {
        let challenge = challenge();
        let approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        let endpoint = MockWatchEndpoint::new("watch-1", true, UserPresenceEvidence::None).unwrap();
        let key = endpoint.public_key();
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify(&challenge, &approval, "different-watch", &key, 150),
            Err(ProtocolError::IdentityMismatch)
        );
        let mut downgraded = approval.clone();
        downgraded.version = 0;
        assert_eq!(
            WatchApproval::from_json(&serde_json::to_string(&downgraded).unwrap()),
            Err(ProtocolError::UnsupportedVersion)
        );
        let mut malformed = approval;
        malformed.nonce.pop();
        assert_eq!(
            WatchApproval::from_json(&serde_json::to_string(&malformed).unwrap()),
            Err(ProtocolError::Malformed)
        );
    }

    #[test]
    fn rejects_wrong_host_account_session_and_action_in_signed_transcript() {
        let challenge = challenge();
        let enrollment = test_enrollment();
        let endpoint =
            MockWatchEndpoint::new("watch-1", true, UserPresenceEvidence::ExplicitConfirmation)
                .unwrap();
        let approval = endpoint.clone().approve(&challenge, 150).unwrap();
        let mut variants = Vec::new();
        let mut changed = approval.clone();
        changed.host_id = "other-host".into();
        variants.push(changed);
        let mut changed = approval.clone();
        changed.local_user_id = "other-user".into();
        variants.push(changed);
        let mut changed = approval.clone();
        changed.session_id = "other-session".into();
        variants.push(changed);
        let mut changed = approval;
        changed.action = "other-action".into();
        variants.push(changed);
        for variant in variants {
            let mut guard = ReplayGuard::default();
            assert_eq!(
                guard.verify_enrolled(Some(&enrollment), &challenge, &variant, 150),
                Err(ProtocolError::Malformed)
            );
        }
    }

    #[test]
    fn rejects_missing_presence_user_rejection_transport_failure_and_tampering() {
        assert_eq!(
            simulate(&WatchEndpointFixture {
                user_presence: UserPresenceEvidence::None,
                ..fixture()
            })
            .unwrap()
            .authentication
            .reason
            .as_deref(),
            Some("explicit user-presence evidence is missing")
        );
        assert_eq!(
            simulate(&WatchEndpointFixture {
                watch_approved: false,
                ..fixture()
            })
            .unwrap()
            .authentication
            .reason
            .as_deref(),
            Some("Watch user rejected the request")
        );
        assert_eq!(
            simulate(&WatchEndpointFixture {
                transport_connected: false,
                ..fixture()
            })
            .unwrap()
            .authentication
            .reason
            .as_deref(),
            Some("transport is disconnected or unavailable")
        );
        let challenge = challenge();
        let mut approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        approval.approved = false;
        let endpoint = MockWatchEndpoint::new("watch-1", true, UserPresenceEvidence::None).unwrap();
        let key = endpoint.public_key();
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify(&challenge, &approval, "watch-1", &key, 150),
            Err(ProtocolError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_conflicting_identity_late_relay_and_wrong_session() {
        assert_eq!(
            simulate(&WatchEndpointFixture {
                presented_watch_identity: "unknown-watch".into(),
                ..fixture()
            })
            .unwrap()
            .authentication
            .reason
            .as_deref(),
            Some("endpoint identity does not match enrollment")
        );
        assert_eq!(
            simulate(&WatchEndpointFixture {
                challenge_lifetime_ms: 10,
                response_delay_ms: 20,
                ..fixture()
            })
            .unwrap()
            .authentication
            .reason
            .as_deref(),
            Some("challenge or response is expired or outside its validity window")
        );
        let challenge = challenge();
        let mut approval = approval_for(&challenge, UserPresenceEvidence::ExplicitConfirmation);
        approval.session_id = "other-session".into();
        let endpoint = MockWatchEndpoint::new("watch-1", true, UserPresenceEvidence::None).unwrap();
        let key = endpoint.public_key();
        let mut guard = ReplayGuard::default();
        assert_eq!(
            guard.verify(&challenge, &approval, "watch-1", &key, 150),
            Err(ProtocolError::Malformed)
        );
    }

    #[test]
    fn report_is_strictly_versioned_json() {
        let report = simulate(&fixture()).unwrap();
        let value: Value = serde_json::to_value(report).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["source"], "offline_mock_simulation_only");
        assert_eq!(value["mock_simulation"], true);
        assert_eq!(value["bluez_capability_discovery"], "not_performed");
        assert_eq!(
            value["experimental_gatt_transport"],
            "not_used_by_offline_mock"
        );
        assert_eq!(value["hardware_watch_observations"], "not_run");
        assert_eq!(value["authorization"], "never_evaluated");
    }
}
