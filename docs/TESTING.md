# Katana Testing Guide

> **Test Architecture, Fault Injection Suites, and Reproducibility**  
> Reference: [prd.md §22, §26](file:///home/topfloorboss/Desktop/KATANA/prd.md)

---

## 1. Testing Philosophy

Katana's testing strategy is built around deterministic replayability, strict anti-inflation verification, and rigorous negative controls:

1. **Pure Offline Verification:** The entire analysis pipeline (`src/lib.rs`, `src/events.rs`, `src/causal_rules.rs`, `src/evidence.rs`, `src/diagnosis.rs`) is completely decoupled from kernel execution. Tests execute without root privileges by feeding recorded traces or synthetic event streams.
2. **Never Overclaim (Anti-Inflation):** Negative tests verify that lack of causation is faithfully diagnosed as `NotBlocked`, `BlockedUnattributed`, `Ambiguous`, or `Correlated` (with disclaimers), never falsely labeled `Causal`.
3. **Determinism:** Replaying identical event streams across multiple trials must produce bit-for-bit identical diagnostic reports and ranking tuples.

---

## 2. Test Suites Overview

The repository currently provides 29 comprehensive test cases across four suites:

```text
tests/
├── anti_inflation_tests.rs     # 2 tests: monotonic rule ceilings, renderer linting
├── block_io_tests.rs           # 8 tests: BIO-1, BIO-2, sysfs resolution, wire decoders, replay
├── fault_injection_tests.rs    # 12 tests: Tests 1–8 (cycles, loss, negative controls, futex2, PID reuse)
└── fw1_tests.rs                # 7 tests: Futex wake pairing (cross-CPU, migration, loss, key mismatch)
```

Run all tests:
```bash
cargo test
```

---

## 3. Ground-Truth Fault Injectors (PRD §22)

| Test ID | Test Name | Scenario / Ground Truth | Expected Diagnosis | Negative / Anti-Inflation Check |
|---|---|---|---|---|
| **Test 1** | Simple futex wake | T1 blocks on futex; T2 issues `FUTEX_WAKE` matching address. | `FindingKind::FutexWakeChain` with `EvidenceClass::Causal`. | Must never claim T2 "owns" or "holds" a non-PI futex. |
| **Test 1b** | Multi-waiter wake | Multiple threads wait on same futex; partial wake issued. | Exactly pairs woken waiters; unreconciled waiters remain unaffected. | No waker attributed to unawakened waiters. |
| **Test 1c** | Futex op variants | `WAIT`, `WAIT_BITSET`, `ETIMEDOUT`, `EINTR`, `LOCK_PI`, futex2. | Timeouts/signals produce no waker; PI reports snapshot owner; futex2 flags `Futex2NotSupported`. | No false wakers on timeouts or interrupts. |
| **Test 2** | Runqueue contention | Target thread delayed on runqueue by competing CPU hogs. | `FindingKind::SchedRunqDelay` with `EvidenceClass::Correlated`. | Runqueue delay is never labeled `Causal`. |
| **Test 3** | Multi-hop wake chain | T3 wakes T2, which wakes T1 (chain depth 2). | Reconstructs ordered causal chain `T1 ← T2 ← T3` up to terminal waker. | Bounded depth (`depth ≤ 8`); wake cycles detected as `Cycle` (not deadlock). |
| **Test 4** | Negative control (idle/spin) | Target thread is pure CPU-bound; unrelated threads swap futexes. | `DiagnosisStatus::NotBlocked`. | No causal edges attributed to target; no futex findings. |
| **Test 4b** | Negative control (timer sleep) | Target thread blocks in `nanosleep` or voluntary off-CPU sleep. | `FindingKind::BlockedUnattributed`. | Never attributes a thread waker to a timer interrupt. |
| **Test 5** | Event loss / ring overflow | Sequence counter gaps or buffer overflow injected. | `LossLedger` records drop count; completeness degrades to `Partial`. | Incomplete chains are never marked `Complete`. |
| **Test 6** | Process exit during window | Target terminates mid-window. | Report indicates exit without crashing or hanging collector. | Graceful exit with exit code `0` or `14`. |
| **Test 7** | Identity & PID reuse | Re-used PID with differing start time or boot ID. | Rejects mismatched PID, exiting with code `4`. | Stale process state never conflated with new process. |
| **Test 8** | Replay determinism | Same trace replayed across 100 consecutive iterations. | Identical diagnosis, ranking, and evidence tuples. | Absolute reproducibility. |
| **BIO-1** | Direct sync block I/O | `O_DIRECT`/`fsync` issued by target; completion unblocks target. | `FindingKind::BlockIoWait` with `EvidenceClass::Causal`. | Requires request ID match and submitter context match. |
| **BIO-2** | Async writeback I/O | Background flushers (`kworker`) commit pages during sleep. | `FindingKind::BlockIoCorrelated` with `EvidenceClass::Correlated`. | Mandatory disclaimer: *"This trace does not establish a causal link."* |

---

## 4. How to Add a New Injector / Replay Fixture

To add a new tracepoint injector or golden replay scenario:

### Step 1: Create or Record the Fixture
Create a JSON trace fixture in `fixtures/<name>.json` containing target metadata and the sequence of raw events:
```json
{
  "target": {
    "tgid": 1000,
    "pid": 1000,
    "comm": "my_workload",
    "start_time_ticks": 500000,
    "boot_id": "00000000-0000-0000-0000-000000000000"
  },
  "events": [
    {
      "event_type": 1,
      "cpu": 0,
      "t_mono_ns": 1000000000,
      "tid": 1000,
      "comm": "my_workload",
      "prev_state": 1,
      "next_tid": 2000,
      "next_comm": "other_task"
    }
  ]
}
```

### Step 2: Write an Integration Test
Add a test function in `tests/fault_injection_tests.rs` or a dedicated test file:
```rust
#[test]
fn test_my_new_scenario() {
    let trace_path = "fixtures/my_fixture.json";
    let trace = load_trace(trace_path).expect("valid fixture");
    let result = katana::analyze(&trace.target, trace.events);

    assert_eq!(result.status, DiagnosisStatus::Found);
    // Verify anti-inflation assertions
    for ev in &result.evidence {
        if ev.class == EvidenceClass::Correlated {
            assert!(ev.summary.contains("does not establish a causal link"));
        }
    }
}
```

### Step 3: Run Scope and Lint Verification
```bash
./scripts/check-scope.sh
cargo test --test fault_injection_tests
```
