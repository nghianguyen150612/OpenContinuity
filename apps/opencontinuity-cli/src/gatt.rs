//! Opt-in Linux BlueZ GATT research transports. P007 is synthetic; P009 carries
//! separately signed research messages and neither participates in OS
//! authentication or authorization.

use opencontinuity_core::watch_auth_research::{HostEnrollment, WatchEnrollment};
use p256::ecdsa::SigningKey;
use serde::Serialize;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

pub const MAX_DURATION_SECONDS: u16 = 300;
pub const MIN_DURATION_SECONDS: u16 = 1;
pub const MAX_AUTH_RUNTIME_SECONDS: u16 = 60;
pub const MAX_TEST_ENROLLMENT_LIFETIME_MS: u64 = 3_600_000;
pub const BLUEZ_METHOD_TIMEOUT: Duration = Duration::from_secs(2);
pub const BLUEZ_SETUP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Serialize)]
pub struct ExperimentReport {
    pub status: &'static str,
    pub stage: &'static str,
    pub adapter: String,
    pub runtime_seconds: u16,
    pub shutdown_reason: Option<&'static str>,
    pub application_registered: bool,
    pub advertisement_registered: bool,
    pub application_registration_succeeded: bool,
    pub advertisement_registration_succeeded: bool,
    pub cleanup_errors: Vec<String>,
    pub controller_le_peripheral_capability: &'static str,
    pub supported_advertising_instances: Option<u8>,
    pub active_advertising_instances: Option<u8>,
    pub error_class: Option<&'static str>,
    pub error: Option<String>,
    pub authorization: &'static str,
}

#[derive(Debug)]
pub struct ExperimentError {
    pub report: Box<ExperimentReport>,
}

/// Per-invocation, in-memory inputs for the separately opted-in P009 server.
/// Enrollments must be independently supplied and P008 test-only until a
/// reviewed production enrollment scheme exists.
pub struct AuthTransportConfig {
    pub host_signing_key: SigningKey,
    pub host_enrollment: HostEnrollment,
    pub watch_enrollment: WatchEnrollment,
    pub enrollment_expires_at_unix_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct AuthExperimentReport {
    pub transport: ExperimentReport,
    pub research_decision: Option<&'static str>,
    pub host_public_key_fingerprint_sha256: String,
    pub watch_public_key_fingerprint_sha256: String,
    pub enrollment_expires_at_unix_ms: u64,
    pub authorization: &'static str,
}

pub fn validate_options(adapter: &str, duration: u16, experimental: bool) -> Result<(), String> {
    if !experimental {
        return Err("the GATT server requires explicit --experimental opt-in".into());
    }
    if !(MIN_DURATION_SECONDS..=MAX_DURATION_SECONDS).contains(&duration) {
        return Err(format!(
            "duration must be between {MIN_DURATION_SECONDS} and {MAX_DURATION_SECONDS} seconds"
        ));
    }
    let suffix = adapter.strip_prefix("hci").unwrap_or_default();
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("adapter must explicitly name a BlueZ adapter such as hci0".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use futures_lite::{FutureExt, StreamExt};
    use opencontinuity_core::gatt_wire::{
        ControlCommand, FrameAssembler, FrameProgress, MAX_ATT_MTU, MIN_ATT_MTU, WIRE_VERSION,
        WireError,
    };
    use opencontinuity_core::{
        watch_auth_research::{
            Decision, ReplayGuard, VerifiedHostRequest, WatchAuthResponse, build_host_request,
            verify_host_request, verify_watch_response,
        },
        watch_auth_transport::{
            BLE_INACTIVITY_TIMEOUT, Direction as AuthDirection, REQUEST_TRANSFER_TIMEOUT,
            RESPONSE_TRANSFER_TIMEOUT, RequestChunkMailbox, RequestCommand, SignedEnvelope,
            TransferAssembler, TransportError as AuthTransportError, WAITING_FOR_CLIENT_TIMEOUT,
            WAITING_FOR_CONSENT_TIMEOUT, parse_request_command,
        },
    };
    use p256::ecdsa::SigningKey;
    use sha2::{Digest, Sha256};
    use std::{collections::HashMap, fs::File, io::Read, sync::Mutex, thread};
    use zbus::{
        DBusError,
        blocking::{Connection, Proxy},
        fdo::ObjectManager,
        zvariant::{ObjectPath, OwnedObjectPath, OwnedValue},
    };

    const BLUEZ: &str = "org.bluez";
    const OM: &str = "org.freedesktop.DBus.ObjectManager";
    const ADAPTER: &str = "org.bluez.Adapter1";
    const GATT_MANAGER: &str = "org.bluez.GattManager1";
    const ADVERTISING_MANAGER: &str = "org.bluez.LEAdvertisingManager1";
    const SERVICE_UUID: &str = "8e6f0001-6f4e-4c4f-9b4f-4f70656e436f";
    const VERSION_UUID: &str = "8e6f0002-6f4e-4c4f-9b4f-4f70656e436f";
    const CHALLENGE_UUID: &str = "8e6f0003-6f4e-4c4f-9b4f-4f70656e436f";
    const EXCHANGE_UUID: &str = "8e6f0004-6f4e-4c4f-9b4f-4f70656e436f";
    const RESPONSE_UUID: &str = "8e6f0005-6f4e-4c4f-9b4f-4f70656e436f";
    const CONTROL_UUID: &str = "8e6f0006-6f4e-4c4f-9b4f-4f70656e436f";
    const AUTH_SERVICE_UUID: &str = opencontinuity_core::watch_auth_transport::SERVICE_UUID;
    const AUTH_REQUEST_DATA_UUID: &str =
        opencontinuity_core::watch_auth_transport::REQUEST_DATA_UUID;
    const AUTH_REQUEST_CONTROL_UUID: &str =
        opencontinuity_core::watch_auth_transport::REQUEST_CONTROL_UUID;
    const AUTH_RESPONSE_DATA_UUID: &str =
        opencontinuity_core::watch_auth_transport::RESPONSE_DATA_UUID;
    const AUTH_RESPONSE_CONTROL_UUID: &str =
        opencontinuity_core::watch_auth_transport::RESPONSE_CONTROL_UUID;
    const AUTH_STATUS_UUID: &str = opencontinuity_core::watch_auth_transport::STATUS_UUID;

    #[derive(Debug, DBusError)]
    #[zbus(prefix = "org.bluez.Error")]
    enum BluezGattError {
        Failed(String),
        InvalidArgs(String),
        InvalidValueLength(String),
        NotPermitted(String),
        NotAuthorized(String),
        InvalidOffset(String),
        #[zbus(error)]
        ZBus(zbus::Error),
    }

    fn as_bluez_error(error: WireError) -> BluezGattError {
        use WireError::*;
        match error {
            FrameTooLarge | MessageTooLarge | LengthMismatch => {
                BluezGattError::InvalidValueLength(format!("invalid GATT frame: {error:?}"))
            }
            SessionMismatch
            | VersionMismatch
            | DuplicateOrOutOfOrder
            | StartRequired
            | MessageInProgress
            | PartialMessageExpired
            | AlreadyComplete
            | Malformed
            | InvalidAttMtu => BluezGattError::Failed(format!("invalid GATT frame: {error:?}")),
        }
    }

    const MAX_CLIENTS: usize = 16;
    const MAX_DEVICE_PATH: usize = 128;
    const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
    const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

    fn response_transfer_timeout(now: Instant, signed_deadline: Instant) -> Duration {
        RESPONSE_TRANSFER_TIMEOUT.min(signed_deadline.saturating_duration_since(now))
    }

    fn consent_deadline_elapsed(
        consent_deadline: Option<Instant>,
        response_started: bool,
        now: Instant,
    ) -> bool {
        !response_started && consent_deadline.is_some_and(|deadline| now >= deadline)
    }

    fn option_u16(
        options: &HashMap<String, OwnedValue>,
        key: &str,
        default: u16,
    ) -> Result<u16, BluezGattError> {
        match options.get(key) {
            None => Ok(default),
            Some(value) => u16::try_from(value.clone())
                .map_err(|_| BluezGattError::InvalidArgs(format!("invalid {key} option"))),
        }
    }

    fn option_device(options: &HashMap<String, OwnedValue>) -> Result<String, BluezGattError> {
        let path = options
            .get("device")
            .and_then(|value| OwnedObjectPath::try_from(value.clone()).ok())
            .map(|path| path.to_string())
            .ok_or_else(|| {
                BluezGattError::InvalidArgs("missing or invalid device option".into())
            })?;
        if path.len() > MAX_DEVICE_PATH || !path.starts_with("/org/bluez/") {
            return Err(BluezGattError::InvalidArgs("invalid device path".into()));
        }
        Ok(path)
    }

    fn read_options(
        options: &HashMap<String, OwnedValue>,
    ) -> Result<(usize, usize), BluezGattError> {
        let offset = option_u16(options, "offset", 0)? as usize;
        let mtu = option_u16(options, "mtu", 23)? as usize;
        if !(MIN_ATT_MTU..=MAX_ATT_MTU).contains(&mtu) {
            return Err(BluezGattError::InvalidArgs("invalid ATT MTU".into()));
        }
        Ok((offset, mtu))
    }

    fn read_fragment(value: &[u8], offset: usize, mtu: usize) -> Result<Vec<u8>, BluezGattError> {
        if offset > value.len() {
            return Err(BluezGattError::InvalidOffset(
                "read offset exceeds value length".into(),
            ));
        }
        let end = (offset + mtu - 1).min(value.len());
        Ok(value[offset..end].to_vec())
    }

    fn random_bytes<const N: usize>() -> std::io::Result<[u8; N]> {
        let mut bytes = [0u8; N];
        File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    // Exactly one phase owns the assembler or result. A result keeps its original
    // ID until its last read fragment is delivered; no new challenge is issued.
    enum SessionPhase {
        ChallengeIssued(FrameAssembler),
        Receiving(FrameAssembler),
        AwaitingCompletion(FrameAssembler),
        ResponseAvailable { value: Vec<u8>, since: Instant },
        Consumed,
    }

    struct ClientSession {
        session_id: [u8; 8],
        challenge: [u8; 16],
        phase: SessionPhase,
        last_activity: Instant,
    }

    impl ClientSession {
        fn new() -> std::io::Result<Self> {
            let session_id = random_bytes()?;
            Ok(Self {
                session_id,
                challenge: random_bytes()?,
                phase: SessionPhase::ChallengeIssued(FrameAssembler::new(session_id)),
                last_activity: Instant::now(),
            })
        }

        fn challenge_bytes(&self) -> Vec<u8> {
            [
                vec![WIRE_VERSION],
                self.session_id.to_vec(),
                self.challenge.to_vec(),
            ]
            .concat()
        }

        fn status_response(&self, status: u8, payload: &[u8]) -> Vec<u8> {
            let digest = Sha256::digest(payload);
            [
                vec![WIRE_VERSION, status],
                self.session_id.to_vec(),
                (payload.len() as u16).to_le_bytes().to_vec(),
                digest.to_vec(),
            ]
            .concat()
        }

        fn error_response(&self, error: WireError) -> Vec<u8> {
            [
                vec![WIRE_VERSION, 0x80],
                self.session_id.to_vec(),
                vec![error_code(error)],
            ]
            .concat()
        }

        fn respond(&mut self, value: Vec<u8>) {
            self.phase = SessionPhase::ResponseAvailable {
                value,
                since: Instant::now(),
            };
        }

        fn expired(&self, now: Instant) -> bool {
            now.saturating_duration_since(self.last_activity) > IDLE_TIMEOUT
                || matches!(&self.phase, SessionPhase::ResponseAvailable { since, .. } if now.saturating_duration_since(*since) > RESPONSE_TIMEOUT)
        }
    }

    struct AuthSessionInner {
        transfer_id: [u8; 8],
        envelope: Option<SignedEnvelope>,
        envelope_length: u16,
        owner: Option<String>,
        request_mailbox: Option<RequestChunkMailbox>,
        requested_frame: Option<Vec<u8>>,
        request_complete: bool,
        response_assembler: Option<TransferAssembler>,
        verified_request: Option<VerifiedHostRequest>,
        response_replay: ReplayGuard,
        expires_at_ms: u64,
        signed_deadline: Instant,
        client_deadline: Instant,
        consent_deadline: Option<Instant>,
        retired: bool,
        terminal_reason: Option<&'static str>,
        decision: Option<&'static str>,
    }

    struct AuthTransportState {
        host_enrollment: HostEnrollment,
        watch_enrollment: WatchEnrollment,
        enrollment_expires_at_unix_ms: u64,
        host_fingerprint: String,
        watch_fingerprint: String,
        host_signing_key: Mutex<Option<SigningKey>>,
        inner: Mutex<Option<AuthSessionInner>>,
    }

    impl AuthTransportState {
        fn new(config: AuthTransportConfig) -> Result<Self, String> {
            if !config.host_enrollment.test_only || !config.watch_enrollment.test_only {
                return Err(
                    "P009 live enrollment is blocked; only explicit test-only pins are accepted"
                        .into(),
                );
            }
            if config.host_enrollment.state
                != opencontinuity_core::watch_auth_research::EnrollmentState::Active
                || config.watch_enrollment.state
                    != opencontinuity_core::watch_auth_research::EnrollmentState::Active
                || config.host_enrollment.key_algorithm
                    != opencontinuity_core::watch_auth_research::KeyAlgorithm::P256EcdsaSha256
                || config.watch_enrollment.key_algorithm
                    != opencontinuity_core::watch_auth_research::KeyAlgorithm::P256EcdsaSha256
                || config.host_enrollment.key_version != 1
                || config.watch_enrollment.key_version != 1
                || config.watch_enrollment.bound_host_enrollment_id
                    != config.host_enrollment.enrollment_id
                || config.watch_enrollment.bound_host_identity
                    != config.host_enrollment.host_identity
                || config.watch_enrollment.bound_linux_uid != config.host_enrollment.linux_uid
            {
                return Err(
                    "P009 Watch enrollment is inactive or not bound to the selected host/account"
                        .into(),
                );
            }
            let now_ms = unix_now_ms()?;
            if config.enrollment_expires_at_unix_ms <= now_ms {
                return Err(
                    "test enrollment is expired; import and pin a fresh test enrollment".into(),
                );
            }
            if config.enrollment_expires_at_unix_ms - now_ms > MAX_TEST_ENROLLMENT_LIFETIME_MS {
                return Err("test enrollment exceeds the one-hour validity limit".into());
            }
            let host_public = hex_bytes(
                config
                    .host_signing_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes(),
            );
            if host_public != config.host_enrollment.public_key_sec1_hex {
                return Err(
                    "host signing key does not match the independently pinned host public key"
                        .into(),
                );
            }
            let watch_public = decode_lower_hex(&config.watch_enrollment.public_key_sec1_hex)?;
            p256::ecdsa::VerifyingKey::from_sec1_bytes(&watch_public).map_err(|_| {
                "Watch enrollment public key is incomplete or invalid P-256 SEC1".to_owned()
            })?;
            let host_fingerprint = fingerprint(
                config
                    .host_signing_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes(),
            );
            let watch_fingerprint = fingerprint(&watch_public);
            Ok(Self {
                host_enrollment: config.host_enrollment,
                watch_enrollment: config.watch_enrollment,
                enrollment_expires_at_unix_ms: config.enrollment_expires_at_unix_ms,
                host_fingerprint,
                watch_fingerprint,
                host_signing_key: Mutex::new(Some(config.host_signing_key)),
                inner: Mutex::new(None),
            })
        }

        /// Sign only after BlueZ has registered both objects and the
        /// advertisement is ready, so setup delay cannot consume P008 TTL.
        fn activate(&self, ready_at: Instant) -> Result<(), String> {
            self.activate_at(ready_at, unix_now_ms()?)
        }

        fn activate_at(&self, ready_at: Instant, now_ms: u64) -> Result<(), String> {
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| "research session state lock poisoned".to_owned())?;
            if guard.is_some() {
                return Err("research session was already initialized".into());
            }
            if self.enrollment_expires_at_unix_ms <= now_ms {
                return Err(
                    "test enrollment expired during BlueZ setup; no request was signed".into(),
                );
            }
            let signing_key = self
                .host_signing_key
                .lock()
                .map_err(|_| "host signing key state lock poisoned".to_owned())?
                .take()
                .ok_or_else(|| "host signing key was already consumed".to_owned())?;
            let enrollment_remaining_ms = self.enrollment_expires_at_unix_ms - now_ms;
            let request_lifetime_ms =
                opencontinuity_core::watch_auth_research::MAX_REQUEST_LIFETIME_MS
                    .min(enrollment_remaining_ms);
            let signed = build_host_request(
                &self.host_enrollment,
                &signing_key,
                self.watch_enrollment.enrollment_id.clone(),
                now_ms,
                request_lifetime_ms,
            )
            .map_err(|error| {
                format!("could not create fresh P008 request at readiness: {error}")
            })?;
            drop(signing_key);
            let canonical = signed
                .request
                .canonical_bytes()
                .map_err(|error| format!("could not encode fresh P008 request: {error}"))?;
            let envelope = SignedEnvelope::new(
                AuthDirection::HostToWatch,
                &canonical,
                &signed.signature_raw,
            )
            .map_err(|error| format!("fresh P008 request is not a valid envelope: {error:?}"))?;
            let verified_request = verify_host_request(
                Some(&self.host_enrollment),
                signed.request.clone(),
                &signed.signature_raw,
                &self.watch_enrollment.enrollment_id,
                now_ms,
                &mut ReplayGuard::default(),
            )
            .map_err(|error| format!("fresh host request verification failed: {error}"))?;
            let transfer_id = random_bytes::<8>()
                .map_err(|error| format!("random source unavailable: {error}"))?;
            let envelope_length = u16::try_from(envelope.as_bytes().len())
                .map_err(|_| "signed request envelope is too large".to_owned())?;
            let signed_remaining = signed
                .request
                .expires_at_ms
                .checked_sub(now_ms)
                .ok_or_else(|| "fresh signed request expired before readiness".to_owned())?;
            *guard = Some(AuthSessionInner {
                transfer_id,
                envelope: Some(envelope),
                envelope_length,
                owner: None,
                request_mailbox: None,
                requested_frame: None,
                request_complete: false,
                response_assembler: None,
                verified_request: Some(verified_request),
                response_replay: ReplayGuard::default(),
                expires_at_ms: signed.request.expires_at_ms,
                signed_deadline: ready_at + Duration::from_millis(signed_remaining),
                client_deadline: ready_at + WAITING_FOR_CLIENT_TIMEOUT,
                consent_deadline: None,
                retired: false,
                terminal_reason: None,
                decision: None,
            });
            Ok(())
        }

        fn claim_owner(
            inner: &mut AuthSessionInner,
            device: &str,
            now: Instant,
            now_ms: u64,
        ) -> Result<(), BluezGattError> {
            if inner.retired || inner.decision.is_some() {
                return Err(BluezGattError::NotPermitted(
                    "research session is no longer available".into(),
                ));
            }
            if now >= inner.signed_deadline || now_ms >= inner.expires_at_ms {
                Self::retire(inner, "signed_request_expired");
                return Err(BluezGattError::NotPermitted(
                    "signed P008 request expired; start a new research server session for a fresh request, nonce, and session".into(),
                ));
            }
            if inner.owner.is_none() && now >= inner.client_deadline {
                Self::retire(inner, "client_wait_timeout");
                return Err(BluezGattError::NotPermitted(
                    "no Watch connected before the bounded client wait ended; start a new research server session".into(),
                ));
            }
            if consent_deadline_elapsed(
                inner.consent_deadline,
                inner.response_assembler.is_some(),
                now,
            ) {
                Self::retire(inner, "consent_timeout");
                return Err(BluezGattError::NotPermitted(
                    "waiting-for-consent deadline elapsed; start a new session with a fresh signed request".into(),
                ));
            }
            match &inner.owner {
                Some(owner) if owner != device => {
                    return Err(BluezGattError::NotPermitted(
                        "another client owns this research session".into(),
                    ));
                }
                Some(_) => return Ok(()),
                None => {}
            }
            let envelope = inner.envelope.as_ref().ok_or_else(|| {
                BluezGattError::NotPermitted("research request was invalidated".into())
            })?;
            let started_at = Instant::now();
            let mailbox = RequestChunkMailbox::new_with_timeout(
                inner.transfer_id,
                device,
                envelope,
                23,
                started_at,
                REQUEST_TRANSFER_TIMEOUT
                    .min(inner.signed_deadline.saturating_duration_since(started_at)),
                Some(BLE_INACTIVITY_TIMEOUT),
            )
            .map_err(auth_transport_error)?;
            inner.owner = Some(device.to_owned());
            inner.request_mailbox = Some(mailbox);
            Ok(())
        }

        fn read(
            &self,
            device: &str,
            kind: CharacteristicKind,
            offset: usize,
            mtu: usize,
        ) -> Result<Vec<u8>, BluezGattError> {
            if offset != 0 {
                return Err(BluezGattError::InvalidOffset(
                    "P009 values use explicit chunk indices".into(),
                ));
            }
            let now = Instant::now();
            let now_ms = unix_now_ms().map_err(BluezGattError::Failed)?;
            let mut guard = self.inner.lock().map_err(|_| {
                BluezGattError::Failed("research session state lock poisoned".into())
            })?;
            let inner = guard.as_mut().ok_or_else(|| BluezGattError::NotPermitted(
                "research transport is not ready; request signing follows successful BlueZ registration".into()
            ))?;
            Self::claim_owner(inner, device, now, now_ms)?;
            let value = match kind {
                CharacteristicKind::AuthStatus => [
                    vec![
                        b'O',
                        b'A',
                        opencontinuity_core::watch_auth_transport::PROTOCOL_VERSION,
                    ],
                    inner.transfer_id.to_vec(),
                    inner.envelope_length.to_le_bytes().to_vec(),
                ]
                .concat(),
                CharacteristicKind::AuthRequestData => {
                    inner
                        .request_mailbox
                        .as_ref()
                        .ok_or_else(|| {
                            BluezGattError::NotPermitted("request mailbox is unavailable".into())
                        })?
                        .validate_live(device, inner.transfer_id, Instant::now())
                        .map_err(auth_transport_error)?;
                    inner.requested_frame.clone().ok_or_else(|| {
                        BluezGattError::NotPermitted(
                            "request frame requires an explicit GET".into(),
                        )
                    })?
                }
                _ => {
                    return Err(BluezGattError::NotPermitted(
                        "characteristic is not readable".into(),
                    ));
                }
            };
            if value.len() > mtu.saturating_sub(1) {
                return Err(BluezGattError::InvalidValueLength(
                    "P009 read value exceeds the negotiated ATT limit".into(),
                ));
            }
            Ok(value)
        }

        fn write(
            &self,
            device: &str,
            kind: CharacteristicKind,
            value: &[u8],
            mtu: usize,
        ) -> Result<(), BluezGattError> {
            let now = Instant::now();
            let now_ms = unix_now_ms().map_err(BluezGattError::Failed)?;
            self.write_at(device, kind, value, mtu, now, now_ms)
        }

        fn write_at(
            &self,
            device: &str,
            kind: CharacteristicKind,
            value: &[u8],
            mtu: usize,
            now: Instant,
            now_ms: u64,
        ) -> Result<(), BluezGattError> {
            let mut guard = self.inner.lock().map_err(|_| {
                BluezGattError::Failed("research session state lock poisoned".into())
            })?;
            let inner = guard.as_mut().ok_or_else(|| BluezGattError::NotPermitted(
                "research transport is not ready; request signing follows successful BlueZ registration".into()
            ))?;
            Self::claim_owner(inner, device, now, now_ms)?;
            match kind {
                CharacteristicKind::AuthRequestControl => {
                    let (transfer_id, command) =
                        parse_request_command(value).map_err(auth_transport_error)?;
                    if transfer_id != inner.transfer_id {
                        return Err(auth_transport_error(AuthTransportError::SessionMismatch));
                    }
                    let mailbox = inner.request_mailbox.as_mut().ok_or_else(|| {
                        BluezGattError::NotPermitted("request mailbox is unavailable".into())
                    })?;
                    match command {
                        RequestCommand::Get { index } => {
                            inner.requested_frame = Some(
                                mailbox
                                    .get(device, transfer_id, index as usize, now)
                                    .map_err(auth_transport_error)?
                                    .to_vec(),
                            );
                        }
                        RequestCommand::Ack { index } => {
                            mailbox
                                .acknowledge(device, transfer_id, index as usize, now)
                                .map_err(auth_transport_error)?;
                            inner.requested_frame = None;
                        }
                        RequestCommand::Complete => {
                            mailbox
                                .complete(device, transfer_id, now)
                                .map_err(auth_transport_error)?;
                            inner.request_complete = true;
                            inner.consent_deadline = Some(
                                now + WAITING_FOR_CONSENT_TIMEOUT
                                    .min(inner.signed_deadline.saturating_duration_since(now)),
                            );
                            inner.requested_frame = None;
                        }
                        RequestCommand::Cancel => {
                            mailbox
                                .cancel(device, transfer_id)
                                .map_err(auth_transport_error)?;
                            Self::retire(inner, "watch_cancelled");
                        }
                    }
                    Ok(())
                }
                CharacteristicKind::AuthResponseData => {
                    if !inner.request_complete {
                        return Err(BluezGattError::NotPermitted(
                            "signed request delivery must complete first".into(),
                        ));
                    }
                    if inner.decision.is_some() {
                        return Err(BluezGattError::NotPermitted(
                            "research response was already completed".into(),
                        ));
                    }
                    let starts_response = inner.response_assembler.is_none();
                    if starts_response {
                        let phase_timeout = response_transfer_timeout(now, inner.signed_deadline);
                        if phase_timeout.is_zero() {
                            Self::retire(inner, "signed_request_expired");
                            return Err(BluezGattError::NotPermitted(
                                "signed request expired before the Watch response began; start a new session".into(),
                            ));
                        }
                        inner.response_assembler = Some(TransferAssembler::new_with_timeout(
                            inner.transfer_id,
                            AuthDirection::WatchToHost,
                            device,
                            now,
                            phase_timeout,
                            Some(BLE_INACTIVITY_TIMEOUT),
                        ));
                    }
                    let result = inner
                        .response_assembler
                        .as_mut()
                        .expect("assembler created above")
                        .push(device, AuthDirection::WatchToHost, value, mtu, now)
                        .map_err(auth_transport_error);
                    if result.is_err() {
                        Self::retire(inner, "invalid_response_transport");
                    } else if starts_response {
                        // Consent has been delivered and the first valid response
                        // frame owns the separate response-transfer deadline.
                        inner.consent_deadline = None;
                    }
                    result
                }
                CharacteristicKind::AuthResponseControl => {
                    let (transfer_id, command) =
                        parse_request_command(value).map_err(auth_transport_error)?;
                    if transfer_id != inner.transfer_id {
                        return Err(auth_transport_error(AuthTransportError::SessionMismatch));
                    }
                    match command {
                        RequestCommand::Cancel => {
                            Self::retire(inner, "watch_cancelled");
                            Ok(())
                        }
                        RequestCommand::Complete => {
                            if !inner.request_complete {
                                return Err(BluezGattError::NotPermitted(
                                    "signed request delivery must complete first".into(),
                                ));
                            }
                            let envelope = inner
                                .response_assembler
                                .as_mut()
                                .ok_or_else(|| {
                                    auth_transport_error(AuthTransportError::IncompleteTransfer)
                                })?
                                .complete(device, transfer_id, now)
                                .map_err(auth_transport_error)?;
                            let response = WatchAuthResponse::parse_canonical(envelope.canonical())
                                .map_err(|error| {
                                    BluezGattError::NotAuthorized(format!(
                                        "invalid signed response: {error}"
                                    ))
                                })?;
                            let AuthSessionInner {
                                verified_request,
                                response_replay,
                                ..
                            } = &mut *inner;
                            let verified_request = verified_request.as_ref().ok_or_else(|| {
                                BluezGattError::NotPermitted(
                                    "verified request token was discarded".into(),
                                )
                            })?;
                            if let Err(error) = verify_watch_response(
                                &self.host_enrollment,
                                Some(&self.watch_enrollment),
                                verified_request,
                                &response,
                                envelope.signature(),
                                now_ms,
                                response_replay,
                            ) {
                                Self::retire(inner, "response_verification_failed");
                                return Err(BluezGattError::NotAuthorized(format!(
                                    "P008 response verification failed: {error}"
                                )));
                            }
                            inner.decision = Some(match response.decision {
                                Decision::Approved => "approved",
                                Decision::Rejected => "rejected",
                            });
                            inner.verified_request = None;
                            Ok(())
                        }
                        _ => Err(auth_transport_error(
                            AuthTransportError::InvalidChunkCommand,
                        )),
                    }
                }
                _ => Err(BluezGattError::NotPermitted(
                    "characteristic is not writable".into(),
                )),
            }
        }

        fn invalidate(&self, device: &str) {
            if let Ok(mut guard) = self.inner.lock() {
                if let Some(inner) = guard.as_mut() {
                    if inner.owner.as_deref() == Some(device) {
                        Self::retire(inner, "connection_lost");
                    }
                }
            }
        }

        fn invalidate_all(&self) {
            if let Ok(mut guard) = self.inner.lock() {
                if let Some(inner) = guard.as_mut() {
                    Self::retire(inner, "server_stopped");
                }
            }
        }

        fn prune(&self, connected: &HashMap<String, bool>) {
            if let Ok(mut guard) = self.inner.lock() {
                let Some(inner) = guard.as_mut() else {
                    return;
                };
                if inner
                    .owner
                    .as_ref()
                    .is_some_and(|owner| !connected.get(owner).copied().unwrap_or(false))
                {
                    Self::retire(inner, "connection_lost");
                } else if Instant::now() >= inner.signed_deadline
                    || unix_now_ms().is_ok_and(|now| now >= inner.expires_at_ms)
                {
                    Self::retire(inner, "signed_request_expired");
                } else if inner.owner.is_none() && Instant::now() >= inner.client_deadline {
                    Self::retire(inner, "client_wait_timeout");
                } else if consent_deadline_elapsed(
                    inner.consent_deadline,
                    inner.response_assembler.is_some(),
                    Instant::now(),
                ) {
                    Self::retire(inner, "consent_timeout");
                }
            }
        }

        fn decision(&self) -> Option<&'static str> {
            self.inner
                .lock()
                .ok()
                .and_then(|guard| guard.as_ref().and_then(|inner| inner.decision))
        }

        fn host_fingerprint(&self) -> &str {
            &self.host_fingerprint
        }

        fn watch_fingerprint(&self) -> &str {
            &self.watch_fingerprint
        }

        fn deadline_shutdown_reason(&self) -> Option<&'static str> {
            let mut guard = self.inner.lock().ok()?;
            let inner = guard.as_mut()?;
            if let Some(reason) = inner.terminal_reason {
                return Some(reason);
            }
            let now = Instant::now();
            let reason = if now >= inner.signed_deadline
                || unix_now_ms().is_ok_and(|now_ms| now_ms >= inner.expires_at_ms)
            {
                Some("signed_request_expired")
            } else if inner.owner.is_none() && now >= inner.client_deadline {
                Some("client_wait_timeout")
            } else if consent_deadline_elapsed(
                inner.consent_deadline,
                inner.response_assembler.is_some(),
                now,
            ) {
                Some("consent_timeout")
            } else {
                None
            }?;
            Self::retire(inner, reason);
            Some(reason)
        }

        fn retire(inner: &mut AuthSessionInner, reason: &'static str) {
            inner.retired = true;
            inner.terminal_reason = Some(reason);
            inner.envelope = None;
            inner.request_mailbox = None;
            inner.requested_frame = None;
            inner.response_assembler = None;
            inner.verified_request = None;
        }
    }

    fn unix_now_ms() -> Result<u64, String> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .map_err(|_| "system clock is before the Unix epoch".to_owned())
    }

    fn hex_bytes(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        encoded
    }

    fn decode_lower_hex(value: &str) -> Result<Vec<u8>, String> {
        if value.len() % 2 != 0
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("public key must use lowercase hexadecimal SEC1 bytes".into());
        }
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let digit = |byte: u8| match byte {
                    b'0'..=b'9' => Ok(byte - b'0'),
                    b'a'..=b'f' => Ok(byte - b'a' + 10),
                    _ => Err("invalid lowercase hexadecimal public key".to_owned()),
                };
                Ok((digit(pair[0])? << 4) | digit(pair[1])?)
            })
            .collect()
    }

    fn fingerprint(public_key: &[u8]) -> String {
        hex_bytes(&Sha256::digest(public_key))
    }

    fn auth_transport_error(error: AuthTransportError) -> BluezGattError {
        use AuthTransportError as E;
        match error {
            E::FrameTooLarge | E::MessageTooLarge | E::LengthMismatch => {
                BluezGattError::InvalidValueLength(format!(
                    "invalid research transport value: {error:?}"
                ))
            }
            E::OwnerMismatch
            | E::SessionMismatch
            | E::DirectionMismatch
            | E::VersionMismatch
            | E::DuplicateOrOutOfOrder
            | E::StartRequired
            | E::MessageInProgress
            | E::InvalidEnvelope
            | E::Malformed
            | E::InvalidAttMtu
            | E::TransferExpired
            | E::InactivityExpired
            | E::IncompleteTransfer
            | E::AlreadyCompleted
            | E::DuplicateCompletion
            | E::Cancelled
            | E::InvalidChunkCommand => BluezGattError::NotPermitted(format!(
                "research transport rejected value: {error:?}"
            )),
        }
    }

    fn error_code(error: WireError) -> u8 {
        use WireError::*;
        match error {
            InvalidAttMtu => 1,
            FrameTooLarge => 2,
            Malformed => 3,
            VersionMismatch => 4,
            SessionMismatch => 5,
            DuplicateOrOutOfOrder => 6,
            StartRequired => 7,
            MessageInProgress => 8,
            MessageTooLarge => 9,
            LengthMismatch => 10,
            PartialMessageExpired => 11,
            AlreadyComplete => 12,
        }
    }

    struct GattState {
        clients: Mutex<HashMap<String, ClientSession>>,
        auth_transport: Option<Arc<AuthTransportState>>,
    }

    impl Default for GattState {
        fn default() -> Self {
            Self {
                clients: Mutex::new(HashMap::new()),
                auth_transport: None,
            }
        }
    }

    impl GattState {
        fn with_auth_transport(auth_transport: Arc<AuthTransportState>) -> Self {
            Self {
                clients: Mutex::new(HashMap::new()),
                auth_transport: Some(auth_transport),
            }
        }

        fn client_value(
            &self,
            device: &str,
            kind: CharacteristicKind,
            offset: usize,
            mtu: usize,
        ) -> Result<Vec<u8>, BluezGattError> {
            if kind == CharacteristicKind::Version {
                return read_fragment(&[WIRE_VERSION], offset, mtu);
            }
            if kind.is_auth() {
                return self
                    .auth_transport
                    .as_ref()
                    .ok_or_else(|| {
                        BluezGattError::NotPermitted(
                            "authenticated transport is unavailable".into(),
                        )
                    })?
                    .read(device, kind, offset, mtu);
            }
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let now = Instant::now();
            clients.retain(|_, session| !session.expired(now));
            if kind == CharacteristicKind::Challenge
                && offset == 0
                && clients
                    .get(device)
                    .is_none_or(|s| matches!(s.phase, SessionPhase::Consumed))
            {
                if !clients.contains_key(device) && clients.len() >= MAX_CLIENTS {
                    return Err(BluezGattError::NotPermitted("client limit reached".into()));
                }
                clients.insert(
                    device.to_owned(),
                    ClientSession::new().map_err(|error| {
                        BluezGattError::Failed(format!(
                            "operating-system random source failed: {error}"
                        ))
                    })?,
                );
            }
            let session = clients
                .get_mut(device)
                .ok_or_else(|| BluezGattError::NotPermitted("read a new challenge first".into()))?;
            let value = match kind {
                CharacteristicKind::Challenge
                    if matches!(
                        session.phase,
                        SessionPhase::ChallengeIssued(_)
                            | SessionPhase::Receiving(_)
                            | SessionPhase::AwaitingCompletion(_)
                    ) =>
                {
                    session.challenge_bytes()
                }
                CharacteristicKind::Response => match &session.phase {
                    SessionPhase::ResponseAvailable { value, .. } => value.clone(),
                    _ => {
                        return Err(BluezGattError::NotPermitted(
                            "no synthetic response is available".into(),
                        ));
                    }
                },
                _ => {
                    return Err(BluezGattError::NotPermitted(
                        "characteristic is unavailable in this session phase".into(),
                    ));
                }
            };
            let fragment = read_fragment(&value, offset, mtu)?;
            session.last_activity = now;
            if kind == CharacteristicKind::Response
                && offset + fragment.len() == value.len()
                && fragment.len() < mtu - 1
            {
                session.phase = SessionPhase::Consumed;
            }
            Ok(fragment)
        }

        fn write_exchange(
            &self,
            device: &str,
            frame: &[u8],
            mtu: usize,
        ) -> Result<(), BluezGattError> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let now = Instant::now();
            clients.retain(|_, session| !session.expired(now));
            let session = clients.get_mut(device).ok_or_else(|| {
                BluezGattError::NotPermitted(
                    "read the challenge characteristic before writing".into(),
                )
            })?;
            let result = match &mut session.phase {
                SessionPhase::ChallengeIssued(assembler) | SessionPhase::Receiving(assembler) => {
                    assembler.push(frame, mtu, now)
                }
                _ => {
                    return Err(BluezGattError::NotPermitted(
                        "exchange unavailable in this session phase".into(),
                    ));
                }
            };
            session.last_activity = now;
            match result {
                Ok(progress) => {
                    let old = std::mem::replace(&mut session.phase, SessionPhase::Consumed);
                    let assembler = match old {
                        SessionPhase::ChallengeIssued(a) | SessionPhase::Receiving(a) => a,
                        _ => unreachable!(),
                    };
                    session.phase = match progress {
                        FrameProgress::Partial { .. } => SessionPhase::Receiving(assembler),
                        FrameProgress::Complete(_) => SessionPhase::AwaitingCompletion(assembler),
                    };
                    Ok(())
                }
                Err(error) => {
                    session.respond(session.error_response(error));
                    Err(as_bluez_error(error))
                }
            }
        }

        fn write_auth(
            &self,
            device: &str,
            kind: CharacteristicKind,
            value: &[u8],
            mtu: usize,
        ) -> Result<(), BluezGattError> {
            self.auth_transport
                .as_ref()
                .ok_or_else(|| {
                    BluezGattError::NotPermitted("authenticated transport is unavailable".into())
                })?
                .write(device, kind, value, mtu)
        }

        fn write_control(&self, device: &str, value: &[u8]) -> Result<(), BluezGattError> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| BluezGattError::Failed("GATT client state lock poisoned".into()))?;
            let now = Instant::now();
            clients.retain(|_, session| !session.expired(now));
            let session = clients.get_mut(device).ok_or_else(|| {
                BluezGattError::NotPermitted(
                    "read the challenge characteristic before writing".into(),
                )
            })?;
            let (command, payload) = match &session.phase {
                SessionPhase::ChallengeIssued(a)
                | SessionPhase::Receiving(a)
                | SessionPhase::AwaitingCompletion(a) => {
                    if a.partial_expired(now) {
                        (Err(WireError::PartialMessageExpired), None)
                    } else {
                        (
                            a.check_control(value),
                            a.completed_payload().map(ToOwned::to_owned),
                        )
                    }
                }
                _ => {
                    return Err(BluezGattError::NotPermitted(
                        "control unavailable in this session phase".into(),
                    ));
                }
            };
            session.last_activity = now;
            match command {
                Ok(ControlCommand::Complete) => {
                    session.respond(session.status_response(
                        0,
                        payload.as_deref().expect("complete requires payload"),
                    ));
                    Ok(())
                }
                Ok(ControlCommand::Cancel) => {
                    session.respond(session.status_response(2, b"cancelled"));
                    Ok(())
                }
                Err(error) => {
                    session.respond(session.error_response(error));
                    Err(as_bluez_error(error))
                }
            }
        }

        fn invalidate(&self, device: &str) -> Result<(), String> {
            self.clients
                .lock()
                .map_err(|_| "GATT client state lock poisoned".to_owned())?
                .remove(device);
            if let Some(auth) = &self.auth_transport {
                auth.invalidate(device);
            }
            Ok(())
        }

        fn prune_devices(&self, connected: &HashMap<String, bool>) -> Result<(), String> {
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| "GATT client state lock poisoned".to_owned())?;
            let now = Instant::now();
            clients.retain(|device, session| {
                connected.get(device).copied().unwrap_or(false) && !session.expired(now)
            });
            if let Some(auth) = &self.auth_transport {
                auth.prune(connected);
            }
            Ok(())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CallKey {
        sender: String,
        serial: u32,
    }

    fn call_key(header: &zbus::message::Header<'_>) -> Option<CallKey> {
        Some(CallKey {
            sender: header.sender()?.to_string(),
            serial: header.primary().serial_num().get(),
        })
    }

    #[derive(Default)]
    struct GateInner {
        healthy: bool,
        current: Option<CallKey>,
        done: bool,
        claimed: bool,
        reason: Option<String>,
    }

    struct MonitorGate {
        owner: String,
        inner: Mutex<GateInner>,
        changed: std::sync::Condvar,
    }

    impl MonitorGate {
        fn new(owner: &str) -> Self {
            Self {
                owner: owner.to_owned(),
                inner: Mutex::new(GateInner::default()),
                changed: std::sync::Condvar::new(),
            }
        }

        fn fail(&self, state: &GattState, reason: impl Into<String>) {
            // Close the gate before clearing state: no later GATT operation can
            // begin, even if the stream dies while an earlier operation runs.
            if let Ok(mut inner) = self.inner.lock() {
                inner.healthy = false;
                if inner.reason.is_none() {
                    inner.reason = Some(reason.into());
                }
                self.changed.notify_all();
            }
            if let Ok(mut clients) = state.clients.lock() {
                clients.clear();
            }
            if let Some(auth) = &state.auth_transport {
                auth.invalidate_all();
            }
        }

        fn with_live<T>(
            &self,
            operation: impl FnOnce() -> Result<T, BluezGattError>,
        ) -> Result<T, BluezGattError> {
            let inner = self
                .inner
                .lock()
                .map_err(|_| BluezGattError::Failed("monitor gate poisoned".into()))?;
            if !inner.healthy {
                return Err(BluezGattError::Failed(
                    "disconnect monitor unavailable".into(),
                ));
            }
            // Keep the gate locked until the state operation is finished, so a
            // concurrent failure cannot clear state before an admitted call uses it.
            let result = operation();
            drop(inner);
            result
        }

        fn healthy(&self) -> Result<(), String> {
            let inner = self
                .inner
                .lock()
                .map_err(|_| "monitor gate poisoned".to_owned())?;
            if inner.healthy {
                Ok(())
            } else {
                Err(inner
                    .reason
                    .clone()
                    .unwrap_or_else(|| "monitor stopped".into()))
            }
        }

        async fn begin<'a>(
            &'a self,
            header: &zbus::message::Header<'_>,
            state: &GattState,
        ) -> Result<CallPermit<'a>, BluezGattError> {
            let key = call_key(header)
                .ok_or_else(|| BluezGattError::Failed("missing method call identity".into()))?;
            if key.sender != self.owner {
                return Err(BluezGattError::NotAuthorized(
                    "only the registered BlueZ owner may invoke GATT methods".into(),
                ));
            }
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                {
                    let mut inner = self
                        .inner
                        .lock()
                        .map_err(|_| BluezGattError::Failed("monitor gate poisoned".into()))?;
                    if !inner.healthy {
                        return Err(BluezGattError::Failed(
                            "disconnect monitor unavailable".into(),
                        ));
                    }
                    if inner.current.as_ref() == Some(&key) && !inner.claimed {
                        inner.claimed = true;
                        return Ok(CallPermit { gate: self });
                    }
                }
                if Instant::now() >= deadline {
                    self.fail(state, "disconnect monitor did not order the request");
                    return Err(BluezGattError::Failed(
                        "disconnect monitor did not order the request".into(),
                    ));
                }
                // Do not block zbus's executor while an earlier handler awaits its turn.
                async_io::Timer::after(Duration::from_millis(2)).await;
            }
        }

        fn order_call(&self, key: CallKey) -> Result<(), String> {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| "monitor gate poisoned".to_owned())?;
            if !inner.healthy {
                return Err("monitor stopped".into());
            }
            inner.current = Some(key);
            inner.done = false;
            inner.claimed = false;
            self.changed.notify_all();
            let deadline = Instant::now() + Duration::from_secs(1);
            while inner.healthy && !inner.done {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("GATT call did not finish within monitor deadline".into());
                }
                let (next, _) = self
                    .changed
                    .wait_timeout(inner, remaining)
                    .map_err(|_| "monitor gate poisoned".to_owned())?;
                inner = next;
            }
            inner.current = None;
            if inner.healthy {
                Ok(())
            } else {
                Err("monitor stopped".into())
            }
        }
    }

    struct CallPermit<'a> {
        gate: &'a MonitorGate,
    }
    impl Drop for CallPermit<'_> {
        fn drop(&mut self) {
            if let Ok(mut inner) = self.gate.inner.lock() {
                inner.done = true;
                self.gate.changed.notify_all();
            }
        }
    }

    struct SignalMonitor {
        gate: Arc<MonitorGate>,
        state: Arc<GattState>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl SignalMonitor {
        fn start(
            connection: &Connection,
            owner: &str,
            adapter_path: &str,
            app_path: &str,
            state: Arc<GattState>,
        ) -> Result<Self, String> {
            // The unfiltered stream includes both signals and addressed method calls,
            // in the same receive order. Install all bus matches before snapshot.
            let mut messages = zbus::blocking::MessageIterator::from(connection).into_inner();
            let bus = Proxy::new(
                connection,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
            )
            .map_err(|e| e.to_string())?;
            let rules = [
                format!("type='signal',sender='{owner}',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path_namespace='{adapter_path}'"),
                format!("type='signal',sender='{owner}',interface='org.freedesktop.DBus.ObjectManager',member='InterfacesRemoved',path='/'"),
                "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.bluez'".to_owned(),
            ];
            for rule in rules {
                bus.call::<_, _, ()>("AddMatch", &(rule,))
                    .map_err(|e| format!("signal subscription failed: {e}"))?;
            }
            let gate = Arc::new(MonitorGate::new(owner));
            gate.inner
                .lock()
                .map_err(|_| "monitor gate poisoned".to_owned())?
                .healthy = true;
            let worker_gate = gate.clone();
            let worker_state = state.clone();
            let owner = owner.to_owned();
            let adapter_path = adapter_path.to_owned();
            let app_path = app_path.to_owned();
            let thread = thread::Builder::new()
                .name("bluez-session-monitor".into())
                .spawn(move || {
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || -> Result<(), String> {
                            loop {
                                let event = futures_lite::future::block_on(async {
                                    async { Some(messages.next().await) }
                                        .or(async {
                                            async_io::Timer::after(Duration::from_millis(100))
                                                .await;
                                            None
                                        })
                                        .await
                                });
                                if worker_gate.healthy().is_err() {
                                    return Ok(());
                                }
                                let Some(event) = event else {
                                    continue;
                                };
                                let message = event
                                    .ok_or_else(|| "D-Bus monitor stream ended".to_owned())?
                                    .map_err(|e| format!("D-Bus monitor stream failed: {e}"))?;
                                let header = message.header();
                                let path = header.path().map(|p| p.as_str()).unwrap_or("");
                                let interface =
                                    header.interface().map(|i| i.as_str()).unwrap_or("");
                                let member = header.member().map(|m| m.as_str()).unwrap_or("");
                                if header.message_type() == zbus::message::Type::MethodCall
                                    && path.starts_with(&format!("{app_path}/service0/char"))
                                    && interface == "org.bluez.GattCharacteristic1"
                                    && matches!(member, "ReadValue" | "WriteValue")
                                {
                                    let key = call_key(&header)
                                        .ok_or_else(|| "GATT call without sender".to_owned())?;
                                    if key.sender == owner {
                                        worker_gate.order_call(key)?;
                                    }
                                } else if header.message_type() == zbus::message::Type::Signal {
                                    let sender = header.sender().map(|s| s.as_str()).unwrap_or("");
                                    if sender == owner
                                        && interface == "org.freedesktop.DBus.Properties"
                                        && member == "PropertiesChanged"
                                        && (path == adapter_path
                                            || path.starts_with(&format!("{adapter_path}/")))
                                    {
                                        let (name, changed, invalidated): (
                                            String,
                                            HashMap<String, OwnedValue>,
                                            Vec<String>,
                                        ) = message.body().deserialize().map_err(|e| {
                                            format!("malformed PropertiesChanged: {e}")
                                        })?;
                                        if name == ADAPTER && path == adapter_path {
                                            if invalidated.iter().any(|s| s == "Powered") {
                                                return Err(
                                                    "adapter power state invalidated".into()
                                                );
                                            }
                                            if let Some(value) = changed.get("Powered") {
                                                if !bool::try_from(value.clone()).map_err(|_| {
                                                    "invalid Powered property".to_owned()
                                                })? {
                                                    return Err("adapter powered off".into());
                                                }
                                            }
                                        }
                                        if name == "org.bluez.Device1" && path != adapter_path {
                                            if invalidated.iter().any(|s| s == "Connected") {
                                                worker_state.invalidate(path)?;
                                            }
                                            if let Some(value) = changed.get("Connected") {
                                                if !bool::try_from(value.clone()).map_err(|_| {
                                                    "invalid Connected property".to_owned()
                                                })? {
                                                    worker_state.invalidate(path)?;
                                                }
                                            }
                                        }
                                    } else if sender == owner
                                        && interface == "org.freedesktop.DBus.ObjectManager"
                                        && member == "InterfacesRemoved"
                                    {
                                        let (removed, interfaces): (OwnedObjectPath, Vec<String>) =
                                            message.body().deserialize().map_err(|e| {
                                                format!("malformed InterfacesRemoved: {e}")
                                            })?;
                                        let removed = removed.as_str();
                                        if removed == adapter_path
                                            && interfaces.iter().any(|i| {
                                                matches!(
                                                    i.as_str(),
                                                    ADAPTER | GATT_MANAGER | ADVERTISING_MANAGER
                                                )
                                            })
                                        {
                                            return Err("adapter or manager removed".into());
                                        }
                                        if removed.starts_with(&format!("{adapter_path}/"))
                                            && interfaces.iter().any(|i| i == "org.bluez.Device1")
                                        {
                                            worker_state.invalidate(removed)?;
                                        }
                                    } else if interface == "org.freedesktop.DBus"
                                        && member == "NameOwnerChanged"
                                        && sender == "org.freedesktop.DBus"
                                    {
                                        let (name, old, new): (String, String, String) =
                                            message.body().deserialize().map_err(|e| {
                                                format!("malformed NameOwnerChanged: {e}")
                                            })?;
                                        if name == BLUEZ && old == owner && new != owner {
                                            return Err("BlueZ owner changed".into());
                                        }
                                    }
                                }
                            }
                        },
                    ));
                    let reason = match outcome {
                        Ok(Ok(())) => "monitor stopped".into(),
                        Ok(Err(error)) => error,
                        Err(_) => "monitor thread panicked".into(),
                    };
                    worker_gate.fail(&worker_state, reason);
                })
                .map_err(|e| format!("could not start signal monitor: {e}"))?;
            Ok(Self {
                gate,
                state,
                thread: Some(thread),
            })
        }
    }

    impl Drop for SignalMonitor {
        fn drop(&mut self) {
            self.gate.fail(&self.state, "monitor stopping");
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CharacteristicKind {
        Version,
        Challenge,
        Exchange,
        Response,
        Control,
        AuthRequestData,
        AuthRequestControl,
        AuthResponseData,
        AuthResponseControl,
        AuthStatus,
    }

    impl CharacteristicKind {
        fn is_auth(self) -> bool {
            matches!(
                self,
                Self::AuthRequestData
                    | Self::AuthRequestControl
                    | Self::AuthResponseData
                    | Self::AuthResponseControl
                    | Self::AuthStatus
            )
        }
    }

    struct GattService {
        uuid: String,
        characteristics: Vec<String>,
    }

    #[zbus::interface(name = "org.bluez.GattService1")]
    impl GattService {
        #[zbus(property, name = "UUID")]
        fn uuid(&self) -> &str {
            &self.uuid
        }

        #[zbus(property)]
        fn primary(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn characteristics(&self) -> Vec<OwnedObjectPath> {
            self.characteristics
                .iter()
                .filter_map(|path| OwnedObjectPath::try_from(path.as_str()).ok())
                .collect()
        }
    }

    struct GattCharacteristic {
        service: String,
        uuid: &'static str,
        flags: Vec<String>,
        kind: CharacteristicKind,
        state: Arc<GattState>,
        gate: Arc<MonitorGate>,
    }

    #[zbus::interface(name = "org.bluez.GattCharacteristic1")]
    impl GattCharacteristic {
        #[zbus(property, name = "UUID")]
        fn uuid(&self) -> &str {
            self.uuid
        }

        #[zbus(property)]
        fn service(&self) -> OwnedObjectPath {
            OwnedObjectPath::try_from(self.service.as_str()).expect("constant D-Bus object path")
        }

        #[zbus(property)]
        fn flags(&self) -> Vec<String> {
            self.flags.clone()
        }

        async fn read_value(
            &self,
            options: HashMap<String, OwnedValue>,
            #[zbus(header)] header: zbus::message::Header<'_>,
        ) -> Result<Vec<u8>, BluezGattError> {
            let _permit = self.gate.begin(&header, &self.state).await?;
            let device = option_device(&options)?;
            let (offset, mtu) = read_options(&options)?;
            self.gate
                .with_live(|| self.state.client_value(&device, self.kind, offset, mtu))
        }

        async fn write_value(
            &self,
            value: Vec<u8>,
            options: HashMap<String, OwnedValue>,
            #[zbus(header)] header: zbus::message::Header<'_>,
        ) -> Result<(), BluezGattError> {
            let _permit = self.gate.begin(&header, &self.state).await?;
            let device = option_device(&options)?;
            let (offset, mtu) = read_options(&options)?;
            if offset != 0 || value.len() > mtu - 3 {
                return Err(BluezGattError::InvalidValueLength(
                    "invalid write offset or length".into(),
                ));
            }
            if let Some(value) = options.get("type") {
                let write_type = String::try_from(value.clone())
                    .map_err(|_| BluezGattError::InvalidArgs("invalid write type".into()))?;
                if write_type != "request" {
                    return Err(BluezGattError::NotPermitted(
                        "only write requests are supported".into(),
                    ));
                }
            }
            if let Some(value) = options.get("prepare-authorize") {
                let prepared = bool::try_from(value.clone()).map_err(|_| {
                    BluezGattError::InvalidArgs("invalid prepare-authorize option".into())
                })?;
                if prepared {
                    return Err(BluezGattError::NotPermitted(
                        "prepared writes are unsupported".into(),
                    ));
                }
            }
            self.gate.with_live(|| match self.kind {
                CharacteristicKind::Exchange => self.state.write_exchange(&device, &value, mtu),
                CharacteristicKind::Control => self.state.write_control(&device, &value),
                CharacteristicKind::AuthRequestControl
                | CharacteristicKind::AuthResponseData
                | CharacteristicKind::AuthResponseControl => {
                    self.state.write_auth(&device, self.kind, &value, mtu)
                }
                _ => Err(BluezGattError::NotPermitted(
                    "characteristic is read-only".into(),
                )),
            })
        }
    }

    struct LeAdvertisement {
        service_uuid: String,
        local_name: String,
    }

    #[zbus::interface(name = "org.bluez.LEAdvertisement1")]
    impl LeAdvertisement {
        #[zbus(property, name = "Type")]
        fn type_(&self) -> &str {
            "peripheral"
        }

        #[zbus(property, name = "ServiceUUIDs")]
        fn service_uuids(&self) -> Vec<String> {
            vec![self.service_uuid.clone()]
        }

        #[zbus(property)]
        fn local_name(&self) -> &str {
            &self.local_name
        }

        fn release(&self) {}
    }

    fn managed_objects(
        connection: &Connection,
        owner: &str,
    ) -> Result<zbus::fdo::ManagedObjects, zbus::Error> {
        let proxy = Proxy::new(connection, owner, "/", OM)?;
        proxy.call("GetManagedObjects", &())
    }

    fn bluez_owner(connection: &Connection) -> Result<String, zbus::Error> {
        let proxy = Proxy::new(
            connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )?;
        proxy.call("GetNameOwner", &(BLUEZ,))
    }

    fn has_interface(objects: &zbus::fdo::ManagedObjects, path: &str, interface: &str) -> bool {
        ObjectPath::try_from(path)
            .ok()
            .and_then(|path| objects.get(&path))
            .is_some_and(|interfaces| interfaces.keys().any(|name| name.as_str() == interface))
    }

    fn adapter_powered(objects: &zbus::fdo::ManagedObjects, path: &str) -> Option<bool> {
        let object_path = ObjectPath::try_from(path).ok()?;
        let adapter = objects.get(&object_path)?;
        adapter
            .iter()
            .find(|(name, _)| name.as_str() == ADAPTER)?
            .1
            .get("Powered")
            .and_then(|value| bool::try_from(value.clone()).ok())
    }

    fn manager_capability(
        objects: &zbus::fdo::ManagedObjects,
        path: &str,
        property: &str,
    ) -> Option<u8> {
        let path = ObjectPath::try_from(path).ok()?;
        objects
            .get(&path)?
            .iter()
            .find(|(name, _)| name.as_str() == ADVERTISING_MANAGER)?
            .1
            .get(property)
            .and_then(|value| u8::try_from(value.clone()).ok())
    }

    fn device_connections(objects: &zbus::fdo::ManagedObjects) -> HashMap<String, bool> {
        objects
            .iter()
            .filter_map(|(path, interfaces)| {
                let properties = interfaces
                    .iter()
                    .find(|(name, _)| name.as_str() == "org.bluez.Device1")?
                    .1;
                let connected = properties
                    .get("Connected")
                    .and_then(|value| bool::try_from(value.clone()).ok())?;
                Some((path.to_string(), connected))
            })
            .collect()
    }

    fn call_manager(
        connection: &Connection,
        owner: &str,
        path: &str,
        interface: &str,
        method: &str,
        object: &str,
    ) -> Result<(), zbus::Error> {
        let proxy = Proxy::new(connection, owner, path, interface)?;
        let object_path = OwnedObjectPath::try_from(object).map_err(zbus::Error::from)?;
        let options: HashMap<String, OwnedValue> = HashMap::new();
        if method.starts_with("Unregister") {
            proxy.call(method, &(object_path,))
        } else {
            proxy.call(method, &(object_path, options))
        }
    }

    struct Registration {
        connection: Connection,
        adapter_path: String,
        app_path: String,
        ad_path: String,
        app_registered: bool,
        ad_registered: bool,
        owner: String,
    }

    impl Registration {
        fn cleanup(&mut self) -> Vec<String> {
            let mut errors = Vec::new();
            // A restarted BlueZ owns no registrations from the old connection.
            // Do not send unregister requests to its replacement owner.
            if let Ok(owner) = bluez_owner(&self.connection) {
                if owner != self.owner {
                    self.ad_registered = false;
                    self.app_registered = false;
                    return errors;
                }
            }
            if self.ad_registered {
                match call_manager(
                    &self.connection,
                    &self.owner,
                    &self.adapter_path,
                    ADVERTISING_MANAGER,
                    "UnregisterAdvertisement",
                    &self.ad_path,
                ) {
                    Ok(()) => self.ad_registered = false,
                    Err(error) if registration_already_gone(&error) => {
                        self.ad_registered = false;
                    }
                    Err(error) => errors.push(format!("advertisement unregister failed: {error}")),
                }
            }
            if self.app_registered {
                let proxy = Proxy::new(
                    &self.connection,
                    self.owner.as_str(),
                    self.adapter_path.as_str(),
                    GATT_MANAGER,
                );
                let result = proxy.and_then(|proxy| {
                    let app = OwnedObjectPath::try_from(self.app_path.as_str())
                        .map_err(zbus::Error::from)?;
                    proxy.call("UnregisterApplication", &(app,))
                });
                match result {
                    Ok(()) => self.app_registered = false,
                    Err(error) if registration_already_gone(&error) => {
                        self.app_registered = false;
                    }
                    Err(error) => {
                        errors.push(format!("GATT application unregister failed: {error}"))
                    }
                }
            }
            errors
        }
    }

    fn classify_dbus_error(error: &zbus::Error) -> &'static str {
        classify_error_text(&error.to_string())
    }

    fn registration_already_gone(error: &zbus::Error) -> bool {
        let text = error.to_string().to_lowercase();
        text.contains("doesnotexist") || text.contains("unknownobject")
    }

    fn classify_error_text(error: &str) -> &'static str {
        let text = error.to_lowercase();
        if text.contains("accessdenied")
            || text.contains("access denied")
            || text.contains("not authorized")
        {
            "dbus_access_denied"
        } else if text.contains("org.bluez.error.notpermitted")
            || text.contains("org.bluez.error.alreadyexists")
            || text.contains("org.bluez.error.invalidlength")
            || text.contains("advertising slots")
        {
            "bluez_registration_rejected"
        } else if text.contains("serviceunknown")
            || text.contains("name has no owner")
            || text.contains("org.bluez is not provided")
        {
            "bluez_service_unavailable"
        } else {
            "dbus_error"
        }
    }

    fn err_report(
        adapter: &str,
        duration: u16,
        stage: &'static str,
        error: impl ToString,
    ) -> ExperimentError {
        let error = error.to_string();
        let error_lower = error.to_lowercase();
        let error_class = if error_lower.contains("accessdenied")
            || error_lower.contains("access denied")
            || error_lower.contains("not authorized")
        {
            Some("dbus_access_denied")
        } else if error_lower.contains("org.bluez.error.notpermitted")
            || error_lower.contains("org.bluez.error.alreadyexists")
            || error_lower.contains("advertising slots")
        {
            Some("bluez_registration_rejected")
        } else if error_lower.contains("system dbus unavailable")
            || error_lower.contains("system d-bus unavailable")
        {
            Some("system_bus_unavailable")
        } else if error_lower.contains("org.bluez is not provided")
            || error_lower.contains("serviceunknown")
            || error_lower.contains("namehasnoowner")
        {
            Some("bluez_service_unavailable")
        } else {
            None
        };
        ExperimentError {
            report: Box::new(ExperimentReport {
                status: "failed",
                stage,
                adapter: adapter.into(),
                runtime_seconds: duration,
                shutdown_reason: None,
                application_registered: false,
                advertisement_registered: false,
                application_registration_succeeded: false,
                advertisement_registration_succeeded: false,
                cleanup_errors: Vec::new(),
                controller_le_peripheral_capability: "unknown",
                supported_advertising_instances: None,
                active_advertising_instances: None,
                error_class,
                error: Some(error),
                authorization: "never_evaluated",
            }),
        }
    }

    fn setup_deadline_error(
        adapter: &str,
        duration: u16,
        stage: &'static str,
        deadline: Instant,
    ) -> Option<ExperimentError> {
        setup_deadline_elapsed(Instant::now(), deadline).then(|| {
            err_report(
                adapter,
                duration,
                stage,
                "BlueZ preflight/registration exceeded the 30-second setup deadline; no signed request was activated",
            )
        })
    }

    fn setup_deadline_elapsed(now: Instant, deadline: Instant) -> bool {
        now >= deadline
    }

    pub fn serve(
        adapter: &str,
        duration: u16,
        terminate: Arc<AtomicBool>,
    ) -> Result<ExperimentReport, ExperimentError> {
        serve_inner(adapter, duration, terminate, None)
    }

    pub fn serve_auth(
        adapter: &str,
        duration: u16,
        terminate: Arc<AtomicBool>,
        config: AuthTransportConfig,
    ) -> Result<AuthExperimentReport, ExperimentError> {
        let auth_transport =
            Arc::new(AuthTransportState::new(config).map_err(|error| {
                err_report(adapter, duration, "signed_request_preflight", error)
            })?);
        let transport = serve_inner(adapter, duration, terminate, Some(auth_transport.clone()))?;
        Ok(AuthExperimentReport {
            transport,
            research_decision: auth_transport.decision(),
            host_public_key_fingerprint_sha256: auth_transport.host_fingerprint().to_owned(),
            watch_public_key_fingerprint_sha256: auth_transport.watch_fingerprint().to_owned(),
            enrollment_expires_at_unix_ms: auth_transport.enrollment_expires_at_unix_ms,
            authorization: "never_evaluated",
        })
    }

    fn serve_inner(
        adapter: &str,
        duration: u16,
        terminate: Arc<AtomicBool>,
        auth_transport: Option<Arc<AuthTransportState>>,
    ) -> Result<ExperimentReport, ExperimentError> {
        let setup_deadline = Instant::now() + BLUEZ_SETUP_TIMEOUT;
        let connection = zbus::blocking::connection::Builder::system()
            .and_then(|builder| builder.method_timeout(BLUEZ_METHOD_TIMEOUT).build())
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("system D-Bus unavailable: {error}"),
                )
            })?;
        if let Some(error) = setup_deadline_error(adapter, duration, "preflight", setup_deadline) {
            return Err(error);
        }
        let owner = bluez_owner(&connection).map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("BlueZ owner unavailable: {error}"),
            )
        })?;
        if let Some(error) = setup_deadline_error(adapter, duration, "preflight", setup_deadline) {
            return Err(error);
        }
        let adapter_path = format!("/org/bluez/{adapter}");
        let nonce = random_bytes::<8>().map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("OS random source unavailable: {error}"),
            )
        })?;
        let mut token = String::with_capacity(16);
        for byte in nonce {
            use std::fmt::Write as _;
            write!(&mut token, "{byte:02x}").expect("writing to String cannot fail");
        }
        let app_path = format!("/org/opencontinuity/gatt_{token}");
        let ad_path = format!("/org/opencontinuity/advertisement_{token}");
        let service_path = format!("{app_path}/service0");
        let state = Arc::new(match auth_transport {
            Some(auth_transport) => GattState::with_auth_transport(auth_transport),
            None => GattState::default(),
        });
        let monitor =
            SignalMonitor::start(&connection, &owner, &adapter_path, &app_path, state.clone())
                .map_err(|error| err_report(adapter, duration, "preflight", error))?;
        if let Some(error) = setup_deadline_error(adapter, duration, "preflight", setup_deadline) {
            return Err(error);
        }
        let current_owner = bluez_owner(&connection).map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("BlueZ owner unavailable after subscription: {error}"),
            )
        })?;
        if current_owner != owner {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "BlueZ owner changed during subscription",
            ));
        }
        let objects = managed_objects(&connection, &owner).map_err(|error| {
            err_report(
                adapter,
                duration,
                "preflight",
                format!("BlueZ discovery {}: {error}", classify_dbus_error(&error)),
            )
        })?;
        if let Some(error) = setup_deadline_error(adapter, duration, "preflight", setup_deadline) {
            return Err(error);
        }
        monitor
            .gate
            .healthy()
            .map_err(|error| err_report(adapter, duration, "preflight", error))?;
        if let Some(error) = setup_deadline_error(adapter, duration, "preflight", setup_deadline) {
            return Err(error);
        }
        if !objects
            .contains_key(&ObjectPath::try_from(adapter_path.as_str()).expect("valid adapter path"))
        {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "explicit adapter is absent from BlueZ ObjectManager",
            ));
        }
        if !has_interface(&objects, &adapter_path, GATT_MANAGER) {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "selected adapter does not expose org.bluez.GattManager1 (unsupported)",
            ));
        }
        if !has_interface(&objects, &adapter_path, ADVERTISING_MANAGER) {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "selected adapter does not expose org.bluez.LEAdvertisingManager1 (unsupported)",
            ));
        }
        if adapter_powered(&objects, &adapter_path) != Some(true) {
            return Err(err_report(
                adapter,
                duration,
                "preflight",
                "selected adapter is not reported powered; this experiment never powers it on",
            ));
        }
        let supported_instances = manager_capability(&objects, &adapter_path, "SupportedInstances");
        let active_instances = manager_capability(&objects, &adapter_path, "ActiveInstances");

        let (service_uuid, local_name, specs) = if state.auth_transport.is_some() {
            (
                AUTH_SERVICE_UUID,
                "OC Auth Research",
                vec![
                    (
                        "char0",
                        AUTH_REQUEST_DATA_UUID,
                        vec!["read".to_owned()],
                        CharacteristicKind::AuthRequestData,
                    ),
                    (
                        "char1",
                        AUTH_REQUEST_CONTROL_UUID,
                        vec!["write".to_owned()],
                        CharacteristicKind::AuthRequestControl,
                    ),
                    (
                        "char2",
                        AUTH_RESPONSE_DATA_UUID,
                        vec!["write".to_owned()],
                        CharacteristicKind::AuthResponseData,
                    ),
                    (
                        "char3",
                        AUTH_RESPONSE_CONTROL_UUID,
                        vec!["write".to_owned()],
                        CharacteristicKind::AuthResponseControl,
                    ),
                    (
                        "char4",
                        AUTH_STATUS_UUID,
                        vec!["read".to_owned()],
                        CharacteristicKind::AuthStatus,
                    ),
                ],
            )
        } else {
            (
                SERVICE_UUID,
                "OC Research",
                vec![
                    (
                        "char0",
                        VERSION_UUID,
                        vec!["read".to_owned()],
                        CharacteristicKind::Version,
                    ),
                    (
                        "char1",
                        CHALLENGE_UUID,
                        vec!["read".to_owned()],
                        CharacteristicKind::Challenge,
                    ),
                    (
                        "char2",
                        EXCHANGE_UUID,
                        vec!["write".to_owned()],
                        CharacteristicKind::Exchange,
                    ),
                    (
                        "char3",
                        RESPONSE_UUID,
                        vec!["read".to_owned()],
                        CharacteristicKind::Response,
                    ),
                    (
                        "char4",
                        CONTROL_UUID,
                        vec!["write".to_owned()],
                        CharacteristicKind::Control,
                    ),
                ],
            )
        };
        let char_paths = specs
            .iter()
            .map(|(name, _, _, _)| format!("{service_path}/{name}"))
            .collect::<Vec<_>>();
        connection
            .object_server()
            .at(
                OwnedObjectPath::try_from(app_path.as_str()).expect("valid application path"),
                ObjectManager,
            )
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("could not export GATT ObjectManager: {error}"),
                )
            })?;
        connection
            .object_server()
            .at(
                OwnedObjectPath::try_from(service_path.as_str()).expect("valid service path"),
                GattService {
                    uuid: service_uuid.to_owned(),
                    characteristics: char_paths.clone(),
                },
            )
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("could not export GATT service: {error}"),
                )
            })?;
        for ((_, uuid, flags, kind), path) in specs.iter().zip(char_paths.iter()) {
            connection
                .object_server()
                .at(
                    OwnedObjectPath::try_from(path.as_str()).expect("valid characteristic path"),
                    GattCharacteristic {
                        service: service_path.clone(),
                        uuid,
                        flags: flags.clone(),
                        kind: *kind,
                        state: state.clone(),
                        gate: monitor.gate.clone(),
                    },
                )
                .map_err(|error| {
                    err_report(
                        adapter,
                        duration,
                        "preflight",
                        format!("could not export GATT characteristic: {error}"),
                    )
                })?;
        }
        connection
            .object_server()
            .at(
                OwnedObjectPath::try_from(ad_path.as_str()).expect("valid advertisement path"),
                LeAdvertisement {
                    service_uuid: service_uuid.to_owned(),
                    local_name: local_name.to_owned(),
                },
            )
            .map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "preflight",
                    format!("could not export LE advertisement: {error}"),
                )
            })?;

        let mut registration = Registration {
            connection: connection.clone(),
            adapter_path: adapter_path.clone(),
            app_path: app_path.clone(),
            ad_path: ad_path.clone(),
            app_registered: false,
            ad_registered: false,
            owner: owner.clone(),
        };
        let app_object = OwnedObjectPath::try_from(app_path.as_str()).expect("validated path");
        let options: HashMap<String, OwnedValue> = HashMap::new();
        let manager = Proxy::new(
            &connection,
            owner.as_str(),
            adapter_path.as_str(),
            GATT_MANAGER,
        )
        .map_err(|error| {
            err_report(
                adapter,
                duration,
                "registering_application",
                format!("{}: {error}", classify_dbus_error(&error)),
            )
        })?;
        monitor
            .gate
            .healthy()
            .map_err(|error| err_report(adapter, duration, "preflight", error))?;
        if let Some(error) =
            setup_deadline_error(adapter, duration, "registering_application", setup_deadline)
        {
            return Err(error);
        }
        if let Err(error) = manager.call::<_, _, ()>("RegisterApplication", &(app_object, options))
        {
            // A timed-out call may have registered despite its missing reply.
            if matches!(error, zbus::Error::InputOutput(_) | zbus::Error::Failure(_)) {
                registration.app_registered = true;
            }
            let cleanup = registration.cleanup();
            let mut failure = err_report(
                adapter,
                duration,
                "registering_application",
                format!(
                    "BlueZ GATT application registration failed ({}): {error}",
                    classify_dbus_error(&error)
                ),
            );
            failure.report.cleanup_errors = cleanup;
            failure.report.application_registered = registration.app_registered;
            return Err(failure);
        }
        registration.app_registered = true;
        if let Some(mut failure) =
            setup_deadline_error(adapter, duration, "registering_application", setup_deadline)
        {
            failure.report.application_registration_succeeded = true;
            failure.report.application_registered = registration.app_registered;
            failure.report.cleanup_errors = registration.cleanup();
            return Err(failure);
        }
        if let Err(error) = monitor.gate.healthy() {
            let cleanup = registration.cleanup();
            let mut failure = err_report(
                adapter,
                duration,
                "registering_application",
                format!("disconnect monitor failed: {error}"),
            );
            failure.report.application_registration_succeeded = true;
            failure.report.application_registered = registration.app_registered;
            failure.report.cleanup_errors = cleanup;
            return Err(failure);
        }

        if let Err(error) = call_manager(
            &connection,
            &owner,
            &adapter_path,
            ADVERTISING_MANAGER,
            "RegisterAdvertisement",
            &ad_path,
        ) {
            if matches!(error, zbus::Error::InputOutput(_) | zbus::Error::Failure(_)) {
                registration.ad_registered = true;
            }
            let cleanup = registration.cleanup();
            let mut failure = err_report(
                adapter,
                duration,
                "registering_advertisement",
                format!(
                    "BlueZ advertisement registration failed ({}): {error}",
                    classify_dbus_error(&error)
                ),
            );
            failure.report.application_registration_succeeded = true;
            failure.report.application_registered = registration.app_registered;
            failure.report.advertisement_registered = registration.ad_registered;
            failure.report.cleanup_errors = cleanup;
            return Err(failure);
        }
        registration.ad_registered = true;
        if let Some(mut failure) = setup_deadline_error(
            adapter,
            duration,
            "registering_advertisement",
            setup_deadline,
        ) {
            failure.report.application_registration_succeeded = true;
            failure.report.advertisement_registration_succeeded = true;
            failure.report.application_registered = registration.app_registered;
            failure.report.advertisement_registered = registration.ad_registered;
            failure.report.cleanup_errors = registration.cleanup();
            return Err(failure);
        }

        let start = Instant::now();
        if let Some(auth) = &state.auth_transport {
            if let Err(error) = auth.activate(start) {
                let cleanup_errors = registration.cleanup();
                let mut failure = err_report(
                    adapter,
                    duration,
                    "initializing_signed_request",
                    format!(
                        "authenticated research did not become ready: {error}; no fallback request was emitted"
                    ),
                );
                failure.report.application_registration_succeeded = true;
                failure.report.advertisement_registration_succeeded = true;
                failure.report.application_registered = registration.app_registered;
                failure.report.advertisement_registered = registration.ad_registered;
                failure.report.cleanup_errors = cleanup_errors;
                return Err(failure);
            }
        }
        let deadline = start + Duration::from_secs(duration as u64);
        let mut last_probe = Instant::now();
        let active_result: Result<&'static str, ExperimentError> = (|| loop {
            monitor.gate.healthy().map_err(|error| {
                err_report(
                    adapter,
                    duration,
                    "active",
                    format!("disconnect monitor failed: {error}"),
                )
            })?;
            if terminate.load(std::sync::atomic::Ordering::Relaxed) {
                break Ok("termination_signal");
            }
            if state
                .auth_transport
                .as_ref()
                .is_some_and(|auth| auth.decision().is_some())
            {
                break Ok("research_exchange_complete");
            }
            if let Some(reason) = state
                .auth_transport
                .as_ref()
                .and_then(|auth| auth.deadline_shutdown_reason())
            {
                break Ok(reason);
            }
            if Instant::now() >= deadline {
                break Ok("timeout");
            }
            if last_probe.elapsed() >= Duration::from_secs(1) {
                let current_owner = bluez_owner(&connection).map_err(|error| {
                    err_report(
                        adapter,
                        duration,
                        "active",
                        format!("D-Bus connection or BlueZ owner lost: {error}"),
                    )
                })?;
                if current_owner != owner {
                    break Err(err_report(
                        adapter,
                        duration,
                        "active",
                        "BlueZ service restarted",
                    ));
                }
                let current = managed_objects(&connection, &owner).map_err(|error| {
                    err_report(
                        adapter,
                        duration,
                        "active",
                        format!("D-Bus connection or BlueZ state lost: {error}"),
                    )
                })?;
                if !has_interface(&current, &adapter_path, ADAPTER)
                    || adapter_powered(&current, &adapter_path) != Some(true)
                    || !has_interface(&current, &adapter_path, GATT_MANAGER)
                    || !has_interface(&current, &adapter_path, ADVERTISING_MANAGER)
                {
                    break Ok("adapter_lost");
                }
                state
                    .prune_devices(&device_connections(&current))
                    .map_err(|error| err_report(adapter, duration, "active", error))?;
                last_probe = Instant::now();
            }
            thread::sleep(Duration::from_millis(100));
        })();
        let elapsed = start.elapsed().as_secs().min(u16::MAX as u64) as u16;
        let cleanup_errors = registration.cleanup();
        if let Err(mut failure) = active_result {
            failure.report.application_registration_succeeded = true;
            failure.report.advertisement_registration_succeeded = true;
            failure.report.application_registered = registration.app_registered;
            failure.report.advertisement_registered = registration.ad_registered;
            failure.report.cleanup_errors = cleanup_errors;
            return Err(failure);
        }
        let shutdown_reason = active_result.expect("error handled");
        if !cleanup_errors.is_empty() {
            let mut failure = err_report(adapter, duration, "stopping", "cleanup failed");
            failure.report.shutdown_reason = Some(shutdown_reason);
            failure.report.application_registration_succeeded = true;
            failure.report.advertisement_registration_succeeded = true;
            failure.report.application_registered = registration.app_registered;
            failure.report.advertisement_registered = registration.ad_registered;
            failure.report.cleanup_errors = cleanup_errors;
            return Err(failure);
        }
        Ok(ExperimentReport {
            status: "stopped",
            stage: "stopped",
            adapter: adapter.into(),
            runtime_seconds: elapsed,
            shutdown_reason: Some(shutdown_reason),
            application_registered: false,
            advertisement_registered: false,
            application_registration_succeeded: true,
            advertisement_registration_succeeded: true,
            cleanup_errors: Vec::new(),
            // Manager interfaces establish API availability, not controller radio capability.
            controller_le_peripheral_capability: "unknown",
            supported_advertising_instances: supported_instances,
            active_advertising_instances: active_instances,
            error_class: None,
            error: None,
            authorization: "never_evaluated",
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use opencontinuity_core::{
            watch_auth_research::{
                ConsentResult, EnrollmentState, HostAuthRequest, KeyAlgorithm, ManualDecision,
                create_watch_response,
            },
            watch_auth_transport::{encode_frames as encode_auth_frames, encode_request_command},
        };
        use p256::{FieldBytes, ecdsa::SigningKey};

        #[test]
        fn dbus_errors_keep_policy_denial_distinct_from_missing_service() {
            assert_eq!(
                classify_error_text("org.freedesktop.DBus.Error.AccessDenied"),
                "dbus_access_denied"
            );
            assert_eq!(
                classify_error_text("org.bluez.Error.NotPermitted: advertising slots exhausted"),
                "bluez_registration_rejected"
            );
            assert_eq!(
                classify_error_text(
                    "org.freedesktop.DBus.Error.ServiceUnknown: org.bluez is not provided"
                ),
                "bluez_service_unavailable"
            );
            assert_eq!(
                classify_error_text("org.freedesktop.DBus.Error.NoReply"),
                "dbus_error"
            );
        }

        #[test]
        fn setup_readiness_deadline_is_bounded_and_includes_the_exact_boundary() {
            let started = Instant::now();
            let deadline = started + BLUEZ_SETUP_TIMEOUT;
            assert!(!setup_deadline_elapsed(
                started + BLUEZ_SETUP_TIMEOUT - Duration::from_millis(1),
                deadline
            ));
            assert!(setup_deadline_elapsed(deadline, deadline));
            assert!(setup_deadline_elapsed(
                deadline + Duration::from_millis(1),
                deadline
            ));
        }

        #[test]
        fn response_transfer_gets_its_own_window_after_consent_and_stays_inside_signed_expiry() {
            let request_complete = Instant::now();
            let consent_deadline = request_complete + WAITING_FOR_CONSENT_TIMEOUT;
            let response_started = consent_deadline - Duration::from_millis(1);
            let signed_deadline = response_started + Duration::from_secs(40);

            assert!(consent_deadline_elapsed(
                Some(consent_deadline),
                false,
                consent_deadline
            ));
            assert!(!consent_deadline_elapsed(
                Some(consent_deadline),
                true,
                consent_deadline + Duration::from_secs(1)
            ));
            assert_eq!(
                response_transfer_timeout(response_started, signed_deadline),
                RESPONSE_TRANSFER_TIMEOUT
            );

            let response_started_near_signed_expiry = signed_deadline - Duration::from_secs(3);
            assert_eq!(
                response_transfer_timeout(response_started_near_signed_expiry, signed_deadline),
                Duration::from_secs(3)
            );
        }

        fn test_key(scalar: u8) -> SigningKey {
            SigningKey::from_bytes(&FieldBytes::from([scalar; 32])).unwrap()
        }

        fn test_key_hex(key: &SigningKey) -> String {
            use std::fmt::Write as _;

            let mut encoded = String::with_capacity(130);
            for byte in key.verifying_key().to_encoded_point(false).as_bytes() {
                write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
            }
            encoded
        }

        fn auth_state() -> (
            AuthTransportState,
            SignedEnvelope,
            VerifiedHostRequest,
            HostEnrollment,
            WatchEnrollment,
            SigningKey,
        ) {
            let host_key = test_key(1);
            let watch_key = test_key(2);
            let host = HostEnrollment {
                enrollment_id: "host-enrollment-p009-test".into(),
                host_identity: "linux-p009-test".into(),
                linux_uid: 1000,
                public_key_sec1_hex: test_key_hex(&host_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let watch = WatchEnrollment {
                enrollment_id: "watch-enrollment-p009-test".into(),
                watch_identity: "watch-p009-test".into(),
                bound_host_enrollment_id: host.enrollment_id.clone(),
                bound_host_identity: host.host_identity.clone(),
                bound_linux_uid: host.linux_uid,
                public_key_sec1_hex: test_key_hex(&watch_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let now_ms = unix_now_ms().unwrap();
            let state = AuthTransportState::new(AuthTransportConfig {
                host_signing_key: host_key.clone(),
                host_enrollment: host.clone(),
                watch_enrollment: watch.clone(),
                enrollment_expires_at_unix_ms: now_ms + 3_600_000,
            })
            .unwrap();
            let ready_at = Instant::now();
            state.activate(ready_at).unwrap();
            let envelope = state
                .inner
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .envelope
                .as_ref()
                .unwrap()
                .clone();
            let request = HostAuthRequest::parse_canonical(envelope.canonical()).unwrap();
            let verified = verify_host_request(
                Some(&host),
                request,
                envelope.signature(),
                &watch.enrollment_id,
                unix_now_ms().unwrap(),
                &mut ReplayGuard::default(),
            )
            .unwrap();
            (state, envelope, verified, host, watch, watch_key)
        }

        #[test]
        fn signed_request_waits_for_readiness_and_restart_generates_fresh_request_material() {
            let host_key = test_key(11);
            let watch_key = test_key(12);
            let host = HostEnrollment {
                enrollment_id: "host-p009a-test".into(),
                host_identity: "linux-p009a-test".into(),
                linux_uid: 1000,
                public_key_sec1_hex: test_key_hex(&host_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let watch = WatchEnrollment {
                enrollment_id: "watch-p009a-test".into(),
                watch_identity: "watch-research-p009a".into(),
                bound_host_enrollment_id: host.enrollment_id.clone(),
                bound_host_identity: host.host_identity.clone(),
                bound_linux_uid: host.linux_uid,
                public_key_sec1_hex: test_key_hex(&watch_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let initial_wall = unix_now_ms().unwrap();
            let config = || AuthTransportConfig {
                host_signing_key: host_key.clone(),
                host_enrollment: host.clone(),
                watch_enrollment: watch.clone(),
                enrollment_expires_at_unix_ms: initial_wall + 3_600_000,
            };
            let first = AuthTransportState::new(config()).unwrap();
            assert!(first.inner.lock().unwrap().is_none());
            assert!(
                first
                    .read(
                        "/org/bluez/hci0/dev_01",
                        CharacteristicKind::AuthStatus,
                        0,
                        23
                    )
                    .is_err()
            );

            let setup_started = Instant::now();
            let ready_at = setup_started + Duration::from_secs(120);
            let readiness_wall = initial_wall + 120_000;
            first.activate_at(ready_at, readiness_wall).unwrap();
            let first_guard = first.inner.lock().unwrap();
            let first_session = first_guard.as_ref().unwrap();
            let first_envelope = first_session.envelope.as_ref().unwrap();
            let first_request =
                HostAuthRequest::parse_canonical(first_envelope.canonical()).unwrap();
            assert_eq!(first_request.issued_at_ms, readiness_wall);
            assert_eq!(first_request.expires_at_ms, readiness_wall + 60_000);
            assert_eq!(
                first_session.signed_deadline,
                ready_at + Duration::from_secs(60)
            );
            drop(first_guard);

            let second = AuthTransportState::new(config()).unwrap();
            second
                .activate_at(ready_at + Duration::from_secs(1), readiness_wall + 1_000)
                .unwrap();
            let second_guard = second.inner.lock().unwrap();
            let second_request = HostAuthRequest::parse_canonical(
                second_guard
                    .as_ref()
                    .unwrap()
                    .envelope
                    .as_ref()
                    .unwrap()
                    .canonical(),
            )
            .unwrap();
            assert_ne!(first_request.session_id, second_request.session_id);
            assert_ne!(first_request.nonce, second_request.nonce);
        }

        #[test]
        fn signed_request_expiry_is_clamped_to_the_test_enrollment_expiry() {
            let host_key = test_key(31);
            let watch_key = test_key(32);
            let host = HostEnrollment {
                enrollment_id: "host-p009a-short-pin".into(),
                host_identity: "linux-p009a-short-pin".into(),
                linux_uid: 1000,
                public_key_sec1_hex: test_key_hex(&host_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let watch = WatchEnrollment {
                enrollment_id: "watch-p009a-short-pin".into(),
                watch_identity: "watch-p009a-short-pin".into(),
                bound_host_enrollment_id: host.enrollment_id.clone(),
                bound_host_identity: host.host_identity.clone(),
                bound_linux_uid: host.linux_uid,
                public_key_sec1_hex: test_key_hex(&watch_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let initial_wall = unix_now_ms().unwrap();
            let enrollment_expiry = initial_wall + 5_000;
            let state = AuthTransportState::new(AuthTransportConfig {
                host_signing_key: host_key,
                host_enrollment: host,
                watch_enrollment: watch,
                enrollment_expires_at_unix_ms: enrollment_expiry,
            })
            .unwrap();
            let ready_at = Instant::now();
            let ready_wall = initial_wall + 1_000;
            state.activate_at(ready_at, ready_wall).unwrap();

            let guard = state.inner.lock().unwrap();
            let session = guard.as_ref().unwrap();
            let request =
                HostAuthRequest::parse_canonical(session.envelope.as_ref().unwrap().canonical())
                    .unwrap();
            assert_eq!(request.expires_at_ms, enrollment_expiry);
            assert_eq!(session.signed_deadline, ready_at + Duration::from_secs(4));
        }

        #[test]
        fn expired_or_mismatched_test_enrollment_fails_before_request_initialization() {
            let host_key = test_key(21);
            let watch_key = test_key(22);
            let host = HostEnrollment {
                enrollment_id: "host-p009a-expiry".into(),
                host_identity: "linux-p009a-expiry".into(),
                linux_uid: 1000,
                public_key_sec1_hex: test_key_hex(&host_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let watch = WatchEnrollment {
                enrollment_id: "watch-p009a-expiry".into(),
                watch_identity: "watch-research-expiry".into(),
                bound_host_enrollment_id: host.enrollment_id.clone(),
                bound_host_identity: host.host_identity.clone(),
                bound_linux_uid: host.linux_uid,
                public_key_sec1_hex: test_key_hex(&watch_key),
                key_algorithm: KeyAlgorithm::P256EcdsaSha256,
                key_version: 1,
                state: EnrollmentState::Active,
                test_only: true,
            };
            let now_ms = unix_now_ms().unwrap();
            let expired = AuthTransportState::new(AuthTransportConfig {
                host_signing_key: host_key.clone(),
                host_enrollment: host.clone(),
                watch_enrollment: watch.clone(),
                enrollment_expires_at_unix_ms: now_ms + 1,
            })
            .unwrap();
            assert!(expired.activate_at(Instant::now(), now_ms + 1).is_err());
            assert!(expired.inner.lock().unwrap().is_none());

            let overlong = AuthTransportState::new(AuthTransportConfig {
                host_signing_key: host_key.clone(),
                host_enrollment: host.clone(),
                watch_enrollment: watch.clone(),
                enrollment_expires_at_unix_ms: now_ms + MAX_TEST_ENROLLMENT_LIFETIME_MS + 1,
            });
            assert!(overlong.is_err());

            let mismatched = AuthTransportState::new(AuthTransportConfig {
                host_signing_key: test_key(23),
                host_enrollment: host,
                watch_enrollment: watch,
                enrollment_expires_at_unix_ms: now_ms + 3_600_000,
            });
            assert!(mismatched.is_err());
        }

        fn status_transfer_id(state: &AuthTransportState, owner: &str) -> [u8; 8] {
            state
                .read(owner, CharacteristicKind::AuthStatus, 0, 23)
                .unwrap()[3..11]
                .try_into()
                .unwrap()
        }

        fn command(id: [u8; 8], command: RequestCommand) -> Vec<u8> {
            encode_request_command(id, command)
        }

        fn deliver_request(
            state: &AuthTransportState,
            envelope: &SignedEnvelope,
            owner: &str,
            id: [u8; 8],
        ) {
            let frames = encode_auth_frames(AuthDirection::HostToWatch, id, envelope, 23).unwrap();
            for (index, expected) in frames.iter().enumerate() {
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Get { index: index as u8 }),
                        23,
                    )
                    .unwrap();
                assert_eq!(
                    state
                        .read(owner, CharacteristicKind::AuthRequestData, 0, 23)
                        .unwrap(),
                    *expected
                );
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Ack { index: index as u8 }),
                        23,
                    )
                    .unwrap();
            }
            state
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Complete),
                    23,
                )
                .unwrap();
        }

        #[test]
        fn authenticated_gatt_request_delivery_is_explicit_owner_bound_and_single_client() {
            let (state, envelope, _, _, _, _) = auth_state();
            let first = "/org/bluez/hci0/dev_01";
            let second = "/org/bluez/hci0/dev_02";
            let status = state
                .read(first, CharacteristicKind::AuthStatus, 0, 23)
                .unwrap();
            assert_eq!(&status[..3], b"OA\x01");
            assert_eq!(
                u16::from_le_bytes([status[11], status[12]]) as usize,
                envelope.as_bytes().len()
            );
            assert!(
                state
                    .read(second, CharacteristicKind::AuthStatus, 0, 23)
                    .is_err()
            );
            let id: [u8; 8] = status[3..11].try_into().unwrap();
            let frames = encode_auth_frames(AuthDirection::HostToWatch, id, &envelope, 23).unwrap();
            for (index, expected) in frames.iter().enumerate() {
                state
                    .write(
                        first,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Get { index: index as u8 }),
                        23,
                    )
                    .unwrap();
                let read = state
                    .read(first, CharacteristicKind::AuthRequestData, 0, 23)
                    .unwrap();
                assert_eq!(&read, expected);
                assert_eq!(
                    state
                        .read(first, CharacteristicKind::AuthRequestData, 0, 23)
                        .unwrap(),
                    read
                );
                state
                    .write(
                        first,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Ack { index: index as u8 }),
                        23,
                    )
                    .unwrap();
                assert!(
                    state
                        .read(first, CharacteristicKind::AuthRequestData, 0, 23)
                        .is_err()
                );
            }
            state
                .write(
                    first,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Complete),
                    23,
                )
                .unwrap();
            assert!(
                state
                    .write(
                        second,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Complete),
                        23,
                    )
                    .is_err()
            );
            state.invalidate(first);
            assert!(
                state
                    .read(second, CharacteristicKind::AuthStatus, 0, 23)
                    .is_err()
            );
        }

        #[test]
        fn authenticated_gatt_verifies_signed_denial_and_rejects_duplicate_completion() {
            let (state, request_envelope, verified, host, watch, watch_key) = auth_state();
            let owner = "/org/bluez/hci0/dev_01";
            let id = status_transfer_id(&state, owner);
            let request_frames =
                encode_auth_frames(AuthDirection::HostToWatch, id, &request_envelope, 23).unwrap();
            for (index, _) in request_frames.iter().enumerate() {
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Get { index: index as u8 }),
                        23,
                    )
                    .unwrap();
                state
                    .read(owner, CharacteristicKind::AuthRequestData, 0, 23)
                    .unwrap();
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Ack { index: index as u8 }),
                        23,
                    )
                    .unwrap();
            }
            state
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Complete),
                    23,
                )
                .unwrap();
            let signed = create_watch_response(
                verified,
                &watch,
                &watch_key,
                ManualDecision::Reject,
                ConsentResult::ExplicitUserDecision,
                unix_now_ms().unwrap(),
            )
            .unwrap();
            let response = SignedEnvelope::new(
                AuthDirection::WatchToHost,
                &signed.response.canonical_bytes().unwrap(),
                &signed.signature_raw,
            )
            .unwrap();
            let response_frames =
                encode_auth_frames(AuthDirection::WatchToHost, id, &response, 23).unwrap();
            for frame in response_frames {
                state
                    .write(owner, CharacteristicKind::AuthResponseData, &frame, 23)
                    .unwrap();
            }
            let completion = command(id, RequestCommand::Complete);
            state
                .write(
                    owner,
                    CharacteristicKind::AuthResponseControl,
                    &completion,
                    23,
                )
                .unwrap();
            assert_eq!(state.decision(), Some("rejected"));
            assert!(
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthResponseControl,
                        &completion,
                        23
                    )
                    .is_err()
            );
            assert_eq!(state.host_enrollment.enrollment_id, host.enrollment_id);
            assert_eq!(state.watch_enrollment.enrollment_id, watch.enrollment_id);
        }

        #[test]
        fn authenticated_gatt_cancel_timeout_and_wrong_owner_invalidate_the_session() {
            let (state, _, _, _, _, _) = auth_state();
            let owner = "/org/bluez/hci0/dev_01";
            let id = status_transfer_id(&state, owner);
            let other = "/org/bluez/hci0/dev_02";
            assert_eq!(
                state
                    .write(
                        other,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Cancel),
                        23
                    )
                    .unwrap_err()
                    .to_string(),
                "org.bluez.Error.NotPermitted: another client owns this research session"
            );
            state
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Cancel),
                    23,
                )
                .unwrap();
            assert!(
                state
                    .read(owner, CharacteristicKind::AuthStatus, 0, 23)
                    .is_err()
            );

            let (expired, _, _, _, _, _) = auth_state();
            let mut guard = expired.inner.lock().unwrap();
            let inner = guard.as_mut().unwrap();
            inner.signed_deadline = Instant::now() - Duration::from_millis(1);
            drop(guard);
            assert!(
                expired
                    .read(owner, CharacteristicKind::AuthStatus, 0, 23)
                    .is_err()
            );
        }

        #[test]
        fn signed_request_expiring_during_consent_retires_before_response_transfer() {
            let (state, request, _, _, _, _) = auth_state();
            let owner = "/org/bluez/hci0/dev_01";
            let id = status_transfer_id(&state, owner);
            deliver_request(&state, &request, owner, id);

            {
                let mut guard = state.inner.lock().unwrap();
                let inner = guard.as_mut().unwrap();
                inner.signed_deadline = Instant::now() - Duration::from_millis(1);
                inner.expires_at_ms = unix_now_ms().unwrap();
            }

            assert_eq!(
                state.deadline_shutdown_reason(),
                Some("signed_request_expired")
            );
            assert_eq!(state.decision(), None);
            assert!(
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthResponseData,
                        &[b'O', b'A', 1],
                        23
                    )
                    .is_err()
            );
        }

        #[test]
        fn response_started_before_consent_deadline_finishes_in_its_own_transfer_window() {
            let (state, request, verified, _, watch, watch_key) = auth_state();
            let owner = "/org/bluez/hci0/dev_01";
            let id = status_transfer_id(&state, owner);
            deliver_request(&state, &request, owner, id);

            let wall_now = unix_now_ms().unwrap();
            let response_started_at = Instant::now() + Duration::from_secs(1);
            let response_started_wall_ms = wall_now + 1_000;
            let consent_deadline = response_started_at + Duration::from_millis(20);
            {
                let mut guard = state.inner.lock().unwrap();
                guard.as_mut().unwrap().consent_deadline = Some(consent_deadline);
            }

            let signed = create_watch_response(
                verified,
                &watch,
                &watch_key,
                ManualDecision::Reject,
                ConsentResult::ExplicitUserDecision,
                response_started_wall_ms,
            )
            .unwrap();
            let response = SignedEnvelope::new(
                AuthDirection::WatchToHost,
                &signed.response.canonical_bytes().unwrap(),
                &signed.signature_raw,
            )
            .unwrap();
            let response_frames =
                encode_auth_frames(AuthDirection::WatchToHost, id, &response, 23).unwrap();
            state
                .write_at(
                    owner,
                    CharacteristicKind::AuthResponseData,
                    &response_frames[0],
                    23,
                    response_started_at,
                    response_started_wall_ms,
                )
                .unwrap();
            assert!(
                state
                    .inner
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .consent_deadline
                    .is_none()
            );

            let after_consent_deadline = consent_deadline + Duration::from_millis(1);
            let after_consent_wall_ms = response_started_wall_ms + 21;
            for frame in response_frames.iter().skip(1) {
                state
                    .write_at(
                        owner,
                        CharacteristicKind::AuthResponseData,
                        frame,
                        23,
                        after_consent_deadline,
                        after_consent_wall_ms,
                    )
                    .unwrap();
            }
            state
                .write_at(
                    owner,
                    CharacteristicKind::AuthResponseControl,
                    &command(id, RequestCommand::Complete),
                    23,
                    after_consent_deadline,
                    after_consent_wall_ms,
                )
                .unwrap();
            assert_eq!(state.decision(), Some("rejected"));
        }

        #[test]
        fn authenticated_gatt_cancel_is_terminal_before_delivery_after_delivery_and_mid_response() {
            let owner = "/org/bluez/hci0/dev_01";
            let (before_delivery, _, _, _, _, _) = auth_state();
            let id = status_transfer_id(&before_delivery, owner);
            before_delivery
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Cancel),
                    23,
                )
                .unwrap();
            assert_eq!(
                before_delivery.deadline_shutdown_reason(),
                Some("watch_cancelled")
            );

            let (during_delivery, _, _, _, _, _) = auth_state();
            let id = status_transfer_id(&during_delivery, owner);
            during_delivery
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Get { index: 0 }),
                    23,
                )
                .unwrap();
            during_delivery
                .read(owner, CharacteristicKind::AuthRequestData, 0, 23)
                .unwrap();
            during_delivery
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Cancel),
                    23,
                )
                .unwrap();
            assert_eq!(
                during_delivery.deadline_shutdown_reason(),
                Some("watch_cancelled")
            );

            let (after_delivery, request, _, _, _, _) = auth_state();
            let id = status_transfer_id(&after_delivery, owner);
            deliver_request(&after_delivery, &request, owner, id);
            after_delivery
                .write(
                    owner,
                    CharacteristicKind::AuthResponseControl,
                    &command(id, RequestCommand::Cancel),
                    23,
                )
                .unwrap();
            assert_eq!(
                after_delivery.deadline_shutdown_reason(),
                Some("watch_cancelled")
            );

            let (mid_response, request, verified, host, watch, watch_key) = auth_state();
            let id = status_transfer_id(&mid_response, owner);
            deliver_request(&mid_response, &request, owner, id);
            let signed = create_watch_response(
                verified,
                &watch,
                &watch_key,
                ManualDecision::Reject,
                ConsentResult::ExplicitUserDecision,
                unix_now_ms().unwrap(),
            )
            .unwrap();
            let response = SignedEnvelope::new(
                AuthDirection::WatchToHost,
                &signed.response.canonical_bytes().unwrap(),
                &signed.signature_raw,
            )
            .unwrap();
            let first_frame = encode_auth_frames(AuthDirection::WatchToHost, id, &response, 23)
                .unwrap()
                .remove(0);
            mid_response
                .write(
                    owner,
                    CharacteristicKind::AuthResponseData,
                    &first_frame,
                    23,
                )
                .unwrap();
            mid_response
                .write(
                    owner,
                    CharacteristicKind::AuthResponseControl,
                    &command(id, RequestCommand::Cancel),
                    23,
                )
                .unwrap();
            assert_eq!(
                mid_response.deadline_shutdown_reason(),
                Some("watch_cancelled")
            );
            assert_eq!(mid_response.decision(), None);
            assert_eq!(host.enrollment_id, "host-enrollment-p009-test");
        }

        #[test]
        fn connection_loss_during_request_get_read_ack_retires_the_owner() {
            let (state, _, _, _, _, _) = auth_state();
            let owner = "/org/bluez/hci0/dev_01";
            let id = status_transfer_id(&state, owner);
            state
                .write(
                    owner,
                    CharacteristicKind::AuthRequestControl,
                    &command(id, RequestCommand::Get { index: 0 }),
                    23,
                )
                .unwrap();
            state
                .read(owner, CharacteristicKind::AuthRequestData, 0, 23)
                .unwrap();
            state.invalidate(owner);
            assert_eq!(state.deadline_shutdown_reason(), Some("connection_lost"));
            assert!(
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthRequestControl,
                        &command(id, RequestCommand::Ack { index: 0 }),
                        23,
                    )
                    .is_err()
            );
            assert!(
                state
                    .read(
                        "/org/bluez/hci0/dev_02",
                        CharacteristicKind::AuthStatus,
                        0,
                        23
                    )
                    .is_err()
            );
        }

        #[test]
        fn authenticated_gatt_retires_a_complete_response_with_a_bad_signature() {
            let (state, request_envelope, verified, _host, watch, watch_key) = auth_state();
            let owner = "/org/bluez/hci0/dev_01";
            let id = status_transfer_id(&state, owner);
            deliver_request(&state, &request_envelope, owner, id);
            let signed = create_watch_response(
                verified,
                &watch,
                &watch_key,
                ManualDecision::Reject,
                ConsentResult::ExplicitUserDecision,
                unix_now_ms().unwrap(),
            )
            .unwrap();
            let bad_signature = [0x99; 64];
            let response = SignedEnvelope::new(
                AuthDirection::WatchToHost,
                &signed.response.canonical_bytes().unwrap(),
                &bad_signature,
            )
            .unwrap();
            let frames = encode_auth_frames(AuthDirection::WatchToHost, id, &response, 23).unwrap();
            for frame in frames {
                state
                    .write(owner, CharacteristicKind::AuthResponseData, &frame, 23)
                    .unwrap();
            }
            assert!(
                state
                    .write(
                        owner,
                        CharacteristicKind::AuthResponseControl,
                        &command(id, RequestCommand::Complete),
                        23
                    )
                    .is_err()
            );
            assert_eq!(state.decision(), None);
            assert_eq!(
                state.deadline_shutdown_reason(),
                Some("response_verification_failed")
            );
        }

        fn challenge(state: &GattState, device: &str) -> [u8; 8] {
            state
                .client_value(device, CharacteristicKind::Challenge, 0, 517)
                .unwrap()[1..9]
                .try_into()
                .unwrap()
        }

        fn finish(state: &GattState, device: &str, id: [u8; 8]) {
            let frame =
                opencontinuity_core::gatt_wire::encode_frame(id, 0, true, true, 1, b"x").unwrap();
            state.write_exchange(device, &frame, 23).unwrap();
            state
                .write_control(
                    device,
                    &opencontinuity_core::gatt_wire::encode_control(id, ControlCommand::Complete),
                )
                .unwrap();
        }

        #[test]
        fn result_stays_with_original_session_until_final_fragment_consumed() {
            let state = GattState::default();
            let device = "/org/bluez/hci0/dev_01";
            let id = challenge(&state, device);
            finish(&state, device, id);
            assert!(
                state
                    .client_value(device, CharacteristicKind::Challenge, 0, 23)
                    .is_err()
            );
            let first = state
                .client_value(device, CharacteristicKind::Response, 0, 23)
                .unwrap();
            assert_eq!(&first[2..10], &id);
            assert!(
                state
                    .client_value(device, CharacteristicKind::Challenge, 0, 23)
                    .is_err()
            );
            let second = state
                .client_value(device, CharacteristicKind::Response, 22, 23)
                .unwrap();
            assert_eq!(first.len() + second.len(), 44);
            assert!(
                state
                    .client_value(device, CharacteristicKind::Challenge, 0, 23)
                    .is_err()
            );
            assert!(
                state
                    .client_value(device, CharacteristicKind::Response, 44, 23)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                state
                    .client_value(device, CharacteristicKind::Response, 0, 23)
                    .is_err()
            );
            let next = challenge(&state, device);
            assert_ne!(id, next);
            assert!(
                state
                    .client_value(device, CharacteristicKind::Response, 0, 23)
                    .is_err()
            );
            let old =
                opencontinuity_core::gatt_wire::encode_frame(id, 0, true, true, 1, b"x").unwrap();
            assert!(state.write_exchange(device, &old, 23).is_err());
            let error = state
                .client_value(device, CharacteristicKind::Response, 0, 23)
                .unwrap();
            assert_eq!(&error[2..10], &next);
        }

        #[test]
        fn cancellation_expiry_and_reconnect_invalidate_pending_work() {
            let state = GattState::default();
            let device = "/org/bluez/hci0/dev_01";
            let id = challenge(&state, device);
            let partial =
                opencontinuity_core::gatt_wire::encode_frame(id, 0, true, false, 2, b"x").unwrap();
            state.write_exchange(device, &partial, 23).unwrap();
            state
                .write_control(
                    device,
                    &opencontinuity_core::gatt_wire::encode_control(id, ControlCommand::Cancel),
                )
                .unwrap();
            assert!(state.write_exchange(device, &partial, 23).is_err());
            let response = state
                .client_value(device, CharacteristicKind::Response, 0, 517)
                .unwrap();
            assert_eq!(response[1], 2);
            let next = challenge(&state, device);
            finish(&state, device, next);
            if let SessionPhase::ResponseAvailable { since, .. } =
                &mut state.clients.lock().unwrap().get_mut(device).unwrap().phase
            {
                *since = Instant::now() - RESPONSE_TIMEOUT - Duration::from_secs(1);
            } else {
                panic!("expected response");
            }
            assert!(
                state
                    .client_value(device, CharacteristicKind::Response, 0, 23)
                    .is_err()
            );
            let third = challenge(&state, device);
            assert_ne!(next, third);
            state.prune_devices(&HashMap::new()).unwrap();
            let reconnected = challenge(&state, device);
            assert_ne!(third, reconnected);
        }

        #[test]
        fn concurrent_clients_cannot_read_or_replace_each_others_results() {
            let state = GattState::default();
            let first = "/org/bluez/hci0/dev_01";
            let second = "/org/bluez/hci0/dev_02";
            let first_id = challenge(&state, first);
            let second_id = challenge(&state, second);
            finish(&state, first, first_id);
            assert!(
                state
                    .client_value(second, CharacteristicKind::Response, 0, 517)
                    .is_err()
            );
            finish(&state, second, second_id);
            let a = state
                .client_value(first, CharacteristicKind::Response, 0, 517)
                .unwrap();
            let b = state
                .client_value(second, CharacteristicKind::Response, 0, 517)
                .unwrap();
            assert_eq!(&a[2..10], &first_id);
            assert_eq!(&b[2..10], &second_id);
            assert_ne!(&a[2..10], &b[2..10]);
        }

        #[test]
        fn client_limit_and_idle_expiry_are_bounded() {
            let state = GattState::default();
            for index in 0..MAX_CLIENTS {
                challenge(&state, &format!("/org/bluez/hci0/dev_{index}"));
            }
            assert!(
                state
                    .client_value(
                        "/org/bluez/hci0/dev_extra",
                        CharacteristicKind::Challenge,
                        0,
                        23
                    )
                    .is_err()
            );
            state
                .clients
                .lock()
                .unwrap()
                .get_mut("/org/bluez/hci0/dev_0")
                .unwrap()
                .last_activity = Instant::now() - IDLE_TIMEOUT - Duration::from_secs(1);
            challenge(&state, "/org/bluez/hci0/dev_extra");
            assert_eq!(state.clients.lock().unwrap().len(), MAX_CLIENTS);
        }

        #[test]
        fn monitor_failure_closes_gate_before_clearing_sessions() {
            let state = GattState::default();
            let gate = MonitorGate::new(":1.1");
            gate.inner.lock().unwrap().healthy = true;
            challenge(&state, "/org/bluez/hci0/dev_01");
            gate.fail(&state, "test failure");
            assert!(state.clients.lock().unwrap().is_empty());
            assert!(
                gate.with_live(|| state.client_value(
                    "/org/bluez/hci0/dev_01",
                    CharacteristicKind::Challenge,
                    0,
                    23
                ))
                .is_err()
            );
            assert!(state.clients.lock().unwrap().is_empty());
        }

        #[test]
        fn options_and_offsets_are_strict() {
            let mut options = HashMap::new();
            options.insert("mtu".into(), OwnedValue::from(22u16));
            assert!(read_options(&options).is_err());
            options.insert("mtu".into(), OwnedValue::from(518u16));
            assert!(read_options(&options).is_err());
            options.insert("mtu".into(), OwnedValue::from(true));
            assert!(read_options(&options).is_err());
            options.clear();
            options.insert("offset".into(), OwnedValue::from(true));
            assert!(read_options(&options).is_err());
            assert!(option_device(&options).is_err());
            assert!(read_fragment(b"abc", 4, 23).is_err());
            assert_eq!(read_fragment(b"abc", 3, 23).unwrap(), b"");
        }

        #[test]
        fn client_sessions_are_isolated_and_disconnect_drops_state() {
            let state = GattState::default();
            let first = state
                .client_value(
                    "/org/bluez/hci0/dev_01",
                    CharacteristicKind::Challenge,
                    0,
                    517,
                )
                .unwrap();
            let second = state
                .client_value(
                    "/org/bluez/hci0/dev_02",
                    CharacteristicKind::Challenge,
                    0,
                    517,
                )
                .unwrap();
            assert_ne!(&first[1..9], &second[1..9]);
            let first_id: [u8; 8] = first[1..9].try_into().unwrap();
            let frame =
                opencontinuity_core::gatt_wire::encode_frame(first_id, 0, true, true, 1, b"x")
                    .unwrap();
            assert!(
                state
                    .write_exchange("/org/bluez/hci0/dev_02", &frame, 23)
                    .is_err()
            );
            state
                .prune_devices(&HashMap::from([(
                    "/org/bluez/hci0/dev_01".to_owned(),
                    true,
                )]))
                .unwrap();
            let clients = state.clients.lock().unwrap();
            assert!(clients.contains_key("/org/bluez/hci0/dev_01"));
            assert!(!clients.contains_key("/org/bluez/hci0/dev_02"));
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{serve, serve_auth};

#[cfg(not(target_os = "linux"))]
pub fn serve(
    adapter: &str,
    duration: u16,
    _terminate: Arc<AtomicBool>,
) -> Result<ExperimentReport, ExperimentError> {
    Err(ExperimentError {
        report: Box::new(ExperimentReport {
            status: "failed",
            stage: "preflight",
            adapter: adapter.into(),
            runtime_seconds: duration,
            shutdown_reason: None,
            application_registered: false,
            advertisement_registered: false,
            application_registration_succeeded: false,
            advertisement_registration_succeeded: false,
            cleanup_errors: Vec::new(),
            controller_le_peripheral_capability: "unknown",
            supported_advertising_instances: None,
            active_advertising_instances: None,
            error_class: Some("unsupported_platform"),
            error: Some("experimental BlueZ GATT transport is Linux-only".into()),
            authorization: "never_evaluated",
        }),
    })
}

#[cfg(not(target_os = "linux"))]
pub fn serve_auth(
    adapter: &str,
    duration: u16,
    _terminate: Arc<AtomicBool>,
    _config: AuthTransportConfig,
) -> Result<AuthExperimentReport, ExperimentError> {
    Err(ExperimentError {
        report: Box::new(ExperimentReport {
            status: "failed",
            stage: "preflight",
            adapter: adapter.into(),
            runtime_seconds: duration,
            shutdown_reason: None,
            application_registered: false,
            advertisement_registered: false,
            application_registration_succeeded: false,
            advertisement_registration_succeeded: false,
            cleanup_errors: Vec::new(),
            controller_le_peripheral_capability: "unknown",
            supported_advertising_instances: None,
            active_advertising_instances: None,
            error_class: Some("unsupported_platform"),
            error: Some("authenticated research GATT transport is Linux-only".into()),
            authorization: "never_evaluated",
        }),
    })
}

#[allow(dead_code)]
fn _duration(_: Duration) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_explicit_opt_in_adapter_and_bounded_duration() {
        assert!(validate_options("hci0", 60, false).is_err());
        assert!(validate_options("hci0", 0, true).is_err());
        assert!(validate_options("hci0", 301, true).is_err());
        assert!(validate_options("hci0", 60, true).is_ok());
        assert!(validate_options("/org/bluez/hci0", 60, true).is_err());
        assert!(validate_options("hci0;pair", 60, true).is_err());
    }
}
