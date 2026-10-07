//! Event definitions, wire decoding, ordering, and loss ledger.

use serde::{Deserialize, Serialize};

use crate::futex::{FutexEnter, FutexExit};
use crate::scheduler::{EventRef, SchedSwitch, SchedWaking, SchedWakeup, ThreadId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Switch(SchedSwitch),
    Waking(SchedWaking),
    Wakeup(SchedWakeup),
    Fork { child: ThreadId },
    Exit,
    FutexEnter(FutexEnter),
    FutexExit(FutexExit),
    UnsupportedSyscall { nr: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts_ns: u64,
    pub r#ref: EventRef,
    pub thread: ThreadId,
    pub kind: EventKind,
}

impl Event {
    pub fn new(ts_ns: u64, r#ref: EventRef, thread: ThreadId, kind: EventKind) -> Self {
        Self {
            ts_ns,
            r#ref,
            thread,
            kind,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LossInterval {
    pub cpu: u16,
    pub t_lo: u64,
    pub t_hi: u64,
    pub n_lost: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LossLedger {
    pub intervals: Vec<LossInterval>,
    pub reserve_fail_total: u64,
    pub tracked_full: u64,
    pub read_user_fail: u64,
    pub clock_anomalies: u64,
}

impl LossLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_loss(&mut self, cpu: u16, t_lo: u64, t_hi: u64, n_lost: u64) {
        self.intervals.push(LossInterval {
            cpu,
            t_lo,
            t_hi,
            n_lost,
        });
    }

    pub fn overlaps(&self, cpu: u16, t_start: u64, t_end: u64) -> bool {
        self.intervals
            .iter()
            .any(|i| i.cpu == cpu && i.t_lo <= t_end && i.t_hi >= t_start)
    }

    pub fn any_loss_between(&self, t_start: u64, t_end: u64) -> bool {
        self.intervals.iter().any(|i| i.t_lo <= t_end && i.t_hi >= t_start)
            || self.reserve_fail_total > 0
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty() && self.reserve_fail_total == 0
    }
}

/// Normalizes events by sorting into presentation order `(ts_ns, cpu, seq)`
/// and inspecting per-CPU sequence gaps to populate the `LossLedger`.
pub fn normalize_and_detect_loss(events: &mut [Event]) -> LossLedger {
    let mut ledger = LossLedger::new();

    // Sort into presentation and deterministic processing order
    events.sort_by(|a, b| {
        a.ts_ns
            .cmp(&b.ts_ns)
            .then_with(|| a.r#ref.cpu.cmp(&b.r#ref.cpu))
            .then_with(|| a.r#ref.seq.cmp(&b.r#ref.seq))
    });

    // Check per-CPU sequence progression
    use std::collections::HashMap;
    let mut per_cpu_last: HashMap<u16, (u64, u64)> = HashMap::new(); // cpu -> (last_seq, last_ts)

    for ev in events.iter() {
        if let Some((last_seq, last_ts)) = per_cpu_last.get_mut(&ev.r#ref.cpu) {
            if ev.ts_ns < *last_ts {
                ledger.clock_anomalies += 1;
            }
            if ev.r#ref.seq > *last_seq + 1 {
                let n_lost = ev.r#ref.seq - *last_seq - 1;
                ledger.record_loss(ev.r#ref.cpu, *last_ts, ev.ts_ns, n_lost);
            }
            *last_seq = ev.r#ref.seq;
            *last_ts = ev.ts_ns;
        } else {
            per_cpu_last.insert(ev.r#ref.cpu, (ev.r#ref.seq, ev.ts_ns));
        }
    }

    ledger
}
