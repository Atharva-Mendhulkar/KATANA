# ADR-015: Continuous Watch Mode with Bounded Retention Buffer

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 3 Implementation

## Context
While fixed-window post-incident profiling (`katana explain <PID> --duration 3000ms`) is effective when anomalies are continuous, transient latency spikes (p99 tail latency outliers occurring sporadically) require continuous observation. However, unbounded event collection consumes unbounded memory, risking out-of-memory crashes on production servers.

## Alternatives Considered
1. **Unbounded Event Accumulation:** Append all kernel trace events to memory until a spike occurs. Unviable on production hosts.
2. **Periodic Polling:** Wake every $N$ seconds, sample `/proc/<pid>/wchan`. Misses sub-second transient stalls and lacks tracepoint causality.

## Decision
Implement a continuous watcher (`katana watch <PID>` via `katana::watcher::Watcher`):
- **Circular Retention Buffer:** Holds a memory-bounded sliding window of recent tracepoint events (`VecDeque<Event>`, default capacity 5,000 events).
- **Latency Threshold Trigger:** Monitors thread off-CPU intervals (`sched_switch`, `sched_waking`, and `futex_wait`) in real-time. When a blocking duration exceeds `--threshold-ms` (default 50 ms), the watcher captures the retained causal window and executes the deterministic `Engine::analyze_with_identity`.
- **Sliding Window Diagnosis:** Emits an immediate episode diagnosis (terminal text, JSON line, or static HTML report).
- **Continuous Operation:** Resumes monitoring immediately after reporting without leaking memory or thread descriptors.

## Consequences
- Enables continuous monitoring for tail latency spikes with strictly constant memory footprint.
- Tested and verifiable via deterministic unprivileged replay event streams (`--replay <file>`).
