use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Instant;

use katana::block_io::{BlockRqComplete, BlockRqIssue};
use katana::events::{Event, EventKind, LossLedger};
use katana::scheduler::{EventRef, SchedSwitch, TaskState, ThreadId};
use katana::Engine;

const N_TRIALS: usize = 20;

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
    println!("============================================================");
    println!("Katana Phase 2 Microbenchmarks (PRD §24, §32)");
    println!("N = {} trials, warm-up discarded, metrics: median, IQR, 95% CI", N_TRIALS);
    println!("============================================================");

    // Warm-up run
    {
        let _ = File::create("target/bench_warmup.tmp").and_then(|mut f| f.write_all(b"warmup"));
        let _ = std::fs::remove_file("target/bench_warmup.tmp");
    }

    let mut fsync_us_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);
    let mut decode_mops_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);
    let mut engine_events_per_sec_trials: Vec<f64> = Vec::with_capacity(N_TRIALS);

    let mut csv_rows: Vec<String> = Vec::new();
    csv_rows.push("trial,fsync_us,decode_mops,engine_throughput_events_per_sec".to_string());

    // Prepare raw frame for decode benchmark
    let mut raw_buf = vec![0u8; 68];
    raw_buf[0..8].copy_from_slice(&1_000_000_000u64.to_le_bytes());
    raw_buf[8..16].copy_from_slice(&1u64.to_le_bytes());
    raw_buf[16..20].copy_from_slice(&101u32.to_le_bytes());
    raw_buf[20..24].copy_from_slice(&100u32.to_le_bytes());
    raw_buf[24..26].copy_from_slice(&0u16.to_le_bytes());
    raw_buf[26] = 9; // KT_TYPE_BLOCK_ISSUE
    raw_buf[28..32].copy_from_slice(&0x80001u32.to_le_bytes());
    raw_buf[32..36].copy_from_slice(&8u32.to_le_bytes());
    raw_buf[36..44].copy_from_slice(&0xdeadbeef_u64.to_le_bytes());
    raw_buf[44..52].copy_from_slice(&4096u64.to_le_bytes());
    raw_buf[52..56].copy_from_slice(&101u32.to_le_bytes());
    raw_buf[56..60].copy_from_slice(&100u32.to_le_bytes());
    raw_buf[60..62].copy_from_slice(b"WS");

    // Prepare synthetic block I/O stream for B4 engine benchmark (50,000 events)
    let stream_size = 50_000;
    let target = ThreadId::new(4217, 4217);
    let mut stream: Vec<Event> = Vec::with_capacity(stream_size);
    let mut ts = 1_000_000_000u64;
    for i in 0..(stream_size / 4) {
        let req_id = i as u64 + 1;
        stream.push(Event::new(
            ts,
            EventRef::new(0, (i * 4 + 1) as u64),
            target,
            EventKind::BlockRqIssue(BlockRqIssue::new(0x80001, req_id, 4096, 8, "WS", target, ts)),
        ));
        ts += 1_000;
        stream.push(Event::new(
            ts,
            EventRef::new(0, (i * 4 + 2) as u64),
            target,
            EventKind::Switch(SchedSwitch {
                prev: target,
                next: ThreadId::new(100, 100),
                prev_state: TaskState::IoWait,
                preempted: false,
                in_iowait: true,
            }),
        ));
        ts += 50_000;
        stream.push(Event::new(
            ts,
            EventRef::new(0, (i * 4 + 3) as u64),
            ThreadId::new(0, 0),
            EventKind::BlockRqComplete(BlockRqComplete::new(0x80001, req_id, 4096, 0, ts)),
        ));
        ts += 1_000;
        stream.push(Event::new(
            ts,
            EventRef::new(0, (i * 4 + 4) as u64),
            target,
            EventKind::Switch(SchedSwitch {
                prev: ThreadId::new(100, 100),
                next: target,
                prev_state: TaskState::Running,
                preempted: false,
                in_iowait: false,
            }),
        ));
        ts += 10_000;
    }

    let engine = Engine::new(8);

    println!("[1/3] Benchmarking physical filesystem synchronous block write...");
    println!("[2/3] Benchmarking in-kernel eBPF binary frame decoding...");
    println!("[3/3] Benchmarking Cell B4 Engine analysis throughput on {} block events...", stream.len());

    for trial in 1..=N_TRIALS {
        // 1. Physical SSD fsync latency test
        let fsync_start = Instant::now();
        let path = format!("target/bench_fsync_trial_{}.tmp", trial);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .expect("Failed to create bench file");
        file.write_all(&[0xaa; 4096]).expect("Failed write");
        file.sync_all().expect("Failed fsync");
        let fsync_dur = fsync_start.elapsed().as_micros() as f64;
        let _ = std::fs::remove_file(&path);
        fsync_us_trials.push(fsync_dur);

        // 2. Binary wire decoder throughput (100,000 iterations)
        let n_decodes = 100_000;
        let dec_start = Instant::now();
        for _ in 0..n_decodes {
            let _ = Event::decode_raw(&raw_buf).unwrap();
        }
        let dec_dur = dec_start.elapsed();
        let dec_mops = (n_decodes as f64 / dec_dur.as_secs_f64()) / 1_000_000.0;
        decode_mops_trials.push(dec_mops);

        // 3. Engine analysis throughput
        let eng_start = Instant::now();
        let _report = engine.analyze(4217, Some(4217), stream.clone(), LossLedger::new());
        let eng_dur = eng_start.elapsed();
        let eng_throughput = (stream.len() as f64) / eng_dur.as_secs_f64();
        engine_events_per_sec_trials.push(eng_throughput);

        csv_rows.push(format!(
            "{},{:.2},{:.2},{:.0}",
            trial, fsync_dur, dec_mops, eng_throughput
        ));
    }

    // Save CSV
    std::fs::create_dir_all("benchmarks").unwrap();
    let csv_content = csv_rows.join("\n") + "\n";
    std::fs::write("benchmarks/results_phase2.csv", csv_content).unwrap();

    // Compute Summary Stats
    let print_stats = |name: &str, unit: &str, data: &[f64]| {
        let med = median(data.to_vec());
        let q1 = percentile(data, 25.0);
        let q3 = percentile(data, 75.0);
        let iqr = q3 - q1;
        let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let (mean, ci_lo, ci_hi) = mean_and_ci95(data);

        println!("\nMetric: {}", name);
        println!("  Median:       {:.2} {}", med, unit);
        println!("  IQR (Q3 - Q1): {:.2} {} [Q1: {:.2}, Q3: {:.2}]", iqr, unit, q1, q3);
        println!("  Min / Max:    {:.2} / {:.2} {}", min, max, unit);
        println!("  Mean (95% CI): {:.2} {} [{:.2}, {:.2}]", mean, unit, ci_lo, ci_hi);
    };

    println!("\n============================================================");
    println!("Benchmark Results Summary (N = {} trials):", N_TRIALS);
    println!("============================================================");
    print_stats("Physical SSD fsync latency", "µs", &fsync_us_trials);
    print_stats("Raw eBPF wire decoding speed", "Mops/sec", &decode_mops_trials);
    print_stats("Cell B4 Engine analysis throughput", "events/sec", &engine_events_per_sec_trials);
    println!("\nRaw data saved to: benchmarks/results_phase2.csv");
}
