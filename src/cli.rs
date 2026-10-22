//! CLI argument parser and exit codes (PRD §16).

pub mod exit_codes {
    pub const SUCCESS: i32 = 0; // Found / NotBlocked
    pub const INTERNAL_ERROR: i32 = 1;
    pub const USAGE_ERROR: i32 = 2;
    pub const UNKNOWN: i32 = 3;
    pub const AMBIGUOUS_OR_INVALID: i32 = 4;
    pub const INSUFFICIENT_PRIVS: i32 = 10;
    pub const TARGET_NOT_FOUND: i32 = 11;
    pub const KERNEL_UNSUPPORTED: i32 = 12;
    pub const BPF_ATTACH_FAILED: i32 = 13;
    pub const TARGET_EXITED_EARLY: i32 = 14;
}

pub const HELP_TEXT: &str = "\
Katana - eBPF-based causal latency diagnosis tool

Usage:
  katana explain <PID> [options]
  katana watch <PID> [options]
  katana demo [scenario] [options]
  katana doctor
  katana fixtures

Commands:
  explain <PID>         Single-shot bounded observation window and root-cause analysis
  watch <PID>           Continuous ring-buffer watch mode with anomaly triggering
  demo [scenario]       Run interactive built-in demonstration traces (futex, io, writeback, watch, all)
  interactive, console  Run always-on interactive demonstration console
  doctor                Inspect system capabilities (kernel, eBPF, BTF, permissions)
  fixtures              List available demonstration traces and causal rules

Options:
  --duration <ms|s>     Observation window for explain (500ms..30s, default: 3000ms)
  --watch               Continuous watch mode: buffer events and trigger on threshold
  --threshold <ms|s>    Latency anomaly threshold for watch (default: 50ms)
  --retention <N>       Retention buffer size in events (default: 5000)
  --capacity <N>        Alias for --retention
  --max-episodes <N>    Exit watch mode after N detected episodes
  --html [path]         Export self-contained static HTML report (default: katana_report.html)
  --open                Automatically open exported HTML report in default web browser
  --json                Emit report as JSON to stdout
  --verbose             Include detailed event provenance
  --tid <TID>           Filter by thread ID
  --max-depth <N>       Maximum causality chain depth (1..16, default: 8)
  --replay <path>       Replay trace file
  -h, --help            Print help information
  -v, --version         Print version information";

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub open_browser: bool,
    pub verbose: bool,
    pub max_depth: usize,
    pub replay_path: Option<String>,
    pub demo_scenario: Option<String>,
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
            open_browser: false,
            verbose: false,
            max_depth: 8,
            replay_path: None,
            demo_scenario: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseResult {
    Ok(CliArgs),
    Help(String),
    Version(String),
    Err(String),
}

/// Parses duration strings like "500ms", "3s", "3000" into milliseconds.
fn parse_duration_ms(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty duration".to_string());
    }
    if let Some(stripped) = s.strip_suffix("ms") {
        stripped.trim().parse::<u64>().map_err(|_| "Invalid duration".to_string())
    } else if let Some(stripped) = s.strip_suffix('s') {
        let secs = stripped.trim().parse::<u64>().map_err(|_| "Invalid duration".to_string())?;
        secs.checked_mul(1000).ok_or_else(|| "Duration overflow".to_string())
    } else {
        s.parse::<u64>().map_err(|_| "Invalid duration".to_string())
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

    for (i, row) in dp.iter_mut().enumerate().take(a_chars.len() + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(b_chars.len() + 1) {
        *val = j;
    }

    for (i, ca) in a_chars.iter().enumerate() {
        for (j, cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = (dp[i][j + 1] + 1)
                .min(dp[i + 1][j] + 1)
                .min(dp[i][j] + cost);
        }
    }
    dp[a_chars.len()][b_chars.len()]
}

fn suggest_command(input: &str) -> Option<&'static str> {
    const KNOWN: &[&str] = &["explain", "watch", "demo", "doctor", "fixtures", "interactive", "console"];
    KNOWN
        .iter()
        .copied()
        .filter(|&cmd| levenshtein(input, cmd) <= 2)
        .min_by_key(|&cmd| levenshtein(input, cmd))
}

pub fn parse_args(args: &[String]) -> ParseResult {
    if args.len() < 2 {
        return ParseResult::Err("Usage: katana <command> [options]\n\nTry 'katana interactive' or 'katana --help' for available commands.".to_string());
    }

    // Check for global help or version flags anywhere in args
    for arg in args.iter().skip(1) {
        if arg == "--help" || arg == "-h" || arg == "help" {
            return ParseResult::Help(HELP_TEXT.to_string());
        }
        if arg == "--version" || arg == "-v" || arg == "-V" {
            return ParseResult::Version(format!("katana {}", env!("CARGO_PKG_VERSION")));
        }
    }

    let mut cli = CliArgs::default();
    let mut i = 1;

    // Subcommand dispatch or implicit default
    if args[i] == "explain" || args[i] == "watch" {
        cli.command = args[i].clone();
        i += 1;
    } else if args[i] == "interactive" || args[i] == "console" || args[i] == "shell" || args[i] == "repl" {
        cli.command = "interactive".to_string();
        i += 1;
    } else if args[i] == "demo" {
        cli.command = "demo".to_string();
        i += 1;
        if i < args.len() && !args[i].starts_with("--") {
            cli.demo_scenario = Some(args[i].clone());
            i += 1;
        }
    } else if args[i] == "doctor" {
        cli.command = "doctor".to_string();
        i += 1;
    } else if args[i] == "fixtures" || args[i] == "list" || args[i] == "scenarios" {
        cli.command = "fixtures".to_string();
        i += 1;
    } else if args[i].starts_with("--") {
        // Flag passed without explicit subcommand; default to explain
        cli.command = "explain".to_string();
    } else if args[i].chars().all(|c| c.is_ascii_digit()) {
        // PID passed directly; default to explain
        cli.command = "explain".to_string();
    } else if let Some(sugg) = suggest_command(&args[i]) {
        return ParseResult::Err(format!(
            "Unknown command '{}'. Did you mean '{}'?",
            args[i], sugg
        ));
    } else {
        return ParseResult::Err(format!(
            "Unknown command '{}'. Available commands: explain, watch, demo, doctor, fixtures.",
            args[i]
        ));
    }

    while i < args.len() {
        match args[i].as_str() {
            "--watch" => {
                cli.command = "watch".to_string();
            }
            "--duration" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --duration".to_string());
                }
                let ms = match parse_duration_ms(&args[i]) {
                    Ok(v) => v,
                    Err(_) => return ParseResult::Err("Invalid duration".to_string()),
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
                    let next = &args[i + 1];
                    // If next arg is all digits and PID not yet assigned, treat next arg as PID
                    if cli.pid.is_none() && next.chars().all(|c| c.is_ascii_digit()) && !next.ends_with(".html") {
                        cli.html_path = Some("katana_report.html".to_string());
                    } else {
                        i += 1;
                        cli.html_path = Some(next.clone());
                    }
                } else {
                    cli.html_path = Some("katana_report.html".to_string());
                }
            }
            "--open" => {
                cli.open_browser = true;
                if cli.html_path.is_none() {
                    cli.html_path = Some("katana_report.html".to_string());
                }
            }
            "--threshold" | "--threshold-ms" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --threshold".to_string());
                }
                let th = match parse_duration_ms(&args[i]) {
                    Ok(v) => v,
                    Err(_) => return ParseResult::Err("Invalid threshold value".to_string()),
                };
                if th == 0 {
                    return ParseResult::Err("Threshold must be greater than 0ms".to_string());
                }
                cli.threshold_ms = th;
            }
            "--retention" | "--retention-events" | "--capacity" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --retention".to_string());
                }
                match args[i].parse::<usize>() {
                    Ok(cap) if cap > 0 => cli.retention_capacity = cap,
                    _ => return ParseResult::Err("Invalid retention capacity".to_string()),
                }
            }
            "--max-episodes" => {
                i += 1;
                if i >= args.len() {
                    return ParseResult::Err("Missing value for --max-episodes".to_string());
                }
                match args[i].parse::<usize>() {
                    Ok(ep) if ep > 0 => cli.max_episodes = Some(ep),
                    _ => return ParseResult::Err("Invalid max-episodes value".to_string()),
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

    let requires_pid = cli.command == "explain" || cli.command == "watch";
    if requires_pid && cli.pid.is_none() && cli.replay_path.is_none() {
        return ParseResult::Err(format!(
            "PID is required for 'katana {} <PID>' (or use --replay <path> / try 'katana demo')",
            cli.command
        ));
    }

    ParseResult::Ok(cli)
}
