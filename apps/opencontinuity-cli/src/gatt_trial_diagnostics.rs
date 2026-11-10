//! Explicit opt-in, bounded metadata for local P006/P009 trial lifecycle review.
use serde::Serialize;
use std::io::Write;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

const MAX_EVENTS: usize = 15; // keep room for a terminal record
const MAX_JSON_BYTES: usize = 2 * 1024;

pub fn is_valid_trial_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Start,
    AdapterReady,
    ApplicationRegistration,
    AdvertisementRegistration,
    Session,
    Frame,
    SignedRequestRead,
    SignedResponseFrame,
    Acknowledgement,
    SyntheticExchange,
    SignedExchange,
    ConsentDecision,
    Cancellation,
    Expiration,
    Disconnect,
    Failure,
    CleanupAdvertisement,
    CleanupApplication,
    Terminal,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Attempted,
    Confirmed,
    Failed,
    Observed,
    Cancelled,
    Expired,
    Unknown,
    Approved,
    Rejected,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    sequence: u8,
    elapsed_ms: u32,
    phase: Phase,
    outcome: Outcome,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u8,
    kind: &'static str,
    source: &'static str,
    trial_id: Option<String>,
    authorization: &'static str,
    incomplete: bool,
    frame_write_count: u16,
    signed_write_count: u16,
    signed_request_frame_count: u16,
    signed_response_frame_count: u16,
    acknowledgement_count: u16,
    events: Vec<Event>,
}

pub struct Trace {
    started: Instant,
    inner: Mutex<Report>,
    missed_events: AtomicBool,
}

impl Trace {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            inner: Mutex::new(Report {
                schema_version: 1,
                kind: "gatt_trial_diagnostics",
                source: "host_runtime_observed",
                trial_id: None,
                authorization: "never_evaluated",
                incomplete: false,
                frame_write_count: 0,
                signed_write_count: 0,
                signed_request_frame_count: 0,
                signed_response_frame_count: 0,
                acknowledgement_count: 0,
                events: Vec::with_capacity(MAX_EVENTS + 1),
            }),
            missed_events: AtomicBool::new(false),
        }
    }

    pub fn set_trial_id(&self, bytes: &[u8]) {
        if bytes.len() != 16 {
            self.missed_events.store(true, Ordering::Relaxed);
            return;
        }
        let mut value = String::with_capacity(32);
        for byte in bytes {
            use std::fmt::Write as _;
            let _ = write!(&mut value, "{byte:02x}");
        }
        if let Ok(mut report) = self.inner.try_lock() {
            report.trial_id = Some(value);
        } else {
            self.missed_events.store(true, Ordering::Relaxed);
        }
    }

    /// In-memory fixed metadata only. Never performs I/O from a GATT callback.
    pub fn record(&self, phase: Phase, mut outcome: Outcome) {
        let Ok(mut report) = self.inner.try_lock() else {
            self.missed_events.store(true, Ordering::Relaxed);
            return;
        };
        match phase {
            Phase::Frame => {
                if report.frame_write_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.frame_write_count = report.frame_write_count.saturating_add(1);
                if report.frame_write_count > 1 {
                    return;
                }
            }
            Phase::SignedExchange => {
                if report.signed_write_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.signed_write_count = report.signed_write_count.saturating_add(1);
                if report.signed_write_count > 1 {
                    return;
                }
            }
            Phase::SignedRequestRead => {
                if matches!(outcome, Outcome::Observed) {
                    if report.signed_request_frame_count == u16::MAX {
                        report.incomplete = true;
                        return;
                    }
                    report.signed_request_frame_count =
                        report.signed_request_frame_count.saturating_add(1);
                    return;
                }
                outcome = Outcome::Failed;
            }
            Phase::SignedResponseFrame => {
                if report.signed_response_frame_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.signed_response_frame_count =
                    report.signed_response_frame_count.saturating_add(1);
                return;
            }
            Phase::Acknowledgement => {
                if report.acknowledgement_count == u16::MAX {
                    report.incomplete = true;
                    return;
                }
                report.acknowledgement_count = report.acknowledgement_count.saturating_add(1);
                return;
            }
            _ => {}
        }
        if report.events.len() >= MAX_EVENTS || matches!(phase, Phase::Terminal) {
            report.incomplete = true;
            return;
        }
        let sequence = report.events.len() as u8;
        report.events.push(Event {
            sequence,
            elapsed_ms: self.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
            phase,
            outcome,
        });
    }

    pub fn terminal(&self, outcome: Outcome) {
        if let Ok(mut report) = self.inner.lock() {
            if report.events.len() == MAX_EVENTS {
                report.incomplete = true;
                report.events.pop();
                // Retained sequence numbers are re-numbered to remain contiguous.
                for (index, event) in report.events.iter_mut().enumerate() {
                    event.sequence = index as u8;
                }
            }
            let sequence = report.events.len() as u8;
            report.events.push(Event {
                sequence,
                elapsed_ms: self.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                phase: Phase::Terminal,
                outcome,
            });
        }
    }

    #[cfg(test)]
    pub fn snapshot_json(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(&*self.inner.lock().ok()?).ok()
    }

    pub fn set_trial_id_hex(&self, value: &str) -> bool {
        if !is_valid_trial_id(value) {
            return false;
        }
        if let Ok(mut report) = self.inner.try_lock() {
            report.trial_id = Some(value.to_owned());
            true
        } else {
            self.missed_events.store(true, Ordering::Relaxed);
            false
        }
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        let Ok(report) = self.inner.get_mut() else {
            return;
        };
        report.incomplete |= self.missed_events.load(Ordering::Relaxed);
        if !report
            .events
            .last()
            .is_some_and(|event| matches!(event.phase, Phase::Terminal))
        {
            if report.events.len() >= MAX_EVENTS {
                report.incomplete = true;
                report.events.pop();
            }
            let outcome = if report
                .events
                .iter()
                .any(|event| matches!(event.outcome, Outcome::Failed))
            {
                Outcome::Failed
            } else {
                Outcome::Unknown
            };
            report.events.push(Event {
                sequence: report.events.len() as u8,
                elapsed_ms: self.started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                phase: Phase::Terminal,
                outcome,
            });
        }
        if let Ok(mut bytes) = serde_json::to_vec(report) {
            if bytes.len() > MAX_JSON_BYTES {
                report.incomplete = true;
                let terminal = report
                    .events
                    .last()
                    .is_some_and(|event| matches!(event.phase, Phase::Terminal))
                    .then(|| report.events.pop())
                    .flatten();
                report.events.truncate(8);
                for (index, event) in report.events.iter_mut().enumerate() {
                    event.sequence = index as u8;
                }
                if let Some(mut terminal) = terminal {
                    terminal.sequence = report.events.len() as u8;
                    report.events.push(terminal);
                }
                if let Ok(compact) = serde_json::to_vec(report) {
                    bytes = compact;
                }
            }
            if bytes.len() <= MAX_JSON_BYTES {
                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(&bytes);
                let _ = stderr.write_all(b"\n");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_is_allowlisted_bounded_and_terminal_sequence_is_contiguous() {
        let trace = Trace::new();
        for _ in 0..100 {
            trace.record(Phase::Session, Outcome::Observed);
        }
        trace.terminal(Outcome::Confirmed);
        let report = trace.inner.lock().unwrap();
        assert!(report.incomplete);
        assert!(report.events.len() <= MAX_EVENTS);
        assert!(
            report
                .events
                .last()
                .is_some_and(|event| matches!(event.phase, Phase::Terminal))
        );
        for (index, event) in report.events.iter().enumerate() {
            assert_eq!(event.sequence as usize, index);
        }
        assert_eq!(report.authorization, "never_evaluated");
        assert!(serde_json::to_vec(&*report).unwrap().len() <= MAX_JSON_BYTES);
    }

    #[test]
    fn report_contains_no_free_form_fields() {
        let trace = Trace::new();
        trace.record(Phase::Session, Outcome::Confirmed);
        let value = serde_json::to_value(&*trace.inner.lock().unwrap()).unwrap();
        assert!(value.get("adapter").is_none());
        assert!(value.get("device").is_none());
        assert!(value.get("payload").is_none());
    }

    #[test]
    fn trial_label_override_is_lowercase_hex_and_local_metadata_only() {
        let trace = Trace::new();
        assert!(!trace.set_trial_id_hex("0123456789ABCDEF0123456789ABCDEF"));
        assert!(!trace.set_trial_id_hex("too-short"));
        assert!(trace.set_trial_id_hex("0123456789abcdef0123456789abcdef"));
        let value: serde_json::Value =
            serde_json::from_slice(&trace.snapshot_json().unwrap()).unwrap();
        assert_eq!(value["trialId"], "0123456789abcdef0123456789abcdef");
        assert_eq!(value["authorization"], "never_evaluated");
    }

    #[test]
    fn unavailable_trial_label_is_explicit_null_in_incomplete_schema() {
        let trace = Trace::new();
        trace.set_trial_id(&[]);
        trace.record(Phase::Start, Outcome::Attempted);
        let value: serde_json::Value =
            serde_json::from_slice(&trace.snapshot_json().unwrap()).unwrap();
        assert!(value["trialId"].is_null());
    }
}
