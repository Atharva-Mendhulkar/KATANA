//! Implementation of Rule FW-1 (Futex Wake Causal Attribution) and evidence model.

use std::collections::HashMap;

use crate::futex::{
    FutexCmd, FutexEnter, FutexExit, FutexKey, FutexScope, WaitInterval, WakeInvocation,
    FUTEX_BITSET_MATCH_ANY,
};
use crate::scheduler::{EventRef, SchedSwitch, SchedWaking, ThreadId, WakerCtx};

/// Cross-CPU timestamp tolerance bound (PRD §9.2, §10.4): ε = 50 µs = 50,000 ns.
pub const EPSILON_NS: u64 = 50_000;

#[derive(Debug, Clone)]
pub enum EventKind {
    Switch(SchedSwitch),
    Waking(SchedWaking),
    FutexEnter(FutexEnter),
    FutexExit(FutexExit),
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceClass {
    Causal,
    Observed,
    Correlated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceBasis {
    Direct,
    Derived,
    Statistical,
    Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceStrength {
    Strong,
    Moderate,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceQuality {
    Full,
    Degraded(String),
    UnverifiedKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Limitation {
    AssumeFutexWakeOnly,
    KeyUnverifiable,
    NoOpenFutexWait,
    InconsistentWakeCount,
    LossOverlap,
    UnrelatedWaker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleId {
    Fw1,
    Fw2,
    Wk1,
}

#[derive(Debug, Clone)]
pub struct CausalWakeEdge {
    pub rule: RuleId,
    pub class: EvidenceClass,
    pub basis: EvidenceBasis,
    pub strength: EvidenceStrength,
    pub quality: EvidenceQuality,
    pub waker: ThreadId,
    pub wakee: ThreadId,
    pub futex_key: Option<FutexKey>,
    pub t_wake: u64,
    pub provenance: Vec<EventRef>,
    pub assumptions: Vec<String>,
    pub limitations: Vec<Limitation>,
}

#[derive(Debug, Clone, Default)]
pub struct LossInterval {
    pub cpu: u16,
    pub t_lo: u64,
    pub t_hi: u64,
    pub n_lost: u64,
}

#[derive(Debug, Clone, Default)]
pub struct LossLedger {
    pub intervals: Vec<LossInterval>,
    pub reserve_fail_total: u64,
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
}

#[derive(Debug, Clone)]
struct InFlightWaking {
    event_ref: EventRef,
    ts_ns: u64,
    wakee: ThreadId,
    waker_ctx: WakerCtx,
}

/// FW-1 Evaluation Engine.
#[derive(Debug, Default)]
pub struct CausalEngine {
    open_waits: HashMap<u32, WaitInterval>,
    closed_waits: Vec<WaitInterval>,
    active_wakes: HashMap<u32, WakeInvocation>,
    in_flight_wakings: HashMap<u32, Vec<InFlightWaking>>,
    pub edges: Vec<CausalWakeEdge>,
    pub untracked_wakes: u64,
}

impl CausalEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a trace of events in chronological/presentation order.
    pub fn process_trace(&mut self, events: &[Event], loss_ledger: &LossLedger) {
        for event in events {
            self.process_event(event, loss_ledger);
        }
    }

    pub fn process_event(&mut self, event: &Event, loss_ledger: &LossLedger) {
        match &event.kind {
            EventKind::FutexEnter(enter) => {
                if enter.cmd.is_wait_family() {
                    let key = FutexKey::new(enter.private, event.thread.tgid, enter.uaddr);
                    let bitset = if enter.cmd == FutexCmd::WaitBitset {
                        enter.val3
                    } else {
                        FUTEX_BITSET_MATCH_ANY
                    };
                    let wait = WaitInterval::new(
                        event.thread,
                        enter.uaddr,
                        key,
                        bitset,
                        event.ts_ns,
                        event.r#ref,
                        enter.pi_word,
                    );
                    self.open_waits.insert(event.thread.tid, wait);
                } else if enter.cmd.is_wake_family() {
                    let key = FutexKey::new(enter.private, event.thread.tgid, enter.uaddr);
                    let bitset = if enter.cmd == FutexCmd::WakeBitset {
                        enter.val3
                    } else {
                        FUTEX_BITSET_MATCH_ANY
                    };
                    let wake = WakeInvocation::new(
                        event.thread,
                        enter.uaddr,
                        key,
                        enter.cmd,
                        bitset,
                        enter.val,
                        event.ts_ns,
                        event.r#ref,
                    );
                    self.active_wakes.insert(event.thread.tid, wake);
                    self.in_flight_wakings.insert(event.thread.tid, Vec::new());
                }
            }
            EventKind::Waking(waking) => {
                if let Some(candidates) = self.in_flight_wakings.get_mut(&event.thread.tid) {
                    candidates.push(InFlightWaking {
                        event_ref: event.r#ref,
                        ts_ns: event.ts_ns,
                        wakee: waking.wakee,
                        waker_ctx: waking.waker_ctx,
                    });
                }
            }
            EventKind::FutexExit(exit) => {
                if let Some(mut wait) = self.open_waits.remove(&event.thread.tid) {
                    wait.close(event.ts_ns, event.r#ref, exit.ret);
                    self.closed_waits.push(wait);
                } else if let Some(mut wake) = self.active_wakes.remove(&event.thread.tid) {
                    wake.close(event.ts_ns, event.r#ref, exit.ret);
                    let candidates = self
                        .in_flight_wakings
                        .remove(&event.thread.tid)
                        .unwrap_or_default();
                    self.evaluate_wake_invocation(&wake, &candidates, event.r#ref, exit.ret, loss_ledger);
                }
            }
            EventKind::Switch(_) => {
                // Sched switch is recorded for timeline intervals
            }
        }
    }

    fn evaluate_wake_invocation(
        &mut self,
        wake: &WakeInvocation,
        candidates: &[InFlightWaking],
        exit_ref: EventRef,
        ret: i64,
        loss_ledger: &LossLedger,
    ) {
        let mut matched_edges = Vec::new();

        for candidate in candidates {
            // Check waker context: IRQ and Kthread wakeups cannot be attributed as FW-1 task wakes
            if candidate.waker_ctx == WakerCtx::Irq || candidate.waker_ctx == WakerCtx::Kthread {
                continue;
            }

            // Condition 2: Timing containment within ε tolerance
            let enter_ts_bound = wake.enter_ts.saturating_sub(EPSILON_NS);
            let exit_ts_bound = wake.exit_ts.unwrap_or(wake.enter_ts).saturating_add(EPSILON_NS);
            let timing_ok = candidate.ts_ns >= enter_ts_bound && candidate.ts_ns <= exit_ts_bound;
            if !timing_ok {
                continue;
            }

            // Condition 2: Same-CPU sequence order checks
            // E3 enter, E4 waking, E5 exit
            let seq_enter_ok = if wake.enter_ref.cpu == candidate.event_ref.cpu {
                wake.enter_ref.seq < candidate.event_ref.seq
            } else {
                true // cross-CPU migration: seq counters are on distinct CPUs
            };

            let seq_exit_ok = if candidate.event_ref.cpu == exit_ref.cpu {
                candidate.event_ref.seq < exit_ref.seq
            } else {
                true // cross-CPU migration
            };

            let seq_enter_exit_ok = if wake.enter_ref.cpu == exit_ref.cpu {
                wake.enter_ref.seq < exit_ref.seq
            } else {
                true
            };

            if !seq_enter_ok || !seq_exit_ok || !seq_enter_exit_ok {
                continue;
            }

            // Condition 3: Find open wait interval for wakee at candidate.ts_ns
            let wait_interval = self
                .open_waits
                .get(&candidate.wakee.tid)
                .filter(|w| w.is_open_at(candidate.ts_ns))
                .or_else(|| {
                    self.closed_waits
                        .iter()
                        .rev()
                        .find(|w| w.thread.tid == candidate.wakee.tid && w.is_open_at(candidate.ts_ns))
                });

            let Some(wait) = wait_interval else {
                // No open futex wait: plain wake only (LIM_NO_OPEN_FUTEX_WAIT)
                continue;
            };

            // Condition 4: Key and bitset verification
            let bitset_match = (wait.bitset & wake.bitset) != 0;
            if !bitset_match {
                // Disjoint bitsets: kernel will not wake this waiter
                continue;
            }

            match (wait.key.scope, wake.key.scope) {
                (FutexScope::Private, FutexScope::Private) => {
                    if wait.key.tgid == wake.key.tgid && wait.uaddr == wake.uaddr {
                        // All 4 conditions satisfied: Rule FW-1
                        matched_edges.push((
                            RuleId::Fw1,
                            EvidenceClass::Causal,
                            EvidenceBasis::Derived,
                            EvidenceStrength::Moderate,
                            EvidenceQuality::Full,
                            candidate.wakee,
                            Some(wake.key),
                            candidate.ts_ns,
                            vec![wake.enter_ref, candidate.event_ref, exit_ref],
                            vec!["ASSUME_FUTEX_WAKE_ONLY".to_string()],
                            Vec::new(),
                        ));
                    }
                    // If address or tgid did not match, condition 4 fails (wrong key)
                }
                _ => {
                    // Shared futex or cross-process: address cannot be verified across address spaces (FW-2)
                    matched_edges.push((
                        RuleId::Fw2,
                        EvidenceClass::Causal,
                        EvidenceBasis::Direct,
                        EvidenceStrength::Moderate,
                        EvidenceQuality::UnverifiedKey,
                        candidate.wakee,
                        None,
                        candidate.ts_ns,
                        vec![wake.enter_ref, candidate.event_ref, exit_ref],
                        vec![],
                        vec![Limitation::KeyUnverifiable],
                    ));
                }
            }
        }

        let count_seen = matched_edges.len() as i64;

        // Consistency check (§10.4): ret vs count_seen
        let inconsistent_count = count_seen > ret && ret >= 0;
        if ret > count_seen && ret >= 0 {
            self.untracked_wakes += (ret - count_seen) as u64;
        }

        // Loss check: check if loss ledger recorded gaps overlapping this wake window
        let t_lo = wake.enter_ts;
        let t_hi = wake.exit_ts.unwrap_or(t_lo);
        let has_loss_overlap = loss_ledger.overlaps(wake.enter_ref.cpu, t_lo, t_hi)
            || loss_ledger.overlaps(exit_ref.cpu, t_lo, t_hi)
            || candidates.iter().any(|c| loss_ledger.overlaps(c.event_ref.cpu, t_lo, t_hi));

        for (
            rule,
            class,
            basis,
            mut strength,
            mut quality,
            wakee,
            futex_key,
            t_wake,
            provenance,
            assumptions,
            mut limitations,
        ) in matched_edges
        {
            if inconsistent_count {
                // Contradiction: more wakees observed than ret reported by kernel
                strength = EvidenceStrength::Weak;
                limitations.push(Limitation::InconsistentWakeCount);
            }

            if has_loss_overlap {
                quality = EvidenceQuality::Degraded("Loss interval overlaps wake window".to_string());
                limitations.push(Limitation::LossOverlap);
                strength = EvidenceStrength::Weak;
            }

            self.edges.push(CausalWakeEdge {
                rule,
                class,
                basis,
                strength,
                quality,
                waker: wake.thread,
                wakee,
                futex_key,
                t_wake,
                provenance,
                assumptions,
                limitations,
            });
        }
    }
}
