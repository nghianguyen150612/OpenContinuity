//! Read-only AirDrop/AWDL diagnostic report contracts.
//!
//! These types describe local observations. They do not represent a transfer,
//! Apple identity, authorization, or physical interoperability result.

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    BackendNotInstalled,
    BackendVersionUnsupported,
    RadioNotPrepared,
    RadioStateUnknown,
    BackendProbeFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    LocalSysfs,
    OwlyDoctor,
    Unknown,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_defaults_keep_authorization_and_qualification_unknown() {
        let report = AirDropReport::unknown();
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schemaVersion"], json!(1));
        assert_eq!(value["authorization"], json!("never_evaluated"));
        assert_eq!(value["physicalQualification"], json!("not_qualified"));
        assert_eq!(value["receiverAvailable"], json!(false));
        assert_eq!(value["liveAirdropInteroperability"], json!("unknown"));
        assert!(value["radio"].get("bssid").is_none());
        assert!(value["radio"].get("ssid").is_none());
        assert!(value["radio"].get("interfaceName").is_none());
    }
}
