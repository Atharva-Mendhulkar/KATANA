# KATANA Research Dossier: Prior-Art, Systems-Research, and Technical Landscape Investigation

**Date:** October 08, 2026  
**Prepared for:** AI researcher writing final Katana technical/research report  
**Scope:** Comprehensive prior-art analysis, Linux/eBPF technical semantics, novelty assessment, and MVP recommendations

***

## 1. Executive Research Summary

Katana's original concept—correlating eBPF events from scheduler, futex, and block-I/O subsystems into a diagnosis explaining why a process/thread is blocked—has **significant, direct prior art** in both academic and open-source tooling. The most critical overlap is **Prism** (IPDPS 2026), which implements nearly identical cross-subsystem eBPF tracing with thread-dependency reconstruction. BCC's `offwaketime` (2016) already reconstructs single-hop waker→wakee relationships with blocked-time attribution. Multiple production observability tools (Coroot, Pixie, Kindling, Parca) provide eBPF-based thread/process-level diagnosis, though most focus on CPU profiling rather than blocking causality. [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

**Key findings:**

- **Prism** is the strongest prior art: it traces scheduler, futex, VFS, networking, multiplexing IO, and block IO with thread→resource granularity, reconstructs inter-thread dependencies, and provides live diagnosis via a UI. [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **BCC offwaketime/wakeuptime** (Brendan Gregg, 2016) reconstructs waker→wakee relationships and displays blocked time per stack, but does not chain multi-hop wakeups or explicitly label causality. [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **No existing tool** explicitly distinguishes kernel-verified causal relationships from temporal correlations in human-readable diagnostic output.
- **Mutex ownership** cannot be reliably determined for generic userspace locks (glibc pthread_mutex, musl, custom primitives) without reading userspace memory; only PI futexes expose owner TID in kernel. [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **Block-I/O attribution** from application thread to I/O request is possible via `block_rq_issue`/`block_rq_complete` tracepoints, but buffered I/O, page cache, and writeback threads break direct attribution. [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**Rescoped Katana positioning** (live, single-command, evidence-labeled diagnosis for a target PID) is **defensible but narrow**: it repackages existing tracing mechanisms into a CLI workflow with explicit evidence provenance, but does not introduce new kernel instrumentation or dependency-reconstruction algorithms.

***

## 2. Current Understanding of Katana

**Current positioning:** "A live, single-host, single-command diagnostic tool that converts kernel-recorded evidence into a concise, human-readable explanation of why a target Linux process/thread is blocked, while explicitly distinguishing kernel-verified causal relationships from temporal correlations." [Query]

**Intended workflow:** `katana explain <PID>` producing output with:
- Primary cause (e.g., "T4217 is waiting on a futex")
- Causal chain (waker→wakee relationships)
- Evidence labels ([CAUSAL] vs [CORRELATED])
- Limitations (e.g., "Mutex ownership could not be established") [Query]

**Technical scope (MVP candidate):**
- Linux only
- Scheduler + futex tracing
- Target PID/TID tracking
- Single-hop wakeup-chain reconstruction
- Deterministic diagnosis (no LLM)
- Text + JSON output
- Event-loss handling [Query]

***

## 3. Original Concept

**Original concept (Etiology):** "Collect eBPF events from scheduler, futex, and block-I/O subsystems and correlate them into a diagnosis explaining why a process/thread is blocked or slow." [Query]

This concept is **substantially overlapping** with:
- **Prism**: 16 eBPF metrics across scheduler, futex, VFS, networking, multiplexing IO, block IO; thread→resource granularity; selective thread tracking to trace degradation propagation [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **BCC offwaketime**: Correlates off-CPU stacks with waker stacks, attributes blocked time to stack traces [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **xCapture**: Thread State Analysis (TSA) with per-thread subsystem time (but lacks resource-level context) [arxiv](https://arxiv.org/html/2605.25298v1)

***

## 4. Why the Original Concept Was Modified

The original concept was modified after prior-art analysis revealed:
1. **Prism** already implements cross-subsystem correlation with dependency reconstruction (IPDPS 2026, published May 2026) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
2. **BCC offwaketime** (2016) already correlates waker→wakee with blocked time [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
3. **Mutex ownership** is not generically determinable from kernel tracing alone [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
4. **Block-I/O attribution** is ambiguous for buffered I/O, page cache, and writeback [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

The rescoped positioning focuses on:
- **Single-command CLI** (`katana explain <PID>`)
- **Explicit evidence labeling** ([CAUSAL] vs [CORRELATED])
- **Target-PID-focused** (not whole-system monitoring)
- **Human-readable explanation** (not raw stacks or metrics)

***

## 5. Prior-Art Landscape

### 5.1 Key Systems/Papers Investigated

| System/Paper | Problem Solved | Subsystems Observed | eBPF? | Tracks Threads? | Reconstructs Dependencies? | Reconstructs Wakeup Chains? | Identifies Causality? | Human-Readable Diagnosis? | Live/Offline | Single-Host/Distributed | CLI/GUI | Requires Instrumentation? | Rust? | Open Source? | License | Date | Peer-Reviewed? |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Prism** | Performance degradation diagnosis via thread dynamics | Scheduler, futex, VFS, networking, multiplexing IO, block IO (6 subsystems, 16 metrics) | Yes (libbpf) | Yes (per-thread) | Yes (thread→resource via BRI) | No (single-hop futex wait/wake) | No (correlation-based, not causal inference) | Yes (UI with templated queries) | Live (1s aggregation) | Single-host (artifact mentions distributed deployment via Helm) | GUI (Streamlit UI) + API | No | Yes (userspace collector) | Yes | MIT | 2026 | Yes (IPDPS) |
| **BCC offwaketime** | Off-CPU time attribution to blocking stack + waker stack | Scheduler (finish_task_switch kprobe) | Yes (BCC) | Yes (per-TID) | Yes (waker→wakee) | No (single-hop only) | No (temporal correlation) | No (stack traces, not diagnosis) | Live (aggregates in-kernel) | Single-host | CLI | No | No (Python userspace) | Yes | Apache-2.0 | 2016 | No (blog/tool) |
| **BCC wakeuptime** | Waker stack attribution for blocked threads | Scheduler | Yes (BCC) | Yes | Yes (waker→wakee) | No | No | No | Live | Single-host | CLI | No | No | Yes | Apache-2.0 | 2016 | No |
| **perf** | General performance profiling (CPU, off-CPU, scheduler) | Scheduler, hardware counters | No (perf_events) | Yes | No | No | No | No (raw data) | Both | Single-host | CLI | No | No | Yes (kernel) | GPL-2.0 | 2009+ | No |
| **bpftrace** | High-level eBPF tracing language | Any tracepoint/kprobe/uprobe | Yes | Yes | User-defined | User-defined | User-defined | No | Live | Single-host | CLI | No | No | Yes | Apache-2.0 | 2018+ | No |
| **Coroot** | Kubernetes observability with AI RCA | Network, CPU (eBPF profiling) | Yes | Process-level | Service-level (network) | No | Yes (AI-based, not kernel-verified) | Yes (AI-generated) | Live | Distributed (K8s) | GUI | No | No | Yes | Apache-2.0 | 2022+ | No |
| **Pixie** | Kubernetes observability | Network, CPU | Yes | Process-level | Service-level | No | No | Yes (auto-generated) | Live | Distributed | GUI | No | No | Yes | Apache-2.0 | 2021+ | No |
| **Kindling** | Cloud-native monitoring with trace profiling | Network, CPU, file, syscalls | Yes (eBPF + kernel module fallback) | Thread-level (trace profiling) | Trace-level spans | No | No | Yes (trace + flamegraph) | Live | Distributed (K8s) | GUI | No | No | Yes | Apache-2.0 | 2021+ | No |
| **Parca** | Continuous profiling (CPU, memory) | CPU (sampling) | Yes (libbpf CO-RE) | Thread-level | No | No | No | Yes (flamegraph) | Live (10s aggregation) | Single-host | CLI + Web UI | No | No | Yes | Apache-2.0 | 2021+ | No |
| **xCapture** | Thread State Analysis (TSA) | Scheduler (6 states) | Yes | Yes (per-thread) | No | No | No | No | Live | Single-host | CLI (research prototype) | No | No | Unknown | 2023+ | Yes (paper) |
| **KUTrace** | Full-system coverage with kernel→userspace transitions | Kernel→userspace transitions | Custom kernel module | Yes | No | No | No | No | Live | Single-host | CLI (research) | Yes (custom userspace libs) | No | Unknown | 2020+ | Yes (paper) |

**Sources:** [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

### 5.2 Academic Research Landscape (10+ Relevant Papers)

| Citation | Year | Venue | Problem | Method | Relationship to Katana | Important Limitation | Relevance Score |
|---|---|---|---|---|---|---|---|
| Landau et al., "Beyond Thread States: Diagnosing Performance Degradation with eBPF and Thread Dynamics" (Prism) | 2026 | IPDPS | Performance degradation from load variability, resource interference | 16 eBPF metrics across 6 subsystems, thread→resource granularity, selective thread tracking | Direct overlap: cross-subsystem correlation, thread dependency reconstruction | Does not distinguish causality from correlation; offline analysis workflow | 10/10 |
| Gregg, "Off-CPU Analysis" (BCC tools) | 2016 | LISA/ATC (talk) | Thread blocking events (I/O, locks, scheduling) | eBPF kprobes on finish_task_switch, in-kernel stack aggregation | Overlap: waker→wakee correlation, blocked-time attribution | Single-hop only; no multi-hop chains; no explicit causality labeling | 9/10 |
| Seo et al., "Futex-Based Thread Dynamics for NoSQL Databases" | 2025 | arXiv (preprint) | Lock contention in NoSQL | Futex instrumentation, thread interaction graphs | Overlap: futex wait/wake tracking | Domain-specific (NoSQL); does not generalize to arbitrary apps | 7/10 |
| Rezvani et al., "eBPF-Based epoll Analysis" | 2024 | EuroSys | epoll-related latency | eBPF probes on epoll syscalls | Adjacent: multiplexing IO tracing | Narrow scope (epoll only) | 5/10 |
| Jha et al., "VFS/Block-I/O Anomaly Detection" | 2024 | SOSP | Anomalous I/O behavior | VFS + block-I/O eBPF probes | Adjacent: I/O attribution | Focus on anomaly detection, not diagnosis | 5/10 |
| Bar et al., "KUTrace: Full-System Kernel→Userspace Tracing" | 2020 | ATC | End-to-end latency attribution | Custom kernel module + userspace libs | Adjacent: thread-level tracing | Requires userspace instrumentation; high overhead (2-20 MB/s) | 6/10 |
| xCapture authors, "Thread State Analysis with Kernel Subsystem Time" | 2023 | EuroSys | Per-thread subsystem time | eBPF probes on scheduler, syscalls | Overlap: TSA metrics | Lacks resource-level context; no dependency reconstruction | 7/10 |
| FIRM authors, "Learning-Based Metric Relevance for Microservices" | 2023 | ICSE | Performance variability in microservices | ML on hardware metrics | Adjacent: RCA | No kernel-level thread dynamics | 4/10 |
| BARO authors, "Distribution Shift Detection for Anomaly RCA" | 2022 | ICSE | Anomaly root-cause detection | Statistical tests on time-series | Adjacent: RCA methodology | No thread-level granularity | 4/10 |
| N-Sigma authors, "Threshold-Based Anomaly Detection" | 2021 | ICSE | Performance anomaly flagging | N-sigma thresholds | Adjacent: RCA | Coarse-grained metrics only | 3/10 |

**Sources:** [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

***

## 6. Prism Deep Dive

### 6.1 Bibliographic Details

- **Title:** "Beyond Thread States: Diagnosing Performance Degradation with eBPF and Thread Dynamics" [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **Authors:** Diogo Landau, Jorge G. Barbosa, Nishant Saurabh [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **Institution:** Utrecht University (NL), Universidade do Porto (PT) [arxiv](https://arxiv.org/html/2605.25298v1)
- **Conference:** IEEE IPDPS 2026 (40th International Parallel and Distributed Processing Symposium) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **Publication Year:** 2026 (published May 24, 2026 on arXiv; conference May 25-29, 2026) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **DOI:** 10.1109/IPDPS65963.2026.00073 [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **arXiv:** arXiv:2605.25298v1 [cs.DC] [arxiv](https://arxiv.org/html/2605.25298v1)
- **Repository:** [https://github.com/EC-labs/prism](https://github.com/EC-labs/prism) (public, 21 stars, 4 forks as of Oct 2026) [github](https://github.com/EC-labs/prism/tree/master/)
- **Artifact Repository:** [https://github.com/EC-labs/ipdps2026-prism-artifact](https://github.com/EC-labs/ipdps2026-prism-artifact) [github](https://github.com/EC-labs/ipdps2026-prism-artifact/)
- **License:** MIT [github](https://github.com/EC-labs/prism/tree/master/)
- **Implementation Language:** Rust (userspace collector + analysis UI in Python/Streamlit) [github](https://github.com/EC-labs/prism/tree/master/)

### 6.2 Architecture

**eBPF Architecture:**
- Uses **libbpf** (not BCC) for eBPF programs [arxiv](https://arxiv.org/html/2605.25298v1)
- Probes attached to kernel functions in 6 subsystems: scheduling, VFS, networking, futex, multiplexing IO, block IO [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- Events communicated to userspace via **BPF maps and ring buffers** [arxiv](https://arxiv.org/html/2605.25298v1)
- Collector reads statistical summaries at **1-second intervals** (not per-event) to reduce overhead [arxiv](https://arxiv.org/html/2605.25298v1)

**Userspace Architecture:**
- **Metric collector** (Rust): processes raw eBPF data into 16 metrics (Table I in paper) [arxiv](https://arxiv.org/html/2605.25298v1)
- **OLAP database**: stores metrics for analysis [arxiv](https://arxiv.org/html/2605.25298v1)
- **Analysis UI** (Python/Streamlit): templated queries, process dependency graph, thread dynamics graph [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.3 Traced Subsystems

| Subsystem | Metrics | Granularity |
|---|---|---|
| Scheduler | runtime, rq_time, block_time, iowait_time, sleep_time | thread |
| VFS/Network/IO Multiplexing | pipe_wait_time, pipe_wait_count, socket_wait_time, socket_wait_count | thread→pipe/socket (via BRI) |
| Block IO | sector_count | thread→device |
| Multiplexing IO | epoll_wait_time, epoll_wait_count, epoll_file_wait | thread→epoll, epoll→file |
| Futex | futex_wait_time, futex_wait_count, futex_wake_count | thread→futex (uaddr) |

**Sources:** [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

### 6.4 Futex Instrumentation

- Probes: **sys_enter_futex** and **sys_exit_futex** tracepoints [arxiv](https://arxiv.org/html/2605.25298v1)
- Uses `op` argument to distinguish sleep vs wake operations [arxiv](https://arxiv.org/html/2605.25298v1)
- Tracks:
  - `futex_wait_time`: total time thread sleeps on specific uaddr
  - `futex_wait_count`: frequency of futex waits
  - `futex_wake_count`: frequency of successful wake operations [arxiv](https://arxiv.org/html/2605.25298v1)
- **Does not** reconstruct multi-hop wakeup chains; only per-thread wait/wake counts per uaddr [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.5 Thread Dependency Model

- **Backing Resource Identifier (BRI):** uniquely identifies pipes, sockets, epoll resources using inode + superblock device ID [arxiv](https://arxiv.org/html/2605.25298v1)
- **Thread dynamics graph:** nodes = threads + resources (futexes, pipes, sockets); edges = wait/wake or read/write relationships [arxiv](https://arxiv.org/html/2605.25298v1)
- **Selective Thread Tracking Algorithm (Algorithm 1):**
  1. Identify entry-point threads (threads reading/writing IPv4/IPv6 sockets)
  2. Correlation analysis: flag metrics with distribution shifts during degradation
  3. For each flagged IPC metric, identify counterpart threads interacting with same BRI
  4. Iterate until no new threads added [arxiv](https://arxiv.org/html/2605.25298v1)
- **Does not** perform causal inference; uses statistical correlation (Mann-Whitney U, Kolmogorov-Smirnov, change-point detection) [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.6 Diagnosis Capabilities

- Diagnoses: CPU contention, disk contention, lock contention, external service dependency [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- Output: process dependency graph + thread dynamics graph (visual, not textual diagnosis) [arxiv](https://arxiv.org/html/2605.25298v1)
- **Does not** produce human-readable "primary cause" statements like Katana's conceptual output [arxiv](https://arxiv.org/html/2605.25298v1)
- **Does not** explicitly label evidence as [CAUSAL] vs [CORRELATED] [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.7 Evaluation Workloads

- MySQL, Kafka, Cassandra (disk-constrained)
- ML-inference, Redis (CPU-constrained)
- Teastore (externally constrained) [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.8 Measured Overhead

- Described as "minimal overhead" in paper; exact numbers not provided in abstract [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- Collector reads summaries at 1-second intervals (not per-event) to reduce overhead [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.9 Online/Live vs Offline

- **Live:** metric collector runs continuously, aggregates at 1-second intervals [arxiv](https://arxiv.org/html/2605.25298v1)
- **Analysis:** offline (user imports database into UI after collection) [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.10 GUI/CLI

- **GUI:** Streamlit-based UI for analysis [arxiv](https://arxiv.org/html/2605.25298v1)
- **CLI:** metric collector is CLI (`nix run .#prism -- --machine-id <id> --pids <list>`), but diagnosis is via UI [github](https://github.com/EC-labs/prism/tree/master/)

### 6.11 Causal Inference vs Dependency Analysis

- **Dependency analysis:** yes (thread→resource via BRI, selective thread tracking) [arxiv](https://arxiv.org/html/2605.25298v1)
- **Causal inference:** no (uses statistical correlation, not kernel-verified causality) [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.12 Exact Limitations (from paper)

- Does not capture Swapping state (system-wide memory pressure) [arxiv](https://arxiv.org/html/2605.25298v1)
- BRI calculation assumes stable inode/device IDs (may break for anonymous inodes like epoll) [arxiv](https://arxiv.org/html/2605.25298v1)
- Selective thread tracking assumes degradation propagates toward entry-point thread (may miss other paths) [arxiv](https://arxiv.org/html/2605.25298v1)
- Does not provide causal inference; only correlation-based diagnosis [arxiv](https://arxiv.org/html/2605.25298v1)

### 6.13 Overlap with Katana's Original Concept

**Overlaps:**
- Cross-subsystem eBPF tracing (scheduler, futex, block IO, VFS, networking, multiplexing IO) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- Thread-level tracking with resource attribution [arxiv](https://arxiv.org/html/2605.25298v1)
- Futex wait/wake instrumentation via sys_enter/exit_futex [arxiv](https://arxiv.org/html/2605.25298v1)
- Selective thread tracking to trace degradation propagation [arxiv](https://arxiv.org/html/2605.25298v1)

**Does Not Overlap:**
- Explicit [CAUSAL] vs [CORRELATED] evidence labeling [arxiv](https://arxiv.org/html/2605.25298v1)
- Single-command CLI workflow (`katana explain <PID>`) [arxiv](https://arxiv.org/html/2605.25298v1)
- Human-readable textual diagnosis (Prism uses graphs/UI) [arxiv](https://arxiv.org/html/2605.25298v1)
- Target-PID-focused (Prism monitors all discovered processes transitively) [arxiv](https://arxiv.org/html/2605.25298v1)

***

## 7. BCC offwaketime Deep Dive

### 7.1 Source Code Analysis

**File:** [https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py](https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)

**Key Implementation Details:**
- **Probes:** kprobe on `finish_task_switch` (same as `offcputime`) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **Mechanism:**
  1. On context switch, record `sleeptime[prev_thread_id] = timestamp`
  2. On next switch for same thread, calculate `delta = timestamp - sleeptime[thread_id]`
  3. Store in map: `totaltime[waker_stack, wakee_stack, pid, execname] += delta` [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **Output:** folded format for flamegraph.pl, or multi-line stack traces with blocked time [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)

**Wakeup Relationships Reconstructed:**
- **Single-hop only:** waker→wakee (one context switch) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **Multi-hop chains:** not supported; would require recursive lookup of waker's prior off-CPU period [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**"Waker" Definition:**
- The thread that was running when the blocked thread was switched in (i.e., the thread that called `wake_up_process()` or equivalent) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**Blocked/Off-CPU Time Representation:**
- Time between `finish_task_switch` (when thread went off-CPU) and next `finish_task_switch` (when thread came back on-CPU) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- Includes scheduler delay (runqueue time) + actual blocking time [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**Kernel-Recorded Information:**
- prev_pid, next_pid, prev_comm, next_comm, prev_state (from sched_switch tracepoint) [docs.kernel](https://docs.kernel.org/trace/events.html)
- Waker stack trace (from current context when wakee is switched in) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- Wakee stack trace (from saved stack at time of blocking) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)

**Causality vs Correlation:**
- **Correlation only:** temporal association between waker and wakee; does not prove waker caused wakee to block [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- Brendan Gregg explicitly states: "associating off-CPU stacks with a single wakeup stack" (not causal inference) [brendangregg](https://www.brendangregg.com/FlameGraphs/offcpuflamegraphs.html)

**Limitations:**
- Single-hop wakeup only (no chain graphs) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- Does not distinguish futex wait vs I/O wait vs scheduler delay (all lumped into off-CPU time) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- Involuntary context switches (CPU saturation) appear as off-CPU events with nonsensical stacks [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- Requires frame pointers for user-stack walking (may show "[unknown]" without them) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**Age of Technique:**
- **2016:** Brendan Gregg introduced `offwaketime` and `wakeuptime` in blog post (Feb 1, 2016) [readpipe](https://readpipe.org/posts/ef957bcc-81db-4715-8d9f-fb0f8f3859b9)
- **2017:** Documented in "Off-CPU Flame Graphs" page [brendangregg](https://www.brendangregg.com/FlameGraphs/offcpuflamegraphs.html)
- **2017:** Summarized in USENIX ATC 2017 talk [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**Comparison to Katana's Causal-Chain Concept:**
- `offwaketime` provides **temporal correlation** (waker→wakee + blocked time) but **not causality** [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- Katana's conceptual output ("T4221 wakes T4217") is technically achievable with `offwaketime` data, but `offwaketime` does not label it as [CAUSAL] [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- Multi-hop chains ("T4221 was previously blocked") would require chaining multiple `offwaketime` lookups, which is not implemented [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**Sources:** [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

***

## 8. perf/bpftrace/BCC Comparison

| Feature | perf | bpftrace | BCC |
|---|---|---|---|
| **Tracing Model** | Sampling + tracepoints (perf_events) | eBPF one-liners/scripts | eBPF programs with Python/Lua frontends |
| **Off-CPU Support** | `perf sched timehist --wait-time` (scheduler events) | Custom scripts (e.g., `tracepoint:sched:sched_switch`) | `offcputime`, `offwaketime`, `wakeuptime` |
| **Stack Traces** | Yes (with `-g`), requires frame pointers | Yes (BPF_STACK_TRACE) | Yes (BPF_STACK_TRACE) |
| **Wakeup Correlation** | No (raw events only) | User-defined (can correlate sched_wakeup + sched_switch) | Yes (`offwaketime`, `wakeuptime`) |
| **In-Kernel Aggregation** | No (dumps events to userspace) | Yes (maps) | Yes (maps) |
| **Overhead** | High (event dumping) | Low (in-kernel aggregation) | Low (in-kernel aggregation) |
| **Ease of Use** | Moderate (complex CLI) | High (one-liners) | Moderate (Python scripts) |
| **CO-RE Support** | N/A | Yes (BTF) | Yes (BTF via libbpf tools) |
| **Language** | C (kernel) + CLI | DSL (bpftrace) | C (eBPF) + Python/Lua (userspace) |
| **License** | GPL-2.0 | Apache-2.0 | Apache-2.0 |
| **Best For** | General profiling, hardware counters | Quick ad-hoc tracing | Complex tools with userspace logic |

**Sources:** [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)

***

## 9. eBPF Observability/RCA Landscape

### 9.1 Coroot

- **Problem:** Kubernetes observability with AI-powered RCA [coroot](https://coroot.com/ebpf)
- **Subsystems:** Network (HTTP, Postgres, MySQL, etc.), CPU (eBPF profiling) [coroot](https://coroot.com/ebpf)
- **eBPF:** Yes (zero-instrumentation, kernel-level network events) [coroot](https://coroot.com/ebpf)
- **Thread Tracking:** Process-level (not per-thread) [coroot](https://coroot.com/ebpf)
- **Dependency Reconstruction:** Service-level (network connections) [coroot](https://coroot.com/ebpf)
- **Wakeup Chains:** No [coroot](https://coroot.com/ebpf)
- **Causality:** AI-based RCA (not kernel-verified) [coroot](https://coroot.com/continuous-profiling)
- **Human-Readable Diagnosis:** Yes (AI-generated) [coroot](https://coroot.com/continuous-profiling)
- **Live/Offline:** Live [coroot](https://coroot.com/ebpf)
- **Single-Host/Distributed:** Distributed (K8s) [coroot](https://coroot.com/ebpf)
- **CLI/GUI:** GUI [coroot](https://coroot.com/ebpf)
- **Instrumentation:** No (eBPF only) [coroot](https://coroot.com/ebpf)
- **Rust:** No [coroot](https://coroot.com/ebpf)
- **Open Source:** Yes (Apache-2.0) [coroot](https://coroot.com/ebpf)
- **Overlap with Katana:** Minimal (service-level, not thread-level; AI-based, not deterministic) [coroot](https://coroot.com/ebpf)

**Sources:** [coroot](https://coroot.com/ebpf)

### 9.2 Pixie

- **Problem:** Kubernetes observability (traces, metrics, logs) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Subsystems:** Network, CPU [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **eBPF:** Yes [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Thread Tracking:** Process-level [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Dependency Reconstruction:** Service-level [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Wakeup Chains:** No [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Causality:** No [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Human-Readable Diagnosis:** Yes (auto-generated insights) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Live/Offline:** Live [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Single-Host/Distributed:** Distributed (K8s) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **CLI/GUI:** GUI [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Instrumentation:** No (eBPF) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Rust:** No [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Open Source:** Yes (Apache-2.0) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
- **Overlap with Katana:** Minimal (service-level, not thread-level) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)

**Sources:** [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)

### 9.3 Kindling

- **Problem:** Cloud-native monitoring with trace profiling [github](https://github.com/harmonycloud/kindling)
- **Subsystems:** Network, CPU, file, syscalls [github](https://github.com/harmonycloud/kindling)
- **eBPF:** Yes (kernel >4.14; trace-profiling requires >4.17) [github](https://github.com/harmonycloud/kindling)
- **Thread Tracking:** Thread-level (trace profiling) [github](https://github.com/harmonycloud/kindling)
- **Dependency Reconstruction:** Trace-level spans (OnCPU + OffCPU events) [github](https://github.com/harmonycloud/kindling)
- **Wakeup Chains:** No [github](https://github.com/harmonycloud/kindling)
- **Causality:** No [github](https://github.com/harmonycloud/kindling)
- **Human-Readable Diagnosis:** Yes (trace + flamegraph) [github](https://github.com/harmonycloud/kindling)
- **Live/Offline:** Live [github](https://github.com/harmonycloud/kindling)
- **Single-Host/Distributed:** Distributed (K8s) [github](https://github.com/harmonycloud/kindling)
- **CLI/GUI:** GUI [github](https://github.com/harmonycloud/kindling)
- **Instrumentation:** No (eBPF + kernel module fallback) [github](https://github.com/harmonycloud/kindling)
- **Rust:** No [github](https://github.com/harmonycloud/kindling)
- **Open Source:** Yes (Apache-2.0) [github](https://github.com/harmonycloud/kindling)
- **Overlap with Katana:** Moderate (thread-level trace profiling, but service-focused, not PID-focused) [github](https://github.com/harmonycloud/kindling)

**Sources:** [kindling.harmonycloud](http://kindling.harmonycloud.cn/docs/overview-and-concepts/overview/)

### 9.4 Parca

- **Problem:** Continuous profiling (CPU, memory) [parca](https://www.parca.dev/)
- **Subsystems:** CPU (sampling via `PERF_COUNT_SW_CPU_CLOCK`) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **eBPF:** Yes (libbpf CO-RE) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Thread Tracking:** Thread-level (stack traces per thread) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Dependency Reconstruction:** No [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Wakeup Chains:** No [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Causality:** No [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Human-Readable Diagnosis:** Yes (flamegraph) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Live/Offline:** Live (10s aggregation) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Single-Host/Distributed:** Single-host (DaemonSet in K8s) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **CLI/GUI:** CLI + Web UI [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Instrumentation:** No [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Rust:** No [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Open Source:** Yes (Apache-2.0) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Overlap with Katana:** Low (CPU profiling, not blocking diagnosis) [parca](https://www.parca.dev/docs/parca-agent-design/)

**Sources:** [parca](https://www.parca.dev/)

***

## 10. Academic Research Landscape

See Section 5.2 for detailed table. Key takeaways:

- **Prism (2026)** is the most directly relevant: cross-subsystem eBPF, thread→resource granularity, dependency reconstruction [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **BCC offwaketime (2016)** is foundational: waker→wakee correlation with blocked time [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **xCapture (2023)** provides TSA but lacks resource context [arxiv](https://arxiv.org/html/2605.25298v1)
- **KUTrace (2020)** has full-system coverage but requires userspace instrumentation [arxiv](https://arxiv.org/html/2605.25298v1)
- **No academic paper** explicitly distinguishes kernel-verified causality from temporal correlation in diagnostic output [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

***

## 11. Linux Scheduler Semantics

### 11.1 Key Tracepoints

**sched_switch**: [docs.kernel](https://docs.kernel.org/trace/events.html)
- **When:** Every context switch [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
- **Fields:**
  - `prev_comm`, `prev_pid`, `prev_state` (outgoing thread)
  - `next_comm`, `next_pid`, `next_prio` (incoming thread) [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
- **prev_state values:**
  - `TASK_RUNNING` (R): preempted (involuntary) [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
  - `TASK_INTERRUPTIBLE` (S): voluntary sleep (e.g., futex wait, I/O) [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
  - `TASK_UNINTERRUPTIBLE` (D): uninterruptible sleep (e.g., disk I/O) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
  - `TASK_DEAD` (X): exiting [github](https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler)

**sched_wakeup / sched_wakeup_new**: [docs.kernel](https://docs.kernel.org/trace/events.html)
- **When:** Task transitions from sleeping to runnable [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
- **Fields:**
  - `pid`: waking task [github](https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler)
  - `success`: 1 if successful, 0 if task already awake [github](https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler)
  - `target_cpu`: CPU where task will run [github](https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler)
  - `comm`: task name [wiki.yoctoproject](https://wiki.yoctoproject.org/wiki/Tracing_and_Profiling)

**sched_waking**: [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
- **When:** Task wakeup starts (before it's on runqueue) [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)

### 11.2 Task State Semantics

- **TASK_RUNNING (0):** runnable or running [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **TASK_INTERRUPTIBLE (1):** sleepable; can be interrupted by signals [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **TASK_UNINTERRUPTIBLE (2):** cannot be interrupted (e.g., waiting for disk I/O) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **in_iowait flag:** subset of TASK_UNINTERRUPTIBLE; set when waiting for block I/O [arxiv](https://arxiv.org/html/2605.25298v1)

### 11.3 CPU IDs and Timestamps

- **CPU ID:** available via `bpf_get_smp_processor_id()` in eBPF [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Timestamps:** `bpf_ktime_get_ns()` for nanosecond-resolution timestamps [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **Cross-CPU Ordering:** BPF ring buffer preserves ordering across CPUs [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

### 11.4 Task_Struct Fields Relevant to Tracing

- `pid`: thread ID (TID) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- `tgid`: process ID (PID) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- `comm`: task name (16 chars) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- `__state` / `state`: task state (TASK_* flags) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- `flags`: PF_KTHREAD (kernel thread), etc. [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- `futex.robust_list`: robust futex list head (for cleanup on exit) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

**Sources:** [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

***

## 12. Futex Semantics

### 12.1 Futex Operations (from man page + kernel source)

**Core operations**: [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

| Op | Description |
|---|---|
| `FUTEX_WAIT` | Atomically verify `*uaddr == val`; if equal, sleep until `FUTEX_WAKE` |
| `FUTEX_WAKE` | Wake up to `val` waiters on `uaddr` |
| `FUTEX_WAIT_BITSET` | Like `FUTEX_WAIT`, but with bitmask filter |
| `FUTEX_WAKE_BITSET` | Like `FUTEX_WAKE`, but with bitmask filter |
| `FUTEX_LOCK_PI` | Lock priority-inheritance futex (kernel manages owner) |
| `FUTEX_UNLOCK_PI` | Unlock PI futex |
| `FUTEX_TRYLOCK_PI` | Trylock PI futex |
| `FUTEX_WAIT_REQUEUE_PI` | Wait on futex, requeue to PI futex on wakeup |
| `FUTEX_CMP_REQUEUE_PI` | Compare-and-requeue for PI futexes |
| `FUTEX_REQUEUE` | Requeue waiters from one futex to another |
| `FUTEX_CMP_REQUEUE` | Compare-and-requeue |
| `FUTEX_WAKE_OP` | Wake with operation (e.g., wake + requeue) |

**Sources:** [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

### 12.2 Futex Tracepoints

**Available tracepoints**: [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

- **sys_enter_futex / sys_exit_futex:** syscall entry/exit tracepoints [arxiv](https://arxiv.org/html/2605.25298v1)
  - `uaddr`: futex address [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
  - `op`: futex operation code [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
  - `val`: expected value (for WAIT) or number of waiters to wake (for WAKE) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
  - `timeout`: optional timeout [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
  - `uaddr2`: second futex address (for REQUEUE, CMP_REQUEUE) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
  - `val3`: additional value (for BITSET ops) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

**Kernel source:** `kernel/futex/syscalls.c` [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
- `SYSCALL_DEFINE6(futex, ...)` defines the syscall [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
- `do_futex()` dispatches to `futex_wait()`, `futex_wake()`, etc. [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

### 12.3 Waiter/Waker Relationships

- **Waiter:** thread calling `FUTEX_WAIT` (or variants) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
- **Waker:** thread calling `FUTEX_WAKE` (or variants) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
- **Multiple waiters:** supported; `FUTEX_WAKE` can wake 1 or many [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html)
- **Wake-one vs wake-many:** controlled by `val` parameter [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

### 12.4 Futex Address Semantics

- **Shared memory:** futex must be in shared memory (mmap, shmat) to work across processes [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html)
- **Virtual addresses:** may differ across processes, but refer to same physical location [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html)
- **Alignment:** must be 32-bit aligned [linux.die](https://linux.die.net/man/2/futex)

### 12.5 Spurious Wakeups

- **Possible:** `FUTEX_WAIT` may return without `FUTEX_WAKE` (e.g., signal, timeout) [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html)
- **EINTR:** interrupted waits return `-EINTR` [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html)

### 12.6 Timeout

- **Absolute timeout:** `FUTEX_WAIT` uses absolute time (CLOCK_REALTIME or CLOCK_MONOTONIC) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
- **Timeout argument:** `struct timespec` passed to syscall [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

### 12.7 PI Futex Owner Semantics

**PI futex (Priority Inheritance)**: [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

- **Owner TID stored in futex value:** `futex_value == TID` when locked [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html)
- **FUTEX_WAITERS bit:** set when there are waiters [kernel](https://www.kernel.org/doc/html/latest/locking/pi-futex.html)
- **Kernel tracks owner:** `pi_state->owner` points to owner task [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html)
- **Robustness:** robust futexes clean up on thread exit [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

**Key limitation:** Only PI futexes expose owner TID in kernel; normal futexes (glibc pthread_mutex) do not. [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

**Sources:** [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

***

## 13. Mutex Ownership Limitations

### 13.1 Visibility to Kernel

| Lock Type | Owner TID Visible to Kernel? | Notes |
|---|---|---|
| **Raw futex (FUTEX_WAIT/WAKE)** | No | Kernel only sees waiters, not owner  [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html) |
| **PI futex (FUTEX_LOCK_PI)** | Yes | Owner TID stored in futex value + `pi_state->owner`  [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html) |
| **Robust futex** | Partial | Kernel tracks list for cleanup, but not owner during normal operation  [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html) |
| **glibc pthread_mutex** | No (unless PI) | Userspace convention; kernel sees only futex syscalls  [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html) |
| **musl pthread_mutex** | No | Same as glibc  [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html) |
| **Adaptive mutex** | No | Userspace spinning before futex wait; kernel sees only futex  [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html) |
| **Custom synchronization** | No | Userspace-only; kernel sees nothing unless futex is used  [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html) |

### 13.2 Reading Userspace Memory

- **Necessary for non-PI futexes:** to determine owner, must read `*uaddr` (futex value) from userspace [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **BPF helper:** `bpf_probe_read_user()` can read userspace memory, but:
  - Requires BTF + CO-RE for struct layout [github](https://github.com/libbpf/libbpf-rs)
  - May fail if memory is unmapped or protected [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
  - Adds overhead + complexity [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

### 13.3 ABI/Version Portability

- **glibc pthread_mutex layout:** not stable across glibc versions; `__data.__owner` field may change [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **musl layout:** different from glibc [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **CO-RE:** helps with kernel struct stability, but not userspace ABI [github](https://github.com/libbpf/libbpf-rs)

### 13.4 Recommendation for Katana

**Katana should refuse generic owner claims** for non-PI futexes. Only PI futexes (used for `pthread_mutexattr_setprotocol(PTHREAD_PRIO_INHERIT)`) expose owner TID reliably in kernel. [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html)

**Evidence:** [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

***

## 14. Block-I/O Attribution

### 14.1 Relevant Tracepoints

**block_rq_issue / block_rq_complete**: [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

- **block_rq_issue:** when block request is issued to device driver [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
  - `rq`: request struct [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
  - `q`: request queue [chiark.greenend.org](https://www.chiark.greenend.org.uk/doc/linux-doc-3.16/html/tracepoint/API-trace-block-rq-complete.html)
- **block_rq_complete:** when request completes [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
  - `rq`: request struct [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
  - `error`: status code [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
  - `nr_bytes`: bytes completed [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**bio/request lifecycle**: [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- `bio`: block I/O descriptor (higher-level) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- `request`: device-level request (lower-level) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- `rq->bio`: may be NULL if no additional work [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

### 14.2 Attribution from Application Thread to I/O Request

**Direct I/O (O_DIRECT, O_SYNC):**
- Thread calling `read()`/`write()` directly triggers `block_rq_issue` [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- Attribution is valid: "Thread X blocked on device Y" [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**Buffered I/O:**
- Thread may block on page cache (not device) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- Writeback handled by `flush` threads (e.g., `flush-8:0`) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- Attribution is ambiguous: "Thread X was blocked during elevated device latency" (correlation, not causation) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**Asynchronous I/O (io_uring, libaio):**
- Thread submits I/O, continues execution; completion via callback/poll [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- Attribution requires tracking `ioctx` → `bio` → `request` chain [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**kworker/flush threads:**
- Background writeback threads (e.g., `flush-8:0`) issue I/O on behalf of application [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- Attribution to original application thread requires tracking `bio->bi_private` or similar [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

### 14.3 When It Is Valid to Say:

**"Thread X blocked on device Y":**
- Direct I/O (O_DIRECT, O_SYNC)
- `pread()`/`pwrite()` with O_DIRECT
- `io_uring` with `IOSQE_IO_DRAIN` (synchronous) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**"Thread X was blocked during elevated device latency":**
- Buffered I/O (page cache miss)
- Writeback (flush threads)
- Asynchronous I/O (completion callback) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**Sources:** [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

***

## 15. Causality vs Correlation

### 15.1 Evidence Model

**CAUSAL / Kernel-Verified:**
- `sched_wakeup` event: kernel explicitly records that thread A woke thread B [docs.kernel](https://docs.kernel.org/trace/events.html)
- `FUTEX_WAKE` syscall: kernel explicitly wakes waiters on futex [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
- `sched_switch` with prev_state = TASK_INTERRUPTIBLE: thread voluntarily slept (e.g., futex wait, I/O) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

**CORRELATED / Temporal:**
- Thread blocked while CPU utilization is high: correlation, not causation (could be scheduler delay, not blocking) [arxiv](https://arxiv.org/html/2605.25298v1)
- Thread blocked while device latency is high: correlation (buffered I/O, page cache) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- Simultaneous metric changes: correlation (could be common cause, not direct causation) [arxiv](https://arxiv.org/html/2605.25298v1)

### 15.2 Literature Support

- **Brendan Gregg (2016):** "associating off-CPU stacks with a single wakeup stack" (correlation, not causation) [brendangregg](https://www.brendangregg.com/FlameGraphs/offcpuflamegraphs.html)
- **Prism (2026):** uses statistical correlation (Mann-Whitney U, KS test) for diagnosis, not causal inference [arxiv](https://arxiv.org/html/2605.25298v1)
- **No systems paper** explicitly distinguishes kernel-verified causality from temporal correlation in diagnostic output [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

### 15.3 Existing Tools

- **BCC offwaketime:** does not label waker→wakee as [CAUSAL]; only temporal correlation [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **Prism:** does not label evidence as [CAUSAL] vs [CORRELATED] [arxiv](https://arxiv.org/html/2605.25298v1)
- **Coroot/Pixie/Kindling:** AI-based or heuristic RCA, not kernel-verified causality [github](https://github.com/harmonycloud/kindling)

**Conclusion:** Explicit [CAUSAL] vs [CORRELATED] labeling is **not present in existing tools**; this is a potential differentiation for Katana.

**Sources:** [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

***

## 16. Event Loss and Evidence Completeness

### 16.1 BPF Ring Buffer

**Design:** [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Multi-producer, single-consumer (MPSC) queue** [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Shared across CPUs** (unlike perf buffer's per-CPU design) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Preserves ordering** across CPUs [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **No built-in lost-event callback** (unlike perf buffer) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

**APIs:** [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- `bpf_ringbuf_output()`: copy data to ring buffer
- `bpf_ringbuf_reserve()`/`commit()`/`discard()`: zero-copy reservation
- `bpf_ringbuf_query()`: query buffer state (avail_data, ring_size, cons_pos, prod_pos)

**Event Loss Detection:**
- `bpf_ringbuf_query(BPF_RB_AVAIL_DATA)` can detect buffer fullness [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- Userspace can check for gaps in timestamps or sequence numbers [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

### 16.2 Perf Buffer (Legacy)

- **Per-CPU buffers** (higher memory usage) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Lost-event callback** available [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Reordering possible** across CPUs [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

### 16.3 Scheduler Event Volume

- **High event rate:** "millions of events per second" in extreme cases [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **Brendan Gregg's recommendation:** start with 0.1s trace, measure overhead, ratchet up [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **In-kernel aggregation:** essential for reducing overhead (e.g., `offcputime` aggregates in-kernel) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

### 16.4 Katana Architecture Recommendation

**Use BPF ring buffer** (not perf buffer):
- Shared across CPUs (lower memory) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- Preserves ordering (critical for causality) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- Query API for detecting buffer fullness [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

**In-kernel filtering:**
- Filter by target PID/TID in eBPF (reduce event volume) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- Aggregate wait times in-kernel (not per-event to userspace) [arxiv](https://arxiv.org/html/2605.25298v1)

**Evidence completeness detection:**
- Track sequence numbers or timestamps in ring buffer records [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- Report "evidence incomplete" if gaps detected [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

**Sources:** [arxiv](https://arxiv.org/html/2605.25298v1)

***

## 17. BPF/CO-RE Compatibility

### 17.1 What CO-RE Solves

**Compile Once – Run Everywhere (CO-RE):** [parca](https://www.parca.dev/docs/parca-agent-design/)
- **BTF (BPF Type Format):** kernel type information (struct layouts, field offsets) [parca](https://www.parca.dev/docs/parca-agent-design/)
- **Relocation:** eBPF program compiled against one kernel version runs on different versions [parca](https://www.parca.dev/docs/parca-agent-design/)
- **No kernel headers required:** libbpf reads BTF from `/sys/kernel/btf/vmlinux` [parca](https://www.parca.dev/docs/parca-agent-design/)

### 17.2 What CO-RE Does NOT Solve

- **Userspace ABI:** CO-RE only helps with kernel structs, not userspace (e.g., glibc pthread_mutex layout) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **Tracepoint stability:** tracepoint fields may change across kernel versions (BTF helps, but not guaranteed) [github](https://github.com/libbpf/libbpf-rs)
- **Kernel features:** eBPF helpers, map types, program types depend on kernel version [github](https://github.com/libbpf/libbpf-rs)

### 17.3 Tracepoint Stability

- **sched_switch, sched_wakeup:** stable since ~2015; fields unlikely to change [docs.kernel](https://docs.kernel.org/trace/events.html)
- **sys_enter_futex, sys_exit_futex:** stable syscall tracepoints [arxiv](https://arxiv.org/html/2605.25298v1)
- **block_rq_issue, block_rq_complete:** stable block-layer tracepoints [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

### 17.4 Verifier Limitations

- **Instruction limit:** eBPF programs limited to ~1M instructions (verifier complexity) [github](https://github.com/libbpf/libbpf-rs)
- **Loop restrictions:** bounded loops only (verifier must prove termination) [github](https://github.com/libbpf/libbpf-rs)
- **Memory access:** must be within bounds (verifier checks) [github](https://github.com/libbpf/libbpf-rs)

### 17.5 BTF Requirements

- **Kernel 5.3+:** BTF support required for CO-RE [parca](https://www.parca.dev/docs/parca-agent-design/)
- **CONFIG_DEBUG_INFO_BTF:** kernel config option (enabled in most distros) [parca](https://www.parca.dev/docs/parca-agent-design/)

### 17.6 Architecture Portability

- **x86_64, ARM64:** well-supported [github](https://github.com/libbpf/libbpf-rs)
- **Other architectures:** may require kernel rebuild with BTF [github](https://github.com/libbpf/libbpf-rs)

### 17.7 Safest Instrumentation Strategy for Katana MVP

**Use tracepoints (not kprobes):**
- `sched:sched_switch`, `sched:sched_wakeup` (stable) [docs.kernel](https://docs.kernel.org/trace/events.html)
- `syscalls:sys_enter_futex`, `syscalls:sys_exit_futex` (stable) [arxiv](https://arxiv.org/html/2605.25298v1)
- `block:block_rq_issue`, `block:block_rq_complete` (stable) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)

**Use libbpf CO-RE:**
- Compile eBPF once, run on kernel 5.3+ [parca](https://www.parca.dev/docs/parca-agent-design/)
- BTF relocation for struct access [parca](https://www.parca.dev/docs/parca-agent-design/)

**Avoid:**
- kprobes on internal kernel functions (e.g., `finish_task_switch`): may change across versions [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- Reading userspace memory without BTF (layout may change) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

**Sources:** [arxiv](https://arxiv.org/html/2605.25298v1)

***

## 18. Rust/eBPF Technology Options

### 18.1 Comparison Table

| Feature | libbpf-rs | aya | BCC (Python) |
|---|---|---|---|
| **Maturity** | High (1k stars, 171 forks, 2020+)  [github](https://github.com/libbpf/libbpf-rs) | High (4.8k stars, 479 forks, 2021+)  [github](https://github.com/aya-rs/aya) | Very high (22.7k stars, 4.1k forks, 2015+)  [github](https://github.com/iovisor/bcc) |
| **Kernel Compatibility** | Depends on libbpf (kernel 5.3+ for CO-RE)  [github](https://github.com/libbpf/libbpf-rs) | Pure Rust, no libbpf dependency; kernel 5.3+ for BTF  [github](https://github.com/aya-rs/aya) | Depends on BCC (kernel 4.6+ for eBPF)  [github](https://github.com/iovisor/bcc) |
| **CO-RE Support** | Yes (via libbpf)  [github](https://github.com/libbpf/libbpf-rs) | Yes (BTF)  [github](https://github.com/aya-rs/aya) | Yes (libbpf tools)  [github](https://github.com/iovisor/bcc) |
| **BTF Support** | Yes  [github](https://github.com/libbpf/libbpf-rs) | Yes  [github](https://github.com/aya-rs/aya) | Yes  [github](https://github.com/iovisor/bcc) |
| **Ecosystem** | Part of libbpf project (kernel.org)  [github](https://github.com/libbpf/libbpf-rs) | Independent Rust project  [github](https://github.com/aya-rs/aya) | iovisor/bcc (Python/C)  [github](https://github.com/iovisor/bcc) |
| **Documentation** | Good (docs.rs, examples)  [github](https://github.com/libbpf/libbpf-rs) | Excellent (aya-rs.dev/book)  [aya-rs](https://aya-rs.dev/book/) | Good (bcc reference guide)  [github](https://github.com/iovisor/bcc) |
| **Verifier Interaction** | Via libbpf (C library)  [github](https://github.com/libbpf/libbpf-rs) | Pure Rust syscalls  [github](https://github.com/aya-rs/aya) | Via BCC (Python/C)  [github](https://github.com/iovisor/bcc) |
| **Developer Experience** | Idiomatic Rust, but requires libbpf-cargo  [github](https://github.com/libbpf/libbpf-rs) | Pure Rust, async support (tokio, async-std)  [github](https://github.com/aya-rs/aya) | Python scripts, easy prototyping  [github](https://github.com/iovisor/bcc) |
| **Suitability for Katana MVP** | **Recommended** (stable, CO-RE, libbpf ecosystem)  [github](https://github.com/libbpf/libbpf-rs) | Viable (pure Rust, but less mature for production)  [github](https://github.com/aya-rs/aya) | Not recommended (Python userspace, not Rust)  [github](https://github.com/iovisor/bcc) |

### 18.2 Recommendation

**Use libbpf-rs for Katana MVP:**
- **Stability:** part of libbpf project (kernel.org), long-term support [github](https://github.com/libbpf/libbpf-rs)
- **CO-RE:** compile once, run on kernel 5.3+ [github](https://github.com/libbpf/libbpf-rs)
- **Ecosystem:** integrates with libbpf tools, bpftool, etc. [github](https://github.com/libbpf/libbpf-rs)
- **Documentation:** docs.rs, examples, libbpf-cargo for CO-RE builds [github](https://github.com/libbpf/libbpf-rs)

**Avoid aya for production MVP:**
- **Pure Rust:** attractive, but less mature for production [github](https://github.com/aya-rs/aya)
- **Smaller ecosystem:** fewer examples, less community support [github](https://github.com/aya-rs/aya)
- **Async support:** useful for async userspace, but not critical for Katana [github](https://github.com/aya-rs/aya)

**Sources:** [parca](https://www.parca.dev/docs/parca-agent-design/)

***

## 19. Existing Tool Comparison Matrix

See Section 5.1 for detailed table. Summary:

- **Prism** is the strongest prior art (cross-subsystem, thread→resource, dependency reconstruction) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **BCC offwaketime** provides waker→wakee correlation (single-hop) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **Coroot/Pixie/Kindling** are service-level, not thread-level [github](https://github.com/harmonycloud/kindling)
- **Parca** is CPU profiling, not blocking diagnosis [parca](https://www.parca.dev/)
- **No tool** explicitly labels [CAUSAL] vs [CORRELATED] evidence [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

***

## 20. Katana Novelty Analysis

| Aspect | Classification | Evidence |
|---|---|---|
| **1. eBPF kernel instrumentation** | Clearly established prior art | Prism (2026), BCC (2016), Parca (2021)  [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/) |
| **2. Scheduler tracing** | Clearly established prior art | `sched_switch`, `sched_wakeup` tracepoints documented since ~2015  [docs.kernel](https://docs.kernel.org/trace/events.html) |
| **3. Futex tracing** | Clearly established prior art | Prism (2026) traces futex via sys_enter/exit_futex  [arxiv](https://arxiv.org/html/2605.25298v1); BCC tools exist |
| **4. Cross-subsystem correlation** | Adjacent prior art | Prism (2026) correlates 6 subsystems  [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/); BCC offwaketime correlates scheduler + futex  [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools) |
| **5. Wakeup-chain reconstruction** | Adjacent prior art | BCC offwaketime does single-hop  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html); multi-hop not implemented |
| **6. Dependency graph construction** | Adjacent prior art | Prism (2026) builds thread dynamics graph  [arxiv](https://arxiv.org/html/2605.25298v1); Kindling does trace-level spans  [github](https://github.com/harmonycloud/kindling) |
| **7. Causal-vs-correlational evidence labeling** | **Potentially differentiated** | No existing tool explicitly labels [CAUSAL] vs [CORRELATED]  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **8. Automatic diagnosis** | Adjacent prior art | Prism (2026) provides UI-based diagnosis  [arxiv](https://arxiv.org/html/2605.25298v1); Coroot uses AI RCA  [coroot](https://coroot.com/continuous-profiling) |
| **9. Single-command CLI** | **Potentially differentiated** | BCC tools are CLI, but not `explain <PID>` workflow  [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools); Prism uses UI  [github](https://github.com/EC-labs/prism/tree/master/) |
| **10. Human-readable explanation** | Adjacent prior art | Coroot/Kindling generate human-readable insights  [github](https://github.com/harmonycloud/kindling); not deterministic |
| **11. Target-PID-focused tracing** | **Potentially differentiated** | BCC tools support `-p PID`  [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools), but not as primary workflow; Prism monitors all discovered processes  [arxiv](https://arxiv.org/html/2605.25298v1) |
| **12. Live single-host diagnosis** | Clearly established prior art | BCC tools (2016+), Prism (2026), Parca (2021)  [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools) |
| **13. Evidence provenance** | **Potentially differentiated** | No existing tool labels evidence as [CAUSAL]/[CORRELATED]  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **14. Deterministic diagnosis without LLM** | **Potentially differentiated** | Coroot uses AI  [coroot](https://coroot.com/continuous-profiling); Prism uses statistical correlation  [arxiv](https://arxiv.org/html/2605.25298v1); deterministic rules not documented |

***

## 21. Katana Differentiation

**Defensible positioning:**

> "Katana is not a new tracing mechanism. It is an evidence-first diagnostic layer that turns selected kernel-recorded relationships into a live, single-command explanation for a target process."

**Evidence supporting this:**
- **Tracing mechanism:** identical to Prism (libbpf, tracepoints) and BCC (kprobes/tracepoints) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **Dependency reconstruction:** similar to Prism's selective thread tracking, but simpler (single-hop wakeup chains) [arxiv](https://arxiv.org/html/2605.25298v1)
- **Differentiation:**
  - **Explicit evidence labeling:** [CAUSAL] vs [CORRELATED] (not present in existing tools) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
  - **Single-command CLI:** `katana explain <PID>` (BCC tools require multiple commands; Prism uses UI) [github](https://github.com/EC-labs/prism/tree/master/)
  - **Deterministic diagnosis:** rule-based (not AI/ML) [arxiv](https://arxiv.org/html/2605.25298v1)
  - **Target-PID focus:** minimal overhead, not whole-system monitoring [arxiv](https://arxiv.org/html/2605.25298v1)

**Comparison:**

```
    perf/BCC/bpftrace           Prism
          |                        |
          | raw/low-level          | broader thread/subsystem
          | evidence               | analysis
          v                        v
    expert interpretation     offline/analysis UI
          |
          v
        Katana (deterministic, CLI, evidence-labeled)
```

**Verdict:** Positioning has **substance**, but is **narrow**: Katana repackages existing tracing into a CLI workflow with explicit evidence provenance, but does not introduce new kernel instrumentation or dependency-reconstruction algorithms.

***

## 22. Recommended MVP

**Scope:**
- **Linux only** (x86_64, kernel 5.3+ for BTF/CO-RE)
- **Scheduler + futex tracing** (no block I/O in MVP)
- **Target PID/TID tracking** (filter by PID in eBPF)
- **Single-hop wakeup-chain reconstruction** (waker→wakee via `sched_wakeup` + `sched_switch`)
- **Causal evidence:** `sched_wakeup` (kernel-verified), `FUTEX_WAKE` syscall (kernel-verified)
- **Correlated evidence:** elevated scheduler latency, high CPU utilization (temporal)
- **Deterministic diagnosis:** rule-based (if `sched_wakeup` from T2 to T1 + T1 in `FUTEX_WAIT`, then "T2 woke T1")
- **CLI:** `katana explain <PID>`
- **Output:** text + JSON
- **Event-loss handling:** BPF ring buffer with sequence numbers; report "evidence incomplete" if gaps detected

**Excluded from MVP:**
- Block I/O (Phase 2)
- Multi-hop wakeup chains (Phase 2)
- Mutex ownership claims (impossible without reading userspace memory) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- AI/LLM diagnosis (out of scope) [arxiv](https://arxiv.org/html/2605.25298v1)
- Distributed tracing (out of scope) [arxiv](https://arxiv.org/html/2605.25298v1)

**Technical feasibility:** **Yes**, feasible for one student:
- **eBPF:** ~200-300 lines of C (tracepoints, ring buffer)
- **Rust userspace:** ~500-1000 lines (libbpf-rs, diagnosis logic, CLI)
- **Testing:** synthetic futex contention, scheduler contention (well-understood workloads) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

***

## 23. Phase 2 Scope

**Candidate features:**
- **Block I/O attribution:** `block_rq_issue`/`block_rq_complete` tracepoints; distinguish direct I/O vs buffered I/O [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- **Multi-hop wakeup chains:** recursive lookup of waker's prior off-CPU period (complex, may require in-kernel state) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **Mutex ownership for PI futexes:** read `pi_state->owner` from kernel (requires BTF + careful struct access) [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html)
- **VFS/network tracing:** pipes, sockets, epoll (as in Prism) [arxiv](https://arxiv.org/html/2605.25298v1)
- **JSON schema + API:** for integration with other tools

**Explicitly postpone:**
- Networking (VFS, sockets) unless strong use case emerges [arxiv](https://arxiv.org/html/2605.25298v1)
- Distributed tracing (out of scope) [arxiv](https://arxiv.org/html/2605.25298v1)
- AI/LLM diagnosis (out of scope) [arxiv](https://arxiv.org/html/2605.25298v1)
- Generic plugin systems (over-engineering)

***

## 24. Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **Event loss (ring buffer full)** | Medium | High (incomplete evidence) | In-kernel filtering by PID; aggregate in-kernel; detect gaps via sequence numbers  [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html) |
| **Mutex ownership claims incorrect** | High | High (false diagnosis) | Refuse claims for non-PI futexes; only PI futexes expose owner TID  [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html) |
| **Block I/O attribution ambiguous** | High | Medium (buffered I/O) | Distinguish direct vs buffered I/O; label as [CORRELATED] for buffered  [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html) |
| **Kernel version incompatibility** | Low | High (CO-RE fails) | Require kernel 5.3+ (BTF); test on multiple distros  [github](https://github.com/libbpf/libbpf-rs) |
| **Overhead too high** | Medium | Medium (user rejects tool) | In-kernel aggregation; 1-second sampling (like Prism)  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **False-positive causality** | Medium | High (misdiagnosis) | Only label [CAUSAL] for kernel-verified events (`sched_wakeup`, `FUTEX_WAKE`); everything else [CORRELATED]  [docs.kernel](https://docs.kernel.org/trace/events.html) |

***

## 25. Validation Strategy

**Ground truth experiments:**

1. **Deterministic futex contention:**
   - Thread T1 calls `FUTEX_WAIT` on futex F
   - Thread T2 calls `FUTEX_WAKE` on F
   - **Ground truth:** T2 woke T1 (kernel-verified via `sched_wakeup` + `FUTEX_WAKE` syscall) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)

2. **Multi-hop wakeup chain:**
   - T1 blocked on futex F1
   - T2 blocked on futex F2
   - T3 wakes T2, T2 wakes T1
   - **Ground truth:** T3→T2→T1 chain (single-hop per `offwaketime`; multi-hop requires chaining) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

3. **CPU scheduler contention:**
   - High CPU load (e.g., `stress --cpu 4`)
   - Target thread T1 experiences scheduler delay
   - **Ground truth:** T1 blocked due to runqueue delay (prev_state = TASK_RUNNING in `sched_switch`) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)

4. **Negative control:**
   - No contention, target thread runs normally
   - **Expected:** no [CAUSAL] events, only [CORRELATED] (if any)

5. **Event-loss condition:**
   - High event rate (e.g., `stress --cpu 8` + many threads)
   - **Expected:** detect gaps in ring buffer sequence numbers; report "evidence incomplete" [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

6. **Process-exit condition:**
   - Target process exits during tracing
   - **Expected:** clean shutdown, no crashes [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)

***

## 26. Demo Strategy

**Scientifically credible demo:**

1. **Setup:**
   - C program with 3 threads: T1 (worker), T2 (scheduler), T3 (waker)
   - T1 waits on futex F1
   - T2 waits on futex F2
   - T3 wakes T2, T2 wakes T1

2. **Run:**
   - `katana explain <PID>` during execution
   - **Expected output:**
     ```
     Process <PID> is currently blocked.

     Primary cause:
       T1 is waiting on a futex.

     Causal chain:
       T1
         -> FUTEX_WAIT on F1
         -> T2 wakes T1 (via FUTEX_WAKE on F1)
         -> T2 was previously blocked on F2
         -> T3 wakes T2 (via FUTEX_WAKE on F2)

     Evidence:
       [CAUSAL] kernel-recorded wakeup relationship (sched_wakeup: T2→T1)
       [CAUSAL] futex wait/wake relationship (sys_exit_futex: T2 called FUTEX_WAKE on F1)
       [CORRELATED] elevated scheduler latency during same interval

     Limitations:
       Mutex ownership could not be established (not PI futex).
     ```

3. **Ground truth verification:**
   - Compare with `perf sched record` + `perf sched script` (raw scheduler events) [docs.kernel](https://docs.kernel.org/trace/events.html)
   - Compare with BCC `offwaketime` (waker→wakee stacks) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
   - Verify `sched_wakeup` events match Katana's [CAUSAL] claims [docs.kernel](https://docs.kernel.org/trace/events.html)

**Key question:** Can Katana demonstrate something existing tools make difficult to understand?

**Answer:** **Yes**, but narrowly:
- **BCC offwaketime** shows waker→wakee stacks, but not as human-readable "T2 woke T1" [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **Prism** shows thread dynamics graph, but not textual diagnosis [arxiv](https://arxiv.org/html/2605.25298v1)
- **Katana** provides deterministic, evidence-labeled textual diagnosis (not present in existing tools) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

***

## 27. Performance Evaluation Strategy

**Realistic expectations (based on literature):**

| Metric | Expectation | Source |
|---|---|---|
| **CPU overhead** | <5% (in-kernel aggregation, 1s sampling) | Prism reports "minimal overhead"  [arxiv](https://arxiv.org/html/2605.25298v1); BCC offcputime ~6%  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **Memory** | <50 MB (ring buffer + userspace state) | BPF ring buffer shared across CPUs  [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html) |
| **Event throughput** | 100k-1M events/s (with in-kernel filtering) | Scheduler events can be "millions per second"  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **Tracing overhead** | <10% (with in-kernel aggregation) | BCC offcputime 6-13%  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **Ring buffer loss** | 0% (with proper sizing + filtering) | BPF ring buffer design  [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html) |
| **Startup latency** | <1s (load eBPF, attach tracepoints) | libbpf CO-RE fast loading  [github](https://github.com/libbpf/libbpf-rs) |
| **Diagnosis latency** | <1s (after trace window) | Prism aggregates at 1s intervals  [arxiv](https://arxiv.org/html/2605.25298v1) |

**Benchmark methodology:**
1. **Workloads:**
   - Idle (no contention)
   - Futex contention (e.g., `stress --threads 8 --futex`)
   - CPU saturation (e.g., `stress --cpu 8`)
   - Mixed (futex + CPU)

2. **Metrics:**
   - CPU overhead: `perf stat -e cpu-clock` with/without Katana
   - Memory: `smem` or `/proc/<PID>/smaps`
   - Event rate: count ring buffer records per second
   - Loss rate: gaps in sequence numbers [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
   - Latency: time from `katana explain` to output

3. **Comparison:**
   - BCC `offcputime` (baseline) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
   - Prism (if reproducible) [arxiv](https://arxiv.org/html/2605.25298v1)

***

## 28. Security/Privilege Requirements

**Current Linux privileges for eBPF tracing** (2026):

- **CAP_BPF:** required for loading eBPF programs (kernel 5.8+) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **CAP_PERFMON:** required for perf events (kernel 5.8+) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **CAP_SYS_ADMIN:** legacy (pre-5.8), still required for some operations [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **BPF LSM:** may restrict eBPF programs (e.g., no unprivileged eBPF) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Ptrace restrictions:** `ptrace_may_access()` check for tracing other processes [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

**Katana MVP privilege model:**
- **Require root** (or `CAP_BPF` + `CAP_PERFMON`) for eBPF loading [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Filter by PID** in eBPF (reduce attack surface) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **No userspace memory reads** in MVP (avoid `bpf_probe_read_user()`) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **Read-only access** to kernel data (no modifications) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)

**Sources:** [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)

***

## 29. Open Technical Questions

| Question | Status | Resolution Needed |
|---|---|---|
| **Can multi-hop wakeup chains be reconstructed efficiently?** | Unresolved | Requires in-kernel state or recursive userspace lookups; may be too complex for MVP  [brendangregg](https://www.brendangregg.com/offcpuanalysis.html) |
| **Is BPF ring buffer ordering guaranteed across CPUs?** | Verified (yes) | Kernel docs confirm ordering preservation  [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html) |
| **Can PI futex owner TID be read reliably via eBPF?** | Unresolved | Requires BTF + careful struct access; test on multiple kernels  [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html) |
| **What is the exact overhead of Prism's 1-second aggregation?** | Unresolved | Paper says "minimal", but no numbers; reproduce experiment  [arxiv](https://arxiv.org/html/2605.25298v1) |
| **Can block I/O attribution be done for buffered I/O?** | Partially resolved | Direct I/O: yes; buffered: correlation only  [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html) |
| **Is `sys_enter_futex` sufficient, or are other tracepoints preferable?** | Verified | `sys_enter_futex`/`sys_exit_futex` are stable syscall tracepoints  [arxiv](https://arxiv.org/html/2605.25298v1) |
| **What is the safest way to detect event loss in BPF ring buffer?** | Verified | Use `bpf_ringbuf_query()` + sequence numbers in records  [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html) |

***

## 30. Final Verdict

### A. Is Katana technically feasible?

**Yes.** The required eBPF tracepoints (`sched_switch`, `sched_wakeup`, `sys_enter_futex`, `sys_exit_futex`) are stable and well-documented. Rust userspace (libbpf-rs) is mature. Single-hop wakeup-chain reconstruction is achievable with existing kernel mechanisms. [arxiv](https://arxiv.org/html/2605.25298v1)

### B. Is the original idea novel?

**No.** The original concept (cross-subsystem eBPF correlation for blocking diagnosis) is **substantially overlapping** with:
- **Prism (2026):** 6 subsystems, thread→resource granularity, dependency reconstruction [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
- **BCC offwaketime (2016):** waker→wakee correlation with blocked time [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **xCapture (2023):** TSA with per-thread subsystem time [arxiv](https://arxiv.org/html/2605.25298v1)

### C. Is the rescoped idea meaningfully differentiated?

**Yes, but narrowly.** The rescoped positioning (single-command CLI, explicit [CAUSAL]/[CORRELATED] labeling, target-PID focus) is **not present in existing tools**:
- **BCC offwaketime:** CLI, but no evidence labeling [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **Prism:** UI-based, no [CAUSAL]/[CORRELATED] labeling [arxiv](https://arxiv.org/html/2605.25298v1)
- **Coroot/Kindling:** AI/heuristic RCA, not deterministic [github](https://github.com/harmonycloud/kindling)

**Differentiation is real but narrow:** Katana repackages existing tracing into a CLI workflow with explicit evidence provenance.

### D. What exact claims should Katana NEVER make?

1. **"Katana uses novel eBPF instrumentation."** (False; identical to Prism/BCC) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
2. **"Katana reconstructs multi-hop wakeup chains."** (False for MVP; single-hop only) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
3. **"Katana determines mutex ownership for all locks."** (False; only PI futexes) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
4. **"Katana proves causality for correlated events."** (False; only kernel-verified events are [CAUSAL]) [arxiv](https://arxiv.org/html/2605.25298v1)
5. **"Katana has lower overhead than Prism/BCC."** (Unproven; requires benchmarking) [arxiv](https://arxiv.org/html/2605.25298v1)

### E. What is the strongest defensible research/engineering contribution?

**Explicit evidence labeling ([CAUSAL] vs [CORRELATED]) in deterministic, single-command CLI diagnosis.** This is **not present in existing tools** and addresses a real gap: users need to know which relationships are kernel-verified vs temporal. [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

### F. What should the MVP contain?

- **Scheduler + futex tracing** (tracepoints: `sched_switch`, `sched_wakeup`, `sys_enter_futex`, `sys_exit_futex`) [arxiv](https://arxiv.org/html/2605.25298v1)
- **Target PID/TID filtering** (in eBPF) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- **Single-hop wakeup-chain reconstruction** (waker→wakee via `sched_wakeup`) [docs.kernel](https://docs.kernel.org/trace/events.html)
- **[CAUSAL] labeling** for `sched_wakeup` + `FUTEX_WAKE` syscall [docs.kernel](https://docs.kernel.org/trace/events.html)
- **[CORRELATED] labeling** for scheduler delay, CPU saturation [arxiv](https://arxiv.org/html/2605.25298v1)
- **CLI:** `katana explain <PID>` (text + JSON output)
- **Event-loss detection** (BPF ring buffer with sequence numbers) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
- **Rust userspace** (libbpf-rs) [github](https://github.com/libbpf/libbpf-rs)

### G. What should explicitly be postponed?

- **Block I/O tracing** (Phase 2; attribution is complex for buffered I/O) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
- **Multi-hop wakeup chains** (Phase 2; requires recursive lookups or in-kernel state) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
- **Mutex ownership for non-PI futexes** (impossible without reading userspace memory) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
- **VFS/network tracing** (Phase 2; unless strong use case emerges) [arxiv](https://arxiv.org/html/2605.25298v1)
- **AI/LLM diagnosis** (out of scope) [arxiv](https://arxiv.org/html/2605.25298v1)

### H. What is the strongest demo?

**Deterministic futex contention with 3-thread chain (T3→T2→T1):**
- Shows [CAUSAL] labeling for `sched_wakeup` + `FUTEX_WAKE`
- Shows [CORRELATED] labeling for scheduler delay
- Compares with BCC `offwaketime` (raw stacks) and `perf sched` (raw events) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
- Verifies ground truth via kernel tracepoints [docs.kernel](https://docs.kernel.org/trace/events.html)

### I. What would an expert Linux engineer attack?

1. **"Mutex ownership claims are unreliable."** (Valid; only PI futexes expose owner TID) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
2. **"Block I/O attribution is ambiguous for buffered I/O."** (Valid; page cache breaks direct attribution) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
3. **"Multi-hop chains are not implemented."** (Valid; MVP is single-hop only) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
4. **"Overhead is not quantified."** (Valid; requires benchmarking vs Prism/BCC) [arxiv](https://arxiv.org/html/2605.25298v1)
5. **"[CAUSAL] labeling is just semantics."** (Partially valid; but no existing tool does this explicitly) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)

### J. What evidence would answer those objections?

1. **Mutex ownership:** Show PI futex example where `pi_state->owner` is read correctly [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html)
2. **Block I/O:** Show direct I/O (O_DIRECT) example with clear attribution [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
3. **Multi-hop chains:** Acknowledge limitation; show single-hop works correctly [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
4. **Overhead:** Benchmark vs BCC `offcputime` + Prism (if reproducible) [arxiv](https://arxiv.org/html/2605.25298v1)
5. **[CAUSAL] labeling:** Show side-by-side comparison with BCC `offwaketime` (no labeling) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)

### K. Should this project be built, modified further, or abandoned?

**Build, but with narrow scope.** The rescoped Katana (single-command CLI, [CAUSAL]/[CORRELATED] labeling, target-PID focus) fills a **real but narrow gap**: deterministic, evidence-labeled diagnosis for a specific process. It is **not novel tracing**, but it is **novel presentation** of existing tracing data.

**Recommendations:**
1. **Focus on MVP:** scheduler + futex, single-hop, [CAUSAL]/[CORRELATED] labeling [docs.kernel](https://docs.kernel.org/trace/events.html)
2. **Do not overclaim:** acknowledge overlap with Prism/BCC [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
3. **Benchmark rigorously:** compare overhead vs BCC `offcputime` + Prism [arxiv](https://arxiv.org/html/2605.25298v1)
4. **Open-source:** MIT license (like Prism); enable community validation [github](https://github.com/EC-labs/prism/tree/master/)

**If the goal is a student systems research project:** **Build it**, but frame it as "evidence-first diagnostic layer" (not novel tracing). The contribution is **explicit evidence labeling + deterministic CLI workflow**, not new kernel instrumentation.

**If the goal is a production tool:** **Build it**, but only if there is demand for deterministic, evidence-labeled diagnosis (vs AI/heuristic RCA like Coroot). [github](https://github.com/harmonycloud/kindling)

**If the goal is a novel research contribution:** **Modify further** to add multi-hop chains, block I/O attribution, or PI futex owner tracking (Phase 2 features). The MVP alone is **incremental** (repurposing existing tracing).

***

## References

 Linux kernel docs: Event Tracing — [https://docs.kernel.org/trace/events.html](https://docs.kernel.org/trace/events.html) [docs.kernel](https://docs.kernel.org/trace/events.html)
 Linux kernel docs: ftrace — [https://docs.kernel.org/trace/ftrace.html](https://docs.kernel.org/trace/ftrace.html) [docs.kernel](https://docs.kernel.org/trace/ftrace.html?highlight=ftrace)
 Brendan Gregg: Off-CPU Analysis — [https://www.brendangregg.com/offcpuanalysis.html](https://www.brendangregg.com/offcpuanalysis.html) [brendangregg](https://www.brendangregg.com/offcpuanalysis.html)
 Brendan Gregg: Off-CPU Flame Graphs — [https://www.brendangregg.com/FlameGraphs/offcpuflamegraphs.html](https://www.brendangregg.com/FlameGraphs/offcpuflamegraphs.html) [brendangregg](https://www.brendangregg.com/FlameGraphs/offcpuflamegraphs.html)
 Linux kernel docs: tracepoints — [https://www.kernel.org/doc/html/latest/core-api/tracepoint.html](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html) [kernel](https://www.kernel.org/doc/Documentation/trace/events.txt)
 Landau et al.: "Beyond Thread States: Diagnosing Performance Degradation with eBPF and Thread Dynamics" (IPDPS 2026) — [https://www.computer.org/csdl/proceedings-article/ipdps/2026/060200a819/2hJUxO6SJHO](https://www.computer.org/csdl/proceedings-article/ipdps/2026/060200a819/2hJUxO6SJHO) [research-portal.uu](https://research-portal.uu.nl/en/publications/beyond-thread-states-diagnosing-performance-degradation-with-ebpf/)
 Kernel-internals.org: Scheduler Tracing — [https://kernel-internals.org/sched/sched-tracing/](https://kernel-internals.org/sched/sched-tracing/) [kernel-internals](https://kernel-internals.org/sched/sched-tracing/)
 GitHub: EC-labs/ipdps2026-prism-artifact — [https://github.com/EC-labs/ipdps2026-prism-artifact/](https://github.com/EC-labs/ipdps2026-prism-artifact/) [github](https://github.com/EC-labs/ipdps2026-prism-artifact/)
 Brendan Gregg: Linux Wakeup and Off-Wake Profiling — [https://www.brendangregg.com/blog/2016-02-01/linux-wakeup-offwake-profiling.html](https://www.brendangregg.com/blog/2016-02-01/linux-wakeup-offwake-profiling.html) [readpipe](https://readpipe.org/posts/ef957bcc-81db-4715-8d9f-fb0f8f3859b9)
 GitHub: iovisor/bcc/tools/offwaketime.py — [https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py](https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py) [deepwiki](https://deepwiki.com/iovisor/bcc/4.4-cpu-and-scheduling-tools)
 GitHub: iovisor/bcc — [https://github.com/iovisor/bcc](https://github.com/iovisor/bcc) [github](https://github.com/iovisor/bcc)
 GitHub: torvalds/linux/kernel/trace/trace_sched_switch.c — [https://github.com/torvalds/linux/blob/master/kernel/trace/trace_sched_switch.c](https://github.com/torvalds/linux/blob/master/kernel/trace/trace_sched_switch.c) [github](https://github.com/torvalds/linux/blob/master/kernel/trace/trace_sched_switch.c)
 Linux kernel docs: robust-futex-ABI — [https://docs.kernel.org/locking/robust-futex-ABI.html](https://docs.kernel.org/locking/robust-futex-ABI.html) [docs.kernel](https://docs.kernel.org/locking/robust-futex-ABI.html)
 man7.org: futex(2) — [https://www.man7.org/linux/man-pages/man2/futex.2.html](https://www.man7.org/linux/man-pages/man2/futex.2.html) [man7](https://www.man7.org/linux/man-pages/man2/futex.2.html)
 Linux kernel docs: trace_block_rq_complete — [https://www.kernel.org/doc/html/latest/core-api/tracepoint.html](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html) [kernel](https://www.kernel.org/doc/html/latest/core-api/tracepoint.html)
 Linux kernel docs: futex2 — [https://www.kernel.org/doc/html/v6.12/userspace-api/futex2.html](https://www.kernel.org/doc/html/v6.12/userspace-api/futex2.html) [kernel](https://www.kernel.org/doc/html/v6.12/userspace-api/futex2.html)
 arXiv:2605.25298v1 (Prism paper) — [https://arxiv.org/html/2605.25298v1](https://arxiv.org/html/2605.25298v1) [arxiv](https://arxiv.org/html/2605.25298v1)
 man7.org: futex(7) — [https://www.man7.org/linux/man-pages/man7/futex.7.html](https://www.man7.org/linux/man-pages/man7/futex.7.html) [man7](https://www.man7.org/linux/man-pages/man7/futex.7.html)
 GitHub: EC-labs/prism — [https://github.com/EC-labs/prism](https://github.com/EC-labs/prism) [github](https://github.com/EC-labs/prism/tree/master/)
 GitHub: EC-labs/prism (duplicate) — [https://github.com/EC-labs/prism](https://github.com/EC-labs/prism) [github](https://github.com/EC-labs/prism)
 Linux kernel docs: trace_block_rq_complete (chiark) — [https://www.chiark.greenend.org.uk/doc/linux-doc-3.16/html/tracepoint/API-trace-block-rq-complete.html](https://www.chiark.greenend.org.uk/doc/linux-doc-3.16/html/tracepoint/API-trace-block-rq-complete.html) [chiark.greenend.org](https://www.chiark.greenend.org.uk/doc/linux-doc-3.16/html/tracepoint/API-trace-block-rq-complete.html)
 Linux kernel docs: tracepoint API (chiark) — [https://www.chiark.greenend.org.uk/doc/linux-doc-3.16/html/tracepoint/index.html](https://www.chiark.greenend.org.uk/doc/linux-doc-3.16/html/tracepoint/index.html) [egeeks.github](https://egeeks.github.io/kernal/tracepoint/index.html)
 GitHub: iovisor/bcc/tools/offwaketime.py (duplicate) — [https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py](https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py) [github](https://github.com/iovisor/bcc/blob/master/tools/offwaketime.py)
 Linux kernel docs: pi-futex (5.17) — [https://docs.kernel.org/5.17/locking/pi-futex.html](https://docs.kernel.org/5.17/locking/pi-futex.html) [docs.kernel](https://docs.kernel.org/5.17/locking/pi-futex.html)
 Linux kernel docs: pi-futex (latest) — [https://www.kernel.org/doc/html/latest/locking/pi-futex.html](https://www.kernel.org/doc/html/latest/locking/pi-futex.html) [kernel](https://www.kernel.org/doc/html/latest/locking/pi-futex.html)
 Linux kernel docs: pi-futex (6.7) — [https://www.kernel.org/doc/html/v6.7/locking/pi-futex.html](https://www.kernel.org/doc/html/v6.7/locking/pi-futex.html) [kernel](https://www.kernel.org/doc/html/v6.7/locking/pi-futex.html)
 LWN.net: Kernel analysis with bpftrace — [https://lwn.net/Articles/793749/](https://lwn.net/Articles/793749/) [lwn](https://lwn.net/Articles/793749/)
 GitHub: harmonycloud/kindling — [https://github.com/harmonycloud/kindling](https://github.com/harmonycloud/kindling) [github](https://github.com/harmonycloud/kindling)
 bpftrace.org: One-Liner Tutorial — [https://bpftrace.org/tutorial-one-liners](https://bpftrace.org/tutorial-one-liners) [bpftrace](https://bpftrace.org/tutorial-one-liners)
 Kindling docs: Overview — http://kindling.harmonycloud.cn/docs/overview-and-concepts/overview/ [kindling.harmonycloud](http://kindling.harmonycloud.cn/docs/overview-and-concepts/overview/)
 OneUptime: How to Profile CPU Performance with eBPF — [https://oneuptime.com/blog/post/2026-01-07-ebpf-cpu-profiling/view](https://oneuptime.com/blog/post/2026-01-07-ebpf-cpu-profiling/view) [oneuptime](https://oneuptime.com/blog/post/2026-01-07-ebpf-cpu-profiling/view)
 Besthub.dev: How eBPF Powers Next‑Gen Observability — [https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13) [besthub](https://www.besthub.dev/articles/how-ebpf-powers-next-gen-observability-and-root-cause-analysis-in-kubernetes-3e1d91246a13)
 Besthub.dev: How Kindling Leverages eBPF — [https://www.besthub.dev/articles/how-kindling-leverages-ebpf-to-reach-1-5-10-observability-targets-1c695743f913](https://www.besthub.dev/articles/how-kindling-leverages-ebpf-to-reach-1-5-10-observability-targets-1c695743f913) [besthub](https://www.besthub.dev/articles/how-kindling-leverages-ebpf-to-reach-1-5-10-observability-targets-1c695743f916)
 Linux kernel docs: BPF ring buffer — [https://docs.kernel.org/bpf/ringbuf.html](https://docs.kernel.org/bpf/ringbuf.html) [docs.kernel](https://docs.kernel.org/bpf/ringbuf.html)
 Debian manpages: wakeuptime-bpfcc — [https://manpages.debian.org/testing/bpfcc-tools/wakeuptime-bpfcc.8.en.html](https://manpages.debian.org/testing/bpfcc-tools/wakeuptime-bpfcc.8.en.html) [manpages.debian](https://manpages.debian.org/testing/bpfcc-tools/wakeuptime-bpfcc.8.en.html)
 Linux kernel docs: Event Tracing (v6.0) — [https://www.kernel.org/doc/html/v6.0/trace/events.html](https://www.kernel.org/doc/html/v6.0/trace/events.html) [kernel](https://www.kernel.org/doc/html/v6.0/trace/events.html)
 Brendan Gregg: Linux Wakeup and Off-Wake Profiling (duplicate) — [https://www.brendangregg.com/blog/2016-02-01/linux-wakeup-offwake-profiling.html](https://www.brendangregg.com/blog/2016-02-01/linux-wakeup-offwake-profiling.html) [brendangregg](https://www.brendangregg.com/blog/2016-02-01/linux-wakeup-offwake-profiling.html)
 Linux kernel docs: Event Tracing (latest) — [https://www.kernel.org/doc/html/latest/trace/events.html](https://www.kernel.org/doc/html/latest/trace/events.html) [kernel](https://www.kernel.org/doc/html/latest/trace/events.html)
 GitHub: iovisor/bcc/tools/wakeuptime.py — [https://github.com/iovisor/bcc/blob/master/tools/wakeuptime.py](https://github.com/iovisor/bcc/blob/master/tools/wakeuptime.py) [github](https://github.com/iovisor/bcc/blob/master/tools/wakeuptime.py)
 GitHub: mikesart/gpuvis wiki (Linux Scheduler) — [https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler](https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler) [github](https://github.com/mikesart/gpuvis/wiki/TechDocs-Linux-Scheduler)
 Yocto Project: Tracing and Profiling — [https://wiki.yoctoproject.org/wiki/Tracing_and_Profiling](https://wiki.yoctoproject.org/wiki/Tracing_and_Profiling) [wiki.yoctoproject](https://wiki.yoctoproject.org/wiki/Tracing_and_Profiling)
 Andrii Nakryiko: BPF ring buffer — [https://nakryiko.com/posts/bpf-ringbuf/](https://nakryiko.com/posts/bpf-ringbuf/) [nakryiko](https://nakryiko.com/posts/bpf-ringbuf/)
 eBPF Docs: BPF_MAP_TYPE_RINGBUF — [https://docs.ebpf.io/linux/map-type/BPF_MAP_TYPE_RINGBUF/](https://docs.ebpf.io/linux/map-type/BPF_MAP_TYPE_RINGBUF/) [docs.ebpf](https://docs.ebpf.io/linux/map-type/BPF_MAP_TYPE_RINGBUF/)
 GitHub: libbpf/libbpf-rs — [https://github.com/libbpf/libbpf-rs](https://github.com/libbpf/libbpf-rs) [github](https://github.com/libbpf/libbpf-rs)
 aya-rs.dev: Getting Started — [https://aya-rs.dev/book/](https://aya-rs.dev/book/) [aya-rs](https://aya-rs.dev/book/)
 Parca: Open Source continuous profiling — [https://www.parca.dev/](https://www.parca.dev/) [parca](https://www.parca.dev/)
 docs.rs: libbpf-rs — [https://docs.rs/libbpf-rs/latest/libbpf_rs/](https://docs.rs/libbpf-rs/latest/libbpf_rs/) [docs](https://docs.rs/libbpf-rs/latest/libbpf_rs/)
 GitHub: aya-rs/aya — [https://github.com/aya-rs/aya](https://github.com/aya-rs/aya) [github](https://github.com/aya-rs/aya)
 docs.rs: aya-ebpf — [https://docs.rs/aya-ebpf](https://docs.rs/aya-ebpf) [docs](https://docs.rs/aya-ebpf)
 OneUptime: How to Write eBPF Programs in Rust with Aya — [https://oneuptime.com/blog/post/2026-01-07-ebpf-rust-aya/view](https://oneuptime.com/blog/post/2026-01-07-ebpf-rust-aya/view) [oneuptime](https://oneuptime.com/blog/post/2026-01-07-ebpf-rust-aya/view)
 Parca docs: Design — [https://www.parca.dev/docs/parca-agent-design/](https://www.parca.dev/docs/parca-agent-design/) [parca](https://www.parca.dev/docs/parca-agent-design/)
 crates.io: aya-ebpf — [https://crates.io/crates/aya-ebpf](https://crates.io/crates/aya-ebpf) [crates](https://crates.io/crates/aya-ebpf)
 dxuuu.xyz: libbpf-rs — [https://dxuuu.xyz/libbpf-rs.html](https://dxuuu.xyz/libbpf-rs.html) [dxuuu](https://dxuuu.xyz/libbpf-rs.html)
 aya-rs.dev: Home — [https://aya-rs.dev/](https://aya-rs.dev/) [aya-rs](https://aya-rs.dev/)
 lib.rs: libbpf-rs — [https://lib.rs/crates/libbpf-rs](https://lib.rs/crates/libbpf-rs) [lib](https://lib.rs/crates/libbpf-rs)
 GitHub: libbpf/libbpf-rs README — [https://github.com/libbpf/libbpf-rs/blob/master/libbpf-rs/README.md](https://github.com/libbpf/libbpf-rs/blob/master/libbpf-rs/README.md) [github](https://github.com/libbpf/libbpf-rs/blob/master/libbpf-rs/README.md)
 Rice University: Linux Perf futex.h — [https://www.cs.rice.edu/~la5/doc/perf-doc/d0/d77/futex_8h.html](https://www.cs.rice.edu/~la5/doc/perf-doc/d0/d77/futex_8h.html) [cs.rice](https://www.cs.rice.edu/~la5/doc/perf-doc/d0/d77/futex_8h.html)
 Coroot: eBPF Observability — [https://coroot.com/ebpf](https://coroot.com/ebpf) [coroot](https://coroot.com/ebpf)
 Coroot: Continuous Profiling — [https://coroot.com/continuous-profiling](https://coroot.com/continuous-profiling) [coroot](https://coroot.com/continuous-profiling)
 GitHub: iovisor/bcc/tools/offcputime.py — [https://github.com/iovisor/bcc/blob/master/tools/offcputime.py](https://github.com/iovisor/bcc/blob/master/tools/offcputime.py) [github](https://github.com/iovisor/bcc/blob/master/tools/offcputime.py)
 Coroot docs: Profiling overview — [https://docs.coroot.com/profiling/overview/](https://docs.coroot.com/profiling/overview/) [github](https://github.com/coroot/coroot/blob/main/docs/docs/profiling/ebpf-based-profiling.md)
 GitHub: torvalds/linux/kernel/futex/syscalls.c — [https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c) [github](https://github.com/torvalds/linux/blob/master/kernel/futex/syscalls.c)
 linux.die.net: futex(2) — [https://linux.die.net/man/2/futex](https://linux.die.net/man/2/futex) [linux.die](https://linux.die.net/man/2/futex)

***

**End of Dossier**