# ADR-008: Refusal of Non-PI Futex Mutex Ownership Inference

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Standard Linux futexes (`FUTEX_WAIT`, `FUTEX_WAKE`) store arbitrary user-space integer values. The Linux kernel does not track which thread owns or holds a regular futex.

## Alternatives Considered
- **Heuristic ownership guesses:** E.g., guessing the owner based on the last waker or user-space thread priority.
- **glibc internal symbol probing / uprobes:** Extremely fragile across libc versions (glibc, musl), compiler optimizations, and custom synchronization primitives (Go channels, Rust parking_lot).

## Decision
Katana explicitly refuses to infer mutex ownership for non-PI futexes. Ownership is reported strictly as:
`owner: unknown (non-PI futex)`
Priority Inheritance (`FUTEX_LOCK_PI`) futexes, which encode the kernel owner TID in the low bits of the futex word, are read and reported as snapshots at wait-entry.

## Consequences
- Protects users from false ownership diagnoses.
- Enforced by golden tests and grep lints across all output.
