# ADR-002: C eBPF with CO-RE and libbpf-rs Loader

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Kernel instrumentation requires high verifier compatibility across Linux 5.15 through 6.12+ without requiring kernel headers at runtime.

## Alternatives Considered
- **Aya (Pure Rust eBPF):** Rapidly evolving, but verifier diagnostics and kernel header integration are less mature than libbpf.
- **BCC (BCC Python):** Requires heavy LLVM runtime on target hosts.

## Decision
Write eBPF probes in C adhering to BPF CO-RE (Compile Once – Run Everywhere) conventions with `vmlinux.h`, loaded from Rust via `libbpf-rs` and `libbpf-cargo`.

## Consequences
- Predictable verifier behavior and low runtime dependencies.
- Build pipeline requires Clang with BPF target support.
