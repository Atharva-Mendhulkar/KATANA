//! Evidence model, rule catalog, strength, quality, and limitations.

use serde::{Deserialize, Serialize};

use crate::scheduler::EventRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceClass {
    Correlated = 0,
    Observed = 1,
    Causal = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceBasis {
    Direct,
    Derived,
    Statistical,
    Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceStrength {
    Weak = 0,
    Moderate = 1,
    Strong = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceQuality {
    Full,
    Degraded(String),
    UnverifiedKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleId {
    Fw1,
    Fw2,
    Wk1,
    Wk2,
    Fb1,
    Pi1,
    Sw1,
    Sw2,
    Cr1,
    Bio1,
    Bio2,
}

impl RuleId {
    pub const fn max_class(&self) -> EvidenceClass {
        match self {
            RuleId::Fw1 => EvidenceClass::Causal,
            RuleId::Fw2 => EvidenceClass::Causal,
            RuleId::Wk1 => EvidenceClass::Causal,
            RuleId::Wk2 => EvidenceClass::Observed,
            RuleId::Fb1 => EvidenceClass::Observed,
            RuleId::Pi1 => EvidenceClass::Observed,
            RuleId::Sw1 => EvidenceClass::Causal,
            RuleId::Sw2 => EvidenceClass::Observed,
            RuleId::Cr1 => EvidenceClass::Correlated, // CR-1 CAN NEVER PRODUCE CAUSAL
            RuleId::Bio1 => EvidenceClass::Causal,    // Direct sync block I/O link
            RuleId::Bio2 => EvidenceClass::Correlated, // BIO-2 CAN NEVER PRODUCE CAUSAL
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            RuleId::Fw1 => "FW-1",
            RuleId::Fw2 => "FW-2",
            RuleId::Wk1 => "WK-1",
            RuleId::Wk2 => "WK-2",
            RuleId::Fb1 => "FB-1",
            RuleId::Pi1 => "PI-1",
            RuleId::Sw1 => "SW-1",
            RuleId::Sw2 => "SW-2",
            RuleId::Cr1 => "CR-1",
            RuleId::Bio1 => "BIO-1",
            RuleId::Bio2 => "BIO-2",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Limitation {
    AssumeFutexWakeOnly,
    KeyUnverifiable,
    NoOpenFutexWait,
    InconsistentWakeCount,
    LossOverlap,
    HistoryBeforeTracking,
    DepthLimit,
    CycleObserved,
    PiWordUnreadable,
    PidnsMismatch,
    ClockDiscontinuity,
    TargetExited,
    UnrelatedWaker,
    WritebackUnattributed,
    AsyncHandoffUnattributed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub class: EvidenceClass,
    pub basis: EvidenceBasis,
    pub strength: EvidenceStrength,
    pub quality: EvidenceQuality,
    pub rule: RuleId,
    pub description: String,
    pub provenance: Vec<EventRef>,
    pub assumptions: Vec<String>,
    pub limitations: Vec<Limitation>,
}

impl Evidence {
    pub fn new(
        id: String,
        requested_class: EvidenceClass,
        basis: EvidenceBasis,
        strength: EvidenceStrength,
        quality: EvidenceQuality,
        rule: RuleId,
        description: String,
        mut provenance: Vec<EventRef>,
        assumptions: Vec<String>,
        limitations: Vec<Limitation>,
    ) -> Self {
        // Enforce structural no-inflation invariant: evidence class cannot exceed rule's max_class
        let class = if requested_class > rule.max_class() {
            rule.max_class()
        } else {
            requested_class
        };

        // Bounded provenance: cap at 8 provenance refs (PRD §12.5)
        if provenance.len() > 8 {
            provenance.truncate(8);
        }

        Self {
            id,
            class,
            basis,
            strength,
            quality,
            rule,
            description,
            provenance,
            assumptions,
            limitations,
        }
    }
}
