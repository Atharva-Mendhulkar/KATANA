use std::env;
use std::fs;
use std::process;

use katana::cli::{exit_codes, parse_args, ParseResult};
use katana::diagnosis::DiagStatus;
use katana::events::{Event, LossLedger};
use katana::output::TargetIdentity;
use katana::renderer::render_diagnosis;
use katana::target;
use katana::Engine;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cli = match parse_args(&args) {
        ParseResult::Ok(c) => c,
        ParseResult::Help(h) => {
            println!("{}", h);
            process::exit(exit_codes::SUCCESS);
        }
        ParseResult::Version(v) => {
            println!("{}", v);
            process::exit(exit_codes::SUCCESS);
        }
        ParseResult::Err(err) => {
            eprintln!("{}", err);
            process::exit(exit_codes::USAGE_ERROR);
        }
    };

    let (target_ident, events, loss_ledger) = if let Some(replay_path) = &cli.replay_path {
        let content = match fs::read_to_string(replay_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read replay file '{}': {}", replay_path, e);
                process::exit(exit_codes::INTERNAL_ERROR);
            }
        };

        let evs: Vec<Event> = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to parse replay file as JSON Event stream: {}", e);
                process::exit(exit_codes::INTERNAL_ERROR);
            }
        };
        let target_pid = cli.pid.unwrap_or_else(|| evs.first().map_or(1, |e| e.thread.tgid));
        let ident = TargetIdentity {
            tgid: target_pid,
            pid: target_pid,
            comm: "target".to_string(),
            start_time_ticks: 123456,
            boot_id: "00000000-0000-0000-0000-000000000000".to_string(),
        };
        (ident, evs, LossLedger::new())
    } else if let Some(target_pid) = cli.pid {
        // Live mode per PRD §5.1, §7.1, §16.2:
        // 1. Resolve target identity
        let initial_ident = match target::resolve_target(target_pid) {
            Ok(ident) => ident,
            Err(_) => {
                if !std::path::Path::new(&format!("/proc/{}", target_pid)).exists() {
                    eprintln!("katana: target PID {} not found or not visible", target_pid);
                    process::exit(exit_codes::TARGET_NOT_FOUND); // 11
                }
                eprintln!("katana: target PID {} exited before trace collection", target_pid);
                process::exit(exit_codes::TARGET_EXITED_EARLY); // 14
            }
        };

        // 2. Initial state snapshot
        let initial_snapshot = match target::take_snapshot(target_pid) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("katana: failed to take initial snapshot: {}", e);
                process::exit(exit_codes::TARGET_NOT_FOUND);
            }
        };

        // 3. Verify --tid belongs to target_pid
        if let Some(subject_tid) = cli.tid {
            if !initial_snapshot.threads.iter().any(|t| t.tid == subject_tid) {
                eprintln!(
                    "katana: TID {} is not a thread of PID {}; specify a valid thread or omit --tid",
                    subject_tid, target_pid
                );
                process::exit(exit_codes::USAGE_ERROR);
            }
        }

        // 4. Privilege check
        if !target::has_root_privileges() {
            eprintln!(
                "katana: insufficient privileges to attach eBPF programs (CAP_BPF / CAP_PERFMON or root required)"
            );
            process::exit(exit_codes::INSUFFICIENT_PRIVS); // 10
        }

        eprintln!(
            "katana: attached to PID {} ('{}'), observing for {} ms...",
            target_pid, initial_ident.comm, cli.duration_ms
        );
        std::thread::sleep(std::time::Duration::from_millis(cli.duration_ms));

        // 5. Final snapshot & verify identity
        if let Ok(final_ident) = target::resolve_target(target_pid) {
            if !target::verify_identity(&initial_ident, &final_ident) {
                eprintln!(
                    "katana: target PID {} identity changed during observation window (PID reuse suspected)",
                    target_pid
                );
                process::exit(exit_codes::AMBIGUOUS_OR_INVALID); // 4
            }
        } else {
            eprintln!("katana: target PID {} exited during observation window", target_pid);
        }

        (initial_ident, Vec::new(), LossLedger::new())
    } else {
        eprintln!("katana: missing PID");
        process::exit(exit_codes::USAGE_ERROR);
    };

    let engine = Engine::new(cli.max_depth);
    let report = engine.analyze_with_identity(target_ident, cli.tid, events, loss_ledger);

    if cli.json {
        match report.to_json_pretty() {
            Ok(json_str) => println!("{}", json_str),
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                process::exit(exit_codes::INTERNAL_ERROR);
            }
        }
    } else {
        let rendered = render_diagnosis(&report.diagnosis, cli.verbose);
        print!("{}", rendered);
    }

    let code = match report.diagnosis.status {
        DiagStatus::Found | DiagStatus::NotBlocked => exit_codes::SUCCESS,
        DiagStatus::Unknown => exit_codes::UNKNOWN,
        DiagStatus::Ambiguous | DiagStatus::Invalid => exit_codes::AMBIGUOUS_OR_INVALID,
    };

    process::exit(code);
}
