//! JSON report output model matching schema/report.v1.json.

use serde::{Deserialize, Serialize};

use crate::diagnosis::Diagnosis;
use crate::evidence::Evidence;

pub const SCHEMA_URI: &str = "https://katana.dev/schema/report.v1.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetIdentity {
    pub tgid: u32,
    pub pid: u32,
    pub comm: String,
    pub start_time_ticks: u64,
    pub boot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub duration_ns: u64,
    pub t_start_ns: u64,
    pub t_end_ns: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionStats {
    pub events_received: u64,
    pub events_lost: u64,
    pub reserve_fail_total: u64,
    pub tracked_full: u64,
    pub read_user_fail: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub target: TargetIdentity,
    pub window: WindowInfo,
    pub diagnosis: Diagnosis,
    pub evidence: Vec<Evidence>,
    pub stats: CollectionStats,
}

impl Report {
    pub fn new(
        target: TargetIdentity,
        window: WindowInfo,
        diagnosis: Diagnosis,
        evidence: Vec<Evidence>,
        stats: CollectionStats,
    ) -> Self {
        Self {
            schema: SCHEMA_URI.to_string(),
            target,
            window,
            diagnosis,
            evidence,
            stats,
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
