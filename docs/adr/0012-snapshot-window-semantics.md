# ADR-012: Snapshot-Plus-Window Temporal Semantics

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
A thread may already be sleeping when Katana starts tracing. Tracing cannot observe events that occurred prior to collection start.

## Alternatives Considered
- **Retroactive inference:** Guessing the start of a wait by reading `/proc/<tid>/wchan` or extrapolating timestamps.

## Decision
Establish explicit temporal window boundaries:
- Take initial `/proc` snapshot at $T_{\text{start}}$ and final snapshot at $T_{\text{end}}$.
- Any thread that was already asleep before $T_{\text{start}}$ without an observed entry transition is flagged as `HistoryBeforeTracking`.
- Snapshots provide `Observed` contextual evidence only; they never generate `Causal` edges.

## Consequences
- Prevents inventing causal links for pre-existing wait states.
- Truncation boundaries are explicit in reports (`completeness: Partial` or `Truncated`).
