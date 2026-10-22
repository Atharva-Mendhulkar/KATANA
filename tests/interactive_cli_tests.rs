//! Tests for Katana Always-On Interactive CLI and Logo Alignment.

use std::io::Cursor;
use katana::cli::{parse_args, ParseResult};
use katana::interactive::run_interactive_loop;
use katana::logo::{KATANA_BANNER, KATANA_LOGO_RAW};

fn to_args(args_str: &str) -> Vec<String> {
    args_str.split_whitespace().map(|s| s.to_string()).collect()
}

#[test]
fn test_logo_dimensions_and_sword_alignment() {
    // Check raw logo
    let lines: Vec<&str> = KATANA_LOGO_RAW.lines().collect();
    assert_eq!(lines.len(), 15, "Logo must have 8 text lines + 7 sword lines = 15 total");

    // The KATANA letter art is lines 0..8
    // Line 7 is " ▀██▀  ▀██▀  ▀░█▀    ░░▓█▀    ▀███▄▀        ▀░█▀    ░░▓█▀    ▀▀▀     ▀▀    ▀░█▀    ░░▓█▀ "
    let last_letter_line = lines[7].trim_end();
    let letter_width = last_letter_line.chars().count();
    assert_eq!(letter_width, 88, "Katana ASCII art width must be 88 characters");

    // Line 10: Top blade
    //   /============| |---------------------------------------------------------------------,
    let top_blade = lines[10];
    assert_eq!(
        top_blade.chars().count(),
        letter_width,
        "Top blade length must exactly match the 88-char width of the katana ascii art"
    );
    assert!(top_blade.starts_with("  /============| |"));
    assert!(top_blade.ends_with(','));

    // Line 11: Mid blade with hamon wave and tip
    // {=| / / / / / /|()}     }     }     }     }     }     }     }     }     }              >
    let mid_blade = lines[11];
    assert_eq!(
        mid_blade.chars().count(),
        letter_width,
        "Mid blade length must exactly match the 88-char width of the katana ascii art"
    );
    assert!(mid_blade.starts_with("{=| / / / / / /|()}"));
    assert!(mid_blade.ends_with('>'));

    // Line 12: Bottom blade
    //   \============| |---------------------------------------------------------------------'
    let bot_blade = lines[12];
    assert_eq!(
        bot_blade.chars().count(),
        letter_width,
        "Bottom blade length must exactly match the 88-char width of the katana ascii art"
    );
    assert!(bot_blade.starts_with("  \\============| |"));
    assert!(bot_blade.ends_with('\''));

    // Verify colored banner contains the logo
    assert!(KATANA_BANNER.contains("▄▓▒▄"));
    assert!(KATANA_BANNER.contains("---------------------------------------------------------------------,"));
}

#[test]
fn test_cli_interactive_subcommand_dispatch() {
    // "katana interactive"
    let res = parse_args(&to_args("katana interactive"));
    match res {
        ParseResult::Ok(cli) => assert_eq!(cli.command, "interactive"),
        other => panic!("Expected Ok(interactive), got {:?}", other),
    }

    // "katana console"
    let res = parse_args(&to_args("katana console"));
    match res {
        ParseResult::Ok(cli) => assert_eq!(cli.command, "interactive"),
        other => panic!("Expected Ok(interactive), got {:?}", other),
    }

    // "katana shell"
    let res = parse_args(&to_args("katana shell"));
    match res {
        ParseResult::Ok(cli) => assert_eq!(cli.command, "interactive"),
        other => panic!("Expected Ok(interactive), got {:?}", other),
    }

    // "katana repl"
    let res = parse_args(&to_args("katana repl"));
    match res {
        ParseResult::Ok(cli) => assert_eq!(cli.command, "interactive"),
        other => panic!("Expected Ok(interactive), got {:?}", other),
    }

    // "katana demo interactive"
    let res = parse_args(&to_args("katana demo interactive"));
    match res {
        ParseResult::Ok(cli) => {
            assert_eq!(cli.command, "demo");
            assert_eq!(cli.demo_scenario, Some("interactive".to_string()));
        }
        other => panic!("Expected Ok(demo interactive), got {:?}", other),
    }
}

#[test]
fn test_interactive_console_session_execution() {
    // Feed a scripted sequence of user commands into the always-on loop:
    // 1: Futex demo
    // 2: I/O demo
    // 3: Writeback demo
    // 4: Watch mode demo
    // 7: Ranker deep dive
    // 8: Grammar deep dive
    // 9: Multihop & cycles
    // 10: Loss ledger
    // 11: BPF architecture
    // 13: Benchmark
    // inspect futex
    // menu
    // logo
    // help
    // exit
    let input_script = "1\n2\n3\n4\n7\n8\n9\n10\n11\n13\ninspect futex\nmenu\nlogo\nhelp\nexit\n";
    let input_cursor = Cursor::new(input_script.as_bytes());
    let mut output_buf = Vec::new();

    run_interactive_loop(input_cursor, &mut output_buf, false);

    let output_str = String::from_utf8(output_buf).expect("Interactive output must be UTF-8");

    // Verify all feature sections executed in the always-on console session
    assert!(output_str.contains("KATANA FEATURE SHOWCASE"));
    assert!(output_str.contains("Rule FW-1: Multi-Waiter Futex Mutex Contention"));
    assert!(output_str.contains("Rule BIO-1: Direct Synchronous Block I/O Bottleneck"));
    assert!(output_str.contains("Rule BIO-2: Asynchronous Writeback Clamping & Anti-Inflation"));
    assert!(output_str.contains("WATCH ANOMALY TRIGGERED"));
    assert!(output_str.contains("8-Criterion Lexicographic Ranking Engine Deep Dive"));
    assert!(output_str.contains("Anti-Inflation Language Discipline & Allowed-Verb Grammar"));
    assert!(output_str.contains("Multi-Hop Backward Causal Graph & Cycle Detection"));
    assert!(output_str.contains("Loss Ledger & Ring Buffer Fault Injection"));
    assert!(output_str.contains("In-Kernel eBPF Tracing Perimeter & Scope Guard"));
    assert!(output_str.contains("High-Throughput Event Processing Microbenchmark"));
    assert!(output_str.contains("Trace Inspection Report for 'futex'"));
    assert!(output_str.contains("Exiting Katana console. Farewell."));
}

#[test]
fn test_interactive_html_export_command() {
    let tmp_html = "target/test_interactive_report.html";
    let input_script = format!("5 {}\nexit\n", tmp_html);
    let input_cursor = Cursor::new(input_script.into_bytes());
    let mut output_buf = Vec::new();

    run_interactive_loop(input_cursor, &mut output_buf, false);

    let output_str = String::from_utf8(output_buf).expect("Interactive output must be UTF-8");
    assert!(output_str.contains("Successfully exported static HTML report"));

    // Verify generated file exists and is valid HTML
    let html_content = std::fs::read_to_string(tmp_html).expect("Exported HTML must exist");
    assert!(html_content.contains("<!DOCTYPE html>"));
    assert!(html_content.contains("KATANA"));
    assert!(html_content.contains("<svg"));

    let _ = std::fs::remove_file(tmp_html);
}
