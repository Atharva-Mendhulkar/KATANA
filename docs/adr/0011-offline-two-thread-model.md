# ADR-011: Offline Two-Thread Architecture without Async Runtime

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Tracing tools often introduce complex asynchronous event-processing pipelines (e.g. Tokio / async Rust), introducing runtime scheduling overhead, worker threads, and non-deterministic event handling.

## Alternatives Considered
- **Tokio async runtime:** Heavy dependency, thread pool scheduling overhead, complicates signal handling.
- **Single-threaded lock-step polling:** May drop kernel events if analysis blocks polling.

## Decision
Adopt a strictly partitioned two-thread model:
1. **Collector Thread:** Dedicated ring buffer poller pushing raw batches over a bounded crossbeam channel.
2. **Main Thread:** Orchestrates window sleep, joins the collector thread, and executes pure functional offline analysis.

## Consequences
- Guaranteed zero analysis overhead during the active collection window.
- The entire analysis engine is a pure function of recorded events, enabling deterministic offline replay tests.
