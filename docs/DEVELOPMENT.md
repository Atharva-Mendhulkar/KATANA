# Katana Development Guide

> **Developer Setup, Build Dependencies, and Verification Workflow**  
> Reference: [prd.md §29, §20](file:///home/topfloorboss/Desktop/KATANA/prd.md)

---

## 1. Prerequisites & Build Dependencies

To build Katana and compile the in-kernel eBPF programs, the host must provide:

- **Rust Toolchain:** Stable Rust (1.75+ or newer, with `cargo`, `rustc`).
- **Clang & LLVM:** `clang` (version 13+ with BPF target support) for compiling C eBPF programs.
- **BPF Tooling:** `bpftool` and `libbpf-dev` / `libbpf` headers.
- **Kernel Headers & BTF:** Kernel configured with `CONFIG_DEBUG_INFO_BTF=y` providing `/sys/kernel/btf/vmlinux`.

### Package Installation

#### Arch Linux / CachyOS
```bash
sudo pacman -S clang llvm bpftool libbpf rust
```

#### Ubuntu / Debian (22.04 LTS+)
```bash
sudo apt update
sudo apt install -y clang llvm libbpf-dev linux-tools-common linux-tools-generic bpftool cargo rustc
```

#### Fedora (38+)
```bash
sudo dnf install -y clang llvm libbpf-devel bpftool cargo rust
```

---

## 2. Building Katana

### Unprivileged User-Space Engine
Katana's analysis core is pure Rust and compiles without root privileges:
```bash
cargo build
cargo build --release
```

### eBPF Program Compilation & Verification
The C eBPF tracepoint probe (`bpf/katana.bpf.c`) is verified via:
```bash
# Verify kernel hook allow-list conformance (PRD §8.2)
./scripts/check-scope.sh
```

---

## 3. Running Test Suites

Katana features four specialized integration and fault-injection test suites:

```bash
# Run all unit and integration tests
cargo test

# Run a specific suite
cargo test --test anti_inflation_tests
cargo test --test block_io_tests
cargo test --test fault_injection_tests
cargo test --test fw1_tests
```

All 29 tests run entirely in unprivileged user space by replaying recorded and synthetic event streams.

---

## 4. Running Benchmarks

Microbenchmarks measure disk I/O commit latency, eBPF packet decoding speed, and engine analysis throughput:

```bash
# Run release benchmark binary
cargo run --release --bin katana-bench
```

Benchmark results and methodology are documented in [`benchmarks/BENCHMARKS.md`](file:///home/topfloorboss/Desktop/KATANA/benchmarks/BENCHMARKS.md).

---

## 5. Privileged Live Execution Setup

Running Katana live against running processes requires kernel tracing capabilities:

### Option A: Capability Grants (Recommended)
Grant Katana ambient capabilities without running as full root:
```bash
sudo setcap cap_bpf,cap_perfmon,cap_sys_ptrace+ep target/release/katana
./target/release/katana explain <PID>
```

### Option B: Sudo / Root
```bash
sudo ./target/release/katana explain <PID>
```

### Option C: QEMU / KVM Virtual Machine
For isolated kernel matrix testing across multiple kernel versions (5.15, 6.1, 6.6, 6.12):
```bash
# Launch test VM with rootfs
qemu-system-x86_64 \
  -kernel /boot/vmlinuz-linux \
  -initrd /boot/initramfs-linux.img \
  -append "console=ttyS0 root=/dev/vda rw" \
  -drive file=test_vm.img,format=qcow2 \
  -enable-kvm -m 2G -smp 2 -nographic
```

---

## 6. Development Rules & Invariants

1. **Anti-Inflation Rule:** Never permit `Causal` claims unless the full mechanism is proven by kernel tracepoints. Correlated events must always include the mandatory disclaimer: *"This trace does not establish a causal link."*
2. **Zero-Bloat Rule (Ponytail Mode):** Implement the smallest change that fully solves the task. Avoid unnecessary dependencies, async frameworks, or premature abstractions.
3. **No Non-PI Mutex Owners:** Plain futex words have no kernel-defined owner. Always report `owner: unknown (non-PI futex)`.
4. **Device Naming Invariant:** An `in_iowait` flag alone never names a device. Device names are reported only when verified block I/O requests are observed.
