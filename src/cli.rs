//! CLI argument parser and exit codes (PRD §16).

pub mod exit_codes {
    pub const SUCCESS: i32 = 0; // Found / NotBlocked
    pub const INTERNAL_ERROR: i32 = 1;
    pub const USAGE_ERROR: i32 = 2;
    pub const UNKNOWN: i32 = 3;
    pub const AMBIGUOUS_OR_INVALID: i32 = 4;
    pub const INSUFFICIENT_PRIVS: i32 = 10;
    pub const TARGET_NOT_FOUND: i32 = 11;
    pub const TARGET_EXITED_EARLY: i32 = 14;
}

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub command: String,
    pub pid: Option<u32>,
    pub tid: Option<u32>,
    pub duration_ms: u64,
    pub threshold_ms: u64,
    pub retention_capacity: usize,
    pub max_episodes: Option<usize>,
    pub json: bool,
    pub html_path: Option<String>,
    pub verbose: bool,
    pub max_depth: usize,
    pub replay_path: Option<String>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            command: "explain".to_string(),
            pid: None,
            tid: None,
            duration_ms: 3000,
            threshold_ms: 50,
            retention_capacity: 5000,
            max_episodes: None,
            json: false,
            html_path: None,
            verbose: false,
            max_depth: 8,
            replay_path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParseResult {
    Ok(CliArgs),
    Help(String),
    Version(String),
    Err(String),
}

pub fn parse_args(args: &[String]) -> ParseResult {
    if args.len() < 2 {
        return ParseResult::Err("Usage: katana explain <PID> [options] | katana watch <PID> [options]".to_string());
    }

    let mut cli = CliArgs::default();
    let mut i = 1;

    if args[i] == "--help" || args[i] == "-h" {
        return ParseResult::Help("Katana - eBPF-based causal latency diagnosis tool\n\nUsage:\n  katana explain <PID> [options]\n  katana watch <PID> [options]\n\nSubcommands:\n  explain           Single-shot bounded observation window and root-cause analysis\n  watch             Continuous ring-buffer watch mode with anomaly triggering\n\nOptions:\n  --duration <ms>       Observation window for explain (default: 3000ms)\n  --threshold <ms>      Latency anomaly threshold for watch (default: 50ms)\n  --retention <N>       Retention buffer size in events (default: 5000)\n  --max-episodes <N>    Exit watch mode after N detected episodes\n  --html [path]         Export self-contained static HTML report (default: katana_report.html)\n  --json                Emit report as JSON to stdout\n  --verbose             Include detailed event provenance\n  --tid <TID>           Filter by thread ID\n  --max-depth <N>       Maximum causality chain depth (1..16, default: 8)\n  --replay <path>       Replay trace file".to_string());
    }

    if args[i] == "--version" || args[i] == "-v" {
        return ParseResult::Version("katana 0.1.0".to_string());
    }

    if args[i] == "explain" || args[i] == "watch" {
        cli.command = args[i].clone();
        i += 1;
    } else if !args[i].starts_with("--") {
        // Default to explain if PID is passed directly
        cli.command = "explain".to_string();
    } else {
        return ParseResult::Err(format!("Unknown subcommand '{}'. Expected 'explain' or 'watch'.", args[i]));
    }

    while i < args.len() {
        match args[i].as_str() {
            "--duration" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --duration".to_string());
                }
                let dur_str = &args[i];
                let ms: u64 = if dur_str.ends_with("ms") {
                    match dur_str[..dur_str.len() - 2].parse() {
                        Ok(v) => v,
                        Err(_) => return ParseResult::Err("Invalid duration".to_string()),
                    }
                } else if dur_str.ends_with('s') {
                    match dur_str[..dur_str.len() - 1].parse::<u64>() {
                        Ok(v) => v * 1000,
                        Err(_) => return ParseResult::Err("Invalid duration".to_string()),
                    }
                } else {
                    match dur_str.parse() {
                        Ok(v) => v,
                        Err(_) => return ParseResult::Err("Invalid duration".to_string()),
                    }
                };
                if !(500..=30000).contains(&ms) {
                    return ParseResult::Err("Duration must be between 500ms and 30000ms".to_string());
                }
                cli.duration_ms = ms;
            }
            "--tid" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --tid".to_string());
                }
                match args[i].parse() {
                    Ok(tid) => cli.tid = Some(tid),
                    Err(_) => return ParseResult::Err("Invalid TID".to_string()),
                }
            }
            "--json" => {
                cli.json = true;
            }
            "--verbose" => {
                cli.verbose = true;
            }
            "--max-depth" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --max-depth".to_string());
                }
                let depth: usize = match args[i].parse() {
                    Ok(d) => d,
                    Err(_) => return ParseResult::Err("Invalid max-depth".to_string()),
                };
                if !(1..=16).contains(&depth) {
                    return ParseResult::Err("Max depth must be between 1 and 16".to_string());
                }
                cli.max_depth = depth;
            }
            "--html" => {
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    i += 1;
                    cli.html_path = Some(args[i].clone());
                } else {
                    cli.html_path = Some("katana_report.html".to_string());
                }
            }
            "--threshold" | "--threshold-ms" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --threshold".to_string());
                }
                match args[i].parse::<u64>() {
                    Ok(th) => cli.threshold_ms = th,
                    Err(_) => return ParseResult::Err("Invalid threshold value".to_string()),
                }
            }
            "--retention" | "--retention-events" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --retention".to_string());
                }
                match args[i].parse::<usize>() {
                    Ok(cap) => cli.retention_capacity = cap,
                    Err(_) => return ParseResult::Err("Invalid retention capacity".to_string()),
                }
            }
            "--max-episodes" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --max-episodes".to_string());
                }
                match args[i].parse::<usize>() {
                    Ok(ep) => cli.max_episodes = Some(ep),
                    Err(_) => return ParseResult::Err("Invalid max-episodes value".to_string()),
                }
            }
            "--replay" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --replay".to_string());
                }
                cli.replay_path = Some(args[i].clone());
            }
            arg if !arg.starts_with("--") && cli.pid.is_none() => {
                match arg.parse() {
                    Ok(p) => cli.pid = Some(p),
                    Err(_) => return ParseResult::Err(format!("Invalid PID: {}", arg)),
                }
            }
            other => {
                return ParseResult::Err(format!("Unrecognized option: {}", other));
            }
        }
        i += 1;
    }

    if cli.pid.is_none() && cli.replay_path.is_none() {
        return ParseResult::Err("PID is required for 'katana explain <PID>'".to_string());
    }

    ParseResult::Ok(cli)
}
