# Katana Limitations & Open Technical Questions (OTQ) Review

**Document Version:** 1.1 (Phase 1–3 Complete)  
**Applicability:** Linux 5.15–6.x (x86_64, arm64)  
**Reference:** [PRD §23, §32, §36](../prd.md)

---

## 1. Overview

Katana operates under a strict **structural no-inflation** principle: it only claims causality when supported by kernel semantics and verified invariant linkage. When observation boundaries, kernel versions, or execution contexts prevent definitive attribution, Katana reports limitations explicitly rather than conjecturing causes.

This document catalogs known gaps, design limitations, and the concrete resolution of Open Technical Questions (**OTQ-1 through OTQ-14**).

---

## 2. Open Technical Questions Resolution Matrix

### 2.1 OTQ-1: Waker in Interrupt / Kernel-Thread Context
* **Question:** How to detect "waker in interrupt context" from a tracepoint BPF program portably (x86_64/arm64)?
* **Resolution in Katana:** 
  - Tracepoint programs determine execution context via `bpf_get_current_task_btf` and `preempt_count` bitmask inspection (`HARDIRQ_MASK | SOFTIRQ_MASK | NMI_MASK`) or thread metadata.
  - Wakers executing in interrupt or kthread context are tagged as `WakerCtx::Irq` or `WakerCtx::Kthread`.
  - Katana strictly excludes interrupted user tasks from being attributed as the cause of a wakeup (verified in `tests/fw1_tests.rs`).

### 2.2 OTQ-2: Encoding of Preemption in `sched_switch`
* **Question:** How to reliably distinguish voluntary sleep from involuntary preemption across kernel versions 5.15–6.x (`prev_state` bit vs. `preempt` field)?
* **Resolution in Katana:**
  - Evaluated via CO-RE field existence and kernel release detection.
  - Threads preempted while runnable are recorded with `Relation::PreemptedBy` (Rule SW-1).
  - Threads yielding voluntarily with `TASK_INTERRUPTIBLE` or `TASK_UNINTERRUPTIBLE` enter sleep tracking (verified in `tests/fault_injection_tests.rs`).

### 2.3 OTQ-3: Cross-CPU Clock Monotonicity & Timing Bounds
* **Question:** What is the cross-CPU clock skew bound for `bpf_ktime_get_ns` on multi-socket hardware, and is $\varepsilon = 50\,\mu\text{s}$ adequate?
* **Resolution in Katana:**
  - `bpf_ktime_get_ns()` provides monotonic time on modern x86_64 invariant TSC and arm64 generic timer architectures.
  - Events are totally ordered by the tuple `(ts_ns, cpu, seq)`.
  - When matching syscall exits and wake intervals across CPUs, an epsilon margin $\varepsilon = 50\,\mu\text{s}$ is applied to accommodate scheduling latency and tracepoint dispatch skew.
  - Any observed clock reversal or sequence gap is logged in `LossLedger` and degrades completeness to `LOSSY`.

### 2.4 OTQ-4: 32-Bit Compat Futex Calls
* **Question:** How are 32-bit userspace compat futex syscalls (`compat_sys_futex`, `futex_time32`) handled on 64-bit kernels?
* **Resolution in Katana:**
  - Katana detects 32-bit userspace targets via `/proc/<pid>/exe` ELF header inspection.
  - 32-bit user targets on 64-bit kernels are explicitly flagged as unsupported attribution (`Limitation::Compat32NotSupported`) to prevent pointer misinterpretation.

### 2.5 OTQ-5: In-Syscall Wake Disambiguation
* **Question:** Does any non-futex in-syscall wake target a task sleeping in `futex_wait`, breaking Rule FW-1's inference?
* **Resolution in Katana:**
  - Evaluated with concurrent memory/lock stress injectors.
  - FW-1 evidence is statically capped at `EvidenceStrength::Moderate` and tagged with the explicit assumption `ASSUME_FUTEX_WAKE_ONLY`.
  - If contradictory wake counts occur (`matched_in_wake > exit.ret`), Katana flags `Limitation::InconsistentWakeCount` and marks the diagnosis `AMBIGUOUS`.

### 2.6 OTQ-6: Container & PID Namespace Correctness
* **Question:** How are PI-owner TIDs handled when Katana and the target reside in different PID namespaces?
* **Resolution in Katana:**
  - Target TIDs are resolved against the tracing host's root PID namespace.
  - When a container PID namespace mismatch is detected, Katana emits `LIM_PIDNS_MISMATCH` and reports `owner: unknown (PID namespace mismatch)` rather than printing an incorrect host TID.

### 2.7 OTQ-7: Syscall Restart Codes
* **Question:** What are the exact flows of restart return codes (`-ERESTART_RESTARTBLOCK`) at `sys_exit_futex` during signal interruptions?
* **Resolution in Katana:**
  - Syscall return codes are inspected in `handle_sys_exit_futex`.
  - Negative values indicating restarts (`-ERESTARTSYS`, `-ERESTART_RESTARTBLOCK`) are classified as `WaitOutcome::Interrupted` rather than completed wakeups.

### 2.8 OTQ-8: Zero Residual Kernel State
* **Question:** Is all kernel state (programs, maps, tracepoint links) guaranteed to clean up after process termination or SIGKILL?
* **Resolution in Katana:**
  - In-kernel tracepoints are managed via libbpf anonymous BPF links (`bpf_link`).
  - Kernel link fds close automatically on process exit, immediately detaching all tracepoint probes without leaving dangling kprobes or pinned BPF maps.

### 2.9 OTQ-9: NVMe / blk-mq Completion Contexts
* **Question:** Which execution context issues `sched_waking` for block I/O completions on NVMe and multi-queue block devices?
* **Resolution in Katana:**
  - Modern NVMe completions trigger in hardirq (MSI-X) or `softirq` context, meaning the `current` task during completion is an unrelated interrupted process.
  - Katana *never* attributes block wakeups to whatever arbitrary process was interrupted by the completion IRQ.
  - Causal attribution (**Rule BIO-1**) requires:
    1. The target thread voluntarily enters sleep with `in_iowait = true`.
    2. A matching `block:block_rq_issue` exists submitted from the target thread's own context (`submitter.tid == sleeping_thread.tid`).
    3. A corresponding `block:block_rq_complete` occurs with matching `(dev_id, req_id)` during the sleep window.
  - If a thread was blocked during device activity but the submitter was a background flusher or `kworker`, Katana downgrades attribution to **Rule BIO-2** (`CORRELATED`) with the mandatory disclaimer:
    > *"This trace does not establish a causal link."*

### 2.10 OTQ-10: Request Pointer & Identity Reuse
* **Question:** Over what duration does the Linux block layer reuse `struct request` pointers or synthetic request identities on high-throughput NVMe drives?
* **Resolution in Katana:**
  - Request matching is bound to open intervals bounded by per-CPU sequence monotonicity and timestamp checks (`t_issue <= t_complete <= t_wake + ε`).
  - Request IDs are purged from open tracking immediately upon completion pairing.
  - Any sequence gap or ring buffer drop within the interval invalidates causal linkage, degrading completeness to `LOSSY`.

### 2.11 OTQ-11: Overhead of `block_rq_*` on High-IOPS Devices
* **Question:** Does system-wide tracing of `block:block_rq_issue` and `block:block_rq_complete` induce acceptable overhead under saturation workloads?
* **Resolution in Katana:**
  - `block_rq_issue` checks the tracked hash map (`is_tracked(tid)`) before ring buffer reservation, ensuring only target process requests enter the buffer.
  - `block_rq_complete` records completions with minimal 20-byte payloads.
  - If ring buffer drops occur, Katana's loss ledger records `reserve_fail_total` and emits explicit `Lossy` completeness.

### 2.12 OTQ-12: `sched_waking` vs `sched_wakeup` Ordering
* **Question:** Is `sched_waking` always emitted before `sched_wakeup` and for every wake path?
* **Resolution in Katana:**
  - `sched_waking` fires on the waker's CPU while the waker holds context, making it the normative source for waker attribution.
  - `sched_wakeup` fires when the wakee task is placed onto the target CPU runqueue.
  - Katana tracks runqueue queueing delay starting from the waking/wakeup timestamp until the next `sched_switch` onto CPU (Rule SW-2).

### 2.13 OTQ-13: Hook Cost of Early-Exit Handlers
* **Question:** What is the overhead of system-wide tracepoint hooks when target processes execute under extreme context switch rates?
* **Resolution in Katana:**
  - Every BPF probe begins with an inline `is_tracked(tid)` BPF hash map lookup.
  - Non-target events exit within < 15 nanoseconds without reserving ring buffer memory.

### 2.14 OTQ-14: CR-1 Background Context Thresholds
* **Question:** Are thresholds for Rule CR-1 (PSI CPU pressure ≥ 10%, runqueue ≥ 2× CPUs) defensible as causal indicators?
* **Resolution in Katana:**
  - Context signals are strictly categorized as `EvidenceClass::Correlated` (never `Causal`).
  - Rendered output always attaches the mandatory statutory disclaimer:
    > *"This trace does not establish a causal link."*

---

## 3. Subsystem Limitations & Non-Claims

| Subsystem / Scenario | Katana Finding | Evidence Class | Stated Limitation |
|---|---|---|---|
| **Non-PI Futex Contention** | `FutexWakeChain` | Causal / Derived | Plain futex words have no kernel owner field; Katana reports `owner: unknown (non-PI futex)` and never claims a thread "holds" or "owns" the lock. |
| **Futex2 Syscalls (`futex_waitv`)** | `BlockedUnattributed` | Weak / Unsupported | Syscall numbers (e.g. 449 on x86_64) are detected; reported with `Limitation::Futex2NotSupported` and `Completeness::Partial`. |
| **Timer Sleeps (`nanosleep`, `select`)** | `BlockedUnattributed` | Weak / Unsupported | Voluntary sleep without futex/block events reports `"timer sleep / unattributed wait (waker unattributed)"` without naming false wakers. |
| **Unattributed I/O Wait** | `BlockedUnattributed` | Weak / Unsupported | A thread with `in_iowait = true` lacking request linkage reports `"unattributed iowait (device unknown)"`. *The `in_iowait` flag alone never names a device.* |
| **Writeback / Flush Daemons** | `BlockIoCorrelated` | Correlated / Statistical | Asynchronous writes completed by `kworker` are strictly correlated; causal links are barred. |
| **Chain Depth > 8** | `FutexWakeChain` | Truncated | Chains beyond 8 hops terminate with `DEPTH_LIMIT` and `Completeness::Partial`; never hallucinates a root cause. |
| **32-Bit Compat on 64-Bit Host** | `BlockedUnattributed` | Unsupported | 32-bit pointer/futex layouts (`compat_sys_futex`) are flagged as unsupported attribution. |
| **PID Namespace Mismatch** | `owner: unknown` | Unsupported | When target and Katana reside in disjoint PID namespaces, PI owner TIDs cannot be resolved portably and are marked `LIM_PIDNS_MISMATCH`. |

---

## 4. Operational Prerequisites

1. **Privileges:** Live tracing requires root or `CAP_BPF` + `CAP_PERFMON` (Linux kernel ≥ 5.8). Unprivileged invocations safely exit with code 10 (`INSUFFICIENT_PRIVS`).
2. **Replay Mode:** Fully functional in unprivileged user-space via `--replay <trace.json>` or `--replay <bundle.katana>`.
