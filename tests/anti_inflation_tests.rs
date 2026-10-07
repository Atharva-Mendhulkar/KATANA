use katana::diagnosis::{Completeness, DiagStatus, Diagnosis, Finding, FindingKind};
use katana::evidence::{
    Evidence, EvidenceBasis, EvidenceClass, EvidenceQuality, EvidenceStrength, RuleId,
};
use katana::renderer::{render_diagnosis, MANDATORY_CORRELATION_SUFFIX, NON_PI_OWNER_UNKNOWN};
use katana::scheduler::ThreadId;

#[test]
fn test_rule_max_class_invariants() {
    // CR-1 can NEVER exceed Correlated
    assert_eq!(RuleId::Cr1.max_class(), EvidenceClass::Correlated);
    // FB-1, PI-1, SW-2, WK-2 can NEVER exceed Observed
    assert_eq!(RuleId::Fb1.max_class(), EvidenceClass::Observed);
    assert_eq!(RuleId::Pi1.max_class(), EvidenceClass::Observed);
    assert_eq!(RuleId::Sw2.max_class(), EvidenceClass::Observed);
    assert_eq!(RuleId::Wk2.max_class(), EvidenceClass::Observed);

    // Attempting to construct CR-1 evidence with Causal class must be clamped
    let ev = Evidence::new(
        "E1".to_string(),
        EvidenceClass::Causal, // Attempt illegal upgrade
        EvidenceBasis::Statistical,
        EvidenceStrength::Weak,
        EvidenceQuality::Full,
        RuleId::Cr1,
        "CPU saturation".to_string(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        ev.class,
        EvidenceClass::Correlated,
        "Evidence class for CR-1 must be clamped to Correlated"
    );
}

#[test]
fn test_renderer_anti_inflation_lint() {
    let subject = ThreadId::new(101, 100);
    let finding = Finding {
        kind: FindingKind::FutexWakeChain,
        subject,
        blocked_duration_ns: 50_000_000,
        explained_fraction_per_mille: 1000,
        chain: None,
        evidence_ids: vec!["E1".to_string()],
        weakest_strength: EvidenceStrength::Moderate,
        has_causal_edge: true,
        is_direct_subject: true,
        hop_count: 1,
        details: "Futex wake chain".to_string(),
        t_start: 1_000_000_000,
    };

    let context_ev = Evidence::new(
        "E2".to_string(),
        EvidenceClass::Correlated,
        EvidenceBasis::Statistical,
        EvidenceStrength::Weak,
        EvidenceQuality::Full,
        RuleId::Cr1,
        "CPU pressure (avg10=25%) coincided with this interval".to_string(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let diag = Diagnosis {
        status: DiagStatus::Found,
        primary: Some(finding),
        alternatives: Vec::new(),
        context: vec![context_ev],
        completeness: Completeness::Complete,
        limitations: Vec::new(),
    };

    let rendered = render_diagnosis(&diag, false);

    // Invariant 1: Mandatory correlation suffix present
    assert!(
        rendered.contains(MANDATORY_CORRELATION_SUFFIX),
        "Rendered correlation MUST contain mandatory disclaimer"
    );

    // Invariant 2: Non-PI futex owner unknown disclosure
    assert!(
        rendered.contains(NON_PI_OWNER_UNKNOWN),
        "Non-PI futex must state owner is unknown"
    );

    // Invariant 3: Forbidden words check
    let lower = rendered.to_lowercase();
    assert!(!lower.contains("holds the mutex"));
    assert!(!lower.contains("holds the lock"));
    assert!(!lower.contains("because of"));
    assert!(!lower.contains("due to"));
}
