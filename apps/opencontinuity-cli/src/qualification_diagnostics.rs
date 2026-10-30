//! Opt-in numeric timing metadata. This trace never grants physical acceptance.
use serde::Serialize;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    ApplicationRegistration,
    AdvertisementRegistration,
    ServerLifetime,
    Cleanup,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    stage: Stage,
    timestamp_unix_ms: u64,
    elapsed_ms: u64,
    outcome: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    schema_version: u8,
    kind: &'static str,
    source: &'static str,
    authorization: &'static str,
    samples: Vec<Sample>,
    #[serde(skip)]
    enabled: bool,
}
impl Trace {
    pub fn new(enabled: bool) -> Self {
        Self {
            schema_version: 1,
            kind: "diagnostic_trace",
            source: "host_preflight",
            authorization: "never_evaluated",
            samples: Vec::new(),
            enabled,
        }
    }
    pub fn record(&mut self, stage: Stage, start: Instant, success: bool) {
        if !self.enabled {
            return;
        }
        self.record_at(
            stage,
            start.elapsed().as_millis() as u64,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            success,
        );
    }
    fn record_at(&mut self, stage: Stage, elapsed_ms: u64, timestamp_unix_ms: u64, success: bool) {
        if !self.enabled {
            return;
        }
        if self.samples.len() == 64 {
            self.samples.remove(0);
        }
        self.samples.push(Sample {
            stage,
            elapsed_ms,
            timestamp_unix_ms,
            outcome: if success { "PASS" } else { "FAIL" },
        });
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        if self.enabled {
            if let Ok(json) = serde_json::to_string(self) {
                eprintln!("{json}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_swift_simulated_timing_vector() {
        let vector: serde_json::Value = serde_json::from_str(include_str!(
            "../../../experiments/watchos/Tests/OpenContinuityWatchTests/Fixtures/p010-diagnostic-vector.json"
        )).unwrap();
        assert_eq!(vector["source"], "simulated");
        let elapsed = vector["finish_ms"].as_u64().unwrap() - vector["start_ms"].as_u64().unwrap();
        let mut trace = Trace::new(true);
        trace.source = "simulated";
        trace.record_at(
            Stage::Cleanup,
            elapsed,
            vector["wall_ms"].as_u64().unwrap(),
            true,
        );
        let json = serde_json::to_value(&trace).unwrap();
        assert_eq!(json["samples"][0]["elapsedMs"], vector["elapsed_ms"]);
        assert_eq!(json["samples"][0]["timestampUnixMs"], vector["wall_ms"]);
        assert_eq!(json["authorization"], vector["authorization"]);
        assert_eq!(json["source"], vector["source"]);
    }
    #[test]
    fn opt_in_bounded_numeric_schema() {
        let mut off = Trace::new(false);
        off.record_at(Stage::Cleanup, 5, 100, true);
        assert!(off.samples.is_empty());
        let mut on = Trace::new(true);
        for i in 0..100 {
            on.record_at(Stage::Cleanup, i, 100, true);
        }
        assert_eq!(on.samples.len(), 64);
        assert_eq!(on.samples[0].elapsed_ms, 36);
        let value = serde_json::to_value(&on).unwrap();
        assert_eq!(value["authorization"], "never_evaluated");
        assert_eq!(value["samples"][0]["timestampUnixMs"], 100);
        assert!(value.get("enabled").is_none());
    }
}
