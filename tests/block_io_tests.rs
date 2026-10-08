//! Phase 2 Block I/O Attribution & Verification Test Suite (PRD §32.1).

use katana::block_io::{BlockRqComplete, BlockRqIssue};
use katana::diagnosis::{DiagStatus, FindingKind};
use katana::events::{Event, EventKind, LossLedger};
use katana::evidence::{Evidence, EvidenceBasis, EvidenceClass, EvidenceQuality, EvidenceStrength, RuleId};
use katana::renderer::{render_diagnosis, MANDATORY_CORRELATION_SUFFIX};
use katana::scheduler::{EventRef, SchedSwitch, TaskState, ThreadId};
use katana::Engine;

#[test]
fn test_bio1_direct_sync_attribution() {
    let target = ThreadId::new(4217, 4217);
    let dev_id = 0x80001; // major 8, minor 1 (e.g. /dev/sda1)
    let req_id = 0xdeadbeef_u64;

    let events = vec![
        // 1. Thread 4217 issues a synchronous direct write/fsync request
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            target,
            EventKind::BlockRqIssue(BlockRqIssue::new(
                dev_id,
                req_id,
                4096,
                8,
                "WS",
                target,
                1_000_000_000,
            )),
        ),
        // 2. Thread 4217 voluntarily goes off-CPU in iowait
        Event::new(
            1_000_050_000,
            EventRef::new(0, 2),
            target,
            EventKind::Switch(SchedSwitch {
                prev: target,
                next: ThreadId::new(100, 100),
                prev_state: TaskState::IoWait,
                preempted: false,
                in_iowait: true,
            }),
        ),
        // 3. Block request completes
        Event::new(
            1_020_000_000,
            EventRef::new(0, 3),
            ThreadId::new(0, 0),
            EventKind::BlockRqComplete(BlockRqComplete::new(
                dev_id,
                req_id,
                4096,
                0,
                1_020_000_000,
            )),
        ),
        // 4. Thread 4217 switches back in
        Event::new(
            1_020_050_000,
            EventRef::new(0, 4),
            target,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(100, 100),
                next: target,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
    ];

    let engine = Engine::new(8);
    let report = engine.analyze(4217, Some(4217), events, LossLedger::new());

    assert_eq!(report.diagnosis.status, DiagStatus::Found);
    let primary = report.diagnosis.primary.as_ref().expect("Expected primary finding");

    assert_eq!(primary.kind, FindingKind::BlockIoWait);
    assert_eq!(primary.subject.tid, 4217);
    assert!(primary.has_causal_edge, "BIO-1 must produce causal edge");
    assert_eq!(primary.weakest_strength, EvidenceStrength::Moderate);
    assert_eq!(primary.explained_fraction_per_mille, 1000);

    let rendered = render_diagnosis(&report.diagnosis, false);
    assert!(
        rendered.contains("Thread 4217 blocked on device dev_t 0x80001"),
        "Rendered text must report blocked device"
    );
}

#[test]
fn test_bio2_writeback_kworker_correlated_only() {
    let target = ThreadId::new(4217, 4217);
    let kworker = ThreadId::new(99, 0); // Background kworker thread
    let dev_id = 0x80001;
    let req_id = 0xcafe_u64;

    let events = vec![
        // 1. Thread 4217 goes off-CPU
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            target,
            EventKind::Switch(SchedSwitch {
                prev: target,
                next: ThreadId::new(100, 100),
                prev_state: TaskState::Sleeping,
                preempted: false,
                in_iowait: false,
            }),
        ),
        // 2. Writeback request is submitted by kworker, NOT thread 4217
        Event::new(
            1_005_000_000,
            EventRef::new(0, 2),
            kworker,
            EventKind::BlockRqIssue(BlockRqIssue::new(
                dev_id,
                req_id,
                8192,
                16,
                "W",
                kworker,
                1_005_000_000,
            )),
        ),
        // 3. Request completes
        Event::new(
            1_015_000_000,
            EventRef::new(0, 3),
            kworker,
            EventKind::BlockRqComplete(BlockRqComplete::new(
                dev_id,
                req_id,
                8192,
                0,
                1_015_000_000,
            )),
        ),
        // 4. Thread 4217 returns to CPU
        Event::new(
            1_020_000_000,
            EventRef::new(0, 4),
            target,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(100, 100),
                next: target,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
    ];

    let engine = Engine::new(8);
    let report = engine.analyze(4217, Some(4217), events, LossLedger::new());

    assert_eq!(report.diagnosis.status, DiagStatus::Found);
    let primary = report.diagnosis.primary.as_ref().expect("Expected primary finding");

    // Per PRD §32.1: Writeback is strictly CORRELATED, never CAUSAL
    assert_eq!(primary.kind, FindingKind::BlockIoCorrelated);
    assert!(!primary.has_causal_edge, "BIO-2 must NEVER claim causal relation");
    assert_eq!(primary.weakest_strength, EvidenceStrength::Weak);

    let rendered = render_diagnosis(&report.diagnosis, false);
    assert!(
        rendered.contains(MANDATORY_CORRELATION_SUFFIX),
        "Rendered output for BIO-2 must contain mandatory correlation disclaimer"
    );
}

#[test]
fn test_bio_request_id_mismatch() {
    let target = ThreadId::new(4217, 4217);
    let dev_id = 0x80001;

    let events = vec![
        // 1. Thread 4217 issues request 0x1111
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            target,
            EventKind::BlockRqIssue(BlockRqIssue::new(
                dev_id,
                0x1111,
                100,
                2,
                "R",
                target,
                1_000_000_000,
            )),
        ),
        // 2. Thread 4217 goes off-CPU
        Event::new(
            1_000_010_000,
            EventRef::new(0, 2),
            target,
            EventKind::Switch(SchedSwitch {
                prev: target,
                next: ThreadId::new(100, 100),
                prev_state: TaskState::IoWait,
                preempted: false,
                in_iowait: true,
            }),
        ),
        // 3. Different request 0x2222 completes (0x1111 never completes)
        Event::new(
            1_010_000_000,
            EventRef::new(0, 3),
            ThreadId::new(0, 0),
            EventKind::BlockRqComplete(BlockRqComplete::new(
                dev_id,
                0x2222,
                1024,
                0,
                1_010_000_000,
            )),
        ),
        // 4. Thread 4217 wakes up
        Event::new(
            1_015_000_000,
            EventRef::new(0, 4),
            target,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(100, 100),
                next: target,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
    ];

    let engine = Engine::new(8);
    let report = engine.analyze(4217, Some(4217), events, LossLedger::new());

    // Mismatched request ID cannot attribute device block
    if let Some(primary) = report.diagnosis.primary {
        assert_ne!(primary.kind, FindingKind::BlockIoWait);
    }
}

#[test]
fn test_bio_anti_inflation_lint() {
    // 1. Rule invariants
    assert_eq!(RuleId::Bio1.max_class(), EvidenceClass::Causal);
    assert_eq!(RuleId::Bio2.max_class(), EvidenceClass::Correlated);

    // 2. Monotonic clamping: attempting to construct Causal evidence with BIO-2 must clamp to Correlated
    let ev = Evidence::new(
        "BIO_TEST".to_string(),
        EvidenceClass::Causal, // Attempt to inflate
        EvidenceBasis::Statistical,
        EvidenceStrength::Weak,
        EvidenceQuality::Full,
        RuleId::Bio2,
        "Testing bio anti inflation".to_string(),
        vec![],
        vec![],
        vec![],
    );

    assert_eq!(
        ev.class,
        EvidenceClass::Correlated,
        "Rule BIO-2 must structurally clamp evidence class to Correlated"
    );
}

#[test]
fn test_resolve_dev_name() {
    use katana::block_io::resolve_dev_name;

    // Unknown or synthetic dev_t falls back to hex format
    let unknown_dev = resolve_dev_name(0xdeadbeef);
    assert_eq!(unknown_dev, "dev_t 0xdeadbeef");

    // Real device on Linux (e.g., major 8, minor 1 -> /dev/sda1 if present)
    let sda1_dev = (8 << 20) | 1;
    let name = resolve_dev_name(sda1_dev);
    // On systems with sda1 it returns "sda1", otherwise fallback format
    assert!(name == "sda1" || name == format!("dev_t 0x{:x}", sda1_dev));
}
