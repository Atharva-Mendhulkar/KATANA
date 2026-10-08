# ADR-006: Deterministic Lexicographic Ranking over ML/LLMs

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Diagnostic tools in infrastructure and systems debugging must be trustworthy, reproducible, and verifiable. Non-deterministic or probabilistic outputs undermine root-cause analysis.

## Alternatives Considered
- **LLM summarization / scoring:** Prone to hallucination, non-deterministic phrasing, and overclaiming ("invented confidence").
- **Trained classifiers (ML):** Fragile across differing workload profiles and opaque to operational engineers.

## Decision
Employ a deterministic rule catalog with lexicographical ranking tuples: `(explained_fraction, has_causal_edge, is_direct_subject, -hop_count, weakest_strength)`. Phrasing is generated strictly via fixed templates gated by an allowed-verb table.

## Consequences
- Guaranteed bit-for-bit replay determinism across identical input traces.
- Transparent explanations linked directly to specific event sequence IDs.
