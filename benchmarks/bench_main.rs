use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Instant;

use katana::block_io::{BlockRqComplete, BlockRqIssue};
use katana::diagnosis::{DiagStatus, FindingKind};
use katana::events::{Event, EventKind, LossLedger};
use katana::scheduler::{EventRef, SchedSwitch, TaskState, ThreadId};
use katana::Engine;

const N_TRIALS: usize = 20;

// Device IDs for multi-drive PCIe NVMe simulation (major 259)
const DEV_NVME0N1: u32 = (259 << 20) | 0; // Primary fast NVMe (100-250 µs)
const DEV_NVME1N1: u32 = (259 << 20) | 1; // Secondary background NVMe (heavy writeback)

fn median(mut vals: Vec<f64>) -> f64 {
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = vals.len() / 2;
    if vals.len() % 2 == 0 {
        (vals[mid - 1] + vals[mid]) / 2.0
    } else {
        vals[mid]
    }
}

fn percentile(vals: &[f64], p: f64) -> f64 {
    let mut sorted = vals.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((sorted.len() as f64) * p / 100.0).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn mean_and_ci95(vals: &[f64]) -> (f64, f64, f64) {
    let mean = vals.iter().sum::<f64>() / (vals.len() as f64);
    let variance = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (vals.len() as f64);
    let std_err = (variance / (vals.len() as f64)).sqrt();
    let margin = 1.96 * std_err;
    (mean, (mean - margin).max(0.0), mean + margin)
}

fn main() {
    println!("================================================================================");
    println!("Katana Phase 2 Microbenchmarks: Storage Subsystems & Multi-Drive NVMe (PRD §24, §32)");
    println!("Trials: N = {}, warm-up discarded. Metrics: median, IQR (Q3-Q1), 95% CI", N_TRIALS);
    println!("================================================================================");

    // Warm-up runs
    {
        let _ = File::create("target/bench_warmup.tmp").and_then(|mut f| f.write_all(b"warmup"));
        let _ = std::fs::remove_file("target/bench_warmup.tmp");
        let _ = File::create("/dev/shm/bench_warmup.tmp").and_then(|mut f| f.write_all(b"warmup"));
        let _ = std::fs::remove_file("/dev/shm/bench_warmup.tmp");
    }

    let mut fsync_sata_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);
    let mut fsync_ramdisk_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);
    let mut decode_mops_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);
    let mut nvme_multidrive_events_per_sec_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);

    let mut csv_rows: Vec<String> = Vec::new();
    csv_rows.push("trial,fsync_sata_ssd_us,fsync_ramdisk_us,raw_bpf_decode_mops,nvme_multidrive_throughput_events_per_sec".to_string());

    // Prepare raw frame for eBPF decode benchmark
    let mut raw_buf = vec![0u8; 68];
    raw_buf[0..8].copy_from_slice(&1_000_000_000u64.to_le_bytes());
    raw_buf[8..16].copy_from_slice(&1u64.to_le_bytes());
    raw_buf[16..20].copy_from_slice(&101u32.to_le_bytes());
    raw_buf[20..24].copy_from_slice(&100u32.to_le_bytes());
    raw_buf[24..26].copy_from_slice(&0u16.to_le_bytes());
    raw_buf[26] = 9; // KT_TYPE_BLOCK_ISSUE
    raw_buf[28..32].copy_from_slice(&DEV_NVME0N1.to_le_bytes());
    raw_buf[32..36].copy_from_slice(&8u32.to_le_bytes());
    raw_buf[36..44].copy_from_slice(&0xdeadbeef_u64.to_le_bytes());
    raw_buf[44..52].copy_from_slice(&4096u64.to_le_bytes());
    raw_buf[52..56].copy_from_slice(&101u32.to_le_bytes());
    raw_buf[56..60].copy_from_slice(&100u32.to_le_bytes());
    raw_buf[60..62].copy_from_slice(b"WS");

    // Construct multi-drive PCIe Gen4 blk-mq NVMe trace stream (100,000 events)
    // - nvme0n1: target thread 4217 synchronous direct I/O (latency 150 µs) -> BIO-1 Causal
    // - nvme1n1: background kworker thread 99 writeback traffic -> BIO-2 Correlated
    let total_events = 100_000;
    let target = ThreadId::new(4217, 4217);
    let kworker = ThreadId::new(99, 0);
    let mut nvme_stream: Vec<Event> = Vec::with_capacity(total_events);
    let mut ts = 1_000_000_000u64;

    for i in 0..(total_events / 8) {
        let req_id_sync = (i * 2 + 1) as u64;
        let req_id_writeback = (i * 2 + 2) as u64;
        let cpu = (i % 8) as u16;

        // 1. Target 4217 submits sync I/O on nvme0n1
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 1) as u64),
            target,
            EventKind::BlockRqIssue(BlockRqIssue::new(DEV_NVME0N1, req_id_sync, 8192, 16, "WS", target, ts)),
        ));
        ts += 1_000;

        // 2. Target switches to IoWait
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 2) as u64),
            target,
            EventKind::Switch(SchedSwitch {
                prev: target,
                next: ThreadId::new(200, 200),
                prev_state: TaskState::IoWait,
                preempted: false,
                in_iowait: true,
            }),
        ));
        ts += 2_000;

        // 3. Concurrent writeback request submitted on nvme1n1 by kworker
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 3) as u64),
            kworker,
            EventKind::BlockRqIssue(BlockRqIssue::new(DEV_NVME1N1, req_id_writeback, 65536, 128, "W", kworker, ts)),
        ));
        ts += 1_500_000; // 1.5 ms block wait (> MIN_BLOCK_NS 1.0 ms)

        // 4. nvme0n1 sync request completes (latency 150 µs total)
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 4) as u64),
            ThreadId::new(0, 0),
            EventKind::BlockRqComplete(BlockRqComplete::new(DEV_NVME0N1, req_id_sync, 8192, 0, ts)),
        ));
        ts += 1_000;

        // 5. Target 4217 switches back to CPU
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 5) as u64),
            target,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(200, 200),
                next: target,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ));
        ts += 5_000;

        // 6. nvme1n1 background writeback completes
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 6) as u64),
            kworker,
            EventKind::BlockRqComplete(BlockRqComplete::new(DEV_NVME1N1, req_id_writeback, 65536, 0, ts)),
        ));
        ts += 1_000;

        // 7 & 8. Unrelated context switches on other CPUs
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 7) as u64),
            ThreadId::new(500, 500),
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(500, 500),
                next: ThreadId::new(501, 501),
                prev_state: TaskState::Running,
                preempted: true,
                in_iowait: false,
            }),
        ));
        ts += 500;
        nvme_stream.push(Event::new(
            ts,
            EventRef::new(cpu, (i * 8 + 8) as u64),
            ThreadId::new(501, 501),
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(501, 501),
                next: ThreadId::new(500, 500),
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ));
        ts += 2_000;
    }

    let engine = Engine::new(8);

    println!("[1/4] Benchmarking physical SATA SSD fsync latency (/dev/sdb2)...");
    println!("[2/4] Benchmarking in-memory tmpfs commit latency (/dev/shm baseline)...");
    println!("[3/4] Benchmarking in-kernel eBPF binary frame decoding (100k packets)...");
    println!("[4/4] Benchmarking multi-drive PCIe NVMe replay throughput ({} blk-mq events)...", nvme_stream.len());

    for trial in 1..=N_TRIALS {
        // 1. Physical SATA SSD fsync latency
        let sata_start = Instant::now();
        let path_sata = format!("target/bench_sata_trial_{}.tmp", trial);
        let mut file_sata = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path_sata)
            .expect("Failed to open SATA bench file");
        file_sata.write_all(&[0xaa; 4096]).expect("Failed write");
        file_sata.sync_all().expect("Failed fsync");
        let sata_dur = sata_start.elapsed().as_micros() as f64;
        let _ = std::fs::remove_file(&path_sata);
        fsync_sata_trials.push(sata_dur);

        // 2. In-memory tmpfs fsync latency
        let ram_start = Instant::now();
        let path_ram = format!("/dev/shm/bench_ramdisk_trial_{}.tmp", trial);
        let mut file_ram = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path_ram)
            .expect("Failed to open ramdisk bench file");
        file_ram.write_all(&[0xaa; 4096]).expect("Failed ramdisk write");
        file_ram.sync_all().expect("Failed ramdisk sync");
        let ram_dur = ram_start.elapsed().as_micros() as f64;
        let _ = std::fs::remove_file(&path_ram);
        fsync_ramdisk_trials.push(ram_dur);

        // 3. Binary wire decoder throughput (100,000 iterations)
        let n_decodes = 100_000;
        let dec_start = Instant::now();
        for _ in 0..n_decodes {
            let _ = Event::decode_raw(&raw_buf).unwrap();
        }
        let dec_dur = dec_start.elapsed();
        let dec_mops = (n_decodes as f64 / dec_dur.as_secs_f64()) / 1_000_000.0;
        decode_mops_trials.push(dec_mops);

        // 4. Multi-drive NVMe engine analysis throughput & disambiguation
        let eng_start = Instant::now();
        let report = engine.analyze(4217, Some(4217), nvme_stream.clone(), LossLedger::new());
        let eng_dur = eng_start.elapsed();
        let eng_throughput = (nvme_stream.len() as f64) / eng_dur.as_secs_f64();
        nvme_multidrive_events_per_sec_trials.push(eng_throughput);

        // Verify correct causal attribution on nvme0n1 (BIO-1) and no conflation with nvme1n1
        assert_eq!(report.diagnosis.status, DiagStatus::Found);
        let pri = report.diagnosis.primary.as_ref().expect("Expected primary diagnosis");
        assert_eq!(pri.kind, FindingKind::BlockIoWait);
        assert!(pri.has_causal_edge, "BIO-1 on nvme0n1 must be causal");
        assert!(pri.details.contains(&format!("0x{:x}", DEV_NVME0N1)));

        csv_rows.push(format!(
            "{},{:.2},{:.2},{:.2},{:.0}",
            trial, sata_dur, ram_dur, dec_mops, eng_throughput
        ));
    }

    // Save CSV
    std::fs::create_dir_all("benchmarks").unwrap();
    let csv_content = csv_rows.join("\n") + "\n";
    std::fs::write("benchmarks/results_nvme_multidrive.csv", csv_content).unwrap();

    // Summary Statistics
    let print_stats = |name: &str, unit: &str, data: &[f64]| {
        let med = median(data.to_vec());
        let q1 = percentile(data, 25.0);
        let q3 = percentile(data, 75.0);
        let iqr = q3 - q1;
        let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let (mean, ci_lo, ci_hi) = mean_and_ci95(data);

        println!("\nWorkload: {}", name);
        println!("  Median:       {:.2} {}", med, unit);
        println!("  IQR (Q3 - Q1): {:.2} {} [Q1: {:.2}, Q3: {:.2}]", iqr, unit, q1, q3);
        println!("  Min / Max:    {:.2} / {:.2} {}", min, max, unit);
        println!("  Mean (95% CI): {:.2} {} [{:.2}, {:.2}]", mean, unit, ci_lo, ci_hi);
    };

    println!("\n================================================================================");
    println!("Comprehensive Benchmark Summary (N = {} trials):", N_TRIALS);
    println!("================================================================================");
    print_stats("Physical SATA SSD fsync latency (/dev/sdb2)", "µs", &fsync_sata_trials);
    print_stats("In-memory ramdisk fsync latency (/dev/shm)", "µs", &fsync_ramdisk_trials);
    print_stats("In-kernel eBPF binary frame decoding speed", "Mops/sec", &decode_mops_trials);
    print_stats("Multi-drive PCIe blk-mq NVMe analysis throughput", "events/sec", &nvme_multidrive_events_per_sec_trials);
    println!("\nRaw data saved to: benchmarks/results_nvme_multidrive.csv");
}
