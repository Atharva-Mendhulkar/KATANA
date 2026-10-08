//! Continuous watch mode with bounded retention buffer (PRD §33, Phase 3).
//!
//! Monitors target thread off-CPU intervals and synchronization stalls continuously.
//! Maintains a circular memory-bounded event retention buffer. When an anomaly
//! exceeding `threshold_ms` is detected, it freezes the causal window and produces
//! an immediate root-cause diagnosis.

use std::collections::{HashMap, VecDeque};

use crate::events::{Event, EventKind, LossLedger};
use crate::output::{Report, TargetIdentity};
use crate::Engine;

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub target_pid: u32,
    pub tid: Option<u32>,
    pub threshold_ms: u64,
    pub retention_capacity: usize,
    pub max_episodes: Option<usize>,
    pub json: bool,
    pub html_path: Option<String>,
    pub verbose: bool,
    pub max_depth: usize,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            target_pid: 1,
            tid: None,
            threshold_ms: 50, // 50 ms latency threshold
            retention_capacity: 5000,
            max_episodes: None,
            json: false,
            html_path: None,
            verbose: false,
            max_depth: 8,
        }
    }
}

#[derive(Debug)]
pub struct Watcher {
    pub config: WatchConfig,
    pub target_identity: TargetIdentity,
    pub retention_buffer: VecDeque<Event>,
    pub thread_sleep_starts: HashMap<u32, u64>,
    pub episodes_captured: usize,
    pub engine: Engine,
}

impl Watcher {
    pub fn new(config: WatchConfig, target_identity: TargetIdentity) -> Self {
        let max_depth = config.max_depth;
        Self {
            config,
            target_identity,
            retention_buffer: VecDeque::with_capacity(1024),
            thread_sleep_starts: HashMap::new(),
            episodes_captured: 0,
            engine: Engine::new(max_depth),
        }
    }

    /// Feeds a single incoming event into the continuous watcher state machine.
    /// Returns Some(Report) if an anomaly episode exceeding threshold_ms was triggered.
    pub fn push_event(&mut self, event: Event) -> Option<Report> {
        let target_pid = self.config.target_pid;
        let monitored_tid = self.config.tid.unwrap_or(target_pid);
        let threshold_ns = self.config.threshold_ms * 1_000_000;

        // Maintain bounded circular retention
        if self.retention_buffer.len() >= self.config.retention_capacity {
            self.retention_buffer.pop_front();
        }
        self.retention_buffer.push_back(event.clone());

        let mut triggered_report: Option<Report> = None;

        match &event.kind {
            EventKind::Switch(sw) => {
                // If monitored thread switched off CPU voluntarily
                if sw.prev.tid == monitored_tid && !sw.preempted {
                    self.thread_sleep_starts.insert(sw.prev.tid, event.ts_ns);
                }

                // If monitored thread switched back on CPU
                if sw.next.tid == monitored_tid {
                    if let Some(start_ts) = self.thread_sleep_starts.remove(&sw.next.tid) {
                        let duration_ns = event.ts_ns.saturating_sub(start_ts);
                        if duration_ns >= threshold_ns {
                            triggered_report = self.trigger_episode(monitored_tid);
                        }
                    }
                }
            }
            EventKind::Waking(w) => {
                // If monitored thread was woken up after an extended sleep
                if w.wakee.tid == monitored_tid {
                    if let Some(&start_ts) = self.thread_sleep_starts.get(&w.wakee.tid) {
                        let duration_ns = event.ts_ns.saturating_sub(start_ts);
                        if duration_ns >= threshold_ns {
                            self.thread_sleep_starts.remove(&w.wakee.tid);
                            triggered_report = self.trigger_episode(monitored_tid);
                        }
                    }
                }
            }
            _ => {}
        }

        triggered_report
    }

    /// Triggers analysis over the current bounded retention window.
    fn trigger_episode(&mut self, monitored_tid: u32) -> Option<Report> {
        self.episodes_captured += 1;
        let snapshot_events: Vec<Event> = self.retention_buffer.iter().cloned().collect();
        let report = self.engine.analyze_with_identity(
            self.target_identity.clone(),
            Some(monitored_tid),
            snapshot_events,
            LossLedger::new(),
        );

        Some(report)
    }

    /// Consumes a stream of events, returning all triggered anomaly reports.
    pub fn process_event_stream(&mut self, events: Vec<Event>) -> Vec<Report> {
        let mut reports = Vec::new();
        for ev in events {
            if let Some(report) = self.push_event(ev) {
                reports.push(report);
                if let Some(max) = self.config.max_episodes {
                    if self.episodes_captured >= max {
                        break;
                    }
                }
            }
        }
        reports
    }
}
