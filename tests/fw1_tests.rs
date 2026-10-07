use katana::causal_rules::{
    CausalEngine, Event, EventKind, EvidenceBasis, EvidenceClass, EvidenceQuality,
    EvidenceStrength, Limitation, LossLedger, RuleId,
};
use katana::futex::{
    FutexCmd, FutexEnter, FutexExit, FutexKey, FutexScope, FUTEX_BITSET_MATCH_ANY,
};
use katana::scheduler::{EventRef, SchedSwitch, SchedWaking, TaskState, ThreadId, WakerCtx};

#[test]
fn test_futex_wake_single_waiter() {
    let t1 = ThreadId::new(101, 100); // Waiter
    let t2 = ThreadId::new(102, 100); // Waker
    let uaddr = 0x7f3a_1000;

    let events = vec![
        // E1: T1 enters FUTEX_WAIT
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // E2: T1 switches out to sleep
        Event::new(
            1_000_000_100,
            EventRef::new(0, 2),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: t1,
                next: ThreadId::new(999, 100),
                prev_state: TaskState::Sleeping,
                preempted: false,
                in_iowait: false,
            }),
        ),
        // E3: T2 enters FUTEX_WAKE
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // E4: sched_waking in T2 context waking T1
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        // E5: T2 exits FUTEX_WAKE with ret=1
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        // E6: T1 switch in
        Event::new(
            2_000_002_000,
            EventRef::new(0, 13),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(999, 100),
                next: t1,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
        // E7: T1 exits FUTEX_WAIT with ret=0
        Event::new(
            2_000_003_000,
            EventRef::new(0, 14),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine = CausalEngine::new();
    let loss_ledger = LossLedger::new();
    engine.process_trace(&events, &loss_ledger);

    assert_eq!(engine.edges.len(), 1, "Exactly one causal wake edge expected");
    let edge = &engine.edges[0];
    assert_eq!(edge.rule, RuleId::Fw1);
    assert_eq!(edge.class, EvidenceClass::Causal);
    assert_eq!(edge.basis, EvidenceBasis::Derived);
    assert_eq!(edge.strength, EvidenceStrength::Moderate);
    assert_eq!(edge.quality, EvidenceQuality::Full);
    assert_eq!(edge.waker, t2);
    assert_eq!(edge.wakee, t1);
    assert_eq!(
        edge.futex_key,
        Some(FutexKey {
            scope: FutexScope::Private,
            tgid: 100,
            uaddr,
        })
    );
    assert_eq!(edge.assumptions, vec!["ASSUME_FUTEX_WAKE_ONLY".to_string()]);
    assert!(edge.limitations.is_empty());
    assert_eq!(engine.untracked_wakes, 0);
}

#[test]
fn test_futex_wake_multi_waiter() {
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let t3 = ThreadId::new(103, 100);
    let t4 = ThreadId::new(104, 100); // Waker
    let uaddr = 0x7f3a_1000;

    let mut events = Vec::new();

    // T1, T2, T3 enter wait
    for (i, tid) in [t1, t2, t3].iter().enumerate() {
        events.push(Event::new(
            1_000_000_000 + i as u64 * 1000,
            EventRef::new(0, (i + 1) as u64),
            *tid,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ));
    }

    // T4 wakes 2 waiters (T1, T2)
    events.push(Event::new(
        2_000_000_000,
        EventRef::new(0, 10),
        t4,
        EventKind::FutexEnter(FutexEnter {
            uaddr,
            uaddr2: 0,
            cmd: FutexCmd::Wake,
            private: true,
            val: 2,
            val3: FUTEX_BITSET_MATCH_ANY,
            has_timeout: false,
            pi_word: None,
        }),
    ));
    events.push(Event::new(
        2_000_000_100,
        EventRef::new(0, 11),
        t4,
        EventKind::Waking(SchedWaking {
            wakee: t1,
            target_cpu: 0,
            waker_ctx: WakerCtx::Task,
        }),
    ));
    events.push(Event::new(
        2_000_000_200,
        EventRef::new(0, 12),
        t4,
        EventKind::Waking(SchedWaking {
            wakee: t2,
            target_cpu: 0,
            waker_ctx: WakerCtx::Task,
        }),
    ));
    events.push(Event::new(
        2_000_001_000,
        EventRef::new(0, 13),
        t4,
        EventKind::FutexExit(FutexExit { ret: 2 }),
    ));

    // T1 and T2 exit wait
    events.push(Event::new(
        2_000_002_000,
        EventRef::new(0, 14),
        t1,
        EventKind::FutexExit(FutexExit { ret: 0 }),
    ));
    events.push(Event::new(
        2_000_003_000,
        EventRef::new(0, 15),
        t2,
        EventKind::FutexExit(FutexExit { ret: 0 }),
    ));

    let mut engine = CausalEngine::new();
    let loss_ledger = LossLedger::new();
    engine.process_trace(&events, &loss_ledger);

    assert_eq!(engine.edges.len(), 2, "Expected exactly 2 edges for the first wake (T1, T2)");
    assert!(engine.edges.iter().any(|e| e.wakee == t1));
    assert!(engine.edges.iter().any(|e| e.wakee == t2));
    assert!(!engine.edges.iter().any(|e| e.wakee == t3), "T3 should not be woken yet");
    assert_eq!(engine.untracked_wakes, 0);

    // Later T4 wakes T3 (val=1, ret=1)
    let second_wake = vec![
        Event::new(
            3_000_000_000,
            EventRef::new(0, 20),
            t4,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            3_000_000_100,
            EventRef::new(0, 21),
            t4,
            EventKind::Waking(SchedWaking {
                wakee: t3,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            3_000_001_000,
            EventRef::new(0, 22),
            t4,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        Event::new(
            3_000_002_000,
            EventRef::new(0, 23),
            t3,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];
    engine.process_trace(&second_wake, &loss_ledger);

    assert_eq!(engine.edges.len(), 3, "Expected total of 3 edges after waking T3");
    assert!(engine.edges.iter().any(|e| e.wakee == t3));
}

#[test]
fn test_futex_wake_wrong_key() {
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);

    // Subtest A: Wrong virtual address
    let events_wrong_addr = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr: 0x7f3a_1000,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr: 0x7f3a_2000, // Different address!
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine = CausalEngine::new();
    let loss_ledger = LossLedger::new();
    engine.process_trace(&events_wrong_addr, &loss_ledger);
    assert_eq!(
        engine.edges.len(),
        0,
        "No FW-1 edge should be emitted when futex address differs"
    );

    // Subtest B: Disjoint bitsets
    let events_disjoint_bitsets = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr: 0x7f3a_1000,
                uaddr2: 0,
                cmd: FutexCmd::WaitBitset,
                private: true,
                val: 0,
                val3: 0b01, // Waiter bitset
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr: 0x7f3a_1000,
                uaddr2: 0,
                cmd: FutexCmd::WakeBitset,
                private: true,
                val: 1,
                val3: 0b10, // Disjoint wake bitset!
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine2 = CausalEngine::new();
    engine2.process_trace(&events_disjoint_bitsets, &loss_ledger);
    assert_eq!(
        engine2.edges.len(),
        0,
        "No FW-1 edge should be emitted when bitsets are disjoint"
    );
}

#[test]
fn test_futex_wake_cross_cpu() {
    let t1 = ThreadId::new(101, 100); // Waiter on CPU 0
    let t2 = ThreadId::new(102, 100); // Waker on CPU 1
    let uaddr = 0x7f3a_1000;

    let events = vec![
        // E1: T1 enters wait on CPU 0
        Event::new(
            1_000_000_000,
            EventRef::new(0, 5),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // E3: T2 enters wake on CPU 1
        Event::new(
            2_000_000_000,
            EventRef::new(1, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // E4: T2 emits sched_waking on CPU 1 targeting CPU 0
        Event::new(
            2_000_000_300,
            EventRef::new(1, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        // E5: T2 exits wake on CPU 1
        Event::new(
            2_000_000_600,
            EventRef::new(1, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        // E7: T1 exits wait on CPU 0
        Event::new(
            2_000_015_000,
            EventRef::new(0, 6),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine = CausalEngine::new();
    let loss_ledger = LossLedger::new();
    engine.process_trace(&events, &loss_ledger);

    assert_eq!(engine.edges.len(), 1, "Cross-CPU wake should succeed under FW-1");
    let edge = &engine.edges[0];
    assert_eq!(edge.rule, RuleId::Fw1);
    assert_eq!(edge.class, EvidenceClass::Causal);
    assert_eq!(edge.basis, EvidenceBasis::Derived);
    assert_eq!(edge.quality, EvidenceQuality::Full);
    assert_eq!(edge.waker, t2);
    assert_eq!(edge.wakee, t1);
    assert_eq!(
        edge.provenance,
        vec![EventRef::new(1, 10), EventRef::new(1, 11), EventRef::new(1, 12)]
    );
}

#[test]
fn test_futex_wake_migration() {
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let uaddr = 0x7f3a_1000;

    let events = vec![
        // T1 waits
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T2 enters wake on CPU 0
        Event::new(
            2_000_000_000,
            EventRef::new(0, 100),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T2 migrates to CPU 1 and emits waking
        Event::new(
            2_000_010_000,
            EventRef::new(1, 20),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        // T2 exits wake on CPU 1
        Event::new(
            2_000_020_000,
            EventRef::new(1, 25),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        // T1 exits wait
        Event::new(
            2_000_030_000,
            EventRef::new(0, 2),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine = CausalEngine::new();
    let loss_ledger = LossLedger::new();
    engine.process_trace(&events, &loss_ledger);

    assert_eq!(
        engine.edges.len(),
        1,
        "Waker task migration between enter and waking must be supported"
    );
    let edge = &engine.edges[0];
    assert_eq!(edge.rule, RuleId::Fw1);
    assert_eq!(edge.waker, t2);
    assert_eq!(edge.wakee, t1);
    assert_eq!(edge.quality, EvidenceQuality::Full);
}

#[test]
fn test_futex_wake_event_loss() {
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let uaddr = 0x7f3a_1000;

    // Subtest A: Loss interval in LossLedger overlapping wake interval
    let events = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
    ];

    let mut loss_ledger = LossLedger::new();
    // Record loss interval overlapping CPU 0 during wake window
    loss_ledger.record_loss(0, 1_999_999_000, 2_000_000_600, 3);

    let mut engine = CausalEngine::new();
    engine.process_trace(&events, &loss_ledger);

    assert_eq!(engine.edges.len(), 1);
    let edge = &engine.edges[0];
    assert_eq!(
        edge.quality,
        EvidenceQuality::Degraded("Loss interval overlaps wake window".to_string())
    );
    assert_eq!(edge.strength, EvidenceStrength::Weak);
    assert!(edge.limitations.contains(&Limitation::LossOverlap));

    // Subtest B: Wake count contradiction (count_seen > ret)
    let t3 = ThreadId::new(103, 100);
    let contradiction_events = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            1_000_000_500,
            EventRef::new(0, 2),
            t3,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // 2 waking events emitted...
        Event::new(
            2_000_000_100,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_000_200,
            EventRef::new(0, 12),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t3,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        // ...but kernel ret was only 1! (count_seen 2 > ret 1)
        Event::new(
            2_000_001_000,
            EventRef::new(0, 13),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
    ];

    let mut engine2 = CausalEngine::new();
    let empty_loss = LossLedger::new();
    engine2.process_trace(&contradiction_events, &empty_loss);

    assert_eq!(engine2.edges.len(), 2);
    for edge in &engine2.edges {
        assert_eq!(edge.strength, EvidenceStrength::Weak);
        assert!(edge.limitations.contains(&Limitation::InconsistentWakeCount));
    }
}

#[test]
fn test_futex_wake_unrelated_waker() {
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let t_unrelated = ThreadId::new(199, 100);
    let uaddr = 0x7f3a_1000;

    // Case A: Unrelated waker thread T_unrelated waking T1
    let events_unrelated_thread = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T2 enters futex wake
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // But the sched_waking comes from T_unrelated, not T2!
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t_unrelated,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine = CausalEngine::new();
    let loss_ledger = LossLedger::new();
    engine.process_trace(&events_unrelated_thread, &loss_ledger);
    assert_eq!(
        engine.edges.len(),
        0,
        "No FW-1 edge should be attributed to T2 when T_unrelated performed the wake"
    );

    // Case B: Waking emitted in IRQ context
    let events_irq = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // Waking has IRQ context!
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Irq,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine2 = CausalEngine::new();
    engine2.process_trace(&events_irq, &loss_ledger);
    assert_eq!(
        engine2.edges.len(),
        0,
        "Wakes occurring in IRQ context must never be attributed under FW-1"
    );

    // Case C: Waker wakes a thread with NO open futex wait interval
    let t_nowait = ThreadId::new(105, 100);
    let events_nowait = vec![
        Event::new(
            2_000_000_000,
            EventRef::new(0, 10),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::Wake,
                private: true,
                val: 1,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_500,
            EventRef::new(0, 11),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t_nowait,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 12),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut engine3 = CausalEngine::new();
    engine3.process_trace(&events_nowait, &loss_ledger);
    assert_eq!(
        engine3.edges.len(),
        0,
        "Wake of thread without open futex wait cannot yield an FW-1 edge"
    );
}
