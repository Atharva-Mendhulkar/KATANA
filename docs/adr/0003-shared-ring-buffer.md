# ADR-003: Shared BPF Ring Buffer over Perf Buffers

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Event ingestion requires monotonic ordering, minimal per-CPU buffer waste, and explicit loss accounting.

## Alternatives Considered
- **Per-CPU Perf Buffers (`BPF_MAP_TYPE_PERF_EVENT_ARRAY`):** Older interface; requires per-CPU allocation, suffers from silent event loss on high-traffic CPUs, and requires userspace multi-stream merging.

## Decision
Use `BPF_MAP_TYPE_RINGBUF` with a single shared 8 MiB buffer.

## Consequences
- Single FIFO reservation order across CPUs.
- Loss is explicitly detected via sequence counters and `bpf_ringbuf_reserve` failure counters.
- Requires Linux kernel ≥ 5.8 (Katana minimum requirement is 5.15).
