# Katana Limitations & Open Technical Questions (OTQ) Review

**Document Version:** 1.0 (Phase 2)  
**Applicability:** Linux 5.15–6.x (x86_64, arm64)  
**Reference:** [PRD §23, §32, §36](file:///home/topfloorboss/Desktop/KATANA/prd.md)

---

## 1. Overview

Katana operates under a strict **structural no-inflation** principle: it only claims causality when supported by kernel semantics and verified invariant linkage. When observation boundaries, kernel versions, or execution contexts prevent definitive attribution, Katana reports limitations explicitly rather than conjecturing causes.

This document catalogs known gaps, design limitations, and the resolution of Phase 2 Open Technical Questions (**OTQ-9–11**).

---

## 2. Phase 2 Open Technical Questions Resolution

### 2.1 OTQ-9: NVMe / blk-mq Completion Contexts
* **Question:** Which execution context issues `sched_waking` for block I/O completions on NVMe and multi-queue block devices (hardirq, softirq/`ksoftirqd`, or threaded worker)?
* **Finding & Architecture:** Modern NVMe completions typically trigger in hardirq (MSI-X) or `softirq` context, meaning the `current` task during completion is an unrelated interrupted process.
* **Resolution in Katana:** 
  - Katana *never* attributes block wakeups to whatever arbitrary process was interrupted by the completion IRQ.
  - Causal attribution (**Rule BIO-1**) requires:
    1. The target thread voluntarily enters sleep with `in_iowait = true`.
    2. A matching `block:block_rq_issue` exists submitted from the target thread's own context (`submitter.tid == sleeping_thread.tid`).
    3. A corresponding `block:block_rq_complete` occurs with matching `(dev_id, req_id)` during the sleep window.
  - If a thread was blocked during device activity but the submitter was a background flusher or `kworker`, Katana downgrades attribution to **Rule BIO-2** (`CORRELATED`) with the mandatory disclaimer:
    > *"This trace does not establish a causal link."*

### 2.2 OTQ-10: Request Pointer & Identity Reuse
* **Question:** Over what duration does the Linux block layer reuse `struct request` pointers or synthetic request identities on high-throughput NVMe drives?
* **Finding & Architecture:** On high-IOPS devices (100k+ IOPS), request pointers or tag IDs can be recycled within sub-millisecond windows.
* **Resolution in Katana:**
  - Request matching is bound to open intervals bounded by per-CPU sequence monotonicity and timestamp checks (`t_issue <= t_complete <= t_wake + ε`).
  - Request IDs are purged from open tracking immediately upon completion pairing.
  - Any sequence gap or ring buffer drop within the interval invalidates causal linkage, degrading completeness to `LOSSY`.

### 2.3 OTQ-11: Overhead of `block_rq_*` on High-IOPS Devices
* **Question:** Does system-wide tracing of `block:block_rq_issue` and `block:block_rq_complete` induce acceptable overhead under saturation workloads?
* **Finding & Architecture:** Unfiltered system-wide block tracing on multi-million IOPS drives can produce heavy ring buffer pressure.
* **Resolution in Katana:**
  - `block_rq_issue` checks the tracked hash map (`is_tracked(tid)`) before ring buffer reservation, ensuring only target process requests enter the buffer.
  - `block_rq_complete` records completions with minimal 20-byte payloads.
  - If ring buffer drops occur, Katana's loss ledger records `reserve_fail_total` and emits explicit `Lossy` completeness.

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
2. **Replay Mode:** Fully functional in unprivileged user-space via `--replay <trace.json>`.
