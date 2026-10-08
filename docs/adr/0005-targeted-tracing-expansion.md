# ADR-005: Targeted PID Tracing with In-Kernel Expansion

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Tracing all scheduler events across all CPUs creates massive event floods, ring buffer overflows, and high CPU overhead on busy hosts.

## Alternatives Considered
- **System-wide tracing with userspace filtering:** Generates tens of millions of events per second on 64-core hosts.
- **cgroup-only tracing:** Useful, but restricts tracing when the target spans multiple cgroups or when cgroup v2 hierarchy is flat.

## Decision
Use an in-kernel BPF hash map of tracked TIDs. Seed it with the target PID and all thread TIDs from `/proc/<pid>/task/`. In the `sched_waking` probe, dynamically expand tracking to the waker thread when a tracked thread is awakened, bounded by expansion depth and map capacity caps.

## Consequences
- Reduces ring buffer traffic by orders of magnitude.
- Unawakened threads outside the target's direct causal graph are completely ignored by the ring buffer.
