//! Diagnosis engine, candidate finding definitions, and lexicographic ranking.

use serde::{Deserialize, Serialize};

use crate::evidence::{Evidence, EvidenceStrength, Limitation};
use crate::graph::CausalChain;
use crate::scheduler::ThreadId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingKind {
    FutexWakeChain,
    FutexWaitUnresolved,
    FutexPiOwnerObserved,
    FutexTimeout,
    FutexInterrupted,
    SchedRunqDelay,
    SchedPreempted,
    BlockedUnattributed,
    NotBlocked,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Completeness {
    Invalid = 0,
    Lossy = 1,
    Partial = 2,
    Complete = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagStatus {
    Found,
    Ambiguous,
    NotBlocked,
    Unknown,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub subject: ThreadId,
    pub blocked_duration_ns: u64,
    pub explained_fraction_per_mille: u32,
    pub chain: Option<CausalChain>,
    pub evidence_ids: Vec<String>,
    pub weakest_strength: EvidenceStrength,
    pub has_causal_edge: bool,
    pub is_direct_subject: bool,
    pub hop_count: usize,
    pub details: String,
    pub t_start: u64,
}

impl Finding {
    /// Lexicographic comparison key for ranking candidate findings (PRD §14.3).
    /// Tuple ordering:
    /// (1) subject_relevance (1 vs 0)
    /// (2) has_causal_basis (1 vs 0)
    /// (3) explained_fraction_per_mille
    /// (4) weakest_strength (Strong > Moderate > Weak)
    /// (5) shorter_chain_depth (less is better, inverted for descending cmp)
    /// (6) tie-breaker: lowest t_start, lowest tid
    pub fn rank_key(&self) -> (u8, u8, u32, u8, std::cmp::Reverse<usize>, std::cmp::Reverse<u64>) {
        (
            if self.is_direct_subject { 1 } else { 0 },
            if self.has_causal_edge { 1 } else { 0 },
            self.explained_fraction_per_mille,
            self.weakest_strength as u8,
            std::cmp::Reverse(self.hop_count),
            std::cmp::Reverse(self.t_start),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    pub status: DiagStatus,
    pub primary: Option<Finding>,
    pub alternatives: Vec<Finding>,
    pub context: Vec<Evidence>,
    pub completeness: Completeness,
    pub limitations: Vec<Limitation>,
}

impl Diagnosis {
    pub fn rank_and_select(
        mut candidates: Vec<Finding>,
        context: Vec<Evidence>,
        completeness: Completeness,
        limitations: Vec<Limitation>,
        has_contradiction: bool,
        is_invalid: bool,
    ) -> Self {
        if is_invalid {
            return Self {
                status: DiagStatus::Invalid,
                primary: None,
                alternatives: candidates,
                context,
                completeness: Completeness::Invalid,
                limitations,
            };
        }

        if has_contradiction {
            return Self {
                status: DiagStatus::Ambiguous,
                primary: None,
                alternatives: candidates,
                context,
                completeness: Completeness::Partial,
                limitations,
            };
        }

        if candidates.is_empty() {
            return Self {
                status: DiagStatus::Unknown,
                primary: None,
                alternatives: Vec::new(),
                context,
                completeness,
                limitations,
            };
        }

        // Sort candidates descending by rank_key
        candidates.sort_by(|a, b| b.rank_key().cmp(&a.rank_key()));

        let primary = candidates.remove(0);
        let status = match primary.kind {
            FindingKind::NotBlocked => DiagStatus::NotBlocked,
            FindingKind::Unknown => DiagStatus::Unknown,
            _ => DiagStatus::Found,
        };

        Self {
            status,
            primary: Some(primary),
            alternatives: candidates,
            context,
            completeness,
            limitations,
        }
    }
}
