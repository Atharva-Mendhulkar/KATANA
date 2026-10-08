# ADR-007: Explicit Evidence Classes and Anti-Inflation Types

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
A major failure mode of latency diagnosis tools is confusing correlation (e.g. high CPU load or background I/O) with true causation (direct locks or synchronization waits).

## Alternatives Considered
- **Scalar confidence scores (e.g., 0.0 to 1.0):** Users interpret arbitrary floats as ground truth, and tools tend to inflate confidence arbitrarily.

## Decision
Categorize all diagnostic facts into three mutually exclusive evidence classes:
1. `Causal`: Mechanism is fully observed and proven by kernel tracepoints (e.g., matching futex key and waking context, or matching direct block I/O request ID).
2. `Correlated`: Temporal or statistical co-occurrence without verified causation. Mandatory disclaimer required: *"This trace does not establish a causal link."*
3. `Observed`: Contextual system state (e.g. `/proc` snapshots).

Enforce monotonic classification ceilings in Rust types (`max_class` on rules; downgraded when events are missing or loss occurs).

## Consequences
- Impossible to inflate a correlated observation into a causal claim.
- Output text is strictly linted and verified by tests (`tests/anti_inflation_tests.rs`).
