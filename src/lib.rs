pub mod block_io;
pub mod cli;
pub mod diagnosis;
pub mod events;
pub mod evidence;
pub mod futex;
pub mod graph;
pub mod output;
pub mod renderer;
pub mod scheduler;
pub mod target;

// Retain compatibility module for existing unit tests
pub mod causal_rules;

use std::collections::HashMap;

use block_io::{resolve_dev_name, BlockRqComplete, BlockRqIssue};
use diagnosis::{Completeness, Diagnosis, Finding, FindingKind};
use events::{normalize_and_detect_loss, Event, EventKind, LossLedger};
use evidence::{
    Evidence, EvidenceBasis, EvidenceClass, EvidenceQuality, EvidenceStrength, Limitation, RuleId,
};
use futex::{FutexCmd, FutexKey, FutexScope, WaitInterval, WaitOutcome, WakeInvocation, FUTEX_BITSET_MATCH_ANY};
use graph::{CausalGraph, Edge, Node, Relation};
use output::{CollectionStats, Report, TargetIdentity, WindowInfo};
use scheduler::{EventRef, ThreadId, WakerCtx};

pub const MIN_BLOCK_NS: u64 = 1_000_000; // 1 ms threshold for blocked finding

pub struct Engine {
    pub max_depth: usize,
}

impl Default for Engine {
    fn default() -> Self {
        Self { max_depth: 8 }
    }
}

impl Engine {
    pub fn new(max_depth: usize) -> Self {
        Self {
            max_depth: if max_depth == 0 { 8 } else { max_depth },
        }
    }

    /// Complete deterministic analysis pipeline with explicit TargetIdentity.
    pub fn analyze_with_identity(
        &self,
        target_identity: TargetIdentity,
        subject_tid: Option<u32>,
        mut events: Vec<Event>,
        mut loss_ledger: LossLedger,
    ) -> Report {
        let target_pid = target_identity.pid;
        let initial_event_count = events.len() as u64;

        // 1. Normalization & event loss gap detection
        let detected_loss = normalize_and_detect_loss(&mut events);
        loss_ledger.intervals.extend(detected_loss.intervals);
        loss_ledger.clock_anomalies += detected_loss.clock_anomalies;

        let subject = ThreadId::new(subject_tid.unwrap_or(target_pid), target_pid);

        // 2. Track futex wait intervals and active wake invocations
        let mut open_waits: HashMap<u32, WaitInterval> = HashMap::new();
        let mut all_waits: Vec<WaitInterval> = Vec::new();
        let mut active_wakes: HashMap<u32, WakeInvocation> = HashMap::new();
        let mut in_flight_wakings: HashMap<u32, Vec<(EventRef, u64, ThreadId, WakerCtx)>> = HashMap::new();

        let mut graph = CausalGraph::new();
        let mut evidence_records: Vec<Evidence> = Vec::new();
        let mut evidence_counter = 1usize;
        let mut has_contradiction = false;
        let mut limitations: Vec<Limitation> = Vec::new();

        // Also track runqueue delays: wakee -> wakeup_ts
        let mut pending_runq: HashMap<u32, (u64, u16)> = HashMap::new();
        let mut runq_delays: Vec<(u32, u64, u64, u16)> = Vec::new(); // tid, wake_ts, delay_ns, cpu

        // Track preemptions: (preempted_tid, preemptor_tid, ts)
        let mut preemptions: Vec<(u32, u32, u64)> = Vec::new();

        // Track Block I/O requests and completed sleep intervals (PRD §32.1)
        let mut open_block_requests: HashMap<u64, BlockRqIssue> = HashMap::new();
        let mut completed_block_requests: Vec<(BlockRqIssue, BlockRqComplete)> = Vec::new();
        let mut thread_sleep_intervals: HashMap<u32, (u64, ThreadId, bool)> = HashMap::new();
        let mut direct_block_waits: Vec<(ThreadId, u32, u64, u64, String)> = Vec::new();
        let mut correlated_block_io: Vec<(ThreadId, u32, u64, u64, String)> = Vec::new();
        let mut unattributed_sleeps: Vec<(ThreadId, u64, u64, bool)> = Vec::new();
        let mut unsupported_syscalls: Vec<(ThreadId, u32, u64)> = Vec::new();

        let mut next_edge_id = 1u64;

        for ev in &events {
            match &ev.kind {
                EventKind::FutexEnter(enter) => {
                    if enter.cmd.is_wait_family() {
                        let key = FutexKey::new(enter.private, ev.thread.tgid, enter.uaddr);
                        let bitset = if enter.cmd == FutexCmd::WaitBitset {
                            enter.val3
                        } else {
                            FUTEX_BITSET_MATCH_ANY
                        };
                        let wait = WaitInterval::new(
                            ev.thread,
                            enter.uaddr,
                            key,
                            bitset,
                            ev.ts_ns,
                            ev.r#ref,
                            enter.pi_word,
                        );
                        open_waits.insert(ev.thread.tid, wait);
                    } else if enter.cmd.is_wake_family() {
                        let key = FutexKey::new(enter.private, ev.thread.tgid, enter.uaddr);
                        let bitset = if enter.cmd == FutexCmd::WakeBitset {
                            enter.val3
                        } else {
                            FUTEX_BITSET_MATCH_ANY
                        };
                        let wake = WakeInvocation::new(
                            ev.thread,
                            enter.uaddr,
                            key,
                            enter.cmd,
                            bitset,
                            enter.val,
                            ev.ts_ns,
                            ev.r#ref,
                        );
                        active_wakes.insert(ev.thread.tid, wake);
                        in_flight_wakings.insert(ev.thread.tid, Vec::new());
                    }
                }
                EventKind::Waking(waking) => {
                    pending_runq.insert(waking.wakee.tid, (ev.ts_ns, waking.target_cpu));

                    if let Some(list) = in_flight_wakings.get_mut(&ev.thread.tid) {
                        list.push((ev.r#ref, ev.ts_ns, waking.wakee, waking.waker_ctx));
                    } else {
                        // Plain wake outside futex syscall (WK-1)
                        if waking.waker_ctx == WakerCtx::Task {
                            let edge_id = next_edge_id;
                            next_edge_id += 1;
                            graph.add_edge(Edge {
                                id: edge_id,
                                src: Node::Thread(ev.thread),
                                dst: Node::Thread(waking.wakee),
                                relation: Relation::WokenBy,
                                t_start: ev.ts_ns,
                                t_end: ev.ts_ns,
                                class: EvidenceClass::Causal,
                                basis: EvidenceBasis::Direct,
                                rule: RuleId::Wk1,
                                provenance: vec![ev.r#ref],
                                limitations: Vec::new(),
                            });
                        }
                    }
                }
                EventKind::Wakeup(wakeup) => {
                    pending_runq.insert(wakeup.wakee.tid, (ev.ts_ns, wakeup.target_cpu));
                }
                EventKind::Switch(switch) => {
                    if switch.preempted {
                        preemptions.push((switch.prev.tid, switch.next.tid, ev.ts_ns));
                        let edge_id = next_edge_id;
                        next_edge_id += 1;
                        graph.add_edge(Edge {
                            id: edge_id,
                            src: Node::Thread(switch.next),
                            dst: Node::Thread(switch.prev),
                            relation: Relation::PreemptedBy,
                            t_start: ev.ts_ns,
                            t_end: ev.ts_ns,
                            class: EvidenceClass::Causal,
                            basis: EvidenceBasis::Direct,
                            rule: RuleId::Sw1,
                            provenance: vec![ev.r#ref],
                            limitations: Vec::new(),
                        });
                    } else {
                        thread_sleep_intervals.insert(switch.prev.tid, (ev.ts_ns, switch.prev, switch.in_iowait));
                    }

                    // Check if switch.next had pending runq delay
                    if let Some((wake_ts, cpu)) = pending_runq.remove(&switch.next.tid) {
                        if ev.ts_ns > wake_ts {
                            let delay_ns = ev.ts_ns - wake_ts;
                            runq_delays.push((switch.next.tid, wake_ts, delay_ns, cpu));
                            let edge_id = next_edge_id;
                            next_edge_id += 1;
                            graph.add_edge(Edge {
                                id: edge_id,
                                src: Node::Thread(switch.next),
                                dst: Node::CpuRunq(cpu),
                                relation: Relation::RunqDelayed,
                                t_start: wake_ts,
                                t_end: ev.ts_ns,
                                class: EvidenceClass::Observed,
                                basis: EvidenceBasis::Direct,
                                rule: RuleId::Sw2,
                                provenance: vec![ev.r#ref],
                                limitations: Vec::new(),
                            });
                        }
                    }

                    // Check if switch.next had a completed sleep that matched Block I/O (PRD §32.1)
                    if let Some((sleep_start, sleeping_thread, _in_iowait)) = thread_sleep_intervals.remove(&switch.next.tid) {
                        let sleep_dur = ev.ts_ns.saturating_sub(sleep_start);
                        let matching_comp = completed_block_requests.iter().find(|(_iss, comp)| {
                            comp.complete_ts >= sleep_start && comp.complete_ts <= ev.ts_ns + 50_000
                        });

                        if let Some((iss, _comp)) = matching_comp {
                            let dev_id = iss.dev_id;
                            if iss.submitter.tid == sleeping_thread.tid {
                                // Rule BIO-1: Direct synchronous block I/O attribution
                                let edge_id = next_edge_id;
                                next_edge_id += 1;
                                graph.add_edge(Edge {
                                    id: edge_id,
                                    src: Node::Device(dev_id),
                                    dst: Node::Thread(sleeping_thread),
                                    relation: Relation::BlockedOnDevice,
                                    t_start: sleep_start,
                                    t_end: ev.ts_ns,
                                    class: EvidenceClass::Causal,
                                    basis: EvidenceBasis::Derived,
                                    rule: RuleId::Bio1,
                                    provenance: vec![ev.r#ref],
                                    limitations: Vec::new(),
                                });

                                let ev_id = format!("E{}", evidence_counter);
                                evidence_counter += 1;
                                evidence_records.push(Evidence::new(
                                    ev_id.clone(),
                                    EvidenceClass::Causal,
                                    EvidenceBasis::Derived,
                                    EvidenceStrength::Moderate,
                                    EvidenceQuality::Full,
                                    RuleId::Bio1,
                                    format!(
                                        "TID {} blocked on device 0x{:x} (req 0x{:x}, {} sectors) for {} ms",
                                        sleeping_thread.tid, dev_id, iss.req_id, iss.nr_sector, sleep_dur / 1_000_000
                                    ),
                                    vec![ev.r#ref],
                                    vec!["ASSUME_BLOCK_REQ_LINK".to_string()],
                                    Vec::new(),
                                ));

                                direct_block_waits.push((sleeping_thread, dev_id, sleep_start, sleep_dur, ev_id));
                            } else {
                                // Rule BIO-2: Writeback / kworker or uncorrelated device latency
                                let edge_id = next_edge_id;
                                next_edge_id += 1;
                                graph.add_edge(Edge {
                                    id: edge_id,
                                    src: Node::Device(dev_id),
                                    dst: Node::Thread(sleeping_thread),
                                    relation: Relation::BlockDeviceLatencyCorrelated,
                                    t_start: sleep_start,
                                    t_end: ev.ts_ns,
                                    class: EvidenceClass::Correlated,
                                    basis: EvidenceBasis::Statistical,
                                    rule: RuleId::Bio2,
                                    provenance: vec![ev.r#ref],
                                    limitations: vec![Limitation::WritebackUnattributed],
                                });

                                let ev_id = format!("E{}", evidence_counter);
                                evidence_counter += 1;
                                evidence_records.push(Evidence::new(
                                    ev_id.clone(),
                                    EvidenceClass::Correlated,
                                    EvidenceBasis::Statistical,
                                    EvidenceStrength::Weak,
                                    EvidenceQuality::Full,
                                    RuleId::Bio2,
                                    format!(
                                        "TID {} experienced blocking coinciding with writeback/device 0x{:x} activity. This trace does not establish a causal link.",
                                        sleeping_thread.tid, dev_id
                                    ),
                                    vec![ev.r#ref],
                                    Vec::new(),
                                    vec![Limitation::WritebackUnattributed],
                                ));

                                correlated_block_io.push((sleeping_thread, dev_id, sleep_start, sleep_dur, ev_id));
                            }
                        } else if sleep_dur >= MIN_BLOCK_NS {
                            unattributed_sleeps.push((sleeping_thread, sleep_start, sleep_dur, _in_iowait));
                        }
                    }
                }
                EventKind::FutexExit(exit) => {
                    if let Some(mut wake) = active_wakes.remove(&ev.thread.tid) {
                        wake.close(ev.ts_ns, ev.r#ref, exit.ret);
                        let candidates = in_flight_wakings.remove(&ev.thread.tid).unwrap_or_default();

                        // Evaluate FW-1 / FW-2
                        let mut matched_in_wake = 0i64;
                        for (w_ref, w_ts, wakee, waker_ctx) in &candidates {
                            if *waker_ctx == WakerCtx::Irq || *waker_ctx == WakerCtx::Kthread {
                                continue;
                            }

                            // Timing bounds with EPSILON = 50µs
                            let enter_bound = wake.enter_ts.saturating_sub(50_000);
                            let exit_bound = ev.ts_ns.saturating_add(50_000);
                            if *w_ts < enter_bound || *w_ts > exit_bound {
                                continue;
                            }

                            // Same CPU sequence check
                            if wake.enter_ref.cpu == w_ref.cpu && wake.enter_ref.seq >= w_ref.seq {
                                continue;
                            }
                            if w_ref.cpu == ev.r#ref.cpu && w_ref.seq >= ev.r#ref.seq {
                                continue;
                            }

                            // Match open wait for wakee
                            let wait_found = open_waits
                                .get(&wakee.tid)
                                .filter(|w| w.is_open_at(*w_ts))
                                .cloned()
                                .or_else(|| {
                                    all_waits
                                        .iter()
                                        .rev()
                                        .find(|w| w.thread.tid == wakee.tid && w.is_open_at(*w_ts))
                                        .cloned()
                                });

                            if let Some(wait) = wait_found {
                                if (wait.bitset & wake.bitset) == 0 {
                                    continue;
                                }

                                if wait.key.scope == FutexScope::Private
                                    && wake.key.scope == FutexScope::Private
                                    && wait.key.tgid == wake.key.tgid
                                    && wait.uaddr == wake.uaddr
                                {
                                    matched_in_wake += 1;
                                    let edge_id = next_edge_id;
                                    next_edge_id += 1;

                                    let mut edge_lims = Vec::new();
                                    let mut strength = EvidenceStrength::Moderate;
                                    let mut quality = EvidenceQuality::Full;

                                    if loss_ledger.overlaps(wake.enter_ref.cpu, wake.enter_ts, ev.ts_ns)
                                        || loss_ledger.overlaps(w_ref.cpu, wake.enter_ts, ev.ts_ns)
                                    {
                                        edge_lims.push(Limitation::LossOverlap);
                                        strength = EvidenceStrength::Weak;
                                        quality = EvidenceQuality::Degraded("Loss interval overlap".to_string());
                                    }

                                    graph.add_edge(Edge {
                                        id: edge_id,
                                        src: Node::Thread(wake.thread),
                                        dst: Node::Thread(*wakee),
                                        relation: Relation::WokenBy,
                                        t_start: wake.enter_ts,
                                        t_end: *w_ts,
                                        class: EvidenceClass::Causal,
                                        basis: EvidenceBasis::Derived,
                                        rule: RuleId::Fw1,
                                        provenance: vec![wake.enter_ref, *w_ref, ev.r#ref],
                                        limitations: edge_lims.clone(),
                                    });

                                    let ev_id = format!("E{}", evidence_counter);
                                    evidence_counter += 1;
                                    evidence_records.push(Evidence::new(
                                        ev_id,
                                        EvidenceClass::Causal,
                                        EvidenceBasis::Derived,
                                        strength,
                                        quality,
                                        RuleId::Fw1,
                                        format!(
                                            "TID {} was released by a futex wake on 0x{:x} from TID {}",
                                            wakee.tid, wait.uaddr, wake.thread.tid
                                        ),
                                        vec![wake.enter_ref, *w_ref, ev.r#ref],
                                        vec!["ASSUME_FUTEX_WAKE_ONLY".to_string()],
                                        edge_lims,
                                    ));
                                } else if wait.key.scope == FutexScope::SharedOrUnknown {
                                    matched_in_wake += 1;
                                    let edge_id = next_edge_id;
                                    next_edge_id += 1;

                                    graph.add_edge(Edge {
                                        id: edge_id,
                                        src: Node::Thread(wake.thread),
                                        dst: Node::Thread(*wakee),
                                        relation: Relation::WokenBy,
                                        t_start: wake.enter_ts,
                                        t_end: *w_ts,
                                        class: EvidenceClass::Causal,
                                        basis: EvidenceBasis::Direct,
                                        rule: RuleId::Fw2,
                                        provenance: vec![wake.enter_ref, *w_ref, ev.r#ref],
                                        limitations: vec![Limitation::KeyUnverifiable],
                                    });
                                }
                            }
                        }

                        if exit.ret >= 0 && matched_in_wake > exit.ret {
                            has_contradiction = true;
                            limitations.push(Limitation::InconsistentWakeCount);
                        }
                    } else if let Some(mut wait) = open_waits.remove(&ev.thread.tid) {
                        wait.close(ev.ts_ns, ev.r#ref, exit.ret);
                        all_waits.push(wait);
                    }
                }
                EventKind::BlockRqIssue(issue) => {
                    open_block_requests.insert(issue.req_id, issue.clone());
                }
                EventKind::BlockRqComplete(complete) => {
                    if let Some(issue) = open_block_requests.remove(&complete.req_id) {
                        completed_block_requests.push((issue, complete.clone()));
                    }
                }
                EventKind::Exit => {
                    if let Some(mut wait) = open_waits.remove(&ev.thread.tid) {
                        wait.close_by_exit(ev.ts_ns, ev.r#ref);
                        all_waits.push(wait);
                    }
                }
                EventKind::UnsupportedSyscall { nr } => {
                    unsupported_syscalls.push((ev.thread, *nr, ev.ts_ns));
                }
                _ => {}
            }
        }

        // Close remaining open waits at trace end
        for (_, wait) in open_waits.drain() {
            all_waits.push(wait);
        }

        let t_start_ns = events.first().map_or(0, |e| e.ts_ns);
        let t_end_ns = events.last().map_or(0, |e| e.ts_ns);
        let duration_ns = t_end_ns.saturating_sub(t_start_ns);

        // Find subject's waits
        let subject_waits: Vec<&WaitInterval> = all_waits
            .iter()
            .filter(|w| w.thread.tid == subject.tid)
            .collect();

        // 3. Generate candidate findings
        let mut candidates: Vec<Finding> = Vec::new();

        if let Some(longest_wait) = subject_waits.iter().max_by_key(|w| {
            let dur = w.exit_ts.unwrap_or(t_end_ns).saturating_sub(w.enter_ts);
            dur
        }) {
            let dur_ns = longest_wait.exit_ts.unwrap_or(t_end_ns).saturating_sub(longest_wait.enter_ts);

            if dur_ns >= MIN_BLOCK_NS || longest_wait.pi_owner.is_some() {
                let chain = graph.reconstruct_chain(longest_wait, &all_waits, self.max_depth);

                if !chain.hops.is_empty() {
                    let has_causal = chain.hops.iter().any(|h| h.edge_rule == RuleId::Fw1 || h.edge_rule == RuleId::Fw2 || h.edge_rule == RuleId::Wk1);
                    candidates.push(Finding {
                        kind: FindingKind::FutexWakeChain,
                        subject,
                        blocked_duration_ns: dur_ns,
                        explained_fraction_per_mille: 1000,
                        chain: Some(chain.clone()),
                        evidence_ids: evidence_records.iter().map(|e| e.id.clone()).collect(),
                        weakest_strength: EvidenceStrength::Moderate,
                        has_causal_edge: has_causal,
                        is_direct_subject: true,
                        hop_count: chain.depth,
                        details: format!("Futex wake chain with {} hops", chain.depth),
                        t_start: longest_wait.enter_ts,
                    });
                } else if longest_wait.outcome == Some(WaitOutcome::Timeout) {
                    candidates.push(Finding {
                        kind: FindingKind::FutexTimeout,
                        subject,
                        blocked_duration_ns: dur_ns,
                        explained_fraction_per_mille: 1000,
                        chain: None,
                        evidence_ids: Vec::new(),
                        weakest_strength: EvidenceStrength::Strong,
                        has_causal_edge: false,
                        is_direct_subject: true,
                        hop_count: 0,
                        details: "Ended by ETIMEDOUT".to_string(),
                        t_start: longest_wait.enter_ts,
                    });
                } else if longest_wait.outcome == Some(WaitOutcome::Interrupted) {
                    candidates.push(Finding {
                        kind: FindingKind::FutexInterrupted,
                        subject,
                        blocked_duration_ns: dur_ns,
                        explained_fraction_per_mille: 1000,
                        chain: None,
                        evidence_ids: Vec::new(),
                        weakest_strength: EvidenceStrength::Strong,
                        has_causal_edge: false,
                        is_direct_subject: true,
                        hop_count: 0,
                        details: "Ended by EINTR".to_string(),
                        t_start: longest_wait.enter_ts,
                    });
                } else if longest_wait.exit_ts.is_none() {
                    candidates.push(Finding {
                        kind: FindingKind::FutexWaitUnresolved,
                        subject,
                        blocked_duration_ns: dur_ns,
                        explained_fraction_per_mille: 1000,
                        chain: None,
                        evidence_ids: Vec::new(),
                        weakest_strength: EvidenceStrength::Strong,
                        has_causal_edge: false,
                        is_direct_subject: true,
                        hop_count: 0,
                        details: "Open at window end without waker".to_string(),
                        t_start: longest_wait.enter_ts,
                    });
                } else if let Some(pi_owner) = longest_wait.pi_owner {
                    candidates.push(Finding {
                        kind: FindingKind::FutexPiOwnerObserved,
                        subject,
                        blocked_duration_ns: dur_ns,
                        explained_fraction_per_mille: 1000,
                        chain: None,
                        evidence_ids: Vec::new(),
                        weakest_strength: EvidenceStrength::Strong,
                        has_causal_edge: false,
                        is_direct_subject: true,
                        hop_count: 0,
                        details: format!("PI futex word named TID {} at entry", pi_owner),
                        t_start: longest_wait.enter_ts,
                    });
                }
            }
        }

        // Check scheduler runqueue delay candidate
        if let Some((_, wake_ts, max_delay, cpu)) = runq_delays.iter().filter(|(t, ..)| *t == subject.tid).max_by_key(|(.., d, _)| *d) {
            if *max_delay >= MIN_BLOCK_NS {
                candidates.push(Finding {
                    kind: FindingKind::SchedRunqDelay,
                    subject,
                    blocked_duration_ns: *max_delay,
                    explained_fraction_per_mille: 800,
                    chain: None,
                    evidence_ids: Vec::new(),
                    weakest_strength: EvidenceStrength::Strong,
                    has_causal_edge: false,
                    is_direct_subject: true,
                    hop_count: 0,
                    details: format!("Runqueue delay of {} ms on CPU {}", max_delay / 1_000_000, cpu),
                    t_start: *wake_ts,
                });
            }
        }

        // Check direct block I/O candidate (Rule BIO-1)
        if let Some((_, dev_id, start_ts, dur, ev_id)) = direct_block_waits
            .iter()
            .filter(|(t, ..)| t.tid == subject.tid)
            .max_by_key(|(.., d, _)| *d)
        {
            if *dur >= MIN_BLOCK_NS {
                candidates.push(Finding {
                    kind: FindingKind::BlockIoWait,
                    subject,
                    blocked_duration_ns: *dur,
                    explained_fraction_per_mille: 1000,
                    chain: None,
                    evidence_ids: vec![ev_id.clone()],
                    weakest_strength: EvidenceStrength::Moderate,
                    has_causal_edge: true,
                    is_direct_subject: true,
                    hop_count: 1,
                    details: resolve_dev_name(*dev_id),
                    t_start: *start_ts,
                });
            }
        }

        // Check correlated block I/O candidate (Rule BIO-2)
        if let Some((_, dev_id, start_ts, dur, ev_id)) = correlated_block_io
            .iter()
            .filter(|(t, ..)| t.tid == subject.tid)
            .max_by_key(|(.., d, _)| *d)
        {
            if *dur >= MIN_BLOCK_NS {
                candidates.push(Finding {
                    kind: FindingKind::BlockIoCorrelated,
                    subject,
                    blocked_duration_ns: *dur,
                    explained_fraction_per_mille: 500,
                    chain: None,
                    evidence_ids: vec![ev_id.clone()],
                    weakest_strength: EvidenceStrength::Weak,
                    has_causal_edge: false, // BIO-2 CAN NEVER BE CAUSAL
                    is_direct_subject: true,
                    hop_count: 0,
                    details: resolve_dev_name(*dev_id),
                    t_start: *start_ts,
                });
            }
        }

        // Check unsupported syscalls for subject (e.g. futex2, PRD §10.2, §32.2)
        if let Some((_, nr, ts)) = unsupported_syscalls.iter().find(|(t, ..)| t.tid == subject.tid) {
            limitations.push(Limitation::Futex2NotSupported);
            candidates.push(Finding {
                kind: FindingKind::BlockedUnattributed,
                subject,
                blocked_duration_ns: 0,
                explained_fraction_per_mille: 0,
                chain: None,
                evidence_ids: Vec::new(),
                weakest_strength: EvidenceStrength::Weak,
                has_causal_edge: false,
                is_direct_subject: true,
                hop_count: 0,
                details: format!("futex2 / syscall {} not supported in this version", nr),
                t_start: *ts,
            });
        }

        // Check generic unattributed sleep intervals (e.g. nanosleep or unattributed iowait)
        if candidates.is_empty() {
            if let Some((_, start_ts, dur, in_iowait)) = unattributed_sleeps
                .iter()
                .filter(|(t, ..)| t.tid == subject.tid)
                .max_by_key(|(.., d, _)| *d)
            {
                candidates.push(Finding {
                    kind: FindingKind::BlockedUnattributed,
                    subject,
                    blocked_duration_ns: *dur,
                    explained_fraction_per_mille: 300,
                    chain: None,
                    evidence_ids: Vec::new(),
                    weakest_strength: EvidenceStrength::Weak,
                    has_causal_edge: false,
                    is_direct_subject: true,
                    hop_count: 0,
                    details: if *in_iowait {
                        "unattributed iowait (device unknown)".to_string()
                    } else {
                        "timer sleep / unattributed wait (waker unattributed)".to_string()
                    },
                    t_start: *start_ts,
                });
            }
        }

        // If no blocked interval qualifies, emit NOT_BLOCKED
        if candidates.is_empty() {
            candidates.push(Finding {
                kind: FindingKind::NotBlocked,
                subject,
                blocked_duration_ns: 0,
                explained_fraction_per_mille: 0,
                chain: None,
                evidence_ids: Vec::new(),
                weakest_strength: EvidenceStrength::Strong,
                has_causal_edge: false,
                is_direct_subject: true,
                hop_count: 0,
                details: "Subject thread remained on-CPU / unblocked".to_string(),
                t_start: 0,
            });
        }

        // 4. Completeness determination (PRD §13.4)
        let completeness = if !loss_ledger.is_empty() {
            Completeness::Lossy
        } else if !limitations.is_empty()
            || candidates.iter().any(|c| c.chain.as_ref().map_or(false, |ch| ch.hops.len() >= 8))
        {
            Completeness::Partial
        } else {
            Completeness::Complete
        };

        let diagnosis = Diagnosis::rank_and_select(
            candidates,
            Vec::new(), // Context correlations (CR-1)
            completeness,
            limitations,
            has_contradiction,
            false,
        );


        Report::new(
            target_identity,
            WindowInfo {
                duration_ns,
                t_start_ns,
                t_end_ns,
            },
            diagnosis,
            evidence_records,
            CollectionStats {
                events_received: initial_event_count,
                events_lost: loss_ledger.intervals.iter().map(|i| i.n_lost).sum(),
                reserve_fail_total: loss_ledger.reserve_fail_total,
                tracked_full: loss_ledger.tracked_full,
                read_user_fail: loss_ledger.read_user_fail,
            },
        )
    }

    /// Complete deterministic analysis pipeline (resolving TargetIdentity from target_pid).
    pub fn analyze(
        &self,
        target_pid: u32,
        subject_tid: Option<u32>,
        events: Vec<Event>,
        loss_ledger: LossLedger,
    ) -> Report {
        let identity = crate::target::resolve_target(target_pid).unwrap_or(TargetIdentity {
            tgid: target_pid,
            pid: target_pid,
            comm: "target".to_string(),
            start_time_ticks: 123456,
            boot_id: "00000000-0000-0000-0000-000000000000".to_string(),
        });
        self.analyze_with_identity(identity, subject_tid, events, loss_ledger)
    }
}
