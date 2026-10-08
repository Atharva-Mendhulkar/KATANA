# ADR-010: Deferral of Block I/O Subsystem to Phase 2

- **Status:** Superseded by [ADR-013](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0013-phase2-block-io-attribution.md)
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Block I/O attribution is fundamentally asynchronous due to the page cache, writeback flusher threads (`kworker`), direct I/O, and layered block devices. Releasing block I/O in Phase 1 without rigorous causal rules risked conflating correlation with causation.

## Alternatives Considered
- Include block I/O in Phase 1 with correlation-only labelling.

## Decision
Defer all block I/O tracepoint hooks (`block_rq_issue`, `block_rq_complete`) to Phase 2, resolving open questions OTQ-9, OTQ-10, and OTQ-11 beforehand. In Phase 1, `in_iowait` blocks are reported as unattributed without naming devices.

## Consequences
- Allowed Phase 1 to ship with zero false-causality risks.
- Paved the way for Phase 2 implementation under ADR-013.
