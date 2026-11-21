//! Evidence-backed AirDrop/AWDL capability and receive lifecycle contracts.
//!
//! These types distinguish local backend state from Apple interoperability.
//! A ready backend is never proof of a real AirDrop exchange or authorization.

use serde::{Deserialize, Serialize};

/// Runtime state of a selected AirDrop/AWDL backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendState {
    Unsupported,
    Unknown,
    Available,
    Blocked,
    Ready,
    Active,
    Failed,
}

/// Status of an independently observed control or data plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaneState {
    Unsupported,
    Unknown,
    Available,
    Blocked,
    Ready,
    Active,
    Failed,
}

/// Operational constraints that prevent live receive from starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    BackendNotInstalled,
    BackendVersionUnsupported,
    RadioNotPrepared,
    RadioStateUnknown,
    BackendProbeFailed,
    PermissionUnavailable,
    DestinationUnavailable,
    InteractiveConsentRequired,
    ConcurrentReceive,
    SessionExpired,
    Cancelled,
    OutputLimitExceeded,
    BackendExited,
    BackendFailed,
}

/// Provenance label for local read-only discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    LocalSysfs,
    OwlyDoctor,
    OwlyProcess,
    Unknown,
}

/// A bounded, sanitized machine-readable backend report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AirDropReport {
    pub schema_version: u8,
    pub kind: &'static str,
    pub source: ObservationSource,
    pub authorization: &'static str,
    pub physical_qualification: &'static str,
    pub backend: BackendState,
    pub control_plane: PlaneState,
    pub data_plane: PlaneState,
    pub advertisement: PlaneState,
    pub receiver_available: bool,
    pub live_airdrop_interoperability: BackendState,
    pub block_reason: Option<BlockReason>,
    pub radio: RadioReport,
    pub upstream_version: Option<String>,
}

impl AirDropReport {
    /// Conservative no-radio baseline. This does not inspect or change devices.
    pub fn unknown() -> Self {
        Self {
            schema_version: 1,
            kind: "airdrop_backend_status",
            source: ObservationSource::Unknown,
            authorization: "never_evaluated",
            physical_qualification: "not_qualified",
            backend: BackendState::Unknown,
            control_plane: PlaneState::Unknown,
            data_plane: PlaneState::Unknown,
            advertisement: PlaneState::Unknown,
            receiver_available: false,
            live_airdrop_interoperability: BackendState::Unknown,
            block_reason: None,
            radio: RadioReport::unknown(),
            upstream_version: None,
        }
    }
}

/// Sanitized local wireless diagnostics. Device addresses and network names
/// are deliberately absent; a PCI ID is present only in this explicit local
/// doctor report and is never included in failure text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioReport {
    pub state: BackendState,
    pub kernel_release: Option<String>,
    pub driver: Option<String>,
    pub pci_id: Option<String>,
    pub regulatory_domain: Option<String>,
    pub wifi_channel: Option<u16>,
    pub associated: Option<bool>,
    pub monitor_mode: BackendState,
    pub wireless_interface_count: u8,
    pub conflicting_wireless_interface_count: u8,
    pub awdl_interface_state: BackendState,
    pub awdl_link_local_address_present: bool,
    pub effective_uid_is_root: bool,
    pub net_admin_capability: bool,
    pub net_raw_capability: bool,
}

impl RadioReport {
    pub fn unknown() -> Self {
        Self {
            state: BackendState::Unknown,
            kernel_release: None,
            driver: None,
            pci_id: None,
            regulatory_domain: None,
            wifi_channel: None,
            associated: None,
            monitor_mode: BackendState::Unknown,
            wireless_interface_count: 0,
            conflicting_wireless_interface_count: 0,
            awdl_interface_state: BackendState::Unknown,
            awdl_link_local_address_present: false,
            effective_uid_is_root: false,
            net_admin_capability: false,
            net_raw_capability: false,
        }
    }
}

/// Receive-session state. Terminal states cannot be reopened in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveState {
    Idle,
    Preparing,
    Active,
    Cancelling,
    Completed,
    Cancelled,
    Expired,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiveLifecycle {
    state: ReceiveState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidReceiveTransition {
    pub from: ReceiveState,
    pub to: ReceiveState,
}

impl std::fmt::Display for InvalidReceiveTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid receive transition {:?} -> {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for InvalidReceiveTransition {}

impl Default for ReceiveLifecycle {
    fn default() -> Self {
        Self {
            state: ReceiveState::Idle,
        }
    }
}

impl ReceiveLifecycle {
    pub fn state(self) -> ReceiveState {
        self.state
    }

    pub fn transition(&mut self, next: ReceiveState) -> Result<(), InvalidReceiveTransition> {
        use ReceiveState as S;
        let allowed = matches!(
            (self.state, next),
            (S::Idle, S::Preparing)
                | (
                    S::Preparing,
                    S::Active | S::Cancelled | S::Expired | S::Failed
                )
                | (
                    S::Active,
                    S::Cancelling | S::Completed | S::Cancelled | S::Expired | S::Failed
                )
                | (
                    S::Cancelling,
                    S::Cancelled | S::Expired | S::Failed | S::Completed
                )
        );
        if !allowed {
            return Err(InvalidReceiveTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn backend_states_and_provenance_are_explicit_and_serialized() {
        let values = [
            (BackendState::Unsupported, "unsupported"),
            (BackendState::Unknown, "unknown"),
            (BackendState::Available, "available"),
            (BackendState::Blocked, "blocked"),
            (BackendState::Ready, "ready"),
            (BackendState::Active, "active"),
            (BackendState::Failed, "failed"),
        ];
        for (state, name) in values {
            assert_eq!(serde_json::to_value(state).unwrap(), json!(name));
            assert_eq!(
                serde_json::from_value::<BackendState>(json!(name)).unwrap(),
                state
            );
        }
        let report = AirDropReport::unknown();
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schemaVersion"], json!(1));
        assert_eq!(value["authorization"], json!("never_evaluated"));
        assert_eq!(value["physicalQualification"], json!("not_qualified"));
        assert_eq!(value["backend"], json!("unknown"));
        assert_eq!(value["advertisement"], json!("unknown"));
        assert!(value["radio"].get("bssid").is_none());
        assert!(value["radio"].get("ssid").is_none());
        assert!(value["radio"].get("interfaceName").is_none());
    }

    #[test]
    fn receive_lifecycle_requires_explicit_setup_and_is_terminal() {
        let mut lifecycle = ReceiveLifecycle::default();
        assert_eq!(lifecycle.state(), ReceiveState::Idle);
        lifecycle.transition(ReceiveState::Preparing).unwrap();
        lifecycle.transition(ReceiveState::Active).unwrap();
        lifecycle.transition(ReceiveState::Cancelling).unwrap();
        lifecycle.transition(ReceiveState::Cancelled).unwrap();
        assert_eq!(lifecycle.state(), ReceiveState::Cancelled);
        assert_eq!(
            lifecycle.transition(ReceiveState::Active),
            Err(InvalidReceiveTransition {
                from: ReceiveState::Cancelled,
                to: ReceiveState::Active
            })
        );
    }

    #[test]
    fn receive_lifecycle_accepts_deadline_and_failure_terminals() {
        let mut expired = ReceiveLifecycle::default();
        expired.transition(ReceiveState::Preparing).unwrap();
        expired.transition(ReceiveState::Expired).unwrap();
        let mut failed = ReceiveLifecycle::default();
        failed.transition(ReceiveState::Preparing).unwrap();
        failed.transition(ReceiveState::Failed).unwrap();
        assert_eq!(expired.state(), ReceiveState::Expired);
        assert_eq!(failed.state(), ReceiveState::Failed);
    }
}
