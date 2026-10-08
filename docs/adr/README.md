# Katana Architecture Decision Records (ADRs)

> Reference: [prd.md §35, §37](file:///home/topfloorboss/Desktop/KATANA/prd.md)

This directory contains the immutable record of technical and architectural decisions for Katana. Once accepted, ADRs are immutable; subsequent shifts require a new ADR that explicitly supersedes the prior decision.

| Number | Title | Status | Scope |
|---|---|---|---|
| [ADR-001](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0001-rust-userspace.md) | Rust Userspace Implementation | Accepted | Core Runtime |
| [ADR-002](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0002-c-ebpf-libbpf.md) | C eBPF with CO-RE and libbpf-rs Loader | Accepted | Kernel Layer |
| [ADR-003](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0003-shared-ring-buffer.md) | Shared BPF Ring Buffer over Perf Buffers | Accepted | Event Ingestion |
| [ADR-004](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0004-tracepoints-over-kprobes.md) | Stable Tracepoints over Kernel Kprobes | Accepted | Hook Selection |
| [ADR-005](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0005-targeted-tracing-expansion.md) | Targeted PID Tracing with In-Kernel Expansion | Accepted | Scoping & Overhead |
| [ADR-006](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0006-deterministic-diagnosis-over-llm.md) | Deterministic Lexicographic Ranking over ML/LLMs | Accepted | Diagnosis Engine |
| [ADR-007](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0007-evidence-classes-anti-inflation.md) | Explicit Evidence Classes and Anti-Inflation Types | Accepted | Evidence Model |
| [ADR-008](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0008-no-generic-mutex-ownership.md) | Refusal of Non-PI Futex Mutex Ownership Inference | Accepted | Futex Analysis |
| [ADR-009](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0009-scheduler-futex-mvp.md) | Scheduler and Futex Scope for Phase 1 MVP | Accepted | Phase 1 Scope |
| [ADR-010](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0010-block-io-deferral.md) | Deferral of Block I/O Subsystem to Phase 2 | Superseded | Phase 1 Scope |
| [ADR-011](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0011-offline-two-thread-model.md) | Offline Two-Thread Architecture without Async Runtime | Accepted | Concurrency |
| [ADR-012](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0012-snapshot-window-semantics.md) | Snapshot-Plus-Window Temporal Semantics | Accepted | Observation Model |
| [ADR-013](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0013-phase2-block-io-attribution.md) | Phase 2 Block I/O Attribution Rules (BIO-1 and BIO-2) | Accepted | Phase 2 Scope |
| [ADR-014](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0014-static-html-graph-visualization.md) | Static HTML Graph Visualization Export | Accepted | Phase 3 Scope |
| [ADR-015](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0015-continuous-watch-mode.md) | Continuous Watch Mode with Bounded Retention Buffer | Accepted | Phase 3 Scope |
