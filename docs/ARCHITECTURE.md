# Katana Architecture

> **Deterministic Latency Root-Cause Analysis for Linux**  
> Reference: [prd.md §7–§12](file:///home/topfloorboss/Desktop/KATANA/prd.md)

---

## 1. System Overview & Data Flow

Katana operates as a single-host, post-incident command-line diagnostic tool. It collects kernel tracepoints during a bounded temporal window (default 3,000 ms), stops collection, and executes a deterministic, pure-functional analysis pipeline offline.

```mermaid
flowchart TD
    TP[Target process: tgid + start_time] --> K[Linux Kernel]
    K --> S[sched tracepoints: switch · waking · fork · exit]
    K --> F[syscalls: sys_enter/exit_futex]
    K --> B[block tracepoints: block_rq_issue · block_rq_complete]
    S --> BP[eBPF In-Kernel Programs: filter + emit]
    F --> BP
    B --> BP
    BP --> M[(BPF Maps: tracked set · stats · seq)]
    BP --> R[[Ring Buffer: BPF_MAP_TYPE_RINGBUF]]
    R --> C[Collector Thread: batch drain]
    C -->|Bounded Channel| N[Event Normalizer: decode · sort · seq loss]
    N --> W[Wait-Interval Tracker: per-TID off-CPU / futex state]
    W --> G[Causal Graph Builder: FW-1, BIO-1, BIO-2 rules]
    G --> E[Evidence Engine: Causal vs. Correlated vs. Observed]
    E --> D[Diagnosis Engine: deterministic lexicographic ranking]
    D --> RND[Renderer: template-driven text with verb table]
    D --> OUT[Output: report.v1.json / CLI exit code]
    P[/proc Snapshot: Target Resolver] --> N
    P --> E
```

---

## 2. Component Specification

Katana's architecture strictly maps to the modular Rust components in `src/`:

| Component | Source File | Responsibility | Inputs | Outputs |
|---|---|---|---|---|
| **Target Resolver** | [`src/target.rs`](file:///home/topfloorboss/Desktop/KATANA/src/target.rs) | Live `/proc/<pid>` snapshotting, target identity `(tgid, start_time_ticks, boot_id)`, PID reuse validation (exit 4), privilege check (exit 10). | Target PID / TID, `/proc` | `TargetIdentity`, `ProcSnapshot` |
| **In-Kernel eBPF** | [`bpf/katana.bpf.c`](file:///home/topfloorboss/Desktop/KATANA/bpf/katana.bpf.c), [`bpf/katana.h`](file:///home/topfloorboss/Desktop/KATANA/bpf/katana.h) | In-kernel filtering against tracked PID hash map, fixed-size wire encoding, ring buffer emission, drop accounting. | Tracepoints | Ring buffer frames, map counters |
| **Wire Decoder & Normalizer** | [`src/events.rs`](file:///home/topfloorboss/Desktop/KATANA/src/events.rs) | Unpacks `kt_hdr` binary frames (`Event::decode_raw`), validates record bounds, orders events by timestamp `t_mono_ns`, tracks sequence gaps via `LossLedger`. | Raw byte buffers | `Vec<Event>`, `LossLedger` |
| **Futex Inspector** | [`src/futex.rs`](file:///home/topfloorboss/Desktop/KATANA/src/futex.rs) | Decodes futex operations (`FUTEX_WAIT`, `FUTEX_WAKE`, `FUTEX_LOCK_PI`), parses flags (private vs. shared), detects unsupported `futex_waitv` (futex2). | Syscall events | `FutexWaitWindow`, `FutexWakeWindow` |
| **Block I/O Subsystem** | [`src/block_io.rs`](file:///home/topfloorboss/Desktop/KATANA/src/block_io.rs) | Tracks bio/request lifecycles (`block_rq_issue` to `block_rq_complete`), sysfs device resolution (`/sys/dev/block/<major>:<minor>`), executes BIO-1 (direct sync) and BIO-2 (writeback correlation). | Block events, sysfs | Block intervals, device metrics |
| **Causal Graph & Rules** | [`src/causal_rules.rs`](file:///home/topfloorboss/Desktop/KATANA/src/causal_rules.rs) | Reconstructs wake chains (Rule FW-1), matches wakers within wait windows, enforces depth bounds (depth ≤ 8), cycles, and waker exclusions (IRQ/kthread). | Intervals, Events | `CausalGraph` (nodes & edges) |
| **Evidence Model** | [`src/evidence.rs`](file:///home/topfloorboss/Desktop/KATANA/src/evidence.rs) | Formal evidence classification (`Causal`, `Correlated`, `Observed`), strength grading (`Direct`, `Derived`, `Weak`), monotonic ceiling enforcement. | Causal edges, stats | `Vec<Evidence>` |
| **Diagnosis Engine** | [`src/diagnosis.rs`](file:///home/topfloorboss/Desktop/KATANA/src/diagnosis.rs) | Lexicographically ranks findings, detects contradictions (`Ambiguous`), handles negative controls (`NotBlocked`, `Unknown`). | Evidence set, Graph | `Diagnosis` |
| **Renderer** | [`src/renderer.rs`](file:///home/topfloorboss/Desktop/KATANA/src/renderer.rs) | Plain text output generation strictly governed by the allowed-verb table; enforces disclaimer injection for correlated findings. | `Diagnosis`, Evidence | Formatted stdout text |
| **Output & CLI** | [`src/output.rs`](file:///home/topfloorboss/Desktop/KATANA/src/output.rs), [`src/cli.rs`](file:///home/topfloorboss/Desktop/KATANA/src/cli.rs) | Schema-compliant JSON serialization (`schema/report.v1.json`), exit code dispatch (0, 1, 2, 3, 4, 10, 11, 14). | Diagnosis, CLI flags | JSON stdout, exit code |

---

## 3. Threading & Execution Model

Katana deliberately rejects complex asynchronous runtimes (e.g. Tokio) in favor of a **two-thread, offline analysis model** (ADR-011):

1. **Collector Thread:**
   - Dedicated thread owning the `BPF_MAP_TYPE_RINGBUF` consumer loop.
   - Polls kernel events with minimal processing, sending batches across a bounded `crossbeam_channel`.
   - Detects buffer pressure and records kernel drop counters (`reserve_fail_total`).

2. **Main Thread:**
   - Performs target identity validation and initial `/proc` snapshotting.
   - Sleeps for the requested trace window (default 3,000 ms).
   - Signals the collector thread to terminate, drains the event channel, and captures the final `/proc` snapshot.
   - Executes offline analysis: decoding, interval collation, causal graph traversal, and diagnosis generation.

**Advantage:** The entire analysis pipeline is a **pure function** of `(TargetIdentity, Snapshot, Vec<Event>, LossLedger)`. This guarantees 100% deterministic replay testing from recorded fixtures without requiring root privileges or live kernels.

---

## 4. eBPF Hook Allow-List

Katana adheres to a minimal, strictly verified set of normative tracepoints audited by [`scripts/check-scope.sh`](file:///home/topfloorboss/Desktop/KATANA/scripts/check-scope.sh):

| Tracepoint Subsystem | Hook Name | Purpose |
|---|---|---|
| `sched` | `sched_switch` | Task context switch, off-CPU sleep duration, preemption detection (`prev_state`). |
| `sched` | `sched_waking` | Normative waker attribution (fires in waker's CPU context). |
| `sched` | `sched_process_fork` | Child thread creation; dynamically expands target tracked set in kernel. |
| `sched` | `sched_process_exit` | Thread exit; removes TID from tracked map and detects target process termination. |
| `syscalls` | `sys_enter_futex`, `sys_exit_futex` | Futex operation code, futex word virtual address, bitset, and return code. |
| `syscalls` | `sys_enter_futex_waitv` | Futex2 detection; flags unsupported syscall with `Limitation::Futex2NotSupported`. |
| `block` | `block_rq_issue` | Submitter TID, device `dev_t`, sector address, and request size. |
| `block` | `block_rq_complete` | Completion timestamp, request duration, and return status for BIO-1/BIO-2 attribution. |

*Deliberately excluded:* `sched_process_exec`, `sched_migrate_task`, raw kprobes, fentry, and uprobes.

---

## 5. Causal Rules & Anti-Inflation Guarantees

Katana enforces a strict taxonomy of evidence classes:
- **`EvidenceClass::Causal`:** The mechanism is completely observed and established by kernel semantics.
  - **Rule FW-1 (Futex Wake):** Target slept in `FUTEX_WAIT`, waker issued `FUTEX_WAKE` matching the exact futex word and bitset, and `sched_waking` occurred inside the wake syscall context without event loss.
  - **Rule BIO-1 (Direct Sync Block I/O):** Target slept in `in_iowait`, submitted direct I/O (`O_DIRECT` or `fsync`), completion matched the request pointer/sector, and waker unblocked the target without loss.
- **`EvidenceClass::Correlated`:** Statistical or temporal co-occurrence without verified causation.
  - **Rule BIO-2 (Writeback Block I/O):** Asynchronous flushers (`kworker`) issued I/O during target sleep. Must include the disclaimer: *"This trace does not establish a causal link."*
  - **Rule CR-1 (CPU Runqueue Saturation):** Runqueue delay observed prior to thread execution.
- **`EvidenceClass::Observed`:** Contextual facts without causal claims (e.g., initial/final `/proc` states).

**Anti-Inflation Type Invariants:**
Rules declare an immutable `max_class`. The type system and unit tests (`tests/anti_inflation_tests.rs`) strictly prevent upgrading evidence (e.g. `Correlated` can never be upgraded to `Causal`).
