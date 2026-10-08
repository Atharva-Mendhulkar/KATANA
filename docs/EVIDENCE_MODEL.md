# Katana Evidence Model & Allowed-Verb Specification

**Document Version:** 1.0  
**Reference:** [PRD §13 (Evidence Model)](file:///home/topfloorboss/Desktop/KATANA/prd.md#L664), [PRD §15 (Explanation Engine)](file:///home/topfloorboss/Desktop/KATANA/prd.md#L777), [PRD §29](file:///home/topfloorboss/Desktop/KATANA/prd.md#L1378)

---

## 1. Core Principle: Structural Anti-Inflation

Katana enforces a strict mathematical monotonicity property across all evidence generation:
> **No analysis step, rendering template, or deletion of trace events may ever elevate an evidence item's class or strength.**

Every claim emitted by Katana traces to an explicit rule in the normative rule catalog with a fixed, type-level `max_class`.

---

## 2. Evidence Classification Hierarchy

```text
       Causal (Direct mechanism demonstrated)
          ▲
          │ [Monotonic Clamp: Downgrades Only]
          │
       Observed (Direct kernel event recorded without causal link)
          ▲
          │
      Correlated (Temporal or statistical overlap; non-causal)
```

| Class | Definition | Allowed Claim |
|---|---|---|
| **Causal** | The recorded kernel event directly caused the target thread's state transition (e.g. `sched_waking` released a thread waiting on the matching futex key; matching block I/O request completed). | "released by", "blocked on", "woke" |
| **Observed** | A concrete kernel event occurred on the CPU or target, but causality is incomplete or unverifiable (e.g. runqueue delay, preemption by higher priority task, unverified futex key). | "preempted by", "experienced runqueue delay of", "observed owner TID" |
| **Correlated** | Phenomena coincided in time with blocking, but no causal link exists in the trace (e.g. system CPU saturation, background writeback I/O). | "coincided with", "experienced blocking during an interval in which...". **Must include:** *"This trace does not establish a causal link."* |

---

## 3. Allowed-Verb Table (PRD §15.1)

| Evidence Class | Permitted Verbs & Phrasing | Strictly Forbidden Verbs |
|---|---|---|
| **Causal** | `blocked on`, `released by`, `woke`, `unblocked by` | `owns the lock`, `holds the mutex` (for non-PI futexes) |
| **Observed** | `preempted by`, `experienced delay`, `observed TID` | `caused by`, `blocked by`, `responsible for` |
| **Correlated** | `coincided with`, `exhibited activity during`, `experienced blocking while...` | `because`, `due to`, `caused`, `slowed down by`, `responsible` |

Any rendering output containing forbidden phrasing fails automated CI checks (`tests/anti_inflation_tests.rs`).

---

## 4. Rule Catalog

| Rule ID | Name | Subsystem | Max Class | Description |
|---|---|---|---|---|
| **FW-1** | Direct Futex Wake | Futex | `Causal` | In-syscall `sched_waking` matches sleeping waiter on private futex key and bitset; `ret` reconciles with wake count. |
| **FW-2** | Shared Futex Wake | Futex | `Causal` | In-syscall wake on shared memory address; key verified as shared, waker tracked. |
| **WK-1** | Direct Wakeup | Scheduler | `Causal` | Direct wake interval containment linking waker and wakee. |
| **WK-2** | Unrelated Waker | Scheduler | `Observed` | Waker was in interrupt context or outside tracked process boundary. |
| **FB-1** | Futex Bitset Disjoint | Futex | `Observed` | Wake bitset and wait bitset had empty intersection; wake did not pair. |
| **PI-1** | PI Owner Snapshot | Futex | `Observed` | Snapshot of PI futex word low 30 bits naming owner TID at syscall entry. |
| **SW-1** | Off-CPU Voluntary Switch | Scheduler | `Causal` | Thread voluntarily switched out to sleep or wait state (`prev_state != Running`). |
| **SW-2** | Runqueue Contention | Scheduler | `Observed` | Latency between `sched_waking` and subsequent `sched_switch` onto CPU. |
| **CR-1** | Host Pressure Correlation | Metrics | `Correlated` | Elevated PSI or host CPU saturation coinciding with blocking interval. |
| **BIO-1** | Direct Sync Block I/O | Block I/O | `Causal` | Request issued in target context (`submitter == waiter`), completion paired with matching `(dev_id, req_id)`. |
| **BIO-2** | Writeback / Unattributed I/O | Block I/O | `Correlated` | Asynchronous writeback submitted by `kworker`; strictly non-causal. |

---

## 5. Evidence Basis & Strength

- **Basis:**
  - `Direct`: Exact kernel tracepoint event payload directly observed.
  - `Derived`: Deduced from interval containment across multiple ordered trace events.
  - `Statistical`: Aggregated distribution or rate over the observation window.
  - `Snapshot`: State sampled at window boundaries (e.g. `/proc/<pid>` or PI word).
- **Strength:**
  - `Strong`: Zero loss, complete provenance, direct linkage.
  - `Moderate`: Derived linkage under documented invariants (e.g. `ASSUME_FUTEX_WAKE_ONLY`).
  - `Weak`: Correlated activity, truncated chain, or unverified keys.
