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

## Current Status (Phase 1 MVP)

Phase 1 MVP is **implemented and tested**:
- **Core Pipeline:** Wire decoding, normalizer, and per-CPU sequence gap loss ledger ([`src/events.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/events.rs)).
- **Futex & Scheduler Instrumentation:** Op decoding, private/shared futex keys, PI snapshot word decoding ([`src/futex.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/futex.rs), [`src/scheduler.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/scheduler.rs)).
- **Causal Rule Catalog:** Rule **FW-1** (futex wake attribution), FW-2, WK-1, FB-1, PI-1, SW-1, SW-2, and CR-1 ([`src/evidence.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/evidence.rs), [`src/causal_rules.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/causal_rules.rs)).
- **Causal Graph & Chains:** Multi-hop backward walk, depth cap (8), and cycle detection ([`src/graph.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/graph.rs)).
- **Diagnosis Engine:** 8-criterion lexicographic ranking model ([`src/diagnosis.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/diagnosis.rs)).
- **Anti-Inflation Renderer:** Allowed-verb table preventing claim inflation ([`src/renderer.rs`](file:///Users/atharvamendhulkar/desktop/katana/src/renderer.rs)).
- **JSON Schema:** Validated against [`schema/report.v1.json`](file:///Users/atharvamendhulkar/desktop/katana/schema/report.v1.json).
- **Test Suite:** 17/17 tests passing across unit, fault injection, and anti-inflation suites.

See [`docs/PHASE1_MVP.md`](file:///Users/atharvamendhulkar/desktop/katana/docs/PHASE1_MVP.md) for detailed technical specifications and [`prd.md`](file:///Users/atharvamendhulkar/desktop/katana/prd.md) for full requirements.

---

## Quick Start

### Build & Test

```bash
# Build binary and library
cargo build

# Run complete test suite (FW-1, fault injection, anti-inflation invariants)
cargo test
```

### Usage

```bash
# Analyze a target process by PID (default 3000ms window)
katana explain 4217

# Target specific thread with custom duration
katana explain 4217 --tid 4220 --duration 2000ms

# Emit machine-readable JSON adhering to schema/report.v1.json
katana explain 4217 --json

# Replay a recorded trace file
katana explain 4217 --replay fixtures/contention.json
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

## Limitations

For known operational boundaries, see [`docs/PHASE1_MVP.md`](file:///Users/atharvamendhulkar/desktop/katana/docs/PHASE1_MVP.md) and [`prd.md`](file:///Users/atharvamendhulkar/desktop/katana/prd.md) §10.6, §12.5, and §16.3.
