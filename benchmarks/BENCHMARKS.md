# Katana Microbenchmarks & Performance Evaluation

**Date:** October 08, 2026  
**Methodology:** Conforms strictly to [PRD §24 (Benchmarking)](file:///home/topfloorboss/Desktop/KATANA/prd.md#L1198) and [PRD §32.1](file:///home/topfloorboss/Desktop/KATANA/prd.md#L1470)  
**Raw Data:** [`benchmarks/results_nvme_multidrive.csv`](file:///home/topfloorboss/Desktop/KATANA/benchmarks/results_nvme_multidrive.csv) and [`benchmarks/results_phase2.csv`](file:///home/topfloorboss/Desktop/KATANA/benchmarks/results_phase2.csv)

---

## 1. Test Environment & Hardware Characterization

| Parameter | Specification |
|---|---|
| **Host System** | Arch Linux (CachyOS x86_64) |
| **Kernel Release** | `7.2.3-1-cachyos` (SMP PREEMPT_DYNAMIC) |
| **CPU Model** | Intel Core i7-8550U @ 1.80GHz (Max 4.00GHz) |
| **Topology** | 1 Socket, 4 Physical Cores, 8 Hardware Threads (SMT On) |
| **Microcode** | `0xf6` |
| **L1d / L2 / L3 Cache** | 128 KiB / 1 MiB / 8 MiB |
| **Primary Physical Disk** | `/dev/sdb` — LITEON CV8-8E128-11 SATA SSD (128 GB) |
| **Secondary Physical Disk** | `/dev/sda` — Seagate ST2000LM007-1R8174 SATA HDD (2 TB, 5400 RPM) |
| **Physical NVMe Note** | The host motherboard contains dual SATA controllers (`sdb` SSD, `sda` HDD); physical PCIe NVMe controller is absent. Multi-drive NVMe environments (`nvme0n1`, `nvme1n1`) are evaluated via blk-mq multi-queue trace streams at 100k events and compared with physical SSD and in-memory commit baselines. |

---

## 2. Experimental Methodology (PRD §24.1)

- **Trials:** $N = 20$ trials per workload ($N \ge 20$ [TARGET], exceeds $\ge 10$ [HARD floor]).
- **Warm-Up:** Initial iteration discarded before measurements begin.
- **Reporting:** Median, Interquartile Range ($\text{IQR} = Q3 - Q1$), Min, Max, and Mean with 95% Confidence Interval ($\text{CI}_{95}$). Single-run numbers are prohibited per PRD §24.1.

---

## 3. Results Summary

### 3.1 Storage Backend Comparison: Physical Flash vs. In-Memory Baseline

| Storage Target | Mechanism | Median Latency | IQR ($Q3 - Q1$) | Min / Max | Mean (95% CI) |
|---|---|---|---|---|---|
| **In-Memory Ramdisk (`/dev/shm`)** | VFS + pagecache sync | **27.00 µs** | 26.00 µs | 14.00 / 76.00 µs | 29.95 µs [23.07, 36.83] |
| **Target PCIe NVMe (Reference)** | NVMe blk-mq DMA | *100–300 µs* | *50–100 µs* | *80 / 400 µs* | *~150 µs* |
| **Physical SATA SSD (`/dev/sdb2`)** | Consumer flash commit | **3,392.00 µs** (3.39 ms) | 440.00 µs | 2,869.00 / 3,676.00 µs | 3,308.40 µs [3,191.00, 3,425.80] |

*Key Finding:* Physical consumer SATA flash exhibits ~10–30× higher write commit latency (3.39 ms) than enterprise PCIe NVMe devices (100–300 µs). The in-memory ramdisk establishes the OS kernel synchronization floor at 27.00 µs.

---

### 3.2 Workload B: In-Kernel eBPF Raw Wire Frame Decoding
Measures decoding throughput of packed kernel frames (`kt_hdr` + `kt_block_issue` / `kt_block_complete`) into normalized `Event` representations via `Event::decode_raw` (100,000 iterations per trial):

- **Median:** **23.41 Mops/sec** (~42.7 ns per packet)
- **IQR ($Q3 - Q1$):** 1.65 Mops/sec [$Q1$: 22.48 Mops/sec, $Q3$: 24.13 Mops/sec]
- **Min / Max:** 20.89 Mops/sec / 25.03 Mops/sec
- **Mean ($95\%$ CI):** 23.16 Mops/sec [22.67 Mops/sec, 23.65 Mops/sec]

*Analysis:* Zero-dependency binary decoding sustains >23 million events/sec per core, well exceeding peak ring-buffer drain requirements (< 100 ns/event).

---

### 3.3 Workload C: Multi-Drive PCIe blk-mq NVMe Engine Analysis Throughput
Evaluates Katana under a 100,000-event multi-drive PCIe Gen4 blk-mq trace stream simulating concurrent devices:
- **`nvme0n1`** (major 259, minor 0): Target thread direct synchronous I/O.
- **`nvme1n1`** (major 259, minor 1): Concurrent background kworker writeback.
- **Causal Disambiguation:** Katana successfully attributes `nvme0n1` as causal (`BlockIoWait`, Rule BIO-1) and isolates `nvme1n1` as correlated writeback (`BlockIoCorrelated`, Rule BIO-2) with **zero cross-device conflation**.

- **Median Analysis Throughput:** **176,195.35 events/sec** (~5.67 µs per event)
- **IQR ($Q3 - Q1$):** 21,188.50 events/sec [$Q1$: 158,364.90, $Q3$: 179,553.40]
- **Min / Max:** 151,225.55 / 180,682.15 events/sec
- **Mean ($95\%$ CI):** 169,729.80 events/sec [165,051.04, 174,408.56]

*Analysis:* Even with concurrent multi-device tracking, cross-drive disambiguation, and interval containment, Katana sustains ~176k events/sec. A 100,000-event window across multi-queue NVMe devices is completely analyzed in ~560 ms.

---

## 4. Benchmark Harness Execution

The release benchmark can be reproduced deterministically with:

```bash
cargo run --release --bin katana-bench
```
Raw trial logs are saved to `benchmarks/results_nvme_multidrive.csv`.
