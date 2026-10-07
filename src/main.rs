use std::env;
use std::fs;
use std::process;

use katana::cli::{exit_codes, parse_args, ParseResult};
use katana::diagnosis::DiagStatus;
use katana::events::{Event, LossLedger};
use katana::renderer::render_diagnosis;
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

    let (pid, events, loss_ledger) = if let Some(replay_path) = &cli.replay_path {
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
        (target_pid, evs, LossLedger::new())
    } else if let Some(target_pid) = cli.pid {
        // In unprivileged environment (e.g. macOS or no bpf), inform user or load fixtures
        eprintln!(
            "katana: attached to PID {}, observing for {} ms...",
            target_pid, cli.duration_ms
        );
        (target_pid, Vec::new(), LossLedger::new())
    } else {
        eprintln!("katana: missing PID");
        process::exit(exit_codes::USAGE_ERROR);
    };

    let engine = Engine::new(cli.max_depth);
    let report = engine.analyze(pid, cli.tid, events, loss_ledger);

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
