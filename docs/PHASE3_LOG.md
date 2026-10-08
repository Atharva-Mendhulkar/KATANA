# Phase 3 Implementation Log: Visualizer & Watch Mode

> Reference: [prd.md §33](file:///home/topfloorboss/Desktop/KATANA/prd.md), [docs/ARCHITECTURE.md](file:///home/topfloorboss/Desktop/KATANA/docs/ARCHITECTURE.md)

---

## 1. Overview & Objectives

In accordance with PRD §33 and user instructions, Phase 3 introduced:
1. **Feature A: Static HTML Graph Visualization Export (`--html [path]`)**
   - Strictly complete black and white only (monochrome palette: pure blacks `#050505`, `#0c0c0c`, crisp borders `#1e1e1e`, pure white accents `#ffffff`, muted grays `#888888`, `#aaaaaa`).
   - Zero emojis anywhere in the document (formally verified by unicode range lint test).
   - Simple modern sleek Swiss typography with generous whitespace and clear hierarchy.
   - Self-contained, zero-dependency HTML5 output with scalable vector graphics (SVG) causal graph topology (works 100% offline).
2. **Feature B: Continuous In-Kernel Watch Mode (`katana watch <PID>`)**
   - Circular memory-bounded retention buffer (`VecDeque<Event>`, default capacity 5,000 events).
   - Monitors thread off-CPU intervals and synchronization stalls continuously.
   - Triggers anomaly diagnosis episode when off-CPU latency exceeds `--threshold-ms` (default 50 ms).
   - Emits episode report to stdout, JSON stream, or exports static HTML report.
   - Fully testable in unprivileged user space via `--replay <path>` stream simulation.

---

## 2. Implemented Components

- [`src/html_export.rs`](file:///home/topfloorboss/Desktop/KATANA/src/html_export.rs):
  - `render_html_report(&Report) -> String`
  - `export_html_file(&Report, &str) -> std::io::Result<()>`
  - Inline vector SVG generator mapping thread nodes, causality rules, hop counters, terminal reasons, and block devices.
  - Formatted evidence ledger and system ingestion statistics table.
- [`src/watcher.rs`](file:///home/topfloorboss/Desktop/KATANA/src/watcher.rs):
  - `WatchConfig`: configuration for threshold, retention capacity, max episodes, JSON, HTML export.
  - `Watcher`: streaming state machine with bounded circular buffer, off-CPU interval tracking, and episode trigger logic.
  - `process_event_stream`: streaming consumer for deterministic replay testing.
- [`src/cli.rs`](file:///home/topfloorboss/Desktop/KATANA/src/cli.rs):
  - Updated argument parser supporting `katana explain` and `katana watch`.
  - Added flags: `--html [path]`, `--threshold <ms>`, `--retention <N>`, `--max-episodes <N>`.
- [`src/main.rs`](file:///home/topfloorboss/Desktop/KATANA/src/main.rs):
  - Integrated watch mode loop and HTML export dispatch for both explain and watch commands.
- [`docs/adr/0014-static-html-graph-visualization.md`](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0014-static-html-graph-visualization.md): Architecture Decision Record for static HTML visualizer.
- [`docs/adr/0015-continuous-watch-mode.md`](file:///home/topfloorboss/Desktop/KATANA/docs/adr/0015-continuous-watch-mode.md): Architecture Decision Record for continuous watch mode with bounded retention.

---

## 3. Test Matrix (33 / 33 Tests Passing)

Added [`tests/phase3_features_tests.rs`](file:///home/topfloorboss/Desktop/KATANA/tests/phase3_features_tests.rs):
- `test_html_export_validity_and_anti_emoji_lint`: Validates HTML5 doctype, SVG elements, monochrome color scheme, and verifies that **0 emojis** exist across the entire output.
- `test_html_export_multi_hop_chain_fixture`: Validates rendering against golden block I/O trace fixture (`fixtures/bio2_writeback.json`).
- `test_watch_mode_threshold_triggering`: Validates that a 10ms sleep does not trigger 50ms threshold, while a 70ms sleep reliably triggers an anomaly episode.
- `test_watch_mode_retention_capacity_bounding`: Proves that pushing 200 events into a 50-event capacity watcher never exceeds the 50-event memory limit.

```text
running 4 tests (tests/phase3_features_tests.rs)
test test_watch_mode_retention_capacity_bounding ... ok
test test_watch_mode_threshold_triggering ... ok
test test_html_export_multi_hop_chain_fixture ... ok
test test_html_export_validity_and_anti_emoji_lint ... ok
test result: ok. 4 passed; 0 failed
```
