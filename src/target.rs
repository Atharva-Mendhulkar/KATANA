//! Target process resolver and identity tracker per PRD §5.1, §7, §31.

use std::fs;
use crate::output::TargetIdentity;

/// Resolves target identity from /proc/<pid>/stat and /proc/sys/kernel/random/boot_id.
pub fn resolve_target(pid: u32) -> Result<TargetIdentity, String> {
    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000000".to_string());

    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = fs::read_to_string(&stat_path)
        .map_err(|e| format!("Cannot read {}: {}", stat_path, e))?;

    // Parse comm between '(' and ')'
    let open_paren = stat_content.find('(').ok_or("Malformed /proc/<pid>/stat")?;
    let close_paren = stat_content.rfind(')').ok_or("Malformed /proc/<pid>/stat")?;
    let comm = stat_content[open_paren + 1..close_paren].to_string();

    let rest = &stat_content[close_paren + 1..].trim();
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // In /proc/<pid>/stat: field 22 (1-based) is starttime, which is index 19 after (comm) and state (field 3)
    let start_time_ticks: u64 = fields.get(19)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    Ok(TargetIdentity {
        tgid: pid,
        pid,
        comm,
        start_time_ticks,
        boot_id,
    })
}

/// Point-in-time snapshot of an individual thread's state (PRD §5.1, §7.1, ADR-012).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSnapshot {
    pub tid: u32,
    pub state: char,
    pub wchan: Option<String>,
    pub syscall: Option<i64>,
}

/// Point-in-time snapshot of target process threads from /proc (PRD §5.1, §7.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TargetSnapshot {
    pub pid: u32,
    pub timestamp_ns: u64,
    pub threads: Vec<ThreadSnapshot>,
}

/// Takes a /proc state snapshot of target process threads.
pub fn take_snapshot(pid: u32) -> Result<TargetSnapshot, String> {
    let task_dir = format!("/proc/{}/task", pid);
    let entries = fs::read_dir(&task_dir)
        .map_err(|e| format!("Cannot read {}: {}", task_dir, e))?;

    let mut threads = Vec::new();
    for entry in entries.flatten() {
        if let Ok(tid_str) = entry.file_name().into_string() {
            if let Ok(tid) = tid_str.parse::<u32>() {
                let stat_path = format!("/proc/{}/task/{}/stat", pid, tid);
                let state = fs::read_to_string(&stat_path)
                    .ok()
                    .and_then(|content| {
                        let close_paren = content.rfind(')')?;
                        content[close_paren + 1..].split_whitespace().next()?.chars().next()
                    })
                    .unwrap_or('?');

                let wchan = fs::read_to_string(format!("/proc/{}/task/{}/wchan", pid, tid))
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "0");

                let syscall = fs::read_to_string(format!("/proc/{}/task/{}/syscall", pid, tid))
                    .ok()
                    .and_then(|s| s.split_whitespace().next().and_then(|n| n.parse::<i64>().ok()));

                threads.push(ThreadSnapshot {
                    tid,
                    state,
                    wchan,
                    syscall,
                });
            }
        }
    }

    Ok(TargetSnapshot {
        pid,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
        threads,
    })
}

/// Verifies that target identity has not changed across the observation window (PRD §5.1).
pub fn verify_identity(initial: &TargetIdentity, current: &TargetIdentity) -> bool {
    initial.tgid == current.tgid
        && initial.start_time_ticks == current.start_time_ticks
        && initial.boot_id == current.boot_id
}

/// Checks if current process has root/privileged effective UID (PRD §16.2 exit 10).
pub fn has_root_privileges() -> bool {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("Uid:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(euid_str) = parts.get(2) {
                    return *euid_str == "0";
                }
            }
        }
    }
    false
}
