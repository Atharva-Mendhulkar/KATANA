# ADR-004: Stable Tracepoints over Kernel Kprobes

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Kernel internal functions can be inlined or modified across kernel minor revisions, causing kprobes to fail or behave inconsistently.

## Alternatives Considered
- **kprobes / fentry on internal functions:** E.g., `do_futex`, `futex_wait_queue`, `try_to_wake_up`. Vulnerable to kernel refactorings (e.g., futex rework in Linux 5.16).

## Decision
Rely exclusively on stable kernel tracepoints (`sched:sched_switch`, `sched:sched_waking`, `syscalls:sys_enter/exit_futex`, `block:block_rq_*`). Audit all programs via `scripts/check-scope.sh`.

## Consequences
- High stability and portability across kernels 5.15–6.12+.
- Internal futex hash buckets and lock structures cannot be inspected directly; accepted trade-off documented in `docs/LIMITATIONS.md`.
