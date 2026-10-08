//! Event definitions, wire decoding, ordering, and loss ledger.

use serde::{Deserialize, Serialize};

use crate::block_io::{BlockRqComplete, BlockRqIssue};
use crate::futex::{FutexEnter, FutexExit};
use crate::scheduler::{EventRef, SchedSwitch, SchedWaking, SchedWakeup, TaskState, ThreadId, WakerCtx};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Switch(SchedSwitch),
    Waking(SchedWaking),
    Wakeup(SchedWakeup),
    Fork { child: ThreadId },
    Exit,
    FutexEnter(FutexEnter),
    FutexExit(FutexExit),
    BlockRqIssue(BlockRqIssue),
    BlockRqComplete(BlockRqComplete),
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

    /// Decodes a raw binary event frame emitted by in-kernel eBPF ringbuffer (PRD §8, §9).
    pub fn decode_raw(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < 28 {
            return Err("Buffer too short for kt_hdr");
        }
        let ts_ns = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let seq = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let tid = u32::from_le_bytes(buf[16..20].try_into().unwrap());
        let tgid = u32::from_le_bytes(buf[20..24].try_into().unwrap());
        let cpu = u16::from_le_bytes(buf[24..26].try_into().unwrap());
        let ev_type = buf[26];
        let _flags = buf[27];

        let r#ref = EventRef::new(cpu, seq);
        let thread = ThreadId::new(tid, tgid);
        let payload = &buf[28..];

        let kind = match ev_type {
            0 => {
                // KT_TYPE_SWITCH
                if payload.len() < 24 {
                    return Err("Buffer too short for kt_switch");
                }
                let prev_tid = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                let next_tid = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                let prev_state_raw = u32::from_le_bytes(payload[8..12].try_into().unwrap());
                let sflags = u16::from_le_bytes(payload[12..14].try_into().unwrap());
                let next_tgid = u32::from_le_bytes(payload[16..20].try_into().unwrap());
                let prev_tgid = u32::from_le_bytes(payload[20..24].try_into().unwrap());

                let prev_state = match prev_state_raw {
                    0 => TaskState::Running,
                    2 => TaskState::IoWait,
                    _ => TaskState::Sleeping,
                };
                let preempted = (sflags & 1) != 0;
                let in_iowait = (sflags & 2) != 0 || prev_state == TaskState::IoWait;

                EventKind::Switch(SchedSwitch {
                    prev: ThreadId::new(prev_tid, prev_tgid),
                    next: ThreadId::new(next_tid, next_tgid),
                    prev_state,
                    preempted,
                    in_iowait,
                })
            }
            1 => {
                // KT_TYPE_WAKING
                if payload.len() < 16 {
                    return Err("Buffer too short for kt_wake");
                }
                let wakee_tid = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                let wakee_tgid = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                let target_cpu = u16::from_le_bytes(payload[8..10].try_into().unwrap());
                let sflags = u16::from_le_bytes(payload[10..12].try_into().unwrap());

                let waker_ctx = if (sflags & 1) != 0 {
                    WakerCtx::Irq
                } else if (sflags & 2) != 0 {
                    WakerCtx::Kthread
                } else {
                    WakerCtx::Task
                };

                EventKind::Waking(SchedWaking {
                    wakee: ThreadId::new(wakee_tid, wakee_tgid),
                    target_cpu,
                    waker_ctx,
                })
            }
            2 => {
                // KT_TYPE_WAKEUP
                if payload.len() < 16 {
                    return Err("Buffer too short for kt_wake");
                }
                let wakee_tid = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                let wakee_tgid = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                let target_cpu = u16::from_le_bytes(payload[8..10].try_into().unwrap());
                EventKind::Wakeup(SchedWakeup {
                    wakee: ThreadId::new(wakee_tid, wakee_tgid),
                    target_cpu,
                })
            }
            5 => EventKind::Exit,
            7 => {
                // KT_TYPE_FUTEX_ENTER
                if payload.len() < 32 {
                    return Err("Buffer too short for kt_futex_enter");
                }
                let uaddr = u64::from_le_bytes(payload[0..8].try_into().unwrap());
                let uaddr2 = u64::from_le_bytes(payload[8..16].try_into().unwrap());
                let op = u32::from_le_bytes(payload[16..20].try_into().unwrap());
                let val = u32::from_le_bytes(payload[20..24].try_into().unwrap());
                let val3 = u32::from_le_bytes(payload[24..28].try_into().unwrap());
                let pi_word = u32::from_le_bytes(payload[28..32].try_into().unwrap());
                let pi_word_valid = if payload.len() > 32 { payload[32] != 0 } else { false };
                let has_timeout = if payload.len() > 33 { payload[33] != 0 } else { false };

                let cmd = match op & 0x7f {
                    0 => crate::futex::FutexCmd::Wait,
                    1 => crate::futex::FutexCmd::Wake,
                    3 => crate::futex::FutexCmd::Requeue,
                    5 => crate::futex::FutexCmd::WakeOp,
                    6 => crate::futex::FutexCmd::LockPi,
                    7 => crate::futex::FutexCmd::UnlockPi,
                    9 => crate::futex::FutexCmd::WaitBitset,
                    10 => crate::futex::FutexCmd::WakeBitset,
                    _ => crate::futex::FutexCmd::Unknown(op),
                };
                let private = (op & 128) != 0;

                EventKind::FutexEnter(FutexEnter {
                    uaddr,
                    uaddr2,
                    cmd,
                    private,
                    val,
                    val3,
                    has_timeout,
                    pi_word: if pi_word_valid { Some(pi_word) } else { None },
                })
            }
            8 => {
                // KT_TYPE_FUTEX_EXIT
                if payload.len() < 8 {
                    return Err("Buffer too short for kt_futex_exit");
                }
                let ret = i64::from_le_bytes(payload[0..8].try_into().unwrap());
                EventKind::FutexExit(FutexExit { ret })
            }
            9 => {
                // KT_TYPE_BLOCK_ISSUE
                if payload.len() < 40 {
                    return Err("Buffer too short for kt_block_issue");
                }
                let dev_id = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                let nr_sector = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                let req_id = u64::from_le_bytes(payload[8..16].try_into().unwrap());
                let sector = u64::from_le_bytes(payload[16..24].try_into().unwrap());
                let submitter_tid = u32::from_le_bytes(payload[24..28].try_into().unwrap());
                let submitter_tgid = u32::from_le_bytes(payload[28..32].try_into().unwrap());
                let rwbs_bytes = &payload[32..40];
                let rwbs_end = rwbs_bytes.iter().position(|&b| b == 0).unwrap_or(8);
                let rwbs = String::from_utf8_lossy(&rwbs_bytes[..rwbs_end]).to_string();

                EventKind::BlockRqIssue(BlockRqIssue::new(
                    dev_id,
                    req_id,
                    sector,
                    nr_sector,
                    &rwbs,
                    ThreadId::new(submitter_tid, submitter_tgid),
                    ts_ns,
                ))
            }
            10 => {
                // KT_TYPE_BLOCK_COMPLETE
                if payload.len() < 20 {
                    return Err("Buffer too short for kt_block_complete");
                }
                let dev_id = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                let nr_bytes = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                let req_id = u64::from_le_bytes(payload[8..16].try_into().unwrap());
                let error = i32::from_le_bytes(payload[16..20].try_into().unwrap());

                EventKind::BlockRqComplete(BlockRqComplete::new(
                    dev_id,
                    req_id,
                    nr_bytes,
                    error,
                    ts_ns,
                ))
            }
            11 => {
                // KT_TYPE_UNSUPPORTED_SYSCALL
                let nr = if payload.len() >= 4 {
                    u32::from_le_bytes(payload[0..4].try_into().unwrap())
                } else {
                    0
                };
                EventKind::UnsupportedSyscall { nr }
            }
            _ => return Err("Unknown kt_type event"),
        };

        Ok(Event::new(ts_ns, r#ref, thread, kind))
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
