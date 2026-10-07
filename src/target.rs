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
