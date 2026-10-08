# Katana

> **Deterministic Latency Root-Cause Analysis for Linux**
>
> Katana is a single-host, bounded-cost tool that diagnoses thread blocking and latency anomalies using kernel tracepoint instrumentation and an evidence-explicit causal graph engine.

---

## What Katana Does

When a thread or process experiences unexpected latency, Katana reconstructs why the thread was blocked and who released it:
1. **Reconstructs Wakeup Causality:** Identifies the exact waker thread and futex key using kernel tracepoint interval containment (`sched:sched_waking`, `sched:sched_switch`, and `syscalls:sys_enter/exit_futex`).
2. **Explicit Causal vs. Correlated Classification:** Never confuses system pressure (e.g. CPU saturation or runqueue depth) with causal mechanisms. Causal claims are emitted exclusively under kernel semantics rules.
3. **Multi-Hop Wake Chains:** Walks backward through chains of sleeping and waking threads up to a bounded depth of 8 hops, detecting wake cycles and thread transitions.
4. **Honest Handling of Loss & Truncation:** Treats event drops and sequence gaps as first-class outputs, degrading diagnosis completeness rather than hallucinating findings.
5. **No Mutex Ownership Claims for Non-PI Futexes:** Because plain futex words have no kernel-defined owner field, Katana never claims a thread "holds" or "owns" a non-PI futex.

---

## Prior Art & Positioning

Katana builds directly on foundations established by prior systems research:
- **Prism (OSDI '21):** Thread dependency tracking and wait-graph modeling.
- **BCC offwaketime / perf sched:** Off-CPU analysis and `sched_waking` waker attribution.
- **Linux Kernel eBPF Infrastructure:** Low-overhead tracepoints and bounded memory ring buffers.

Katana focuses on single-command, bounded-overhead post-incident diagnosis on production hosts. It produces human-readable and machine-verifiable diagnosis reports traceable directly to raw event provenance without background daemon overhead.

---

## Current Status (Phase 2 Implemented & Validated)

Katana has transitioned to **Phase 2** with block I/O attribution, eBPF in-kernel programs, and unsupported attribution guards:
- **Block I/O Subsystem (PRD §32.1):**
  - **Rule BIO-1:** Causal attribution for direct synchronous block I/O (`O_DIRECT`/`fsync`) with submitter context match, completion timestamp alignment, and request ID pairing.
  - **Rule BIO-2:** Structural clamp for asynchronous writeback (`kworker`/flushers); strictly classified as `CORRELATED` with mandatory disclaimer: *"This trace does not establish a causal link."*
  - **Sysfs Device Resolver:** Resolves `dev_t` to device leaf names (`sda1`, `nvme0n1`) via `/sys/dev/block/<major>:<minor>`.
- **eBPF In-Kernel Tracing (`bpf/`):**
  - Tracepoint hooks for `sched:sched_switch`, `sched:sched_waking`, `syscalls:sys_enter/exit_futex`, `block:block_rq_issue`, and `block:block_rq_complete`.
  - Zero-dependency binary wire decoder (`Event::decode_raw`) for packed kernel frames.
  - Formally validated by `scripts/check-scope.sh` against the normative kernel hook allow-list.
- **Target Lifecycle & Safeguards:**
  - Initial/final `/proc/<pid>` snapshotting, `--tid` validation, privilege preflight (exit 10 for unprivileged live runs), and PID reuse verification (exit 4).
- **Unsupported & Negative Controls:**
  - Futex2 syscall detection (`sys_enter_futex_waitv`) reporting `Limitation::Futex2NotSupported` and `Completeness::Partial`.
  - Negative control 4b timer sleep (`nanosleep`) reporting `BlockedUnattributed` without false waker claims.
  - Unattributed `in_iowait` states report uninstrumented wait without inventing device names.
- **Replay Fixtures:** Committed golden traces in `fixtures/` (`bio1_sync_io.json`, `bio2_writeback.json`).
- **Test Suite:** **29/29 tests passing** across 4 test suites (`block_io_tests`, `fault_injection_tests`, `fw1_tests`, `anti_inflation_tests`).

See [`docs/PHASE1_MVP.md`](file:///home/topfloorboss/Desktop/KATANA/docs/PHASE1_MVP.md) and [`docs/PHASE2_LOG.md`](file:///home/topfloorboss/Desktop/KATANA/docs/PHASE2_LOG.md) for implementation logs, and [`docs/LIMITATIONS.md`](file:///home/topfloorboss/Desktop/KATANA/docs/LIMITATIONS.md) for OTQ review and design boundaries.

---

## Quick Start

### Build & Test

```bash
# Build binary and library
cargo build

# Run complete test suite (29 tests)
cargo test

# Verify eBPF scope guard
./scripts/check-scope.sh
```

### Usage

```bash
# Analyze a target process by PID (default 3000ms window, requires root/CAP_BPF)
katana explain 4217

# Target specific thread with custom duration
katana explain 4217 --tid 4220 --duration 2000ms

# Emit machine-readable JSON adhering to schema/report.v1.json
katana explain 4217 --json

# Replay a recorded trace file (unprivileged user space)
katana explain --replay fixtures/bio1_sync_io.json --json
katana explain --replay fixtures/bio2_writeback.json
```

---

## Example Output

### Human-Readable Format
```text
Diagnosis: FutexWakeChain
Thread 4217 was blocked in futex wait for 1000 ms. It was released by a futex wake from TID 4221. Terminal waker was running on CPU before wake.
Lock status: owner: unknown (non-PI futex)
```

### JSON Format (`--json`)
```json
{
  "$schema": "https://katana.dev/schema/report.v1.json",
  "target": {
    "tgid": 4217,
    "pid": 4217,
    "comm": "target",
    "start_time_ticks": 123456,
    "boot_id": "00000000-0000-0000-0000-000000000000"
  },
  "window": {
    "duration_ns": 1000000000,
    "t_start_ns": 1000000000,
    "t_end_ns": 2000000000
  },
  "diagnosis": {
    "status": "Found",
    "primary": {
      "kind": "FutexWakeChain",
      "subject": { "tid": 4217, "tgid": 4217 },
      "blocked_duration_ns": 1000000000,
      "explained_fraction_per_mille": 1000,
      "weakest_strength": "Moderate",
      "has_causal_edge": true,
      "is_direct_subject": true,
      "hop_count": 1
    },
    "alternatives": [],
    "context": [],
    "completeness": "Complete",
    "limitations": []
  },
  "evidence": [ ... ],
  "stats": {
    "events_received": 15,
    "events_lost": 0,
    "reserve_fail_total": 0
  }
}
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Diagnosis produced (`Found`, `NotBlocked`) |
| `1` | Internal error |
| `2` | Usage / argument parsing error |
| `3` | `Unknown` (no cause identified in window) |
| `4` | `Ambiguous` or `Invalid` (loss or contradiction detected) |
| `10` | Insufficient privileges (`CAP_BPF` / `CAP_PERFMON` required) |
| `11` | Target PID not found / not visible |
| `14` | Target exited before trace collection began |

---

## Documentation Index

Per [prd.md §29](file:///home/topfloorboss/Desktop/KATANA/prd.md), Katana maintains dedicated documentation artifacts:

| Document | Purpose |
|---|---|
| [`docs/ARCHITECTURE.md`](file:///home/topfloorboss/Desktop/KATANA/docs/ARCHITECTURE.md) | Component data flow, thread model, and wire event specifications |
| [`docs/DEVELOPMENT.md`](file:///home/topfloorboss/Desktop/KATANA/docs/DEVELOPMENT.md) | Build dependencies, compilation, capabilities, and dev workflow |
| [`docs/TESTING.md`](file:///home/topfloorboss/Desktop/KATANA/docs/TESTING.md) | Fault injection test suites (Tests 1–8, BIO-1/2) and reproducibility |
| [`docs/EVIDENCE_MODEL.md`](file:///home/topfloorboss/Desktop/KATANA/docs/EVIDENCE_MODEL.md) | Evidence taxonomy, allowed-verb vocabulary, and anti-inflation types |
| [`docs/KERNEL_COMPATIBILITY.md`](file:///home/topfloorboss/Desktop/KATANA/docs/KERNEL_COMPATIBILITY.md) | Kernel version compatibility matrix across 5.15–6.12+ |
| [`docs/LIMITATIONS.md`](file:///home/topfloorboss/Desktop/KATANA/docs/LIMITATIONS.md) | Operational boundaries, OTQ experimental log, and non-claims |
| [`benchmarks/BENCHMARKS.md`](file:///home/topfloorboss/Desktop/KATANA/benchmarks/BENCHMARKS.md) | Methodology, hardware characterization, and trial results |
| [`docs/adr/`](file:///home/topfloorboss/Desktop/KATANA/docs/adr/README.md) | Immutable Architecture Decision Records (ADR-001 through ADR-013) |
| [`docs/PHASE1_MVP.md`](file:///home/topfloorboss/Desktop/KATANA/docs/PHASE1_MVP.md) | Phase 1 milestone log and implementation archive |
| [`docs/PHASE2_LOG.md`](file:///home/topfloorboss/Desktop/KATANA/docs/PHASE2_LOG.md) | Phase 2 implementation log and roadmap progression |

---

## Limitations

For known operational boundaries, see [`docs/LIMITATIONS.md`](file:///home/topfloorboss/Desktop/KATANA/docs/LIMITATIONS.md), [`docs/PHASE1_MVP.md`](file:///home/topfloorboss/Desktop/KATANA/docs/PHASE1_MVP.md), and [`prd.md`](file:///home/topfloorboss/Desktop/KATANA/prd.md) §10.6, §12.5, and §16.3.
