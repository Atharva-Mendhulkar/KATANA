//! Interactive demonstrations, built-in trace scenarios, and system environment doctor.

use std::path::Path;
use crate::diagnosis::DiagStatus;
use crate::events::{Event, LossLedger};
use crate::output::TargetIdentity;
use crate::renderer::render_diagnosis;
use crate::watcher::{WatchConfig, Watcher};
use crate::Engine;

pub const FIXTURE_BIO1: &str = include_str!("../fixtures/bio1_sync_io.json");
pub const FIXTURE_BIO2: &str = include_str!("../fixtures/bio2_writeback.json");
pub const FIXTURE_CONTENTION: &str = include_str!("../fixtures/contention.json");

pub const BANNER: &str = crate::logo::KATANA_BANNER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    Futex,
    SyncIo,
    Writeback,
    Watch,
    All,
    Interactive,
}

impl Scenario {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "futex" | "lock" | "contention" | "mutex" => Some(Scenario::Futex),
            "io" | "sync-io" | "sync_io" | "block-io" | "bio1" => Some(Scenario::SyncIo),
            "writeback" | "async-io" | "bio2" => Some(Scenario::Writeback),
            "watch" | "monitor" => Some(Scenario::Watch),
            "all" | "full" | "tour" => Some(Scenario::All),
            "interactive" | "cli" | "console" | "shell" | "repl" => Some(Scenario::Interactive),
            _ => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Scenario::Futex => "Multi-Waiter Futex Mutex Contention (Rule FW-1)",
            Scenario::SyncIo => "Synchronous Direct Block I/O Bottleneck (Rule BIO-1)",
            Scenario::Writeback => "Asynchronous Kernel Writeback Non-Attribution (Rule BIO-2)",
            Scenario::Watch => "Continuous Ring-Buffer Watch Anomaly Trigger (ADR-015)",
            Scenario::All => "Complete Tour of Katana Causal Diagnosis Scenarios",
            Scenario::Interactive => "Always-On Interactive Demonstration Console",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Scenario::Futex => "3 threads contend on a private mutex. TID 4217 is blocked for 1000ms until released by waker TID 4221 via sys_futex(FUTEX_WAKE).",
            Scenario::SyncIo => "Target thread 4217 issues direct synchronous block I/O (REQ_SYNC | REQ_PREFLUSH) to NVMe device 0x80001, sleeping for 20ms in IoWait.",
            Scenario::Writeback => "Target thread dirties page cache; kworker flush thread performs writeback on block device. Clamped to CORRELATED with disclaimer.",
            Scenario::Watch => "Continuous sliding ring-buffer with circular retention (5000 events) and 10ms threshold. Instantly captures & explains off-CPU latency spikes.",
            Scenario::All => "Demonstrates all core latency causal archetypes in sequence.",
            Scenario::Interactive => "Interactive shell allowing full live exploration of all Katana rules and features.",
        }
    }
}

/// Executes a demo scenario, printing styled visual output.
pub fn run_scenario(
    scenario: Scenario,
    html_path: Option<&str>,
    open_browser: bool,
    json: bool,
    verbose: bool,
) {
    if scenario == Scenario::Interactive {
        crate::interactive::run_interactive_cli();
        return;
    }

    if scenario == Scenario::All {
        println!("{}\n", BANNER);
        println!("\x1b[1;32m=== Starting Katana Complete Architecture Tour ===\x1b[0m\n");
        for s in [Scenario::Futex, Scenario::SyncIo, Scenario::Writeback, Scenario::Watch] {
            println!("\x1b[1;33m----------------------------------------------------------------------\x1b[0m");
            println!("\x1b[1;37mScenario: {}\x1b[0m", s.title());
            println!("\x1b[90m{}\x1b[0m\n", s.description());
            run_single_scenario(s, None, false, json, verbose);
            println!();
        }
        if let Some(path) = html_path {
            // Generate HTML for the flagship futex scenario
            let events: Vec<Event> = serde_json::from_str(FIXTURE_CONTENTION).unwrap();
            let ident = make_demo_identity(4217, "demo_worker");
            let engine = Engine::new(8);
            let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());
            if let Err(e) = crate::html_export::export_html_file(&report, path) {
                eprintln!("katana: failed to export HTML report: {}", e);
            } else {
                println!("\x1b[1;32m[✓]\x1b[0m Exported interactive HTML report to \x1b[1m{}\x1b[0m", path);
                if open_browser {
                    open_in_browser(path);
                }
            }
        }
        println!("\x1b[1;32m=== Katana Architecture Tour Completed ===\x1b[0m");
        return;
    }

    if !json {
        println!("{}\n", BANNER);
        println!("\x1b[1;34m[DEMO]\x1b[0m \x1b[1;37m{}\x1b[0m", scenario.title());
        println!("\x1b[90m{}\x1b[0m\n", scenario.description());
    }

    run_single_scenario(scenario, html_path, open_browser, json, verbose);
}

fn run_single_scenario(
    scenario: Scenario,
    html_path: Option<&str>,
    open_browser: bool,
    json: bool,
    verbose: bool,
) {
    let (fixture_json, default_pid, comm) = match scenario {
        Scenario::Futex => (FIXTURE_CONTENTION, 4217, "worker_pool"),
        Scenario::SyncIo => (FIXTURE_BIO1, 4217, "postgres_wal"),
        Scenario::Writeback => (FIXTURE_BIO2, 4217, "file_writer"),
        Scenario::Watch => (FIXTURE_BIO1, 4217, "latency_service"),
        Scenario::All | Scenario::Interactive => unreachable!(),
    };

    let events: Vec<Event> = serde_json::from_str(fixture_json)
        .expect("Embedded demo fixture must be valid JSON");

    let ident = make_demo_identity(default_pid, comm);

    if scenario == Scenario::Watch {
        let watch_cfg = WatchConfig {
            target_pid: default_pid,
            tid: Some(default_pid),
            threshold_ms: 10, // 10ms threshold triggers on the 20ms I/O wait
            retention_capacity: 5000,
            max_episodes: Some(1),
            json,
            html_path: html_path.map(|s| s.to_string()),
            verbose,
            max_depth: 8,
        };

        let mut watcher = Watcher::new(watch_cfg, ident.clone());
        let reports = watcher.process_event_stream(events);

        for (idx, rep) in reports.iter().enumerate() {
            if json {
                if let Ok(j) = rep.to_json_pretty() {
                    println!("{}", j);
                }
            } else {
                println!("\x1b[1;35m--- WATCH ANOMALY TRIGGERED (Episode {}) ---\x1b[0m", idx + 1);
                print!("{}", render_diagnosis(&rep.diagnosis, verbose));
            }
            if let Some(path) = html_path {
                if let Err(e) = crate::html_export::export_html_file(rep, path) {
                    eprintln!("katana: failed to export HTML report: {}", e);
                } else {
                    println!("\x1b[1;32m[✓]\x1b[0m Exported static HTML visual report to \x1b[1m{}\x1b[0m", path);
                    if open_browser {
                        open_in_browser(path);
                    }
                }
            }
        }
        return;
    }

    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(default_pid), events, LossLedger::new());

    if json {
        match report.to_json_pretty() {
            Ok(j) => println!("{}", j),
            Err(e) => eprintln!("Serialization error: {}", e),
        }
    } else {
        let rendered = render_diagnosis(&report.diagnosis, verbose);
        print!("{}", rendered);

        match report.diagnosis.status {
            DiagStatus::Found => {
                println!("\x1b[1;32mStatus: [FOUND]\x1b[0m Primary causal root cause conclusively proven.");
            }
            DiagStatus::Ambiguous => {
                println!("\x1b[1;33mStatus: [AMBIGUOUS]\x1b[0m Competing or non-causal correlations observed.");
            }
            DiagStatus::NotBlocked => {
                println!("\x1b[1;32mStatus: [NOT BLOCKED]\x1b[0m Target thread was actively executing on CPU.");
            }
            DiagStatus::Unknown => {
                println!("\x1b[1;33mStatus: [UNKNOWN]\x1b[0m Cause unidentifiable within observation window.");
            }
            DiagStatus::Invalid => {
                println!("\x1b[1;31mStatus: [INVALID]\x1b[0m Event loss or PID reuse invalidated trace.");
            }
        }
    }

    if let Some(path) = html_path {
        if let Err(e) = crate::html_export::export_html_file(&report, path) {
            eprintln!("katana: failed to export HTML report: {}", e);
        } else {
            println!("\x1b[1;32m[✓]\x1b[0m Exported static HTML visual report to \x1b[1m{}\x1b[0m", path);
            if open_browser {
                open_in_browser(path);
            }
        }
    }
}

/// System environment diagnostics (katana doctor).
pub fn run_doctor(btf_override: Option<&str>) {
    println!("{}\n", BANNER);
    println!("\x1b[1;37mKatana Environment & Subsystem Diagnostics (doctor)\x1b[0m\n");

    println!("Host Architecture:");
    println!("  Operating System:    \x1b[1;36m{}\x1b[0m", std::env::consts::OS);
    println!("  CPU Architecture:    \x1b[1;36m{}\x1b[0m", std::env::consts::ARCH);

    let is_linux = std::env::consts::OS == "linux";
    let has_proc = Path::new("/proc").exists();
    let btf_res = crate::target::resolve_btf_path(btf_override);
    let (has_btf, btf_desc) = match &btf_res {
        Ok(path) => (true, format!("BTF debug info ({})", path.display())),
        Err(_) => (
            false,
            format!(
                "BTF debug info ({})",
                btf_override.unwrap_or("/sys/kernel/btf/vmlinux")
            ),
        ),
    };
    let has_tracefs = Path::new("/sys/kernel/debug/tracing").exists()
        || Path::new("/sys/kernel/tracing").exists();
    let has_root = crate::target::has_root_privileges();

    println!("\nKernel & Trace Subsystems:");
    print_check("Linux /proc filesystem", has_proc);
    print_check(&btf_desc, has_btf);
    print_check("Tracefs debug events (/sys/kernel/debug/tracing)", has_tracefs);
    print_check("Privileged credentials (UID 0 / CAP_BPF)", has_root);

    println!("\nEngine Capabilities on this Host:");
    print_capability("Offline Replay Engine", true, "Deterministic replay without privileges or eBPF");
    print_capability("Static HTML Visualizer", true, "Zero-dependency monochrome SVG topology graph");
    print_capability("Continuous Watch Engine", true, "Bounded ring-buffer sliding window anomaly detector");
    print_capability(
        "Live In-Kernel eBPF Probes",
        is_linux && has_root && has_btf,
        if is_linux && has_root && has_btf {
            "Ready to attach to live target processes"
        } else if !is_linux {
            "Requires Linux host with kernel >= 5.8"
        } else if !has_root {
            "Requires root privileges or CAP_BPF / CAP_PERFMON"
        } else {
            "Requires CONFIG_DEBUG_INFO_BTF enabled in kernel or --btf <path>"
        },
    );

    println!("\nNext Steps:");
    if !is_linux {
        println!("  \x1b[1;33mNote:\x1b[0m You are running on macOS. Katana's offline causal analysis engine,");
        println!("        watch mode, and static HTML exports are fully functional natively.");
        println!("  Run the built-in demo:");
        println!("    \x1b[1;32mkatana demo\x1b[0m                   # Run flagship futex contention demo");
        println!("    \x1b[1;32mkatana demo all\x1b[0m               # Complete 4-scenario architecture tour");
        println!("    \x1b[1;32mkatana demo io --html --open\x1b[0m  # Generate & open interactive SVG visual report");
    } else if !has_btf {
        println!("  Kernel BTF is not available at /sys/kernel/btf/vmlinux.");
        println!("  Supply external BTF or detached debug info:");
        println!("    \x1b[1;32msudo katana doctor --btf /path/to/vmlinux.btf\x1b[0m");
        println!("    \x1b[1;32msudo katana explain <PID> --btf /path/to/vmlinux.btf\x1b[0m");
    } else if !has_root {
        println!("  To analyze live Linux processes, re-run with sudo:");
        println!("    \x1b[1;32msudo katana explain <PID>\x1b[0m");
    } else {
        println!("  Katana is fully primed for live kernel tracing!");
        println!("    \x1b[1;32mkatana explain <PID>\x1b[0m          # Bounded 3-second causal latency diagnosis");
        println!("    \x1b[1;32mkatana watch <PID>\x1b[0m            # Continuous watch mode with anomaly trigger");
    }
}

/// Lists all available trace fixtures and demo scenarios.
pub fn list_fixtures() {
    println!("{}\n", BANNER);
    println!("\x1b[1;37mAvailable Demonstration Scenarios & Trace Fixtures:\x1b[0m\n");
    println!("  \x1b[1m{:<12} {:<8} {:<10} {:<12} {:<36}\x1b[0m", "SCENARIO", "RULE", "SUBJECT", "DURATION", "CAUSE / DESCRIPTION");
    println!("  {}", "─".repeat(80));

    let items = [
        ("futex", "FW-1", "TID 4217", "1000 ms", "Multi-waiter futex lock contention"),
        ("io", "BIO-1", "TID 4217", "20 ms", "Direct synchronous block I/O NVMe stall"),
        ("writeback", "BIO-2", "TID 4217", "20 ms", "Kernel writeback correlation (non-causal)"),
        ("watch", "ADR-15", "TID 4217", "20 ms", "Continuous circular buffer anomaly trigger"),
    ];

    for (name, rule, subject, dur, desc) in items {
        println!("  \x1b[1;36m{:<12}\x1b[0m \x1b[1;33m{:<8}\x1b[0m {:<10} {:<12} {}", name, rule, subject, dur, desc);
    }

    println!("\n\x1b[1mQuick Run Commands:\x1b[0m");
    println!("  katana demo futex                   # Run futex contention demo");
    println!("  katana demo io --html report.html   # Generate HTML visual graph");
    println!("  katana demo all                     # Run complete tour of all scenarios");
}

fn print_check(name: &str, ok: bool) {
    if ok {
        println!("  \x1b[1;32m[✓]\x1b[0m {:<46} \x1b[1;32mYES\x1b[0m", name);
    } else {
        println!("  \x1b[1;31m[✗]\x1b[0m {:<46} \x1b[90mNO\x1b[0m", name);
    }
}

fn print_capability(name: &str, available: bool, note: &str) {
    if available {
        println!("  \x1b[1;32m[AVAILABLE]\x1b[0m   {:<26} \x1b[90m({})\x1b[0m", name, note);
    } else {
        println!("  \x1b[1;33m[UNAVAILABLE]\x1b[0m {:<26} \x1b[90m({})\x1b[0m", name, note);
    }
}

fn make_demo_identity(pid: u32, comm: &str) -> TargetIdentity {
    TargetIdentity {
        tgid: pid,
        pid,
        comm: comm.to_string(),
        start_time_ticks: 123456,
        boot_id: "00000000-0000-0000-0000-000000000000".to_string(),
    }
}

/// Opens the given path in the system default web browser.
pub fn open_in_browser(path: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd").args(["/c", "start", path]).spawn();
    }
}
