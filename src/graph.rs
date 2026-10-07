//! Causal graph representation and multi-hop chain reconstruction.

use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::evidence::{EvidenceBasis, EvidenceClass, Limitation, RuleId};
use crate::futex::{FutexKey, WaitInterval, WaitOutcome};
use crate::scheduler::{EventRef, ThreadId};

pub const MAX_CHAIN_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnattrReason {
    Irq,
    Kthread,
    OutsideTracked,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Node {
    Thread(ThreadId),
    FutexKey(FutexKey),
    CpuRunq(u16),
    Unattributed(UnattrReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Relation {
    Waits,
    WokenBy,
    FutexWake,
    BlockedFor,
    PreemptedBy,
    RunqDelayed,
    PiOwnerAtEntry,
    CorrelatesWith,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: u64,
    pub src: Node,
    pub dst: Node,
    pub relation: Relation,
    pub t_start: u64,
    pub t_end: u64,
    pub class: EvidenceClass,
    pub basis: EvidenceBasis,
    pub rule: RuleId,
    pub provenance: Vec<EventRef>,
    pub limitations: Vec<Limitation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalReason {
    WakerRunning,
    HistoryBeforeTracking,
    Cycle,
    DepthLimit,
    WakerIrq,
    WakerKthread,
    NoWakerObserved,
    Timeout,
    Interrupted,
    OpenAtWindowEnd,
    EndedByExit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainHop {
    pub hop: usize,
    pub waker: ThreadId,
    pub wakee: ThreadId,
    pub futex_key: Option<FutexKey>,
    pub edge_rule: RuleId,
    pub t_wake: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalChain {
    pub hops: Vec<ChainHop>,
    pub terminal_reason: TerminalReason,
    pub depth: usize,
}

#[derive(Debug, Default)]
pub struct CausalGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl CausalGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Reconstructs a multi-hop wakeup chain using bounded backward walk (PRD §12.3).
    pub fn reconstruct_chain(
        &self,
        subject_wait: &WaitInterval,
        all_waits: &[WaitInterval],
        max_depth: usize,
    ) -> CausalChain {
        let mut hops = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(subject_wait.thread.tid);

        let mut cur_wait = subject_wait.clone();
        let mut hop_idx = 0;
        let limit = if max_depth == 0 || max_depth > MAX_CHAIN_DEPTH {
            MAX_CHAIN_DEPTH
        } else {
            max_depth
        };

        let terminal_reason: TerminalReason;

        loop {
            // Find WokenBy edge that woke cur_wait
            let wake_edge = self.edges.iter().find(|e| {
                if e.relation != Relation::WokenBy {
                    return false;
                }
                if let (Node::Thread(_waker), Node::Thread(wakee)) = (&e.src, &e.dst) {
                    if wakee.tid == cur_wait.thread.tid {
                        // Wake occurred during or closing cur_wait
                        return e.t_end >= cur_wait.enter_ts
                            && cur_wait.exit_ts.map_or(true, |ext| e.t_end <= ext + 50_000);
                    }
                }
                false
            });

            let Some(edge) = wake_edge else {
                terminal_reason = match cur_wait.outcome {
                    Some(WaitOutcome::Timeout) => TerminalReason::Timeout,
                    Some(WaitOutcome::Interrupted) => TerminalReason::Interrupted,
                    Some(WaitOutcome::EndedByExit) => TerminalReason::EndedByExit,
                    _ => {
                        if cur_wait.exit_ts.is_none() {
                            TerminalReason::OpenAtWindowEnd
                        } else {
                            TerminalReason::NoWakerObserved
                        }
                    }
                };
                break;
            };

            let waker = match edge.src {
                Node::Thread(t) => t,
                Node::Unattributed(UnattrReason::Irq) => {
                    terminal_reason = TerminalReason::WakerIrq;
                    break;
                }
                Node::Unattributed(UnattrReason::Kthread) => {
                    terminal_reason = TerminalReason::WakerKthread;
                    break;
                }
                _ => {
                    terminal_reason = TerminalReason::HistoryBeforeTracking;
                    break;
                }
            };

            // Detect cycle
            if visited.contains(&waker.tid) {
                hops.push(ChainHop {
                    hop: hop_idx,
                    waker,
                    wakee: cur_wait.thread,
                    futex_key: Some(cur_wait.key),
                    edge_rule: edge.rule,
                    t_wake: edge.t_end,
                });
                terminal_reason = TerminalReason::Cycle;
                break;
            }

            // Record hop
            hops.push(ChainHop {
                hop: hop_idx,
                waker,
                wakee: cur_wait.thread,
                futex_key: Some(cur_wait.key),
                edge_rule: edge.rule,
                t_wake: edge.t_end,
            });

            hop_idx += 1;
            if hop_idx >= limit {
                terminal_reason = TerminalReason::DepthLimit;
                break;
            }

            visited.insert(waker.tid);

            // Find what waker was doing prior to waking: did waker have a prior wait interval?
            let prior_wait = all_waits
                .iter()
                .filter(|w| w.thread.tid == waker.tid && w.exit_ts.map_or(false, |ext| ext <= edge.t_end))
                .max_by_key(|w| w.exit_ts.unwrap_or(0));

            if let Some(prior) = prior_wait {
                cur_wait = prior.clone();
            } else {
                // Waker was running on CPU before this wake
                terminal_reason = TerminalReason::WakerRunning;
                break;
            }
        }

        let depth = hops.len();
        CausalChain {
            hops,
            terminal_reason,
            depth,
        }
    }
}
