//! Text explanation renderer enforcing the structural no-inflation rule (PRD §15).

use crate::diagnosis::{Completeness, DiagStatus, Diagnosis, FindingKind};
use crate::evidence::{EvidenceClass, EvidenceStrength};
use crate::graph::TerminalReason;

pub const MANDATORY_CORRELATION_SUFFIX: &str = "This trace does not establish a causal link.";
pub const NON_PI_OWNER_UNKNOWN: &str = "owner: unknown (non-PI futex)";

/// Allowed verb / phrase generator strictly adhering to PRD §15.1.
pub fn render_relation_verb(class: EvidenceClass, is_futex: bool) -> &'static str {
    match class {
        EvidenceClass::Causal => {
            if is_futex {
                "was released by a futex wake from"
            } else {
                "was woken by"
            }
        }
        EvidenceClass::Observed => "was blocked for",
        EvidenceClass::Correlated => "coincided with",
    }
}

pub fn render_diagnosis(diag: &Diagnosis, verbose: bool) -> String {
    let mut out = String::new();

    // Event loss or invalid warning header
    if diag.completeness == Completeness::Lossy || diag.completeness == Completeness::Invalid {
        out.push_str("[WARNING: Event loss detected in this window. Completeness is LOSSY/INVALID]\n");
    }

    match diag.status {
        DiagStatus::Invalid => {
            out.push_str("Diagnosis: INVALID\n");
            out.push_str("Collection invalidated (identity mismatch or clock discontinuity). No primary finding emitted.\n");
            return out;
        }
        DiagStatus::Ambiguous => {
            out.push_str("Diagnosis: AMBIGUOUS\n");
            out.push_str("Contradictory evidence detected in trace. Conflicting explanations retained without arbitrary tie-breaking:\n");
            for alt in &diag.alternatives {
                out.push_str(&format!("  - Candidate: {:?}: {}\n", alt.kind, alt.details));
            }
            return out;
        }
        DiagStatus::NotBlocked => {
            out.push_str("Diagnosis: NOT_BLOCKED\n");
            if let Some(pri) = &diag.primary {
                out.push_str(&format!(
                    "TID {} was not blocked ≥ MIN_BLOCK_NS during the observation window. {}\n",
                    pri.subject.tid, pri.details
                ));
            } else {
                out.push_str("Subject thread was not blocked in this observation window.\n");
            }
        }
        DiagStatus::Unknown => {
            out.push_str("Diagnosis: UNKNOWN\n");
            out.push_str("No kernel-recorded cause for subject's blocking was observed in this window. This does not mean there is no cause.\n");
        }
        DiagStatus::Found => {
            if let Some(pri) = &diag.primary {
                out.push_str(&format!("Diagnosis: {:?}\n", pri.kind));

                // Prefix for weak strength
                if pri.weakest_strength == EvidenceStrength::Weak {
                    out.push_str("Partially supported: ");
                }

                match pri.kind {
                    FindingKind::FutexWakeChain => {
                        out.push_str(&format!(
                            "Thread {} was blocked in futex wait for {} ms. ",
                            pri.subject.tid,
                            pri.blocked_duration_ns / 1_000_000
                        ));
                        if let Some(chain) = &pri.chain {
                            if let Some(first_hop) = chain.hops.first() {
                                out.push_str(&format!(
                                    "It was released by a futex wake from TID {}. ",
                                    first_hop.waker.tid
                                ));
                            }
                            if chain.hops.len() > 1 {
                                out.push_str("Wake chain: ");
                                for (i, hop) in chain.hops.iter().enumerate() {
                                    if i > 0 {
                                        out.push_str(" <- ");
                                    }
                                    out.push_str(&format!("TID {}", hop.waker.tid));
                                }
                                out.push_str(". ");
                            }
                            match chain.terminal_reason {
                                TerminalReason::WakerRunning => {
                                    out.push_str("Terminal waker was running on CPU before wake.\n");
                                }
                                TerminalReason::HistoryBeforeTracking => {
                                    out.push_str("Waker was outside tracked set prior to wake (history unobserved).\n");
                                }
                                TerminalReason::Cycle => {
                                    out.push_str("Observed wake cycle within window.\n");
                                }
                                TerminalReason::DepthLimit => {
                                    out.push_str("Chain truncated at depth limit 8.\n");
                                }
                                _ => out.push('\n'),
                            }
                        } else {
                            out.push('\n');
                        }
                        out.push_str(&format!("Lock status: {}\n", NON_PI_OWNER_UNKNOWN));
                    }
                    FindingKind::SchedRunqDelay => {
                        out.push_str(&format!(
                            "Thread {} experienced scheduler runqueue delay of {} ms. {}\n",
                            pri.subject.tid,
                            pri.blocked_duration_ns / 1_000_000,
                            pri.details
                        ));
                    }
                    FindingKind::FutexTimeout => {
                        out.push_str(&format!(
                            "Thread {}'s futex wait ended by timeout; no waker is implied.\n",
                            pri.subject.tid
                        ));
                    }
                    FindingKind::FutexInterrupted => {
                        out.push_str(&format!(
                            "Thread {}'s futex wait ended by signal interruption; no waker is implied.\n",
                            pri.subject.tid
                        ));
                    }
                    FindingKind::FutexWaitUnresolved => {
                        out.push_str(&format!(
                            "Thread {} was in futex wait for the whole window; no wake was observed. Katana cannot determine who, if anyone, will wake it. {}\n",
                            pri.subject.tid, NON_PI_OWNER_UNKNOWN
                        ));
                    }
                    FindingKind::BlockIoWait => {
                        out.push_str(&format!(
                            "Thread {} blocked on device {}. Block duration: {} ms.\n",
                            pri.subject.tid,
                            pri.details,
                            pri.blocked_duration_ns / 1_000_000
                        ));
                    }
                    FindingKind::BlockIoCorrelated => {
                        out.push_str(&format!(
                            "Thread {} experienced blocking during an interval in which device {} exhibited activity or elevated latency. {}\n",
                            pri.subject.tid,
                            pri.details,
                            MANDATORY_CORRELATION_SUFFIX
                        ));
                    }
                    FindingKind::BlockedUnattributed => {
                        out.push_str(&format!(
                            "Thread {} was blocked in a state Katana cannot attribute in this version ({}). Katana makes no claim about the cause.\n",
                            pri.subject.tid,
                            pri.details
                        ));
                    }
                    _ => {
                        out.push_str(&format!("Thread {}: {}\n", pri.subject.tid, pri.details));
                    }
                }
            }
        }
    }

    // Context / Correlated evidence section
    if !diag.context.is_empty() {
        out.push_str("\nCorrelated Context (non-causal):\n");
        for ctx in &diag.context {
            out.push_str(&format!(
                "  - {} {}. {}\n",
                ctx.rule.name(),
                ctx.description,
                MANDATORY_CORRELATION_SUFFIX
            ));
        }
    }

    // Limitations section
    if !diag.limitations.is_empty() {
        out.push_str("\nLimitations:\n");
        for lim in &diag.limitations {
            out.push_str(&format!("  - {:?}\n", lim));
        }
    }

    if verbose {
        out.push_str(&format!("\nCompleteness: {:?}\n", diag.completeness));
        if let Some(pri) = &diag.primary {
            if !pri.evidence_ids.is_empty() {
                out.push_str(&format!("Primary Evidence IDs: {}\n", pri.evidence_ids.join(", ")));
            }
        }
        if !diag.context.is_empty() {
            out.push_str("Evidence Provenance Details:\n");
            for ctx in &diag.context {
                let prov_str = if ctx.provenance.is_empty() {
                    "none".to_string()
                } else {
                    ctx.provenance
                        .iter()
                        .map(|p| format!("cpu:{} seq:{}", p.cpu, p.seq))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                out.push_str(&format!(
                    "  - [{} / {:?}]: {} (provenance: [{}])\n",
                    ctx.id, ctx.rule, ctx.description, prov_str
                ));
            }
        }
    }

    out
}
