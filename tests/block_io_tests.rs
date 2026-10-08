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

#[test]
fn test_bio_replay_fixtures() {
    let engine = Engine::new(8);

    // 1. Replay bio1_sync_io.json
    let content_bio1 = std::fs::read_to_string("fixtures/bio1_sync_io.json")
        .expect("fixtures/bio1_sync_io.json must exist");
    let evs_bio1: Vec<Event> = serde_json::from_str(&content_bio1).unwrap();
    let rep_bio1 = engine.analyze(4217, Some(4217), evs_bio1, LossLedger::new());

    assert_eq!(rep_bio1.diagnosis.status, DiagStatus::Found);
    let pri1 = rep_bio1.diagnosis.primary.as_ref().unwrap();
    assert_eq!(pri1.kind, FindingKind::BlockIoWait);
    assert!(pri1.has_causal_edge);

    // 2. Replay bio2_writeback.json
    let content_bio2 = std::fs::read_to_string("fixtures/bio2_writeback.json")
        .expect("fixtures/bio2_writeback.json must exist");
    let evs_bio2: Vec<Event> = serde_json::from_str(&content_bio2).unwrap();
    let rep_bio2 = engine.analyze(4217, Some(4217), evs_bio2, LossLedger::new());

    assert_eq!(rep_bio2.diagnosis.status, DiagStatus::Found);
    let pri2 = rep_bio2.diagnosis.primary.as_ref().unwrap();
    assert_eq!(pri2.kind, FindingKind::BlockIoCorrelated);
    assert!(!pri2.has_causal_edge);

    let rendered = render_diagnosis(&rep_bio2.diagnosis, false);
    assert!(rendered.contains(MANDATORY_CORRELATION_SUFFIX));
}

#[test]
fn test_decode_raw_block_events() {
    // 1. Construct raw kt_hdr (28 bytes) + kt_block_issue (40 bytes)
    let mut buf = vec![0u8; 68];
    // ts_ns = 5_000_000_000
    buf[0..8].copy_from_slice(&5_000_000_000u64.to_le_bytes());
    // seq = 42
    buf[8..16].copy_from_slice(&42u64.to_le_bytes());
    // tid = 1234
    buf[16..20].copy_from_slice(&1234u32.to_le_bytes());
    // tgid = 1234
    buf[20..24].copy_from_slice(&1234u32.to_le_bytes());
    // cpu = 3
    buf[24..26].copy_from_slice(&3u16.to_le_bytes());
    // type = KT_TYPE_BLOCK_ISSUE (9)
    buf[26] = 9;

    // Payload: dev_id = 0x80001, nr_sector = 16, req_id = 0xbeef, sector = 2048, submitter_tid = 1234, rwbs = "WS"
    buf[28..32].copy_from_slice(&0x80001u32.to_le_bytes());
    buf[32..36].copy_from_slice(&16u32.to_le_bytes());
    buf[36..44].copy_from_slice(&0xbeef_u64.to_le_bytes());
    buf[44..52].copy_from_slice(&2048u64.to_le_bytes());
    buf[52..56].copy_from_slice(&1234u32.to_le_bytes());
    buf[56..60].copy_from_slice(&1234u32.to_le_bytes());
    buf[60..62].copy_from_slice(b"WS");

    let event = Event::decode_raw(&buf).expect("Failed to decode raw block issue");
    assert_eq!(event.ts_ns, 5_000_000_000);
    assert_eq!(event.r#ref.cpu, 3);
    assert_eq!(event.r#ref.seq, 42);
    assert_eq!(event.thread.tid, 1234);

    if let EventKind::BlockRqIssue(issue) = event.kind {
        assert_eq!(issue.dev_id, 0x80001);
        assert_eq!(issue.req_id, 0xbeef);
        assert_eq!(issue.nr_sector, 16);
        assert_eq!(issue.sector, 2048);
        assert_eq!(issue.rwbs, "WS");
        assert_eq!(issue.submitter.tid, 1234);
    } else {
        panic!("Decoded event kind was not BlockRqIssue");
    }

    // 2. Construct raw kt_hdr (28 bytes) + kt_block_complete (20 bytes)
    let mut comp_buf = vec![0u8; 48];
    comp_buf[0..8].copy_from_slice(&5_010_000_000u64.to_le_bytes());
    comp_buf[8..16].copy_from_slice(&43u64.to_le_bytes());
    comp_buf[16..20].copy_from_slice(&0u32.to_le_bytes());
    comp_buf[20..24].copy_from_slice(&0u32.to_le_bytes());
    comp_buf[24..26].copy_from_slice(&3u16.to_le_bytes());
    comp_buf[26] = 10; // KT_TYPE_BLOCK_COMPLETE

    // Payload: dev_id = 0x80001, nr_bytes = 8192, req_id = 0xbeef, error = 0
    comp_buf[28..32].copy_from_slice(&0x80001u32.to_le_bytes());
    comp_buf[32..36].copy_from_slice(&8192u32.to_le_bytes());
    comp_buf[36..44].copy_from_slice(&0xbeef_u64.to_le_bytes());
    comp_buf[44..48].copy_from_slice(&0i32.to_le_bytes());

    let comp_event = Event::decode_raw(&comp_buf).expect("Failed to decode raw block complete");
    if let EventKind::BlockRqComplete(comp) = comp_event.kind {
        assert_eq!(comp.dev_id, 0x80001);
        assert_eq!(comp.req_id, 0xbeef);
        assert_eq!(comp.nr_bytes, 8192);
        assert_eq!(comp.error, 0);
    } else {
        panic!("Decoded event kind was not BlockRqComplete");
    }

    // 3. Buffer truncation test
    assert!(Event::decode_raw(&buf[..20]).is_err());
    assert!(Event::decode_raw(&buf[..35]).is_err());
}

#[test]
fn test_decode_raw_sched_and_futex() {
    use katana::futex::FutexCmd;
    use katana::scheduler::WakerCtx;

    // 1. Sched switch packet
    let mut sw_buf = vec![0u8; 52];
    sw_buf[0..8].copy_from_slice(&1_000_000u64.to_le_bytes()); // ts_ns
    sw_buf[8..16].copy_from_slice(&1u64.to_le_bytes()); // seq
    sw_buf[16..20].copy_from_slice(&101u32.to_le_bytes()); // tid
    sw_buf[20..24].copy_from_slice(&100u32.to_le_bytes()); // tgid
    sw_buf[24..26].copy_from_slice(&0u16.to_le_bytes()); // cpu
    sw_buf[26] = 0; // KT_TYPE_SWITCH

    // Payload: prev_tid=101, next_tid=102, prev_state=2 (IoWait), sflags=2 (in_iowait)
    sw_buf[28..32].copy_from_slice(&101u32.to_le_bytes());
    sw_buf[32..36].copy_from_slice(&102u32.to_le_bytes());
    sw_buf[36..40].copy_from_slice(&2u32.to_le_bytes());
    sw_buf[40..42].copy_from_slice(&2u16.to_le_bytes());
    sw_buf[44..48].copy_from_slice(&100u32.to_le_bytes());
    sw_buf[48..52].copy_from_slice(&100u32.to_le_bytes());

    let sw_ev = Event::decode_raw(&sw_buf).expect("Decode switch failed");
    if let EventKind::Switch(sw) = sw_ev.kind {
        assert_eq!(sw.prev.tid, 101);
        assert_eq!(sw.next.tid, 102);
        assert_eq!(sw.prev_state, TaskState::IoWait);
        assert!(sw.in_iowait);
    } else {
        panic!("Decoded event kind was not Switch");
    }

    // 2. Futex enter packet with PI owner
    let mut fe_buf = vec![0u8; 64];
    fe_buf[0..8].copy_from_slice(&2_000_000u64.to_le_bytes());
    fe_buf[8..16].copy_from_slice(&2u64.to_le_bytes());
    fe_buf[16..20].copy_from_slice(&101u32.to_le_bytes());
    fe_buf[20..24].copy_from_slice(&100u32.to_le_bytes());
    fe_buf[24..26].copy_from_slice(&0u16.to_le_bytes());
    fe_buf[26] = 7; // KT_TYPE_FUTEX_ENTER

    // Payload: uaddr=0x7fff_0000, op=6 (LockPi), val=0, pi_word=102, pi_word_valid=1
    fe_buf[28..36].copy_from_slice(&0x7fff_0000u64.to_le_bytes());
    fe_buf[44..48].copy_from_slice(&6u32.to_le_bytes());
    fe_buf[56..60].copy_from_slice(&102u32.to_le_bytes());
    fe_buf[60] = 1; // pi_word_valid

    let fe_ev = Event::decode_raw(&fe_buf).expect("Decode futex enter failed");
    if let EventKind::FutexEnter(fe) = fe_ev.kind {
        assert_eq!(fe.uaddr, 0x7fff_0000);
        assert_eq!(fe.cmd, FutexCmd::LockPi);
        assert_eq!(fe.pi_word, Some(102));
    } else {
        panic!("Decoded event kind was not FutexEnter");
    }

    // 3. Futex exit packet
    let mut fx_buf = vec![0u8; 36];
    fx_buf[0..8].copy_from_slice(&3_000_000u64.to_le_bytes());
    fx_buf[8..16].copy_from_slice(&3u64.to_le_bytes());
    fx_buf[16..20].copy_from_slice(&101u32.to_le_bytes());
    fx_buf[20..24].copy_from_slice(&100u32.to_le_bytes());
    fx_buf[24..26].copy_from_slice(&0u16.to_le_bytes());
    fx_buf[26] = 8; // KT_TYPE_FUTEX_EXIT
    fx_buf[28..36].copy_from_slice(&(-110i64).to_le_bytes()); // ETIMEDOUT

    let fx_ev = Event::decode_raw(&fx_buf).expect("Decode futex exit failed");
    if let EventKind::FutexExit(fx) = fx_ev.kind {
        assert_eq!(fx.ret, -110);
    } else {
        panic!("Decoded event kind was not FutexExit");
    }

    // 4. Sched waking packet
    let mut wk_buf = vec![0u8; 44];
    wk_buf[0..8].copy_from_slice(&4_000_000u64.to_le_bytes());
    wk_buf[8..16].copy_from_slice(&4u64.to_le_bytes());
    wk_buf[16..20].copy_from_slice(&102u32.to_le_bytes());
    wk_buf[20..24].copy_from_slice(&100u32.to_le_bytes());
    wk_buf[24..26].copy_from_slice(&1u16.to_le_bytes());
    wk_buf[26] = 1; // KT_TYPE_WAKING

    // Payload: wakee_tid=101, wakee_tgid=100, target_cpu=0, sflags=1 (Irq)
    wk_buf[28..32].copy_from_slice(&101u32.to_le_bytes());
    wk_buf[32..36].copy_from_slice(&100u32.to_le_bytes());
    wk_buf[36..38].copy_from_slice(&0u16.to_le_bytes());
    wk_buf[38..40].copy_from_slice(&1u16.to_le_bytes()); // Irq

    let wk_ev = Event::decode_raw(&wk_buf).expect("Decode waking failed");
    if let EventKind::Waking(wk) = wk_ev.kind {
        assert_eq!(wk.wakee.tid, 101);
        assert_eq!(wk.waker_ctx, WakerCtx::Irq);
    } else {
        panic!("Decoded event kind was not Waking");
    }
}

