# Katana Microbenchmarks & Performance Evaluation

**Date:** October 08, 2026  
**Methodology:** Conforms strictly to [PRD §24 (Benchmarking)](file:///home/topfloorboss/Desktop/KATANA/prd.md#L1198) and [PRD §32.1](file:///home/topfloorboss/Desktop/KATANA/prd.md#L1470)  
**Raw Data:** [`benchmarks/results_phase2.csv`](file:///home/topfloorboss/Desktop/KATANA/benchmarks/results_phase2.csv)

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
| **Physical NVMe Note** | Host motherboard contains dual SATA controllers (`sdb` SSD, `sda` HDD); physical PCIe NVMe controller is absent. High-IOPS block streams are benchmarked directly against the SSD filesystem and simulated 100k IOPS trace streams per OTQ-11. |

---

## 2. Experimental Methodology (PRD §24.1)

- **Trials:** $N = 20$ trials per workload ($N \ge 20$ [TARGET], exceeds $\ge 10$ [HARD floor]).
- **Warm-Up:** Initial iteration discarded before measurements begin.
- **Reporting:** Median, Interquartile Range ($\text{IQR} = Q3 - Q1$), Min, Max, and Mean with 95% Confidence Interval ($\text{CI}_{95}$). Single-run numbers are prohibited per PRD §24.1.

---

## 3. Results Summary

### 3.1 Workload A: Physical SATA SSD Synchronous Block Write (`fsync`)
Measures real hardware write latency on the host filesystem (`/home` on `/dev/sdb2` SATA SSD) by writing a 4 KiB block with `fsync` / `fdatasync`:

- **Median:** **3,327.00 µs** (3.33 ms)
- **IQR ($Q3 - Q1$):** 527.00 µs [$Q1$: 2,974.00 µs, $Q3$: 3,501.00 µs]
- **Min / Max:** 2,726.00 µs / 3,684.00 µs
- **Mean ($95\%$ CI):** 3,254.20 µs [3,128.75 µs, 3,379.65 µs]

*Analysis:* Conforms to expected flash commit times for consumer SATA SSDs (2.7–3.7 ms per synchronous flush).

### 3.2 Workload B: In-Kernel eBPF Raw Wire Frame Decoding
Measures decoding throughput of packed kernel frames (`kt_hdr` + `kt_block_issue` / `kt_block_complete`) into normalized `Event` representations via `Event::decode_raw` (100,000 iterations per trial):

- **Median:** **24.38 Mops/sec** (~41.0 ns per packet)
- **IQR ($Q3 - Q1$):** 1.41 Mops/sec [$Q1$: 23.69 Mops/sec, $Q3$: 25.10 Mops/sec]
- **Min / Max:** 16.92 Mops/sec / 25.83 Mops/sec
- **Mean ($95\%$ CI):** 23.95 Mops/sec [23.12 Mops/sec, 24.79 Mops/sec]

*Analysis:* Zero-dependency binary decoding sustains >24 million events/sec per core, well exceeding any peak ring-buffer drain rate (PRD §9 target: < 100 ns/event).

### 3.3 Workload C: Cell B4 Engine Analysis Throughput
Measures pure offline replay analysis throughput across a 50,000-event synthetic block I/O stream (simulating 100k IOPS block activity with open request tracking, sleep matching, graph construction, and lexicographic ranking):

- **Median:** **253,195.64 events/sec** (~3.95 µs per event)
- **IQR ($Q3 - Q1$):** 33,593.00 events/sec [$Q1$: 246,956.63, $Q3$: 280,549.63]
- **Min / Max:** 231,998.57 / 285,309.61 events/sec
- **Mean ($95\%$ CI):** 259,925.72 events/sec [252,216.04, 267,635.40]

*Analysis:* A 3,000 ms observation window with 100,000 events is processed in ~390 ms total analysis latency, easily surpassing the PRD Cell B6 explain latency target.

---

## 4. Benchmark Harness Execution

The release benchmark can be reproduced deterministically with:

```bash
cargo run --release --bin katana-bench
```
Raw trial logs are saved to `benchmarks/results_phase2.csv`.
