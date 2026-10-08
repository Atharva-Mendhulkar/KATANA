//! Static HTML causal graph visualizer export (PRD §33, Phase 3).
//!
//! Generates a standalone, zero-dependency, single-file HTML document.
//! Design specification:
//! - Complete black and white only (monochrome palette: pure blacks, whites, grays, hairline borders).
//! - Strictly zero emojis.
//! - Modern sleek Swiss/minimalist typography.
//! - Self-contained inline SVG vector causal graph and CSS (no network dependencies, 100% offline).

use crate::diagnosis::{Completeness, DiagStatus, FindingKind};
use crate::evidence::{Evidence, EvidenceClass};
use crate::output::Report;

/// Renders a full self-contained static HTML report from a Katana Report.
pub fn render_html_report(report: &Report) -> String {
    let mut out = String::with_capacity(16 * 1024);

    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"UTF-8\">\n");
    out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    out.push_str("  <title>KATANA Report - PID ");
    out.push_str(&report.target.pid.to_string());
    out.push_str("</title>\n");
    out.push_str("  <style>\n");
    out.push_str(CSS_STYLES);
    out.push_str("  </style>\n");
    out.push_str("</head>\n<body>\n");

    out.push_str("  <div class=\"container\">\n");

    // Header
    out.push_str("    <header class=\"header\">\n");
    out.push_str("      <div class=\"brand-row\">\n");
    out.push_str("        <span class=\"brand-title\">KATANA</span>\n");
    out.push_str("        <span class=\"brand-sub\">LATENCY ROOT-CAUSE ANALYSIS</span>\n");
    out.push_str("      </div>\n");
    out.push_str("      <div class=\"meta-tag\">SCHEMA v1.0 &bull; OFFLINE REPORT</div>\n");
    out.push_str("    </header>\n\n");

    // Primary Diagnosis Banner
    render_diagnosis_card(&mut out, report);

    // Target & Observation Window Grid
    render_target_grid(&mut out, report);

    // Causal Graph Visualizer (Vector SVG)
    render_causal_graph_svg(&mut out, report);

    // Evidence Ledger
    render_evidence_table(&mut out, &report.evidence);

    // Ingestion & Loss Statistics
    render_stats_card(&mut out, report);

    // Footer
    out.push_str("    <footer class=\"footer\">\n");
    out.push_str("      <div>KATANA LATENCY DIAGNOSTIC ENGINE &bull; DETERMINISTIC RECONSTRUCTION</div>\n");
    out.push_str("      <div>NO HEURISTICS &bull; NO ML INFERENCE &bull; STRICT KERNEL CAUSALITY</div>\n");
    out.push_str("    </footer>\n");

    out.push_str("  </div>\n");
    out.push_str("</body>\n</html>\n");

    out
}

/// Exports the HTML report to a local filesystem path.
pub fn export_html_file(report: &Report, path: &str) -> std::io::Result<()> {
    let html = render_html_report(report);
    std::fs::write(path, html)
}

fn render_diagnosis_card(out: &mut String, report: &Report) {
    let diag = &report.diagnosis;
    let status_str = match diag.status {
        DiagStatus::Found => "DIAGNOSIS FOUND",
        DiagStatus::NotBlocked => "TARGET NOT BLOCKED",
        DiagStatus::Ambiguous => "AMBIGUOUS / CONTRADICTION",
        DiagStatus::Unknown => "UNATTRIBUTED / UNKNOWN",
        DiagStatus::Invalid => "INVALID RUN",
    };

    let completeness_str = match diag.completeness {
        Completeness::Complete => "COMPLETE",
        Completeness::Partial => "PARTIAL",
        Completeness::Lossy => "LOSSY",
        Completeness::Invalid => "INVALID",
    };

    out.push_str("    <section class=\"card diagnosis-card\">\n");
    out.push_str("      <div class=\"card-header-row\">\n");
    out.push_str("        <div class=\"card-title-group\">\n");
    out.push_str("          <span class=\"status-pill\">");
    out.push_str(status_str);
    out.push_str("</span>\n");
    out.push_str("          <span class=\"completeness-tag\">COMPLETENESS: ");
    out.push_str(completeness_str);
    out.push_str("</span>\n");
    out.push_str("        </div>\n");
    out.push_str("      </div>\n");

    if let Some(primary) = &diag.primary {
        let kind_str = format!("{:?}", primary.kind);
        let dur_ms = primary.blocked_duration_ns as f64 / 1_000_000.0;
        let frac_pct = primary.explained_fraction_per_mille as f64 / 10.0;

        out.push_str("      <div class=\"primary-finding-row\">\n");
        out.push_str("        <div class=\"finding-kind\">");
        out.push_str(&kind_str);
        out.push_str("</div>\n");
        out.push_str("        <div class=\"finding-desc\">");
        escape_html_into(out, &primary.details);
        out.push_str("</div>\n");
        out.push_str("      </div>\n");

        out.push_str("      <div class=\"metrics-grid\">\n");
        out.push_str("        <div class=\"metric-box\">\n");
        out.push_str("          <div class=\"metric-label\">BLOCKED DURATION</div>\n");
        out.push_str("          <div class=\"metric-val\">");
        out.push_str(&format!("{:.2} ms", dur_ms));
        out.push_str("</div>\n");
        out.push_str("        </div>\n");

        out.push_str("        <div class=\"metric-box\">\n");
        out.push_str("          <div class=\"metric-label\">EXPLAINED FRACTION</div>\n");
        out.push_str("          <div class=\"metric-val\">");
        out.push_str(&format!("{:.1}%", frac_pct));
        out.push_str("</div>\n");
        out.push_str("        </div>\n");

        out.push_str("        <div class=\"metric-box\">\n");
        out.push_str("          <div class=\"metric-label\">WEAKEST STRENGTH</div>\n");
        out.push_str("          <div class=\"metric-val\">");
        out.push_str(&format!("{:?}", primary.weakest_strength));
        out.push_str("</div>\n");
        out.push_str("        </div>\n");

        out.push_str("        <div class=\"metric-box\">\n");
        out.push_str("          <div class=\"metric-label\">CHAIN DEPTH</div>\n");
        out.push_str("          <div class=\"metric-val\">");
        out.push_str(&format!("{} HOPS", primary.hop_count));
        out.push_str("</div>\n");
        out.push_str("        </div>\n");
        out.push_str("      </div>\n");
    } else {
        out.push_str("      <div class=\"primary-finding-row\">\n");
        out.push_str("        <div class=\"finding-desc\">No blocking anomalies detected for target thread.</div>\n");
        out.push_str("      </div>\n");
    }

    out.push_str("    </section>\n\n");
}

fn render_target_grid(out: &mut String, report: &Report) {
    let t = &report.target;
    let w = &report.window;
    let dur_ms = w.duration_ns as f64 / 1_000_000.0;

    out.push_str("    <section class=\"card meta-grid\">\n");
    out.push_str("      <div class=\"meta-cell\">\n");
    out.push_str("        <span class=\"cell-label\">TARGET PID</span>\n");
    out.push_str("        <span class=\"cell-val font-mono\">");
    out.push_str(&t.pid.to_string());
    out.push_str("</span>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"meta-cell\">\n");
    out.push_str("        <span class=\"cell-label\">PROCESS COMM</span>\n");
    out.push_str("        <span class=\"cell-val font-mono\">");
    escape_html_into(out, &t.comm);
    out.push_str("</span>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"meta-cell\">\n");
    out.push_str("        <span class=\"cell-label\">THREAD GROUP TGID</span>\n");
    out.push_str("        <span class=\"cell-val font-mono\">");
    out.push_str(&t.tgid.to_string());
    out.push_str("</span>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"meta-cell\">\n");
    out.push_str("        <span class=\"cell-label\">WINDOW DURATION</span>\n");
    out.push_str("        <span class=\"cell-val font-mono\">");
    out.push_str(&format!("{:.2} ms", dur_ms));
    out.push_str("</span>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"meta-cell\">\n");
    out.push_str("        <span class=\"cell-label\">BOOT ID</span>\n");
    out.push_str("        <span class=\"cell-val font-mono small\">");
    escape_html_into(out, &t.boot_id);
    out.push_str("</span>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"meta-cell\">\n");
    out.push_str("        <span class=\"cell-label\">START TICKS</span>\n");
    out.push_str("        <span class=\"cell-val font-mono small\">");
    out.push_str(&t.start_time_ticks.to_string());
    out.push_str("</span>\n");
    out.push_str("      </div>\n");
    out.push_str("    </section>\n\n");
}

fn render_causal_graph_svg(out: &mut String, report: &Report) {
    out.push_str("    <section class=\"card graph-card\">\n");
    out.push_str("      <div class=\"section-title\">CAUSAL GRAPH TOPOLOGY</div>\n");
    out.push_str("      <div class=\"graph-wrapper\">\n");

    let primary = report.diagnosis.primary.as_ref();
    let has_chain = primary.and_then(|p| p.chain.as_ref());

    if let Some(chain) = has_chain {
        let hop_count = chain.hops.len();
        let total_nodes = hop_count + 2; // Wakee (subject), intermediate wakers, terminal
        let node_w = 170;
        let node_h = 76;
        let gap_x = 110;
        let start_x = 30;
        let start_y = 45;

        let svg_w = start_x * 2 + (total_nodes * node_w) + ((total_nodes - 1) * gap_x);
        let svg_h = 160;

        out.push_str(&format!(
            "        <svg viewBox=\"0 0 {} {}\" class=\"graph-svg\" xmlns=\"http://www.w3.org/2000/svg\">\n",
            svg_w, svg_h
        ));
        out.push_str("          <defs>\n");
        out.push_str("            <marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\n");
        out.push_str("              <path d=\"M 0 1 L 10 5 L 0 9 z\" fill=\"#ffffff\"/>\n");
        out.push_str("            </marker>\n");
        out.push_str("          </defs>\n");

        let mut curr_x = start_x;

        // Node 0: Target thread
        let subject_tid = primary.map_or(report.target.pid, |p| p.subject.tid);
        render_svg_node(out, curr_x, start_y, node_w, node_h, "TARGET THREAD", &format!("TID {}", subject_tid), "BLOCKED (WAITER)", true);

        for (idx, hop) in chain.hops.iter().enumerate() {
            let next_x = curr_x + node_w + gap_x;
            let arrow_start_x = curr_x + node_w;
            let arrow_end_x = next_x;
            let mid_y = start_y + (node_h / 2);

            let edge_label = format!("{:?}", hop.edge_rule);
            render_svg_edge(out, arrow_start_x, mid_y, arrow_end_x, mid_y, &edge_label);

            let is_last = idx == chain.hops.len() - 1;
            let sub_text = if is_last { "INTERMEDIATE / TERMINAL WAKER" } else { "INTERMEDIATE WAKER" };
            render_svg_node(out, next_x, start_y, node_w, node_h, &format!("HOP {} WAKER", hop.hop), &format!("TID {}", hop.waker.tid), sub_text, false);

            curr_x = next_x;
        }

        // Terminal Reason Node
        let terminal_x = curr_x + node_w + gap_x;
        let mid_y = start_y + (node_h / 2);
        render_svg_edge(out, curr_x + node_w, mid_y, terminal_x, mid_y, "TERMINAL");

        let term_title = format!("{:?}", chain.terminal_reason);
        render_svg_node(out, terminal_x, start_y, node_w, node_h, "TERMINAL REASON", &term_title, "STATUS UNRESOLVED / ROOT", false);

        out.push_str("        </svg>\n");
    } else if let Some(p) = primary {
        // Block I/O or other direct finding
        let svg_w = 640;
        let svg_h = 160;
        out.push_str(&format!(
            "        <svg viewBox=\"0 0 {} {}\" class=\"graph-svg\" xmlns=\"http://www.w3.org/2000/svg\">\n",
            svg_w, svg_h
        ));
        out.push_str("          <defs>\n");
        out.push_str("            <marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\n");
        out.push_str("              <path d=\"M 0 1 L 10 5 L 0 9 z\" fill=\"#ffffff\"/>\n");
        out.push_str("            </marker>\n");
        out.push_str("          </defs>\n");

        render_svg_node(out, 40, 45, 180, 76, "TARGET THREAD", &format!("TID {}", p.subject.tid), "BLOCKED SUBJECT", true);

        let edge_label = match p.kind {
            FindingKind::BlockIoWait => "RULE BIO-1",
            FindingKind::BlockIoCorrelated => "RULE BIO-2",
            FindingKind::SchedRunqDelay => "RULE CR-1",
            _ => "RELATION",
        };
        render_svg_edge(out, 220, 83, 380, 83, edge_label);

        let target_label = match p.kind {
            FindingKind::BlockIoWait | FindingKind::BlockIoCorrelated => "BLOCK DEVICE",
            FindingKind::SchedRunqDelay => "CPU RUNQUEUE",
            FindingKind::NotBlocked => "RUNNING / IDLE",
            _ => "UNATTRIBUTED",
        };
        render_svg_node(out, 380, 45, 200, 76, target_label, &format!("{:?}", p.kind), "EVIDENCE COMPLETE", false);

        out.push_str("        </svg>\n");
    } else {
        out.push_str("        <div class=\"empty-graph-msg\">NO CAUSAL GRAPH EDGES GENERATED (TARGET WAS NOT BLOCKED)</div>\n");
    }

    out.push_str("      </div>\n");
    out.push_str("    </section>\n\n");
}

fn render_svg_node(
    out: &mut String,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    badge: &str,
    primary_txt: &str,
    sub_txt: &str,
    is_subject: bool,
) {
    let stroke = if is_subject { "#ffffff" } else { "#555555" };
    let bg = if is_subject { "#161616" } else { "#101010" };

    out.push_str(&format!(
        "          <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
        x, y, w, h, bg, stroke
    ));
    out.push_str(&format!(
        "          <text x=\"{}\" y=\"{}\" fill=\"#888888\" font-size=\"9\" font-family=\"monospace\" letter-spacing=\"1\">{}</text>\n",
        x + 12, y + 20, badge
    ));
    out.push_str(&format!(
        "          <text x=\"{}\" y=\"{}\" fill=\"#ffffff\" font-size=\"13\" font-family=\"sans-serif\" font-weight=\"600\">{}</text>\n",
        x + 12, y + 42, primary_txt
    ));
    out.push_str(&format!(
        "          <text x=\"{}\" y=\"{}\" fill=\"#777777\" font-size=\"9\" font-family=\"monospace\">{}</text>\n",
        x + 12, y + 60, sub_txt
    ));
}

fn render_svg_edge(out: &mut String, x1: usize, y1: usize, x2: usize, y2: usize, label: &str) {
    out.push_str(&format!(
        "          <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#ffffff\" stroke-width=\"1.5\" marker-end=\"url(#arrow)\"/>\n",
        x1, y1, x2, y2
    ));
    let mid_x = (x1 + x2) / 2;
    out.push_str(&format!(
        "          <text x=\"{}\" y=\"{}\" fill=\"#aaaaaa\" font-size=\"9\" font-family=\"monospace\" text-anchor=\"middle\">{}</text>\n",
        mid_x, y1 - 8, label
    ));
}

fn render_evidence_table(out: &mut String, evidence: &[Evidence]) {
    out.push_str("    <section class=\"card table-card\">\n");
    out.push_str("      <div class=\"section-title\">EVIDENCE LEDGER &bull; CAUSAL VS. CORRELATED</div>\n");
    out.push_str("      <div class=\"table-responsive\">\n");
    out.push_str("        <table class=\"evidence-table\">\n");
    out.push_str("          <thead>\n");
    out.push_str("            <tr>\n");
    out.push_str("              <th style=\"width: 10%;\">ID</th>\n");
    out.push_str("              <th style=\"width: 12%;\">RULE</th>\n");
    out.push_str("              <th style=\"width: 14%;\">CLASS</th>\n");
    out.push_str("              <th style=\"width: 14%;\">STRENGTH</th>\n");
    out.push_str("              <th style=\"width: 50%;\">VERIFIED STATEMENT / DISCLAIMER</th>\n");
    out.push_str("            </tr>\n");
    out.push_str("          </thead>\n");
    out.push_str("          <tbody>\n");

    if evidence.is_empty() {
        out.push_str("            <tr><td colspan=\"5\" class=\"text-muted\">No evidence records captured.</td></tr>\n");
    } else {
        for ev in evidence {
            let class_str = match ev.class {
                EvidenceClass::Causal => "CAUSAL",
                EvidenceClass::Correlated => "CORRELATED",
                EvidenceClass::Observed => "OBSERVED",
            };
            let strength_str = format!("{:?}", ev.strength).to_uppercase();

            out.push_str("            <tr>\n");
            out.push_str("              <td class=\"font-mono\">");
            escape_html_into(out, &ev.id);
            out.push_str("</td>\n");
            out.push_str("              <td class=\"font-mono\">");
            out.push_str(&format!("{:?}", ev.rule));
            out.push_str("</td>\n");
            out.push_str("              <td><span class=\"badge-class\">");
            out.push_str(class_str);
            out.push_str("</span></td>\n");
            out.push_str("              <td class=\"font-mono small\">");
            out.push_str(&strength_str);
            out.push_str("</td>\n");
            out.push_str("              <td class=\"statement-cell\">");
            escape_html_into(out, &ev.description);
            out.push_str("</td>\n");
            out.push_str("            </tr>\n");
        }
    }

    out.push_str("          </tbody>\n");
    out.push_str("        </table>\n");
    out.push_str("      </div>\n");
    out.push_str("    </section>\n\n");
}

fn render_stats_card(out: &mut String, report: &Report) {
    let s = &report.stats;
    out.push_str("    <section class=\"card stats-grid\">\n");
    out.push_str("      <div class=\"stat-box\">\n");
    out.push_str("        <div class=\"stat-label\">EVENTS INGESTED</div>\n");
    out.push_str("        <div class=\"stat-val font-mono\">");
    out.push_str(&s.events_received.to_string());
    out.push_str("</div>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"stat-box\">\n");
    out.push_str("        <div class=\"stat-label\">RING BUFFER LOSS</div>\n");
    out.push_str("        <div class=\"stat-val font-mono\">");
    out.push_str(&s.events_lost.to_string());
    out.push_str("</div>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"stat-box\">\n");
    out.push_str("        <div class=\"stat-label\">RESERVE FAILURES</div>\n");
    out.push_str("        <div class=\"stat-val font-mono\">");
    out.push_str(&s.reserve_fail_total.to_string());
    out.push_str("</div>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"stat-box\">\n");
    out.push_str("        <div class=\"stat-label\">TRACKED MAP FULL</div>\n");
    out.push_str("        <div class=\"stat-val font-mono\">");
    out.push_str(&s.tracked_full.to_string());
    out.push_str("</div>\n");
    out.push_str("      </div>\n");
    out.push_str("    </section>\n\n");
}

fn escape_html_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
}

const CSS_STYLES: &str = r#"
* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  background-color: #050505;
  color: #efefef;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "SF Pro Display", Roboto, "Helvetica Neue", Arial, sans-serif;
  font-size: 13px;
  line-height: 1.6;
  -webkit-font-smoothing: antialiased;
}

.container {
  max-width: 1080px;
  margin: 0 auto;
  padding: 40px 24px 80px 24px;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  border-bottom: 1px solid #222222;
  padding-bottom: 20px;
  margin-bottom: 32px;
}

.brand-row {
  display: flex;
  align-items: baseline;
  gap: 16px;
}

.brand-title {
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 24px;
  font-weight: 800;
  letter-spacing: 2px;
  color: #ffffff;
}

.brand-sub {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 1.5px;
  color: #777777;
}

.meta-tag {
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 10px;
  color: #555555;
  letter-spacing: 1px;
}

.card {
  background-color: #0c0c0c;
  border: 1px solid #1e1e1e;
  border-radius: 4px;
  padding: 24px;
  margin-bottom: 24px;
}

.card-header-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.status-pill {
  display: inline-block;
  background-color: #ffffff;
  color: #000000;
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 1px;
  padding: 4px 10px;
  border-radius: 2px;
}

.completeness-tag {
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 11px;
  color: #888888;
  margin-left: 12px;
  letter-spacing: 0.5px;
}

.primary-finding-row {
  margin-bottom: 24px;
}

.finding-kind {
  font-size: 20px;
  font-weight: 700;
  color: #ffffff;
  margin-bottom: 8px;
}

.finding-desc {
  font-size: 14px;
  color: #bbbbbb;
  max-width: 860px;
}

.metrics-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  border-top: 1px solid #1a1a1a;
  padding-top: 20px;
}

.metric-box {
  background-color: #121212;
  border: 1px solid #1a1a1a;
  padding: 14px 16px;
  border-radius: 3px;
}

.metric-label {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1px;
  color: #666666;
  margin-bottom: 6px;
}

.metric-val {
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 16px;
  font-weight: 700;
  color: #ffffff;
}

.meta-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
  background-color: #080808;
}

.meta-cell {
  padding: 8px 0;
}

.cell-label {
  display: block;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1px;
  color: #666666;
  margin-bottom: 4px;
}

.cell-val {
  font-size: 13px;
  color: #e0e0e0;
}

.font-mono {
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
}

.small {
  font-size: 11px;
}

.section-title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 1.5px;
  color: #777777;
  margin-bottom: 20px;
  border-bottom: 1px solid #1a1a1a;
  padding-bottom: 10px;
}

.graph-wrapper {
  overflow-x: auto;
  padding: 10px 0;
}

.graph-svg {
  min-width: 100%;
  height: auto;
  display: block;
}

.empty-graph-msg {
  color: #666666;
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 12px;
  padding: 24px 0;
  text-align: center;
}

.table-responsive {
  overflow-x: auto;
}

.evidence-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.evidence-table th {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 1px;
  color: #666666;
  border-bottom: 1px solid #222222;
  padding: 10px 12px;
}

.evidence-table td {
  padding: 12px 12px;
  border-bottom: 1px solid #161616;
  font-size: 12px;
  color: #d0d0d0;
  vertical-align: top;
}

.evidence-table tbody tr:hover {
  background-color: #121212;
}

.badge-class {
  display: inline-block;
  border: 1px solid #333333;
  padding: 2px 6px;
  border-radius: 2px;
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  font-size: 10px;
  font-weight: 600;
  color: #ffffff;
}

.statement-cell {
  line-height: 1.5;
  color: #cccccc;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  background-color: #080808;
}

.stat-box {
  padding: 12px 16px;
  border-left: 2px solid #222222;
}

.stat-label {
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 1px;
  color: #555555;
  margin-bottom: 4px;
}

.stat-val {
  font-size: 16px;
  font-weight: 700;
  color: #ffffff;
}

.footer {
  margin-top: 48px;
  padding-top: 24px;
  border-top: 1px solid #1a1a1a;
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: #444444;
  font-family: ui-monospace, "SFMono-Regular", "SF Mono", Menlo, Consolas, monospace;
  letter-spacing: 0.5px;
}
"#;
