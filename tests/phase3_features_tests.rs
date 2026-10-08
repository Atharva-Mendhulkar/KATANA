//! Tests for Phase 3 features:
//! - Feature A: Static HTML Graph Visualization Export (monochrome, zero emojis, modern sleek)
//! - Feature B: Continuous In-Kernel Watch Mode with Bounded Retention Buffer

use katana::events::{Event, LossLedger};
use katana::html_export::render_html_report;
use katana::output::TargetIdentity;
use katana::scheduler::{EventRef, SchedSwitch, SchedWaking, TaskState, ThreadId, WakerCtx};
use katana::watcher::{WatchConfig, Watcher};
use katana::Engine;

fn make_ident(pid: u32) -> TargetIdentity {
    TargetIdentity {
        tgid: pid,
        pid,
        comm: "target_app".to_string(),
        start_time_ticks: 123456,
        boot_id: "00000000-0000-0000-0000-000000000000".to_string(),
    }
}

/// Helper to detect if any character in the string is an emoji.
fn contains_emoji(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{1F600}'..='\u{1F64F}' | // Emoticons
            '\u{1F300}'..='\u{1F5FF}' | // Misc Symbols and Pictographs
            '\u{1F680}'..='\u{1F6FF}' | // Transport and Map
            '\u{1F700}'..='\u{1F77F}' | // Alchemical Symbols
            '\u{1F780}'..='\u{1F7FF}' | // Geometric Shapes Extended
            '\u{1F800}'..='\u{1F8FF}' | // Supplemental Arrows-C
            '\u{1F900}'..='\u{1F9FF}' | // Supplemental Symbols and Pictographs
            '\u{1FA00}'..='\u{1FA6F}' | // Chess Symbols
            '\u{1FA70}'..='\u{1FAFF}' | // Symbols and Pictographs Extended-A
            '\u{2600}'..='\u{26FF}'   | // Misc symbols
            '\u{2700}'..='\u{27BF}'     // Dingbats
        )
    })
}

#[test]
fn test_html_export_validity_and_anti_emoji_lint() {
    let trace_data = include_str!("../fixtures/bio1_sync_io.json");
    let events: Vec<Event> = serde_json::from_str(trace_data).expect("valid fixture");

    let ident = make_ident(4217);
    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());

    let html = render_html_report(&report);

    // 1. Structure assertions
    assert!(html.starts_with("<!DOCTYPE html>"), "Must be valid HTML5");
    assert!(html.contains("<html lang=\"en\">"));
    assert!(html.contains("</html>"));
    assert!(html.contains("<svg"), "Must contain SVG causal graph");
    assert!(html.contains("</svg>"));

    // 2. Strict Black & White / Zero Emoji requirement
    assert!(
        !contains_emoji(&html),
        "CRITICAL: Static HTML report must contain ZERO emojis!"
    );

    // 3. Typography & Sleek Minimalist Structure
    assert!(html.contains("KATANA"));
    assert!(html.contains("LATENCY ROOT-CAUSE ANALYSIS"));
    assert!(html.contains("CAUSAL GRAPH TOPOLOGY"));
    assert!(html.contains("EVIDENCE LEDGER"));

    // 4. Monochrome verification (no saturated web color names)
    let lower_html = html.to_lowercase();
    assert!(!lower_html.contains("color: red"));
    assert!(!lower_html.contains("color: green"));
    assert!(!lower_html.contains("color: blue"));
    assert!(!lower_html.contains("color: yellow"));
}

#[test]
fn test_html_export_multi_hop_chain_fixture() {
    let trace_data = include_str!("../fixtures/bio2_writeback.json");
    let events: Vec<Event> = serde_json::from_str(trace_data).expect("valid fixture");

    let ident = make_ident(4217);
    let engine = Engine::new(8);
    let report = engine.analyze_with_identity(ident, Some(4217), events, LossLedger::new());

    let html = render_html_report(&report);

    assert!(!contains_emoji(&html));
    assert!(html.contains("RULE BIO-2"));
    assert!(html.contains("CORRELATED"));
    assert!(html.contains("This trace does not establish a causal link"));
}

#[test]
fn test_watch_mode_threshold_triggering() {
    let target_tid = 1001;
    let other_tid = 2002;
    let ident = make_ident(target_tid);

    let config = WatchConfig {
        target_pid: target_tid,
        tid: Some(target_tid),
        threshold_ms: 50, // 50 ms threshold
        retention_capacity: 100,
        max_episodes: Some(1),
        json: false,
        html_path: None,
        verbose: false,
        max_depth: 8,
    };

    let mut watcher = Watcher::new(config, ident);

    // Stream 1: Short sleep (10 ms < 50 ms threshold)
    let mut short_events = Vec::new();
    // Target switches off CPU at t = 100 ms
    short_events.push(Event::new(
        100_000_000,
        EventRef::new(0, 1),
        ThreadId::new(target_tid, target_tid),
        katana::events::EventKind::Switch(SchedSwitch {
            prev: ThreadId::new(target_tid, target_tid),
            next: ThreadId::new(other_tid, other_tid),
            prev_state: TaskState::Sleeping,
            preempted: false,
            in_iowait: false,
        }),
    ));
    // Target switches back on CPU at t = 110 ms (10 ms delta)
    short_events.push(Event::new(
        110_000_000,
        EventRef::new(0, 2),
        ThreadId::new(other_tid, other_tid),
        katana::events::EventKind::Switch(SchedSwitch {
            prev: ThreadId::new(other_tid, other_tid),
            next: ThreadId::new(target_tid, target_tid),
            prev_state: TaskState::Running,
            preempted: false,
            in_iowait: false,
        }),
    ));

    let triggered_short = watcher.process_event_stream(short_events);
    assert_eq!(
        triggered_short.len(),
        0,
        "10ms sleep should NOT trigger 50ms watch threshold"
    );

    // Stream 2: Long sleep (70 ms > 50 ms threshold)
    let mut long_events = Vec::new();
    // Target switches off CPU at t = 200 ms
    long_events.push(Event::new(
        200_000_000,
        EventRef::new(0, 3),
        ThreadId::new(target_tid, target_tid),
        katana::events::EventKind::Switch(SchedSwitch {
            prev: ThreadId::new(target_tid, target_tid),
            next: ThreadId::new(other_tid, other_tid),
            prev_state: TaskState::Sleeping,
            preempted: false,
            in_iowait: false,
        }),
    ));
    // Waker wakes target at t = 270 ms (70 ms delta)
    long_events.push(Event::new(
        270_000_000,
        EventRef::new(0, 4),
        ThreadId::new(other_tid, other_tid),
        katana::events::EventKind::Waking(SchedWaking {
            wakee: ThreadId::new(target_tid, target_tid),
            target_cpu: 0,
            waker_ctx: WakerCtx::Task,
        }),
    ));
    // Target switches back on CPU at t = 271 ms
    long_events.push(Event::new(
        271_000_000,
        EventRef::new(0, 5),
        ThreadId::new(other_tid, other_tid),
        katana::events::EventKind::Switch(SchedSwitch {
            prev: ThreadId::new(other_tid, other_tid),
            next: ThreadId::new(target_tid, target_tid),
            prev_state: TaskState::Running,
            preempted: false,
            in_iowait: false,
        }),
    ));

    let triggered_long = watcher.process_event_stream(long_events);
    assert_eq!(
        triggered_long.len(),
        1,
        "70ms sleep MUST trigger 50ms watch threshold"
    );
    assert_eq!(watcher.episodes_captured, 1);
}

#[test]
fn test_watch_mode_retention_capacity_bounding() {
    let target_tid = 5000;
    let ident = make_ident(target_tid);

    let config = WatchConfig {
        target_pid: target_tid,
        tid: Some(target_tid),
        threshold_ms: 100,
        retention_capacity: 50, // Strict 50-event limit
        max_episodes: None,
        json: false,
        html_path: None,
        verbose: false,
        max_depth: 8,
    };

    let mut watcher = Watcher::new(config, ident);

    // Push 200 arbitrary events
    for seq in 1..=200 {
        let ev = Event::new(
            seq * 1_000_000,
            EventRef::new(0, seq),
            ThreadId::new(target_tid, target_tid),
            katana::events::EventKind::Exit,
        );
        watcher.push_event(ev);
    }

    assert_eq!(
        watcher.retention_buffer.len(),
        50,
        "Retention buffer must never exceed configured capacity"
    );
}
