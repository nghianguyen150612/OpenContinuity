//! Portable domain contracts for OpenContinuity.
//!
//! This crate intentionally has no operating-system, Bluetooth, GUI, or
//! authentication integration dependencies.

use serde::{Deserialize, Serialize};

pub mod bluez_research;
pub mod research;
pub mod watch_endpoint;

/// The evidence-backed state of a capability or trust stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Experimental,
    Unsupported,
    Blocked,
    Unknown,
}

/// Separate evidence stages used when assessing a device for a sensitive action.
/// All stages default to unknown; detection alone never implies authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustAssessment {
    pub device_detection: CapabilityState,
    pub trusted_identity: CapabilityState,
    pub cryptographic_authentication: CapabilityState,
    pub session_freshness: CapabilityState,
    pub proximity_assurance: CapabilityState,
    pub operational_readiness: CapabilityState,
}

impl Default for TrustAssessment {
    fn default() -> Self {
        Self {
            device_detection: CapabilityState::Unknown,
            trusted_identity: CapabilityState::Unknown,
            cryptographic_authentication: CapabilityState::Unknown,
            session_freshness: CapabilityState::Unknown,
            proximity_assurance: CapabilityState::Unknown,
            operational_readiness: CapabilityState::Unknown,
        }
    }
}

/// A user-visible feature and the current evidence-backed implementation state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub state: CapabilityState,
    pub summary: String,
}

/// Stable JSON contract emitted by `opencontinuity doctor --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema_version: u8,
    pub project: String,
    pub version: String,
    pub capabilities: Vec<Capability>,
    pub watch_auto_unlock_trust: TrustAssessment,
}

impl DoctorReport {
    /// Capabilities actually implemented or explicitly absent in P001.
    /// This is deliberately static: doctor does not scan devices or infer trust.
    pub fn current(version: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            project: "OpenContinuity".into(),
            version: version.into(),
            capabilities: vec![
                Capability {
                    id: "apple_watch_auto_unlock".into(),
                    state: CapabilityState::Unsupported,
                    summary: "Native protocol support is not implemented in this build.".into(),
                },
                Capability {
                    id: "airdrop".into(),
                    state: CapabilityState::Unsupported,
                    summary:
                        "Native AWDL/AirDrop interoperability is not implemented in this build."
                            .into(),
                },
                Capability {
                    id: "airpods_settings".into(),
                    state: CapabilityState::Unsupported,
                    summary:
                        "AirPods protocol and Settings support is not implemented in this build."
                            .into(),
                },
            ],
            watch_auto_unlock_trust: TrustAssessment::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn capability_states_serialize_in_snake_case() {
        let states = [
            (CapabilityState::Supported, "supported"),
            (CapabilityState::Experimental, "experimental"),
            (CapabilityState::Unsupported, "unsupported"),
            (CapabilityState::Blocked, "blocked"),
            (CapabilityState::Unknown, "unknown"),
        ];
        for (state, expected) in states {
            let encoded = serde_json::to_string(&state).unwrap();
            assert_eq!(encoded, format!("\"{expected}\""));
            assert_eq!(
                serde_json::from_str::<CapabilityState>(&encoded).unwrap(),
                state
            );
        }
    }

    #[test]
    fn detection_does_not_imply_identity_or_authorization() {
        let assessment = TrustAssessment {
            device_detection: CapabilityState::Supported,
            ..TrustAssessment::default()
        };
        assert_eq!(assessment.trusted_identity, CapabilityState::Unknown);
        assert_eq!(
            assessment.cryptographic_authentication,
            CapabilityState::Unknown
        );
        assert_eq!(assessment.operational_readiness, CapabilityState::Unknown);
    }

    #[test]
    fn doctor_json_has_a_stable_schema_and_unknown_trust_defaults() {
        let report = DoctorReport::current("0.1.0");
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schema_version"], json!(1));
        assert_eq!(value["project"], json!("OpenContinuity"));
        assert_eq!(
            value["watch_auto_unlock_trust"]["device_detection"],
            json!("unknown")
        );
        assert_eq!(
            value["watch_auto_unlock_trust"]["cryptographic_authentication"],
            json!("unknown")
        );
        assert_eq!(value["capabilities"][0]["state"], json!("unsupported"));
        assert_eq!(
            serde_json::from_value::<DoctorReport>(value).unwrap(),
            report
        );
    }
}
