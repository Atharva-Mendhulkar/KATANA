//! Block I/O subsystem types, request tracking, and attribution rules (Phase 2, PRD §32.1).

use serde::{Deserialize, Serialize};
use crate::scheduler::ThreadId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockDevice {
    pub major: u32,
    pub minor: u32,
    pub name: String,
}

impl BlockDevice {
    pub fn new(major: u32, minor: u32, name: impl Into<String>) -> Self {
        Self {
            major,
            minor,
            name: name.into(),
        }
    }

    pub fn dev_id(&self) -> u32 {
        (self.major << 20) | (self.minor & 0xfffff)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRqIssue {
    pub dev_id: u32,
    pub req_id: u64,
    pub sector: u64,
    pub nr_sector: u32,
    pub rwbs: String,
    pub submitter: ThreadId,
    pub issue_ts: u64,
}

impl BlockRqIssue {
    pub fn new(
        dev_id: u32,
        req_id: u64,
        sector: u64,
        nr_sector: u32,
        rwbs: impl Into<String>,
        submitter: ThreadId,
        issue_ts: u64,
    ) -> Self {
        Self {
            dev_id,
            req_id,
            sector,
            nr_sector,
            rwbs: rwbs.into(),
            submitter,
            issue_ts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRqComplete {
    pub dev_id: u32,
    pub req_id: u64,
    pub nr_bytes: u32,
    pub error: i32,
    pub complete_ts: u64,
}

impl BlockRqComplete {
    pub fn new(dev_id: u32, req_id: u64, nr_bytes: u32, error: i32, complete_ts: u64) -> Self {
        Self {
            dev_id,
            req_id,
            nr_bytes,
            error,
            complete_ts,
        }
    }
}

/// Resolves a human-readable block device name from dev_t via /sys/dev/block/<major>:<minor> (PRD §32.1).
pub fn resolve_dev_name(dev_id: u32) -> String {
    let major = dev_id >> 20;
    let minor = dev_id & 0xfffff;
    let sysfs_path = format!("/sys/dev/block/{}:{}", major, minor);

    if let Ok(link_dest) = std::fs::read_link(&sysfs_path) {
        if let Some(file_name) = link_dest.file_name().and_then(|f| f.to_str()) {
            return file_name.to_string();
        }
    }
    format!("dev_t 0x{:x}", dev_id)
}
