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

#[test]
fn test_verbose_evidence_provenance_mapping() {
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
    let ev1 = Evidence::new(
        "E1".to_string(),
        EvidenceClass::Causal,
        EvidenceBasis::Derived,
        EvidenceStrength::Moderate,
        EvidenceQuality::Full,
        RuleId::Fw1,
        "TID 101 released by futex wake".to_string(),
        vec![katana::scheduler::EventRef::new(1, 42)],
        vec!["ASSUME_FUTEX_WAKE_ONLY".to_string()],
        Vec::new(),
    );
    let report = katana::output::Report::new(
        katana::output::TargetIdentity {
            tgid: 100,
            pid: 100,
            comm: "target".to_string(),
            start_time_ticks: 1234,
            boot_id: "0000".to_string(),
        },
        katana::output::WindowInfo {
            duration_ns: 100_000_000,
            t_start_ns: 1_000_000_000,
            t_end_ns: 1_100_000_000,
        },
        Diagnosis {
            status: DiagStatus::Found,
            primary: Some(finding),
            alternatives: Vec::new(),
            context: Vec::new(),
            completeness: Completeness::Complete,
            limitations: Vec::new(),
        },
        vec![ev1],
        katana::output::CollectionStats::default(),
    );

    let verbose_out = katana::renderer::render_report(&report, true);
    assert!(verbose_out.contains("Primary Evidence IDs: E1"));
    assert!(verbose_out.contains("Evidence Provenance Records:"));
    assert!(verbose_out.contains("[E1]"));
    assert!(verbose_out.contains("cpu:1 seq:42"));
}

#[test]
fn test_safety_and_forbidden_helper_audit() {
    // PRD §21.2: Enforce zero target modification and normative helper allow-list
    let bpf_source = std::path::Path::new("bpf/katana.bpf.c");
    if bpf_source.exists() {
        let content = std::fs::read_to_string(bpf_source).expect("bpf/katana.bpf.c must be readable");

        // Forbidden BPF helpers (PRD §21.2)
        let forbidden_bpf = [
            "bpf_override_return",
            "bpf_send_signal",
            "bpf_send_signal_thread",
            "bpf_probe_write_user",
            "bpf_set_retval",
        ];
        for f in &forbidden_bpf {
            assert!(
                !content.contains(f),
                "CRITICAL SAFETY VIOLATION: bpf/katana.bpf.c contains forbidden helper '{}'",
                f
            );
        }

        // Normative helper allow-list (PRD §8.2, §21.2)
        let allowed_helpers = [
            "bpf_map_lookup_elem",
            "bpf_map_update_elem",
            "bpf_map_delete_elem",
            "bpf_ktime_get_ns",
            "bpf_get_smp_processor_id",
            "bpf_get_current_pid_tgid",
            "bpf_probe_read_user",
            "bpf_ringbuf_reserve",
            "bpf_ringbuf_submit",
            "bpf_ringbuf_discard",
            "bpf_get_current_task_btf",
        ];

        // Find all bpf_* function pointer declarations or calls in katana.bpf.c
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(pos) = trimmed.find("bpf_") {
                let rest = &trimmed[pos..];
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if ident.starts_with("bpf_") && !ident.starts_with("bpf_map_type") {
                    assert!(
                        allowed_helpers.contains(&ident.as_str()),
                        "UNAPPROVED BPF HELPER: '{}' is not in normative allow-list",
                        ident
                    );
                }
            }
        }
    }

    // Check all Rust sources under src/
    let mut files_to_check = Vec::new();
    fn collect_rs_files(dir: &std::path::Path, acc: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_rs_files(&path, acc);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    acc.push(path);
                }
            }
        }
    }
    collect_rs_files(std::path::Path::new("src"), &mut files_to_check);

    let forbidden_userspace = [
        "ptrace",
        "process_vm_writev",
        "sched_setscheduler",
        "setpriority",
        "bpf_override_return",
        "bpf_send_signal",
    ];

    for file in files_to_check {
        let content = std::fs::read_to_string(&file).expect("source file must be readable");
        for f in &forbidden_userspace {
            assert!(
                !content.contains(f),
                "CRITICAL SAFETY VIOLATION: {} contains forbidden target modification primitive '{}'",
                file.display(),
                f
            );
        }
    }
}

#[test]
fn test_documentation_and_banned_terms_audit() {
    // PRD §27, §29: Check README prior art and zero banned buzzwords
    let readme = std::fs::read_to_string("README.md").expect("README.md must exist");
    assert!(
        readme.contains("Prior Art"),
        "README.md must contain 'Prior Art & Positioning' section per PRD §29"
    );
    assert!(
        readme.contains("Prism") && readme.contains("offwaketime"),
        "README.md must cite Prism and BCC offwaketime"
    );

    let banned_terms = ["novel", "first-ever", "ai-powered", "ai powered"];
    let lower = readme.to_lowercase();
    for term in &banned_terms {
        assert!(
            !lower.contains(term),
            "ANTI-INFLATION VIOLATION: README contains banned marketing buzzword '{}'",
            term
        );
    }
}
