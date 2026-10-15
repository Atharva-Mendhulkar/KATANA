//! Comprehensive integration tests for Katana CLI argument parser and exit codes (PRD §16).

use katana::cli::{exit_codes, parse_args, CliArgs, ParseResult};

fn to_args(args_str: &str) -> Vec<String> {
    args_str.split_whitespace().map(|s| s.to_string()).collect()
}

#[test]
fn test_exit_codes_spec_conformance() {
    // PRD §16.2 normative exit code mapping
    assert_eq!(exit_codes::SUCCESS, 0);
    assert_eq!(exit_codes::INTERNAL_ERROR, 1);
    assert_eq!(exit_codes::USAGE_ERROR, 2);
    assert_eq!(exit_codes::UNKNOWN, 3);
    assert_eq!(exit_codes::AMBIGUOUS_OR_INVALID, 4);
    assert_eq!(exit_codes::INSUFFICIENT_PRIVS, 10);
    assert_eq!(exit_codes::TARGET_NOT_FOUND, 11);
    assert_eq!(exit_codes::KERNEL_UNSUPPORTED, 12);
    assert_eq!(exit_codes::BPF_ATTACH_FAILED, 13);
    assert_eq!(exit_codes::TARGET_EXITED_EARLY, 14);
}

#[test]
fn test_default_cli_args() {
    let def = CliArgs::default();
    assert_eq!(def.command, "explain");
    assert_eq!(def.pid, None);
    assert_eq!(def.tid, None);
    assert_eq!(def.duration_ms, 3000);
    assert_eq!(def.threshold_ms, 50);
    assert_eq!(def.retention_capacity, 5000);
    assert_eq!(def.max_episodes, None);
    assert!(!def.json);
    assert_eq!(def.html_path, None);
    assert!(!def.verbose);
    assert_eq!(def.max_depth, 8);
    assert_eq!(def.replay_path, None);
}

#[test]
fn test_subcommand_dispatch() {
    // explain subcommand
    let res = parse_args(&to_args("katana explain 4217"));
    match res {
        ParseResult::Ok(cli) => {
            assert_eq!(cli.command, "explain");
            assert_eq!(cli.pid, Some(4217));
        }
        other => panic!("Expected Ok, got {:?}", other),
    }

    // watch subcommand
    let res = parse_args(&to_args("katana watch 4217"));
    match res {
        ParseResult::Ok(cli) => {
            assert_eq!(cli.command, "watch");
            assert_eq!(cli.pid, Some(4217));
        }
        other => panic!("Expected Ok, got {:?}", other),
    }

    // implicit explain with bare PID
    let res = parse_args(&to_args("katana 4217"));
    match res {
        ParseResult::Ok(cli) => {
            assert_eq!(cli.command, "explain");
            assert_eq!(cli.pid, Some(4217));
        }
        other => panic!("Expected Ok, got {:?}", other),
    }
}

#[test]
fn test_watch_flag_and_capacity_alias() {
    // katana explain 4217 --watch --threshold 250ms --capacity 10000 (from README)
    let res = parse_args(&to_args(
        "katana explain 4217 --watch --threshold 250ms --capacity 10000",
    ));
    match res {
        ParseResult::Ok(cli) => {
            assert_eq!(cli.command, "watch");
            assert_eq!(cli.pid, Some(4217));
            assert_eq!(cli.threshold_ms, 250);
            assert_eq!(cli.retention_capacity, 10000);
        }
        other => panic!("Expected Ok, got {:?}", other),
    }
}

#[test]
fn test_duration_parsing_and_bounds() {
    // ms suffix
    let res = parse_args(&to_args("katana explain 4217 --duration 500ms"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.duration_ms == 500));

    // s suffix
    let res = parse_args(&to_args("katana explain 4217 --duration 3s"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.duration_ms == 3000));

    // plain integer
    let res = parse_args(&to_args("katana explain 4217 --duration 1500"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.duration_ms == 1500));

    // lower bound: < 500ms
    let res = parse_args(&to_args("katana explain 4217 --duration 499ms"));
    assert!(matches!(res, ParseResult::Err(e) if e.contains("between 500ms and 30000ms")));

    // upper bound: > 30000ms
    let res = parse_args(&to_args("katana explain 4217 --duration 30001ms"));
    assert!(matches!(res, ParseResult::Err(e) if e.contains("between 500ms and 30000ms")));

    // invalid string
    let res = parse_args(&to_args("katana explain 4217 --duration invalid"));
    assert!(matches!(res, ParseResult::Err(e) if e.contains("Invalid duration")));
}

#[test]
fn test_threshold_units_and_validation() {
    // ms suffix
    let res = parse_args(&to_args("katana watch 4217 --threshold 250ms"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.threshold_ms == 250));

    // s suffix
    let res = parse_args(&to_args("katana watch 4217 --threshold 1s"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.threshold_ms == 1000));

    // raw integer
    let res = parse_args(&to_args("katana watch 4217 --threshold 75"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.threshold_ms == 75));

    // threshold 0 must fail
    let res = parse_args(&to_args("katana watch 4217 --threshold 0ms"));
    assert!(matches!(res, ParseResult::Err(e) if e.contains("greater than 0")));
}

#[test]
fn test_retention_capacity_options() {
    // --retention
    let res = parse_args(&to_args("katana watch 4217 --retention 2000"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.retention_capacity == 2000));

    // --retention-events
    let res = parse_args(&to_args("katana watch 4217 --retention-events 3000"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.retention_capacity == 3000));

    // --capacity
    let res = parse_args(&to_args("katana watch 4217 --capacity 8000"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.retention_capacity == 8000));

    // capacity 0 must fail
    let res = parse_args(&to_args("katana watch 4217 --capacity 0"));
    assert!(matches!(res, ParseResult::Err(e) if e.contains("Invalid retention capacity")));
}

#[test]
fn test_max_episodes() {
    let res = parse_args(&to_args("katana watch 4217 --max-episodes 5"));
    assert!(matches!(res, ParseResult::Ok(cli) if cli.max_episodes == Some(5)));

    let res = parse_args(&to_args("katana watch 4217 --max-episodes 0"));
    assert!(matches!(res, ParseResult::Err(e) if e.contains("Invalid max-episodes")));
}

#[test]
fn test_max_depth_bounds() {
    // valid depths
    assert!(matches!(
        parse_args(&to_args("katana explain 4217 --max-depth 1")),
        ParseResult::Ok(cli) if cli.max_depth == 1
    ));
    assert!(matches!(
        parse_args(&to_args("katana explain 4217 --max-depth 16")),
        ParseResult::Ok(cli) if cli.max_depth == 16
    ));

    // out of bounds
    assert!(matches!(
        parse_args(&to_args("katana explain 4217 --max-depth 0")),
        ParseResult::Err(e) if e.contains("between 1 and 16")
    ));
    assert!(matches!(
        parse_args(&to_args("katana explain 4217 --max-depth 17")),
        ParseResult::Err(e) if e.contains("between 1 and 16")
    ));
}

#[test]
fn test_html_export_flag() {
    // default filename when --html is flag only
    let res = parse_args(&to_args("katana explain 4217 --html"));
    assert!(matches!(
        res,
        ParseResult::Ok(cli) if cli.html_path == Some("katana_report.html".to_string())
    ));

    // custom path
    let res = parse_args(&to_args("katana explain 4217 --html my_report.html"));
    assert!(matches!(
        res,
        ParseResult::Ok(cli) if cli.html_path == Some("my_report.html".to_string())
    ));

    // --html followed by PID does not consume PID as html filename
    let res = parse_args(&to_args("katana explain --html 4217"));
    match res {
        ParseResult::Ok(cli) => {
            assert_eq!(cli.pid, Some(4217));
            assert_eq!(cli.html_path, Some("katana_report.html".to_string()));
        }
        other => panic!("Expected Ok with PID 4217, got {:?}", other),
    }
}

#[test]
fn test_replay_trace() {
    // explain with replay
    let res = parse_args(&to_args("katana explain --replay fixtures/bio1_sync_io.json"));
    assert!(matches!(
        res,
        ParseResult::Ok(cli) if cli.replay_path == Some("fixtures/bio1_sync_io.json".to_string())
    ));

    // bare katana with replay (no subcommand)
    let res = parse_args(&to_args("katana --replay fixtures/bio1_sync_io.json"));
    assert!(matches!(
        res,
        ParseResult::Ok(cli) if cli.command == "explain" && cli.replay_path == Some("fixtures/bio1_sync_io.json".to_string())
    ));
}

#[test]
fn test_help_and_version() {
    // --help
    assert!(matches!(parse_args(&to_args("katana --help")), ParseResult::Help(_)));
    assert!(matches!(parse_args(&to_args("katana -h")), ParseResult::Help(_)));
    assert!(matches!(parse_args(&to_args("katana explain --help")), ParseResult::Help(_)));
    assert!(matches!(parse_args(&to_args("katana watch -h")), ParseResult::Help(_)));

    // --version
    assert!(matches!(parse_args(&to_args("katana --version")), ParseResult::Version(v) if v.contains("katana")));
    assert!(matches!(parse_args(&to_args("katana -v")), ParseResult::Version(v) if v.contains("katana")));
    assert!(matches!(parse_args(&to_args("katana explain -v")), ParseResult::Version(v) if v.contains("katana")));
}

#[test]
fn test_errors() {
    // Missing arguments
    assert!(matches!(parse_args(&to_args("katana")), ParseResult::Err(_)));

    // Missing PID when no replay
    assert!(matches!(parse_args(&to_args("katana explain")), ParseResult::Err(e) if e.contains("PID is required")));

    // Unrecognized option
    assert!(matches!(parse_args(&to_args("katana explain 4217 --unknown-flag")), ParseResult::Err(e) if e.contains("Unrecognized option")));

    // Missing option parameter
    assert!(matches!(parse_args(&to_args("katana explain 4217 --duration")), ParseResult::Err(e) if e.contains("Missing value")));
    assert!(matches!(parse_args(&to_args("katana explain 4217 --threshold")), ParseResult::Err(e) if e.contains("Missing value")));
    assert!(matches!(parse_args(&to_args("katana explain 4217 --tid")), ParseResult::Err(e) if e.contains("Missing value")));
    assert!(matches!(parse_args(&to_args("katana explain 4217 --max-depth")), ParseResult::Err(e) if e.contains("Missing value")));
}
