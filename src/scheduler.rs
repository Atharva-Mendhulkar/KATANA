//! Scheduler event definitions and thread context.

use serde::{Deserialize, Serialize};

/// Per-CPU sequence and CPU reference for event provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EventRef {
    pub cpu: u16,
    pub seq: u64,
}

impl EventRef {
    pub const fn new(cpu: u16, seq: u64) -> Self {
        Self { cpu, seq }
    }
}

/// Unique thread identifier inside a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ThreadId {
    pub tid: u32,
    pub tgid: u32,
}

impl ThreadId {
    pub const fn new(tid: u32, tgid: u32) -> Self {
        Self { tid, tgid }
    }
}

/// Execution context of the waker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakerCtx {
    Task,
    Irq,
    Kthread,
    Unknown,
}

/// Normalized task scheduling state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Running,
    Sleeping,
    Preempted,
    IoWait,
}

/// sched:sched_waking event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedWaking {
    pub wakee: ThreadId,
    pub target_cpu: u16,
    pub waker_ctx: WakerCtx,
}

/// sched:sched_wakeup event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedWakeup {
    pub wakee: ThreadId,
    pub target_cpu: u16,
}

/// sched:sched_switch event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedSwitch {
    pub prev: ThreadId,
    pub next: ThreadId,
    pub prev_state: TaskState,
    pub preempted: bool,
    pub in_iowait: bool,
}
