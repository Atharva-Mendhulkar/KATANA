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
    pub json: bool,
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
            json: false,
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
        return ParseResult::Err("Usage: katana explain <PID> [--duration <ms>] [--tid <TID>] [--json] [--verbose] [--max-depth <N>] [--replay <file>]".to_string());
    }

    let mut cli = CliArgs::default();
    let mut i = 1;

    if args[i] == "--help" || args[i] == "-h" {
        return ParseResult::Help("Katana - eBPF-based causal latency diagnosis tool\n\nUsage:\n  katana explain <PID> [options]\n\nOptions:\n  --duration <ms>   Observation window (default: 3000ms)\n  --tid <TID>       Filter by thread ID\n  --json            Emit report as JSON to stdout\n  --verbose         Include detailed event provenance\n  --max-depth <N>   Maximum causality chain depth (1..16, default: 8)\n  --replay <path>   Replay trace file".to_string());
    }

    if args[i] == "--version" || args[i] == "-v" {
        return ParseResult::Version("katana 0.1.0".to_string());
    }

    if args[i] != "explain" {
        return ParseResult::Err(format!("Unknown subcommand '{}'. Expected 'explain'.", args[i]));
    }
    i += 1;

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
