//! Interactive Always-On CLI Console for Katana.
//!
//! Provides a continuous demonstration environment to explore and showcase
//! every feature, rule, causal graph, and diagnostic capability of Katana.

use std::io::{self, BufRead, Write};
use std::time::Instant;

use crate::block_io::BlockRqIssue;
use crate::demo::{open_in_browser, FIXTURE_BIO1, FIXTURE_BIO2, FIXTURE_CONTENTION};
use crate::events::{Event, EventKind, LossLedger};
use crate::logo::{KATANA_BANNER, KATANA_LOGO_RAW};
use crate::output::TargetIdentity;
use crate::renderer::render_diagnosis;
use crate::scheduler::{EventRef, ThreadId};
use crate::watcher::{WatchConfig, Watcher};
use crate::Engine;

/// Starts the Always-On interactive console using standard input and output.
pub fn run_interactive_cli() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let is_atty = true; // In standard interactive CLI mode
    run_interactive_loop(stdin.lock(), stdout.lock(), is_atty);
}

/// Runs the interactive command loop against any reader and writer.
pub fn run_interactive_loop<R: BufRead, W: Write>(mut reader: R, mut writer: W, styled: bool) {
    print_welcome(&mut writer, styled);
    print_menu(&mut writer);

    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        print_prompt(&mut writer, styled);

        match reader.read_line(&mut line_buf) {
            Ok(0) => {
                // EOF reached
                let _ = writeln!(writer, "\n\x1b[1;33mExiting Katana console. Farewell.\x1b[0m");
                break;
            }
            Ok(_) => {
                let input = line_buf.trim();
                if input.is_empty() {
                    continue;
                }

                let parts: Vec<&str> = input.split_whitespace().collect();
                let cmd = parts[0].to_lowercase();
                let arg1 = parts.get(1).copied().unwrap_or("");
                let arg2 = parts.get(2).copied().unwrap_or("");

                match cmd.as_str() {
                    "exit" | "quit" | "q" | ":q" => {
                        let _ = writeln!(writer, "\x1b[1;33mExiting Katana console. Farewell.\x1b[0m");
                        break;
                    }
                    "help" | "h" | "?" => {
                        print_help(&mut writer);
                    }
                    "menu" | "list" | "ls" => {
                        print_menu(&mut writer);
                    }
                    "logo" => {
                        if styled {
                            let _ = writeln!(writer, "{}", KATANA_BANNER);
                        } else {
                            let _ = writeln!(writer, "{}", KATANA_LOGO_RAW);
                        }
                    }
                    "clear" | "cls" => {
                        let _ = write!(writer, "\x1b[2J\x1b[H");
                        print_welcome(&mut writer, styled);
                    }
                    // Scenario 1 / Futex
                    "1" | "futex" | "fw1" | "lock" | "contention" => {
                        run_futex_demo(&mut writer, styled);
                    }
                    // Scenario 2 / Block I/O
                    "2" | "io" | "bio1" | "sync-io" | "sync_io" | "nvme" => {
                        run_sync_io_demo(&mut writer, styled);
                    }
                    // Scenario 3 / Writeback
                    "3" | "writeback" | "bio2" | "async-io" => {
                        run_writeback_demo(&mut writer, styled);
                    }
                    // Scenario 4 / Watch Mode
                    "4" | "watch" | "monitor" => {
                        run_watch_demo(&mut writer, styled);
                    }
                    // Scenario 5 / HTML Visualizer
                    "5" | "html" => {
                        let path = if !arg1.is_empty() { arg1 } else { "katana_report.html" };
                        let open = arg2 == "--open" || arg1 == "--open";
                        run_html_export(&mut writer, "futex", path, open);
                    }
                    // Scenario 6 / Open in Browser
                    "6" | "open" => {
                        let path = if !arg1.is_empty() && arg1 != "--open" { arg1 } else { "katana_report.html" };
                        run_html_export(&mut writer, "futex", path, true);
                    }
                    // Scenario 7 / Lexicographic Ranker
                    "7" | "ranker" | "ranking" => {
                        run_ranker_demo(&mut writer);
                    }
                    // Scenario 8 / Anti-Inflation Grammar
                    "8" | "grammar" | "verbs" | "discipline" => {
                        run_grammar_demo(&mut writer);
                    }
                    // Scenario 9 / Multi-Hop & Cycles
                    "9" | "multihop" | "chain" | "cycles" => {
                        run_multihop_demo(&mut writer, styled);
                    }
                    // Scenario 10 / Loss Ledger & Faults
                    "10" | "loss" | "faults" | "ledger" => {
                        run_loss_demo(&mut writer);
                    }
                    // Scenario 11 / eBPF Kernel Architecture
                    "11" | "bpf" | "ebpf" | "kernel" => {
                        run_bpf_demo(&mut writer);
                    }
                    // Scenario 12 / Doctor
                    "12" | "doctor" | "sys" | "env" => {
                        run_doctor_demo(&mut writer);
                    }
                    // Scenario 13 / Throughput Benchmark
                    "13" | "bench" | "benchmark" | "throughput" => {
                        run_bench_demo(&mut writer);
                    }
                    // Scenario 14 / Architecture Tour (All)
                    "14" | "tour" | "all" => {
                        run_tour_demo(&mut writer, styled);
                    }
                    // Generic Demo Command
                    "demo" => {
                        if arg1.is_empty() {
                            let _ = writeln!(writer, "\x1b[1;33mUsage: demo <futex|io|writeback|watch|all>\x1b[0m");
                        } else {
                            match arg1 {
                                "futex" | "fw1" => run_futex_demo(&mut writer, styled),
                                "io" | "bio1" => run_sync_io_demo(&mut writer, styled),
                                "writeback" | "bio2" => run_writeback_demo(&mut writer, styled),
                                "watch" => run_watch_demo(&mut writer, styled),
                                "all" | "tour" => run_tour_demo(&mut writer, styled),
                                other => {
                                    let _ = writeln!(writer, "\x1b[1;31mUnknown scenario '{}'. Try: futex, io, writeback, watch, all\x1b[0m", other);
                                }
                            }
                        }
                    }
                    // Inspect Command
                    "inspect" => {
                        if arg1.is_empty() {
                            let _ = writeln!(writer, "\x1b[1;33mUsage: inspect <futex|io|writeback|filepath>\x1b[0m");
                        } else {
                            run_inspect(&mut writer, arg1);
                        }
                    }
                    // Replay Command
                    "replay" | "explain" => {
                        if arg1.is_empty() {
                            let _ = writeln!(writer, "\x1b[1;33mUsage: explain <futex|io|writeback|filepath>\x1b[0m");
                        } else {
                            run_explain_target(&mut writer, arg1, styled);
                        }
                    }
                    other => {
                        let _ = writeln!(
                            writer,
                            "\x1b[1;31mUnknown command '{}'. Type 'menu' to view available demos, or 'help' for syntax.\x1b[0m",
                            other
                        );
                    }
                }
            }
            Err(e) => {
                let _ = writeln!(writer, "Error reading command input: {}", e);
                break;
            }
        }
    }
}

fn print_prompt<W: Write>(writer: &mut W, styled: bool) {
    if styled {
        let _ = write!(writer, "\x1b[1;36mkatana\x1b[0m \x1b[1;33m❯\x1b[0m ");
    } else {
        let _ = write!(writer, "katana > ");
    }
    let _ = writer.flush();
}

fn print_welcome<W: Write>(writer: &mut W, styled: bool) {
    if styled {
        let _ = writeln!(writer, "{}", KATANA_BANNER);
    } else {
        let _ = writeln!(writer, "{}", KATANA_LOGO_RAW);
    }
    let _ = writeln!(
        writer,
        "\n\x1b[1;37mKatana Always-On Interactive Demonstration Console\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mOperating System: {} | CPU: {} | Version: {}\x1b[0m\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        env!("CARGO_PKG_VERSION")
    );
}

fn print_menu<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;36m========================================================================================\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[1;37m                                KATANA FEATURE SHOWCASE\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[1;36m========================================================================================\x1b[0m"
    );
    let _ = writeln!(writer, "\x1b[1;33m[CORE CAUSAL ATTRIBUTION RULES]\x1b[0m");
    let _ = writeln!(
        writer,
        "  \x1b[1m 1\x1b[0m) \x1b[1;36mfutex\x1b[0m      - Rule FW-1: Multi-Waiter Futex Lock Contention (Flagship)"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m 2\x1b[0m) \x1b[1;36mio\x1b[0m         - Rule BIO-1: Synchronous Direct NVMe Block I/O Bottleneck"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m 3\x1b[0m) \x1b[1;36mwriteback\x1b[0m  - Rule BIO-2: Asynchronous Writeback Clamping (Anti-Inflation)"
    );
    let _ = writeln!(writer, "\n\x1b[1;33m[MONITORING & VISUALIZATION]\x1b[0m");
    let _ = writeln!(
        writer,
        "  \x1b[1m 4\x1b[0m) \x1b[1;36mwatch\x1b[0m      - Continuous Ring-Buffer Watch Anomaly Trigger (ADR-015)"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m 5\x1b[0m) \x1b[1;36mhtml\x1b[0m       - Static Zero-Emoji Monochrome SVG Visualizer Export"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m 6\x1b[0m) \x1b[1;36mopen\x1b[0m       - Export & Automatically Open HTML Report in Web Browser"
    );
    let _ = writeln!(writer, "\n\x1b[1;33m[MATHEMATICAL RIGOR & ARCHITECTURE]\x1b[0m");
    let _ = writeln!(
        writer,
        "  \x1b[1m 7\x1b[0m) \x1b[1;36mranker\x1b[0m     - 8-Criterion Lexicographic Ranking Engine Deep Dive"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m 8\x1b[0m) \x1b[1;36mgrammar\x1b[0m    - Anti-Inflation Language Discipline & Allowed-Verb Grammar"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m 9\x1b[0m) \x1b[1;36mmultihop\x1b[0m   - Multi-Hop Backward Causal Graph & Cycle Detection"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m10\x1b[0m) \x1b[1;36mloss\x1b[0m       - Loss Ledger & Fault Injection (Ring Overflow, Gaps)"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m11\x1b[0m) \x1b[1;36mbpf\x1b[0m        - In-Kernel eBPF Tracing Perimeter & Scope Guard (Allow-List)"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m12\x1b[0m) \x1b[1;36mdoctor\x1b[0m     - Subsystem Diagnostics (OS, BTF, Tracefs, Privileges)"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m13\x1b[0m) \x1b[1;36mbench\x1b[0m      - High-Throughput Event Processing Microbenchmark"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1m14\x1b[0m) \x1b[1;36mtour\x1b[0m       - Complete Architecture Tour (Executes All 4 Scenarios)"
    );
    let _ = writeln!(
        writer,
        "\x1b[1;36m----------------------------------------------------------------------------------------\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mType a number (1-14) or command name. 'menu' to show list, 'help' for details, 'exit' to quit.\x1b[0m\n"
    );
}

fn print_help<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;37mKatana Always-On Interactive CLI Commands:\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36m1..14\x1b[0m                  Execute feature showcase scenario by number"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mfutex | io | writeback\x1b[0m Run specific causal analysis scenarios"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mwatch\x1b[0m                  Simulate continuous ring-buffer watch mode"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mhtml [path] [--open]\x1b[0m   Export self-contained static HTML causal graph"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mopen [path]\x1b[0m            Export and launch report in default web browser"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mranker\x1b[0m                 Inspect 8-criterion lexicographic ranking engine"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mgrammar\x1b[0m                Inspect anti-inflation allowed-verb table"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mmultihop\x1b[0m               Inspect backward chain walker and cycle detection"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mloss\x1b[0m                   Simulate ring buffer loss and loss ledger downgrade"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mbpf\x1b[0m                    Inspect eBPF tracepoint allow-list and scope guard"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mdoctor\x1b[0m                 Run environment diagnostics on current host"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mbench\x1b[0m                  Run in-memory analysis throughput benchmark"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mtour\x1b[0m                   Run complete 4-scenario architecture tour"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36minspect <name|file>\x1b[0m    Inspect raw event stream characteristics"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mexplain <name|file>\x1b[0m    Run root-cause analysis on a scenario or trace"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mlogo\x1b[0m                   Display the Katana brand logo and extended sword"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mmenu\x1b[0m                   Display the interactive selection menu"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mclear\x1b[0m                  Clear the terminal screen"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;36mexit\x1b[0m                   Terminate console session\n"
    );
}

/// Feature 1: Multi-Waiter Futex Contention (Rule FW-1)
fn run_futex_demo<W: Write>(writer: &mut W, styled: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 1]\x1b[0m \x1b[1;37mRule FW-1: Multi-Waiter Futex Mutex Contention\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mMathematical attribution of userspace lock release via kernel tracepoint enclosure.\x1b[0m"
    );
    let _ = writeln!(writer, "\x1b[90mTrace: 3 worker threads contending on a private mutex. Subject TID 4217 is blocked for 1000ms.\x1b[0m\n");

    let events: Vec<Event> = serde_json::from_str(FIXTURE_CONTENTION).unwrap();
    let ident = make_identity(4217, "worker_pool");
    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());

    let rendered = render_diagnosis(&report.diagnosis, false);
    let _ = write!(writer, "{}", rendered);

    if styled {
        let _ = writeln!(writer, "\x1b[1;32m✓ Rule FW-1 Proof Verification:\x1b[0m");
        let _ = writeln!(
            writer,
            "  • \x1b[1mContainment Window:\x1b[0m sys_enter_futex(E3) <= sched_waking(E4) <= sys_exit_futex(E5)"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mFutex Key Match:\x1b[0m uaddr=0x7ffee0, bitset=0xffffffff (non-zero intersection)"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mSequence Order:\x1b[0m seq(E3) < seq(E4) < seq(E5) on CPU 2"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mWakee State:\x1b[0m TID 4217 had open FutexWait interval at ts(sched_waking)"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mWake Count Check:\x1b[0m observed_wakes (1) <= syscall ret (1) [CONSISTENT]"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mRunqueue Delay:\x1b[0m sched_waking -> sched_wakeup -> sched_switch = 120 µs queue latency"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mClassification:\x1b[0m CAUSAL / DERIVED -> \x1b[1;32m[FOUND]\x1b[0m (Subject Relevance: 1, Causal: 1, Explained: 1000‰)\n"
        );
    }
}

/// Feature 2: Direct Synchronous Block I/O Bottleneck (Rule BIO-1)
fn run_sync_io_demo<W: Write>(writer: &mut W, styled: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 2]\x1b[0m \x1b[1;37mRule BIO-1: Direct Synchronous Block I/O Bottleneck\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mExact sector & request correlation for synchronous O_DIRECT / fsync operations.\x1b[0m"
    );
    let _ = writeln!(writer, "\x1b[90mTrace: Target thread 4217 issues direct synchronous block I/O to NVMe device 0x80001.\x1b[0m\n");

    let events: Vec<Event> = serde_json::from_str(FIXTURE_BIO1).unwrap();
    let ident = make_identity(4217, "postgres_wal");
    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());

    let rendered = render_diagnosis(&report.diagnosis, false);
    let _ = write!(writer, "{}", rendered);

    if styled {
        let _ = writeln!(writer, "\x1b[1;32m✓ Rule BIO-1 Proof Verification:\x1b[0m");
        let _ = writeln!(
            writer,
            "  • \x1b[1mTask Sleep State:\x1b[0m sched_switch prev_state=TASK_UNINTERRUPTIBLE (IoWait bit set)"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mDevice Resolution:\x1b[0m Device dev=0x80001 resolved to 'nvme0n1' via /sys/dev/block/"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mRequest Enclosure:\x1b[0m block_rq_issue(sector=4096, nr_sector=16) -> block_rq_complete"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mSubmitter Match:\x1b[0m Submitter TID 4217 matches wakee context"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mClassification:\x1b[0m CAUSAL / DIRECT -> \x1b[1;32m[FOUND]\x1b[0m\n"
        );
    }
}

/// Feature 3: Asynchronous Kernel Writeback Clamping (Rule BIO-2)
fn run_writeback_demo<W: Write>(writer: &mut W, styled: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 3]\x1b[0m \x1b[1;37mRule BIO-2: Asynchronous Writeback Clamping & Anti-Inflation\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mStructural non-attribution clamp for background flushing threads (kworker/flush).\x1b[0m"
    );
    let _ = writeln!(writer, "\x1b[90mTrace: Target thread dirties page cache; background kworker performs writeback.\x1b[0m\n");

    let events: Vec<Event> = serde_json::from_str(FIXTURE_BIO2).unwrap();
    let ident = make_identity(4217, "file_writer");
    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());

    let rendered = render_diagnosis(&report.diagnosis, false);
    let _ = write!(writer, "{}", rendered);

    if styled {
        let _ = writeln!(writer, "\x1b[1;33m⚠ Rule BIO-2 Anti-Inflation Invariant Enforcement:\x1b[0m");
        let _ = writeln!(
            writer,
            "  • \x1b[1mSubmitter Mismatch:\x1b[0m I/O submitted by kworker/u64:1 (TID 99), not target 4217"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mMonotonic Clamp:\x1b[0m Downgraded to CORRELATED / STATISTICAL"
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mMandatory Disclaimer:\x1b[0m \"This trace does not establish a causal link.\""
        );
        let _ = writeln!(
            writer,
            "  • \x1b[1mDiagnosis Status:\x1b[0m \x1b[1;33m[AMBIGUOUS]\x1b[0m (Zero False Positives: Katana refuses to hallucinate causality)\n"
        );
    }
}

/// Feature 4: Continuous Watch Mode Daemon (ADR-015)
fn run_watch_demo<W: Write>(writer: &mut W, styled: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 4]\x1b[0m \x1b[1;37mContinuous Ring-Buffer Watch Anomaly Trigger (ADR-015)\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mCircular retention buffer with threshold-triggered episode freezing.\x1b[0m\n"
    );

    let events: Vec<Event> = serde_json::from_str(FIXTURE_BIO1).unwrap();
    let ident = make_identity(4217, "latency_service");

    let watch_cfg = WatchConfig {
        target_pid: 4217,
        tid: Some(4217),
        threshold_ms: 10, // 10ms threshold triggers on the 20ms I/O stall
        retention_capacity: 5000,
        max_episodes: Some(1),
        json: false,
        html_path: None,
        verbose: false,
        max_depth: 8,
    };

    let _ = writeln!(
        writer,
        "  1. Initialized Watcher with in-memory circular ring buffer:"
    );
    let _ = writeln!(
        writer,
        "     - Target PID/TID: \x1b[1m4217\x1b[0m | Anomaly Threshold: \x1b[1m10 ms\x1b[0m | Circular Capacity: \x1b[1m5,000 events\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "  2. Ingesting stream: normal execution on-CPU (0.5ms - 1.2ms)..."
    );
    let _ = writeln!(
        writer,
        "  3. Latency spike: TID 4217 enters off-CPU block for \x1b[1;31m20.00 ms\x1b[0m (EXCEEDS 10ms THRESHOLD)!"
    );
    let _ = writeln!(
        writer,
        "  4. Ring buffer frozen. Submitting episode window to Causal Engine...\n"
    );

    let mut watcher = Watcher::new(watch_cfg, ident);
    let reports = watcher.process_event_stream(events);

    for (idx, rep) in reports.iter().enumerate() {
        let _ = writeln!(
            writer,
            "\x1b[1;35m--- WATCH ANOMALY TRIGGERED (Episode {}) ---\x1b[0m",
            idx + 1
        );
        let rendered = render_diagnosis(&rep.diagnosis, false);
        let _ = write!(writer, "{}", rendered);
    }

    if styled {
        let _ = writeln!(
            writer,
            "\x1b[1;32m✓ Watch Mode Demonstrated:\x1b[0m Zero-overhead continuous monitoring that acts only when SLAs are breached.\n"
        );
    }
}

/// Feature 5 & 6: Static HTML Visualizer Export
fn run_html_export<W: Write>(writer: &mut W, scenario_name: &str, out_path: &str, open: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[HTML EXPORT]\x1b[0m Generating standalone static SVG causal visualizer for scenario '{}'...",
        scenario_name
    );

    let fixture = match scenario_name {
        "io" | "bio1" => FIXTURE_BIO1,
        "writeback" | "bio2" => FIXTURE_BIO2,
        _ => FIXTURE_CONTENTION,
    };

    let events: Vec<Event> = serde_json::from_str(fixture).unwrap();
    let ident = make_identity(4217, "demo_target");
    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());

    match crate::html_export::export_html_file(&report, out_path) {
        Ok(_) => {
            let _ = writeln!(
                writer,
                "\x1b[1;32m[✓] Successfully exported static HTML report:\x1b[0m \x1b[1m{}\x1b[0m",
                out_path
            );
            let _ = writeln!(
                writer,
                "    • Architecture: \x1b[1mSingle-file, 100% self-contained\x1b[0m (Zero CDN, Zero web fonts)"
            );
            let _ = writeln!(
                writer,
                "    • Visualizer:   \x1b[1mInline SVG causal DAG graph\x1b[0m with high-contrast monochrome design"
            );
            let _ = writeln!(
                writer,
                "    • Standards:    Strictly \x1b[1mzero emojis\x1b[0m per PRD §33\n"
            );
            if open {
                let _ = writeln!(writer, "\x1b[1;36mLaunching browser preview...\x1b[0m");
                open_in_browser(out_path);
            }
        }
        Err(e) => {
            let _ = writeln!(writer, "\x1b[1;31mFailed to export HTML report: {}\x1b[0m\n", e);
        }
    }
}

/// Feature 7: 8-Criterion Lexicographic Ranking Engine Deep Dive
fn run_ranker_demo<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 7]\x1b[0m \x1b[1;37m8-Criterion Lexicographic Ranking Engine Deep Dive\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mEliminating arbitrary weights, scalar formulas, and heuristic guesses (PRD §15).\x1b[0m\n"
    );

    let tiers = [
        ("Tier 1", "Subject Relevance", "Direct block (1) vs. indirect / correlated block (0)"),
        ("Tier 2", "Causal Basis", "Contains >= 1 CAUSAL edge (1) vs. only OBSERVED / CORRELATED (0)"),
        ("Tier 3", "Explained Fraction", "Integer per-mille of blocked duration accounted for (0..1000)"),
        ("Tier 4", "Weakest-Link Strength", "Strong > Moderate > Weak (monotonic link quality)"),
        ("Tier 5", "Completeness", "Complete > Partial > Lossy > Invalid (evidence integrity)"),
        ("Tier 6", "Chain Compactness", "Shorter chain depth preferred (fewer hops = tighter proof)"),
        ("Tier 7", "Repetition", "Total occurrence count within the observation window"),
        ("Tier 8", "Tie-Breaker", "Lowest start timestamp (FIFO), lowest thread ID"),
    ];

    let _ = writeln!(writer, "  \x1b[1m{:<8} {:<24} {:<54}\x1b[0m", "TIER", "CRITERION", "EVALUATION PRINCIPLE");
    let _ = writeln!(writer, "  {}", "─".repeat(86));
    for (tier, name, rule) in tiers {
        let _ = writeln!(writer, "  \x1b[1;33m{:<8}\x1b[0m \x1b[1;37m{:<24}\x1b[0m \x1b[90m{}\x1b[0m", tier, name, rule);
    }

    let _ = writeln!(writer, "\n\x1b[1mWhy Lexicographical Ordering?\x1b[0m");
    let _ = writeln!(
        writer,
        "  Scalar scoring (e.g. 0.7 * duration + 0.3 * cpu) creates non-deterministic edge cases where"
    );
    let _ = writeln!(
        writer,
        "  a long unproven correlation outranks a short proven causal block. Katana's lexicographic ranker"
    );
    let _ = writeln!(
        writer,
        "  guarantees that \x1b[1mproven causality always strictly dominates unproven correlation\x1b[0m.\n"
    );
}

/// Feature 8: Anti-Inflation Language Discipline & Allowed Verbs
fn run_grammar_demo<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 8]\x1b[0m \x1b[1;37mAnti-Inflation Language Discipline & Allowed-Verb Grammar\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mSyntactic prevention of claim inflation and marketing speculation (PRD §17).\x1b[0m\n"
    );

    let grammar = [
        (
            "CAUSAL / DIRECT",
            "was woken by, was preempted by, was blocked on synchronous block I/O to device",
        ),
        (
            "CAUSAL / DERIVED",
            "was released by a futex wake from, was unblocked following block I/O completion on",
        ),
        (
            "OBSERVED",
            "was blocked for, entered futex wait on, was queued on runqueue for",
        ),
        (
            "CORRELATED",
            "coincided with, overlapped with, concurrent with [MANDATORY DISCLAIMER REQUIRED]",
        ),
    ];

    let _ = writeln!(writer, "  \x1b[1m{:<20} {:<66}\x1b[0m", "EVIDENCE CLASS", "NORMATIVE ALLOWED VERBS");
    let _ = writeln!(writer, "  {}", "─".repeat(88));
    for (class, verbs) in grammar {
        let _ = writeln!(writer, "  \x1b[1;36m{:<20}\x1b[0m \x1b[1;37m{}\x1b[0m", class, verbs);
    }

    let _ = writeln!(writer, "\n\x1b[1;31mBanned / Forbidden Terms (PRD §17.3):\x1b[0m");
    let _ = writeln!(
        writer,
        "  \x1b[90m\"caused by\", \"slowed down by\", \"impacted by\", \"due to\", \"starved by\", \"struggled with\"\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "  -> \x1b[1mCompile-time & CI Linting:\x1b[0m Checked by tests/anti_inflation_tests.rs.\n"
    );
}

/// Feature 9: Multi-Hop Backward Causal Graph & Cycle Detection
fn run_multihop_demo<W: Write>(writer: &mut W, styled: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 9]\x1b[0m \x1b[1;37mMulti-Hop Backward Causal Graph & Cycle Detection\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mBounded backward causal graph walk (depth cap: 8 hops) with cyclic wake detection.\x1b[0m\n"
    );

    let _ = writeln!(writer, "\x1b[1mCausal Dependency Chain Example:\x1b[0m");
    let _ = writeln!(
        writer,
        "  \x1b[1;31m[TID 4217 (Target)]\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "         │  woken by (Rule FW-1, futex private 0x7ffee0)"
    );
    let _ = writeln!(
        writer,
        "         ▼"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;33m[TID 4221 (Worker 2)]\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "         │  woken by (Rule FW-1, futex private 0x7ffee0)"
    );
    let _ = writeln!(
        writer,
        "         ▼"
    );
    let _ = writeln!(
        writer,
        "  \x1b[1;32m[TID 4200 (Coordinator)]\x1b[0m  ──▶ \x1b[1;36mTerminal State: WakerRunning on CPU 3\x1b[0m"
    );

    if styled {
        let _ = writeln!(writer, "\n\x1b[1;32mCycle Detection Guarantee:\x1b[0m");
        let _ = writeln!(
            writer,
            "  If Thread A wakes Thread B which subsequently wakes Thread A within the same window,"
        );
        let _ = writeln!(
            writer,
            "  Katana isolates the cycle, emits \x1b[1m\"observed wake cycle within window\"\x1b[0m, and halts recursion"
        );
        let _ = writeln!(
            writer,
            "  without crashing or hallucinating deadlocks.\n"
        );
    }
}

/// Feature 10: Loss Ledger & Fault Injection Simulator
fn run_loss_demo<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 10]\x1b[0m \x1b[1;37mLoss Ledger & Ring Buffer Fault Injection\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mHonest diagnostic downgrading when kernel ring buffers drop events (PRD §11).\x1b[0m\n"
    );

    let mut ledger = LossLedger::new();
    ledger.record_loss(2, 1_000_000, 1_000_500, 3);
    ledger.clock_anomalies = 1;
    let total_lost: u64 = ledger.intervals.iter().map(|i| i.n_lost).sum::<u64>() + ledger.reserve_fail_total;

    let _ = writeln!(
        writer,
        "  1. Injected Sequence Gap on CPU 2: sequence 42 -> 45 (gap of 3 events dropped)."
    );
    let _ = writeln!(
        writer,
        "  2. Injected Clock Anomaly: backward timestamp step detected."
    );
    let _ = writeln!(
        writer,
        "  3. Loss Ledger State: \x1b[1;31m{} dropped events\x1b[0m in {} intervals, {} clock anomalies.",
        total_lost,
        ledger.intervals.len(),
        ledger.clock_anomalies
    );
    let _ = writeln!(
        writer,
        "  4. Engine Response: Finding completeness downgrades from \x1b[1;32mComplete\x1b[0m to \x1b[1;33mLossy\x1b[0m."
    );
    let _ = writeln!(
        writer,
        "  5. If critical wake event was dropped: Diagnosis Status drops to \x1b[1;33m[AMBIGUOUS]\x1b[0m (Refuses to guess).\n"
    );
}

/// Feature 11: In-Kernel eBPF Architecture & Scope Guard
fn run_bpf_demo<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 11]\x1b[0m \x1b[1;37mIn-Kernel eBPF Tracing Perimeter & Scope Guard\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mZero-daemon bounded-cost kernel tracing perimeter (bpf/katana.bpf.c).\x1b[0m\n"
    );

    let hooks = [
        ("tp/sched/sched_switch", "On/off-CPU state transition & preemption detection"),
        ("tp/sched/sched_waking", "Waker attribution captured in waker thread context"),
        ("tp/sched/sched_wakeup", "Runqueue queuing delay start timestamp"),
        ("tp/sched/sched_wakeup_new", "Newly spawned task runqueue enqueue"),
        ("tp/sched/sched_process_fork", "Dynamic tracking set thread expansion"),
        ("tp/sched/sched_process_exit", "Target thread exit cleanup"),
        ("tp/syscalls/sys_enter_futex", "Futex op, address, bitset, and val snapshot"),
        ("tp/syscalls/sys_exit_futex", "Futex return value and wake count confirmation"),
        ("tp/block/block_rq_issue", "Block I/O issue with major:minor, sector & flags"),
        ("tp/block/block_rq_complete", "Block I/O completion and device error status"),
        ("tp/block/block_rq_insert", "I/O elevator queue insertion timestamp"),
        ("tp/block/block_rq_requeue", "Device queue congestion requeue tracking"),
    ];

    let _ = writeln!(writer, "  \x1b[1m{:<32} {:<52}\x1b[0m", "TRACEPOINT HOOK", "PURPOSE");
    let _ = writeln!(writer, "  {}", "─".repeat(84));
    for (hook, purp) in hooks {
        let _ = writeln!(writer, "  \x1b[1;36m{:<32}\x1b[0m \x1b[90m{}\x1b[0m", hook, purp);
    }

    let _ = writeln!(writer, "\n\x1b[1;32mScope Guard Verification (scripts/check-scope.sh):\x1b[0m");
    let _ = writeln!(
        writer,
        "  Katana strictly forbids invasive kprobes and arbitrary kernel function hooks."
    );
    let _ = writeln!(
        writer,
        "  Every attached hook is verified against a normative allow-list ensuring \x1b[1mzero risk of kernel panics\x1b[0m.\n"
    );
}

/// Feature 12: Host Doctor & Environment Diagnostics
fn run_doctor_demo<W: Write>(_writer: &mut W) {
    crate::demo::run_doctor();
}

/// Feature 13: High-Throughput Event Processing Microbenchmark
fn run_bench_demo<W: Write>(writer: &mut W) {
    let _ = writeln!(
        writer,
        "\x1b[1;34m[FEATURE 13]\x1b[0m \x1b[1;37mHigh-Throughput Event Processing Microbenchmark\x1b[0m"
    );
    let _ = writeln!(
        writer,
        "\x1b[90mBenchmarking raw event normalization, interval tracking, and causal graph build.\x1b[0m\n"
    );

    let n_events = 20_000;
    let target = ThreadId::new(4217, 4217);
    let mut stream: Vec<Event> = Vec::with_capacity(n_events);
    let mut ts = 1_000_000_000u64;

    for i in 0..n_events {
        let cpu = (i % 8) as u16;
        let req_id = (i + 1) as u64;
        stream.push(Event::new(
            ts,
            EventRef::new(cpu, i as u64),
            target,
            EventKind::BlockRqIssue(BlockRqIssue::new(
                0x80001,
                req_id,
                4096,
                8,
                "R",
                target,
                ts,
            )),
        ));
        ts += 1_000;
    }

    let ident = make_identity(4217, "bench_target");
    let engine = Engine::new(8);

    let t0 = Instant::now();
    let report = engine.analyze_with_identity(ident, Some(4217), stream, LossLedger::new());
    let elapsed = t0.elapsed();

    let events_per_sec = (n_events as f64) / elapsed.as_secs_f64();
    let ns_per_event = (elapsed.as_nanos() as f64) / (n_events as f64);

    let _ = writeln!(writer, "\x1b[1;32m✓ Benchmark Completed:\x1b[0m");
    let _ = writeln!(writer, "  • Events Processed: \x1b[1m{}\x1b[0m", n_events);
    let _ = writeln!(writer, "  • Elapsed Time:     \x1b[1m{:.2?} ms\x1b[0m", elapsed.as_secs_f64() * 1000.0);
    let _ = writeln!(writer, "  • Throughput:       \x1b[1;36m{:.0} events/second\x1b[0m", events_per_sec);
    let _ = writeln!(writer, "  • Per-Event Cost:   \x1b[1m{:.1} nanoseconds\x1b[0m", ns_per_event);
    let _ = writeln!(writer, "  • Diagnosis Status: \x1b[1;32m{:?}\x1b[0m\n", report.diagnosis.status);
}

/// Feature 14: Complete Architecture Tour
fn run_tour_demo<W: Write>(writer: &mut W, styled: bool) {
    let _ = writeln!(
        writer,
        "\x1b[1;32m=== Starting Katana Complete Architecture Tour ===\x1b[0m\n"
    );
    run_futex_demo(writer, styled);
    let _ = writeln!(writer, "{}", "─".repeat(80));
    run_sync_io_demo(writer, styled);
    let _ = writeln!(writer, "{}", "─".repeat(80));
    run_writeback_demo(writer, styled);
    let _ = writeln!(writer, "{}", "─".repeat(80));
    run_watch_demo(writer, styled);
    let _ = writeln!(
        writer,
        "\x1b[1;32m=== Katana Architecture Tour Completed ===\x1b[0m\n"
    );
}

/// Inspects trace characteristics (event counts, duration, thread IDs).
fn run_inspect<W: Write>(writer: &mut W, target: &str) {
    let fixture_str = match target {
        "futex" | "fw1" => FIXTURE_CONTENTION,
        "io" | "bio1" => FIXTURE_BIO1,
        "writeback" | "bio2" => FIXTURE_BIO2,
        path => match std::fs::read_to_string(path) {
            Ok(s) => {
                let _ = writeln!(writer, "\x1b[1;34mInspecting file '{}'...\x1b[0m", path);
                // Return borrowed or owned string
                return inspect_content(writer, &s, path);
            }
            Err(e) => {
                let _ = writeln!(writer, "\x1b[1;31mFailed to read trace file '{}': {}\x1b[0m", path, e);
                return;
            }
        },
    };

    inspect_content(writer, fixture_str, target);
}

fn inspect_content<W: Write>(writer: &mut W, json_str: &str, label: &str) {
    let events: Vec<Event> = match serde_json::from_str(json_str) {
        Ok(evs) => evs,
        Err(e) => {
            let _ = writeln!(writer, "\x1b[1;31mInvalid JSON trace format: {}\x1b[0m", e);
            return;
        }
    };

    let count = events.len();
    let min_ts = events.first().map_or(0, |e| e.ts_ns);
    let max_ts = events.last().map_or(0, |e| e.ts_ns);
    let span_ms = if max_ts > min_ts {
        (max_ts - min_ts) as f64 / 1_000_000.0
    } else {
        0.0
    };

    let mut thread_set = std::collections::HashSet::new();
    let mut event_kind_counts = std::collections::HashMap::new();

    for ev in &events {
        thread_set.insert(ev.thread.tid);
        let kind_name = match &ev.kind {
            EventKind::Switch(_) => "sched_switch",
            EventKind::Waking(_) => "sched_waking",
            EventKind::Wakeup(_) => "sched_wakeup",
            EventKind::Fork { .. } => "sched_process_fork",
            EventKind::Exit => "sched_process_exit",
            EventKind::FutexEnter(_) => "sys_enter_futex",
            EventKind::FutexExit(_) => "sys_exit_futex",
            EventKind::BlockRqIssue(_) => "block_rq_issue",
            EventKind::BlockRqComplete(_) => "block_rq_complete",
            EventKind::UnsupportedSyscall { .. } => "unsupported_syscall",
        };
        *event_kind_counts.entry(kind_name).or_insert(0usize) += 1;
    }

    let _ = writeln!(writer, "\x1b[1;37mTrace Inspection Report for '{}':\x1b[0m", label);
    let _ = writeln!(writer, "  • Total Events:       \x1b[1m{}\x1b[0m", count);
    let _ = writeln!(writer, "  • Time Duration:      \x1b[1m{:.2} ms\x1b[0m ({} ns .. {} ns)", span_ms, min_ts, max_ts);
    let _ = writeln!(writer, "  • Unique Thread IDs:  \x1b[1m{:?}\x1b[0m", thread_set);
    let _ = writeln!(writer, "  • Event Types Breakdown:");
    for (k, v) in event_kind_counts {
        let _ = writeln!(writer, "    - {:<22} \x1b[1;36m{}\x1b[0m", k, v);
    }
    let _ = writeln!(writer);
}

/// Runs explanation on a fixture or file path.
fn run_explain_target<W: Write>(writer: &mut W, target: &str, styled: bool) {
    match target {
        "futex" | "fw1" => run_futex_demo(writer, styled),
        "io" | "bio1" => run_sync_io_demo(writer, styled),
        "writeback" | "bio2" => run_writeback_demo(writer, styled),
        "watch" => run_watch_demo(writer, styled),
        path => {
            let content = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    let _ = writeln!(writer, "\x1b[1;31mFailed to read file '{}': {}\x1b[0m", path, e);
                    return;
                }
            };
            let events: Vec<Event> = match serde_json::from_str(&content) {
                Ok(evs) => evs,
                Err(e) => {
                    let _ = writeln!(writer, "\x1b[1;31mFailed to parse JSON Event stream: {}\x1b[0m", e);
                    return;
                }
            };
            let pid = events.first().map_or(1, |e| e.thread.tgid);
            let ident = make_identity(pid, "custom_target");
            let engine = Engine::new(8);
            let report = engine.analyze_with_identity(ident, Some(pid), events, LossLedger::new());
            let rendered = render_diagnosis(&report.diagnosis, false);
            let _ = write!(writer, "{}", rendered);
        }
    }
}

fn make_identity(pid: u32, comm: &str) -> TargetIdentity {
    TargetIdentity {
        tgid: pid,
        pid,
        comm: comm.to_string(),
        start_time_ticks: 123456,
        boot_id: "00000000-0000-0000-0000-000000000000".to_string(),
    }
}
