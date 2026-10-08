# ADR-001: Rust Userspace Implementation

- **Status:** Accepted
- **Deciders:** Katana Engineering Team
- **Date:** Phase 1 Inception

## Context
Katana requires an efficient, memory-safe userspace runtime capable of decoding untrusted, packed binary wire data from eBPF ring buffers, constructing causal graph structures, and evaluating evidence invariants.

## Alternatives Considered
1. **C / libbpf:** Low level, but high risk of memory errors during graph manipulation and report rendering.
2. **Go (Cilium ebpf):** Garbage collected runtime adds unpredictable latency spikes and larger binary size.
3. **Python (BCC):** High runtime overhead, requires compiler toolchain on target machines.

## Decision
Implement the entire userspace pipeline in Rust using the stable toolchain. Confine `unsafe` strictly to FFI boundary points.

## Consequences
- Guarantees memory safety and robust algebraic data types for evidence classification.
- Requires standard Rust toolchain and LLVM/Clang for builds.
