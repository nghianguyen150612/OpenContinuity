//! Read-only protocol research contracts.
//!
//! The types in this module describe sanitized observations only. They do not
//! model pairing, authentication, unlock authorization, or device identity.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const FIXTURE_SCHEMA_VERSION: u8 = 1;
const MAX_OBSERVATIONS: usize = 1024;
const MAX_OBSERVATION_ID_BYTES: usize = 64;
const MAX_DEVICE_NAME_BYTES: usize = 248;
const MAX_COMPANY_IDS: usize = 32;

/// Evidence classification used by P002 research documents and reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Verified,
    Documented,
    Hypothesis,
    Unknown,
    Blocked,
}

/// A constrained topic vocabulary for protocol capability reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchArea {
    BleDiscovery,
    AppleContinuityAdvertisement,
    DeviceIdentity,
    TrustEstablishment,
    SessionAuthentication,
    FreshnessAndReplayResistance,
    ProximityAssurance,
    AppleWatchSe2Compatibility,
    LinuxBluezSupport,
}

/// A citation-bearing evidence claim. Claims remain descriptive and cannot
/// grant authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceClaim {
    pub area: ResearchArea,
    pub status: EvidenceStatus,
    pub summary: String,
    pub references: Vec<String>,
}

/// The only discovery input accepted by the current research CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchFixture {
    pub schema_version: u8,
    pub observations: Vec<DiscoveryObservation>,
}

/// Sanitized discovery metadata. No address, payload, key, or trust field is
/// represented here; labels and RSSI are untrusted input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryObservation {
    pub observation_id: String,
    #[serde(default)]
    pub untrusted_name: Option<String>,
    #[serde(default)]
    pub rssi_dbm: Option<i16>,
    #[serde(default)]
    pub manufacturer_company_ids: Vec<u16>,
}

/// Explicit marker that no authorization assessment has been made. There is
/// intentionally no `Authorized` variant in the read-only research contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationState {
    NeverEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryReport {
    pub schema_version: u8,
    pub source: DiscoverySource,
    pub observations: Vec<DiscoveryObservation>,
    pub authorization: AuthorizationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
    MockFixture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchError {
    InvalidFixture(String),
    Discovery(String),
}

impl std::fmt::Display for ResearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFixture(reason) => write!(formatter, "invalid research fixture: {reason}"),
            Self::Discovery(reason) => write!(formatter, "read-only discovery failed: {reason}"),
        }
    }
}

impl std::error::Error for ResearchError {}

impl ResearchFixture {
    /// Parse and validate a JSON fixture. Unknown fields are rejected so that
    /// addresses, raw advertisement data, or future trust fields cannot be
    /// smuggled into this version of the read-only schema.
    pub fn from_json(input: &str) -> Result<Self, ResearchError> {
        let fixture: Self = serde_json::from_str(input)
            .map_err(|error| ResearchError::InvalidFixture(error.to_string()))?;
        fixture.validate()?;
        Ok(fixture)
    }

    pub fn validate(&self) -> Result<(), ResearchError> {
        if self.schema_version != FIXTURE_SCHEMA_VERSION {
            return Err(ResearchError::InvalidFixture(format!(
                "unsupported schema_version {}; expected {FIXTURE_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.observations.len() > MAX_OBSERVATIONS {
            return Err(ResearchError::InvalidFixture(format!(
                "too many observations: maximum is {MAX_OBSERVATIONS}"
            )));
        }

        let mut ids = HashSet::new();
        for observation in &self.observations {
            let id = observation.observation_id.as_bytes();
            if id.is_empty()
                || id.len() > MAX_OBSERVATION_ID_BYTES
                || !id
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                return Err(ResearchError::InvalidFixture(
                    "observation_id must be 1-64 ASCII letters, digits, '_' or '-'".into(),
                ));
            }
            if !ids.insert(&observation.observation_id) {
                return Err(ResearchError::InvalidFixture(format!(
                    "duplicate observation_id: {}",
                    observation.observation_id
                )));
            }
            if observation
                .untrusted_name
                .as_ref()
                .is_some_and(|name| name.len() > MAX_DEVICE_NAME_BYTES)
            {
                return Err(ResearchError::InvalidFixture(format!(
                    "untrusted_name exceeds {MAX_DEVICE_NAME_BYTES} UTF-8 bytes"
                )));
            }
            if observation
                .rssi_dbm
                .is_some_and(|rssi| !(-127..=20).contains(&rssi))
            {
                return Err(ResearchError::InvalidFixture(
                    "rssi_dbm must be between -127 and 20 inclusive".into(),
                ));
            }
            if observation.manufacturer_company_ids.len() > MAX_COMPANY_IDS {
                return Err(ResearchError::InvalidFixture(format!(
                    "too many manufacturer_company_ids: maximum is {MAX_COMPANY_IDS}"
                )));
            }
            let mut company_ids = HashSet::new();
            if observation
                .manufacturer_company_ids
                .iter()
                .any(|company_id| !company_ids.insert(company_id))
            {
                return Err(ResearchError::InvalidFixture(
                    "manufacturer_company_ids must not contain duplicates".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Backend boundary kept platform-independent. Implementations must only
/// return observations and must not pair, connect, write, or authorize.
pub trait DiscoveryAdapter {
    fn discover(&mut self) -> Result<Vec<DiscoveryObservation>, ResearchError>;
}

/// Deterministic adapter for sanitized fixtures and automated tests.
#[derive(Debug, Clone)]
pub struct MockDiscoveryAdapter {
    fixture: ResearchFixture,
    fail_with: Option<String>,
}

impl MockDiscoveryAdapter {
    pub fn new(fixture: ResearchFixture) -> Result<Self, ResearchError> {
        fixture.validate()?;
        Ok(Self {
            fixture,
            fail_with: None,
        })
    }

    /// Construct a deterministic failing mock for fail-closed tests.
    pub fn failing(
        fixture: ResearchFixture,
        reason: impl Into<String>,
    ) -> Result<Self, ResearchError> {
        fixture.validate()?;
        Ok(Self {
            fixture,
            fail_with: Some(reason.into()),
        })
    }
}

impl DiscoveryAdapter for MockDiscoveryAdapter {
    fn discover(&mut self) -> Result<Vec<DiscoveryObservation>, ResearchError> {
        if let Some(reason) = &self.fail_with {
            return Err(ResearchError::Discovery(reason.clone()));
        }
        Ok(self.fixture.observations.clone())
    }
}

pub fn read_only_report(
    adapter: &mut impl DiscoveryAdapter,
) -> Result<DiscoveryReport, ResearchError> {
    let observations = adapter.discover()?;
    Ok(DiscoveryReport {
        schema_version: 1,
        source: DiscoverySource::MockFixture,
        observations,
        authorization: AuthorizationState::NeverEvaluated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_fixture() -> ResearchFixture {
        ResearchFixture {
            schema_version: 1,
            observations: vec![DiscoveryObservation {
                observation_id: "sample-1".into(),
                untrusted_name: Some("Watch-like label".into()),
                rssi_dbm: Some(-55),
                manufacturer_company_ids: vec![0x004c],
            }],
        }
    }

    #[test]
    fn evidence_states_serialize_and_round_trip() {
        let states = [
            (EvidenceStatus::Verified, "verified"),
            (EvidenceStatus::Documented, "documented"),
            (EvidenceStatus::Hypothesis, "hypothesis"),
            (EvidenceStatus::Unknown, "unknown"),
            (EvidenceStatus::Blocked, "blocked"),
        ];
        for (state, expected) in states {
            let value = serde_json::to_value(state).unwrap();
            assert_eq!(value, json!(expected));
            assert_eq!(
                serde_json::from_value::<EvidenceStatus>(value).unwrap(),
                state
            );
        }
    }

    #[test]
    fn unknown_and_blocked_are_explicit_evidence_states() {
        assert_ne!(EvidenceStatus::Unknown, EvidenceStatus::Blocked);
        assert_eq!(
            serde_json::from_str::<EvidenceStatus>("\"unknown\"").unwrap(),
            EvidenceStatus::Unknown
        );
        assert_eq!(
            serde_json::from_str::<EvidenceStatus>("\"blocked\"").unwrap(),
            EvidenceStatus::Blocked
        );
    }

    #[test]
    fn fixture_rejects_malformed_or_untrusted_fields() {
        let malformed = [
            "not-json",
            r#"{"schema_version":2,"observations":[]}"#,
            r#"{"schema_version":1,"observations":[{"observation_id":"bad id","manufacturer_company_ids":[]}]}"#,
            r#"{"schema_version":1,"observations":[{"observation_id":"x","rssi_dbm":21,"manufacturer_company_ids":[]}]}"#,
            r#"{"schema_version":1,"observations":[],"address":"00:11:22:33:44:55"}"#,
        ];
        for input in malformed {
            assert!(
                ResearchFixture::from_json(input).is_err(),
                "accepted {input}"
            );
        }
    }

    #[test]
    fn fixture_adapter_is_deterministic_and_discovery_never_authorizes() {
        let fixture = valid_fixture();
        let expected = fixture.observations.clone();
        let mut adapter = MockDiscoveryAdapter::new(fixture).unwrap();
        let first = read_only_report(&mut adapter).unwrap();
        let second = read_only_report(&mut adapter).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.observations, expected);
        assert_eq!(first.authorization, AuthorizationState::NeverEvaluated);
        assert_eq!(
            serde_json::to_value(first).unwrap()["authorization"],
            json!("never_evaluated")
        );
    }

    #[test]
    fn adapter_errors_fail_closed_without_a_report() {
        let mut adapter =
            MockDiscoveryAdapter::failing(valid_fixture(), "fixture backend error").unwrap();
        assert_eq!(
            read_only_report(&mut adapter),
            Err(ResearchError::Discovery("fixture backend error".into()))
        );
    }
}
