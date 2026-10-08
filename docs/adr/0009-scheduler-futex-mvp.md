# ADR-009: Scheduler and Futex Scope for Phase 1 MVP

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Attempting to trace all kernel subsystems (network sockets, block devices, IPC pipes, page faults) simultaneously in Phase 1 creates intractable complexity, scope creep, and unverified attribution heuristics.

## Alternatives Considered
- **Broad multi-subsystem MVP:** Instrumenting VFS, TCP, block I/O, and locks in a single release.

## Decision
Confine Phase 1 MVP strictly to the scheduler subsystem (`sched_switch`, `sched_waking`, `sched_process_*`) and futex syscalls (`sys_enter/exit_futex`). Any off-CPU blocks outside futexes are classified as `BlockedUnattributed`.

## Consequences
- Solid, verifiable foundation with 100% causal precision on synchronization bottlenecks.
- Block I/O and external subsystems systematically scheduled for Phase 2.
