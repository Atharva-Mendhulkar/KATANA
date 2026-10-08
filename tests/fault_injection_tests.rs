use katana::diagnosis::{Completeness, DiagStatus, FindingKind};
use katana::events::{Event, EventKind, LossLedger};
use katana::evidence::Limitation;
use katana::futex::{FutexCmd, FutexEnter, FutexExit, FUTEX_BITSET_MATCH_ANY};
use katana::graph::TerminalReason;
use katana::renderer::render_diagnosis;
use katana::scheduler::{EventRef, SchedSwitch, SchedWaking, TaskState, ThreadId, WakerCtx};
use katana::Engine;

#[test]
fn test_futex_operation_variants_1c() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let uaddr = 0x7f00_1000;

    // Variant 1: ETIMEDOUT (-110)
    let events_timeout = vec![
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
                has_timeout: true,
                pi_word: None,
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 2),
            t1,
            EventKind::FutexExit(FutexExit { ret: -110 }),
        ),
    ];
    let rep_timeout = engine.analyze(100, Some(101), events_timeout, LossLedger::new());
    assert_eq!(rep_timeout.diagnosis.status, DiagStatus::Found);
    let pri = rep_timeout.diagnosis.primary.unwrap();
    assert_eq!(pri.kind, FindingKind::FutexTimeout);
    assert!(pri.chain.is_none(), "Timeout must never imply a waker");

    // Variant 2: EINTR (-4)
    let events_intr = vec![
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
            1_500_000_000,
            EventRef::new(0, 2),
            t1,
            EventKind::FutexExit(FutexExit { ret: -4 }),
        ),
    ];
    let rep_intr = engine.analyze(100, Some(101), events_intr, LossLedger::new());
    assert_eq!(rep_intr.diagnosis.primary.unwrap().kind, FindingKind::FutexInterrupted);

    // Variant 3: LOCK_PI with observed owner TID
    let events_pi = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr,
                uaddr2: 0,
                cmd: FutexCmd::LockPi,
                private: true,
                val: 0,
                val3: 0,
                has_timeout: false,
                pi_word: Some(4221), // Low 30 bits owner TID 4221
            }),
        ),
        Event::new(
            2_000_000_000,
            EventRef::new(0, 2),
            t1,
            EventKind::Exit,
        ),
    ];
    let rep_pi = engine.analyze(100, Some(101), events_pi, LossLedger::new());
    let pri_pi = rep_pi.diagnosis.primary.unwrap();
    assert_eq!(pri_pi.kind, FindingKind::FutexPiOwnerObserved);
    assert!(pri_pi.details.contains("4221"));
}

#[test]
fn test_sched_runq_delay_2() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let hog = ThreadId::new(999, 100);

    // T1 is woken, but waits 15 ms in runqueue before running because of hog
    let events = vec![
        // T1 woken at t = 1_000_000_000
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            hog,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        // Hog runs on CPU 0
        Event::new(
            1_005_000_000,
            EventRef::new(0, 2),
            hog,
            EventKind::Switch(SchedSwitch {
                prev: hog,
                next: hog,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
        // T1 finally gets CPU 0 after 15 ms delay at t = 1_015_000_000
        Event::new(
            1_015_000_000,
            EventRef::new(0, 3),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: hog,
                next: t1,
                prev_state: TaskState::Preempted,
                preempted: true,
                in_iowait: false,
            }),
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    let pri = report.diagnosis.primary.unwrap();
    assert_eq!(pri.kind, FindingKind::SchedRunqDelay);
    assert_eq!(pri.blocked_duration_ns, 15_000_000);
}

#[test]
fn test_multi_hop_wakeup_chain_3() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let t3 = ThreadId::new(103, 100);
    let f1 = 0x7f00_1000;
    let f2 = 0x7f00_2000;

    let events = vec![
        // T1 waits on F1
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f1,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T2 waits on F2
        Event::new(
            1_010_000_000,
            EventRef::new(0, 2),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f2,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T3 wakes T2 on F2
        Event::new(
            2_000_000_000,
            EventRef::new(0, 3),
            t3,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f2,
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
            2_000_000_200,
            EventRef::new(0, 4),
            t3,
            EventKind::Waking(SchedWaking {
                wakee: t2,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_000_400,
            EventRef::new(0, 5),
            t3,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        // T2 exits wait on F2
        Event::new(
            2_000_000_600,
            EventRef::new(0, 6),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
        // T2 wakes T1 on F1
        Event::new(
            2_000_010_000,
            EventRef::new(0, 7),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f1,
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
            2_000_010_200,
            EventRef::new(0, 8),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_010_400,
            EventRef::new(0, 9),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        // T1 exits wait on F1
        Event::new(
            2_000_010_600,
            EventRef::new(0, 10),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    let pri = report.diagnosis.primary.unwrap();
    assert_eq!(pri.kind, FindingKind::FutexWakeChain);
    let chain = pri.chain.unwrap();
    assert_eq!(chain.depth, 2, "Chain must be T1 <- T2 <- T3 of depth 2");
    assert_eq!(chain.hops[0].waker, t2);
    assert_eq!(chain.hops[0].wakee, t1);
    assert_eq!(chain.hops[1].waker, t3);
    assert_eq!(chain.hops[1].wakee, t2);
    assert_eq!(chain.terminal_reason, TerminalReason::WakerRunning);
    assert_eq!(report.diagnosis.completeness, Completeness::Complete);
}

#[test]
fn test_chain_cycle_detection_3d() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let f1 = 0x7f00_1000;
    let f2 = 0x7f00_2000;

    // Ping-pong wakes: T1 wakes T2 on F2, then T2 wakes T1 on F1 while T1 had prior wait
    let events = vec![
        // T1 wait 1
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f1,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T2 wait 1
        Event::new(
            1_005_000_000,
            EventRef::new(0, 2),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f2,
                uaddr2: 0,
                cmd: FutexCmd::Wait,
                private: true,
                val: 0,
                val3: FUTEX_BITSET_MATCH_ANY,
                has_timeout: false,
                pi_word: None,
            }),
        ),
        // T1 wakes T2
        Event::new(
            1_010_000_000,
            EventRef::new(0, 3),
            t1,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f2,
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
            1_010_000_100,
            EventRef::new(0, 4),
            t1,
            EventKind::Waking(SchedWaking {
                wakee: t2,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            1_010_000_200,
            EventRef::new(0, 5),
            t1,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        Event::new(
            1_010_000_300,
            EventRef::new(0, 6),
            t2,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
        // T2 wakes T1
        Event::new(
            1_020_000_000,
            EventRef::new(0, 7),
            t2,
            EventKind::FutexEnter(FutexEnter {
                uaddr: f1,
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
            1_020_000_100,
            EventRef::new(0, 8),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            1_020_000_200,
            EventRef::new(0, 9),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        Event::new(
            1_020_000_300,
            EventRef::new(0, 10),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    let pri = report.diagnosis.primary.unwrap();
    let chain = pri.chain.unwrap();
    assert_eq!(chain.terminal_reason, TerminalReason::Cycle, "Ping pong must be detected as cycle");
}

#[test]
fn test_negative_control_4() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);

    // Target T1 is CPU bound (switches between running states, never enters futex wait)
    let events = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: t1,
                next: ThreadId::new(999, 100),
                prev_state: TaskState::Running,
                preempted: true,
                in_iowait: false,
            }),
        ),
        Event::new(
            1_002_000_000,
            EventRef::new(0, 2),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(999, 100),
                next: t1,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    assert_eq!(report.diagnosis.status, DiagStatus::NotBlocked);
    let pri = report.diagnosis.primary.unwrap();
    assert_eq!(pri.kind, FindingKind::NotBlocked);
    assert!(pri.chain.is_none());
}

#[test]
fn test_event_loss_ring_overflow_5() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let uaddr = 0x7f00_1000;

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
        Event::new(
            2_000_002_000,
            EventRef::new(0, 13),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    let mut loss_ledger = LossLedger::new();
    loss_ledger.record_loss(0, 1_999_000_000, 2_001_000_000, 50);

    let report = engine.analyze(100, Some(101), events, loss_ledger);
    assert_eq!(report.diagnosis.completeness, Completeness::Lossy);
    assert_eq!(report.stats.events_lost, 58);
}

#[test]
fn test_process_exit_during_window_6() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let uaddr = 0x7f00_1000;

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
        // Process/thread exits at t = 2_000_000_000
        Event::new(2_000_000_000, EventRef::new(0, 2), t1, EventKind::Exit),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    // Should gracefully conclude without hang, segfault, or false wake
    assert!(report.diagnosis.primary.is_some());
}

#[test]
fn test_replay_determinism_and_monotonicity_8() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);
    let t2 = ThreadId::new(102, 100);
    let uaddr = 0x7f00_1000;

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
            EventRef::new(0, 2),
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
            EventRef::new(0, 3),
            t2,
            EventKind::Waking(SchedWaking {
                wakee: t1,
                target_cpu: 0,
                waker_ctx: WakerCtx::Task,
            }),
        ),
        Event::new(
            2_000_001_000,
            EventRef::new(0, 4),
            t2,
            EventKind::FutexExit(FutexExit { ret: 1 }),
        ),
        Event::new(
            2_000_002_000,
            EventRef::new(0, 5),
            t1,
            EventKind::FutexExit(FutexExit { ret: 0 }),
        ),
    ];

    // Run twice: assert byte-identical JSON serialization
    let r1 = engine.analyze(100, Some(101), events.clone(), LossLedger::new()).to_json_pretty().unwrap();
    let r2 = engine.analyze(100, Some(101), events.clone(), LossLedger::new()).to_json_pretty().unwrap();
    assert_eq!(r1, r2, "Replay determinism violated: JSON output differed across runs");

    // Monotonicity property: deleting the waking event (E4) must NOT upgrade evidence
    let mut reduced_events = events.clone();
    reduced_events.remove(2); // Remove waking
    let rep_reduced = engine.analyze(100, Some(101), reduced_events, LossLedger::new());
    if let Some(pri_red) = rep_reduced.diagnosis.primary {
        assert_ne!(pri_red.kind, FindingKind::FutexWakeChain);
        assert!(!pri_red.has_causal_edge);
    }
}

#[test]
fn test_target_identity_and_snapshot_live() {
    let my_pid = std::process::id();
    let target = katana::target::resolve_target(my_pid).expect("Failed to resolve own process");
    assert_eq!(target.pid, my_pid);
    assert!(!target.comm.is_empty());

    let snapshot = katana::target::take_snapshot(my_pid).expect("Failed to take snapshot of own process");
    assert_eq!(snapshot.pid, my_pid);
    assert!(!snapshot.threads.is_empty());

    let verified = katana::target::verify_identity(&target, &target);
    assert!(verified, "Same identity must verify as valid");
}

#[test]
fn test_unsupported_syscall_futex2() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);

    // futex2 syscall (e.g. sys_futex_waitv, nr=449 on x86_64) emitted for target thread
    let events = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::UnsupportedSyscall { nr: 449 },
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    assert_eq!(report.diagnosis.status, DiagStatus::Found);
    let pri = report.diagnosis.primary.as_ref().unwrap();
    assert_eq!(pri.kind, FindingKind::BlockedUnattributed);
    assert!(pri.details.contains("futex2 / syscall 449"));
    assert!(report.diagnosis.limitations.contains(&Limitation::Futex2NotSupported));
    assert_eq!(report.diagnosis.completeness, Completeness::Partial);

    let rendered = render_diagnosis(&report.diagnosis, false);
    assert!(rendered.contains("was blocked in a state Katana cannot attribute in this version"));
}

#[test]
fn test_negative_control_4b_timer_sleep() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);

    // Target T1 voluntarily sleeps for 10 ms (e.g. nanosleep); no futex or block I/O
    let events = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: t1,
                next: ThreadId::new(999, 100),
                prev_state: TaskState::Sleeping,
                preempted: false,
                in_iowait: false,
            }),
        ),
        Event::new(
            1_010_000_000, // 10 ms sleep
            EventRef::new(0, 2),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(999, 100),
                next: t1,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    assert_eq!(report.diagnosis.status, DiagStatus::Found);
    let pri = report.diagnosis.primary.as_ref().unwrap();
    assert_eq!(pri.kind, FindingKind::BlockedUnattributed);
    assert!(pri.chain.is_none(), "Timer sleep must never imply a waker thread");
    assert!(pri.details.contains("timer sleep"));

    let rendered = render_diagnosis(&report.diagnosis, false);
    assert!(rendered.contains("was blocked in a state Katana cannot attribute in this version"));
}

#[test]
fn test_unattributed_iowait_without_device() {
    let engine = Engine::new(8);
    let t1 = ThreadId::new(101, 100);

    // Target T1 sleeps in iowait for 10 ms, but NO block_rq_* events occurred
    let events = vec![
        Event::new(
            1_000_000_000,
            EventRef::new(0, 1),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: t1,
                next: ThreadId::new(999, 100),
                prev_state: TaskState::IoWait,
                preempted: false,
                in_iowait: true,
            }),
        ),
        Event::new(
            1_010_000_000,
            EventRef::new(0, 2),
            t1,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(999, 100),
                next: t1,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ),
    ];

    let report = engine.analyze(100, Some(101), events, LossLedger::new());
    assert_eq!(report.diagnosis.status, DiagStatus::Found);
    let pri = report.diagnosis.primary.as_ref().unwrap();
    assert_eq!(pri.kind, FindingKind::BlockedUnattributed);
    assert!(pri.details.contains("unattributed iowait"));

    let rendered = render_diagnosis(&report.diagnosis, false);
    // Per PRD §32.1: in_iowait flag alone NEVER names a device
    assert!(!rendered.contains("blocked on device"));
}

