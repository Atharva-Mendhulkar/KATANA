# ADR-013: Phase 2 Block I/O Attribution Rules (BIO-1 and BIO-2)

- **Status:** Accepted (Supersedes [ADR-010](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0010-block-io-deferral.md))
- **Deciders:** Katana Engineering Team
- **Date:** Phase 2 Implementation

## Context
With Phase 1 accepted and open technical questions OTQ-9, OTQ-10, and OTQ-11 evaluated, Katana requires normative attribution rules for block storage devices across direct synchronous I/O and asynchronous page cache writeback.

## Attribution Problem
The task in `block_rq_issue` is the *submitter*, not necessarily the *waiter*. Buffered writes are submitted by `kworker` flushers or journal daemons (`jbd2`) long after the originating process wrote to the page cache.

## Decision
Implement two normative, type-enforced attribution rules:
1. **Rule BIO-1 (Direct Synchronous I/O):**
   - **Preconditions:**
     a. Submitter task context matches the waiting thread (`submitter_tid == waiter_tid`).
     b. Request issued during waiter's `in_iowait` sleep interval.
     c. `block_rq_complete` matches request ID and sector.
     d. Completion followed by `sched_waking(waiter)` in completion context without event loss.
   - **Classification:** `EvidenceClass::Causal`, `Strength::Derived`. Finding: `BlockIoWait`. Allowed verb: *"Thread X blocked on device D"*.
2. **Rule BIO-2 (Asynchronous Writeback):**
   - **Preconditions:** Submitter is a background kernel thread (e.g. `kworker`), but target thread was blocked in `in_iowait` during completion.
   - **Classification:** `EvidenceClass::Correlated`. Finding: `BlockIoCorrelated`.
   - **Mandatory Disclaimer:** *"This trace does not establish a causal link."*
3. **Device Naming Invariant:**
   - The `in_iowait` flag alone never names a device. Unattributed I/O sleep is reported as `FindingKind::BlockedUnattributed`.

## Consequences
- Guarantees that Katana never claims a user process "caused" writeback I/O.
- Enforced at compile time, in unit tests (`tests/block_io_tests.rs`), and in renderer output linter.
