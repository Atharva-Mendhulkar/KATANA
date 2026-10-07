//! Futex syscall operation decoding, key derivation, and wait/wake interval tracking.

use serde::{Deserialize, Serialize};

use crate::scheduler::{EventRef, ThreadId};

pub const FUTEX_WAIT: u32 = 0;
pub const FUTEX_WAKE: u32 = 1;
pub const FUTEX_REQUEUE: u32 = 3;
pub const FUTEX_CMP_REQUEUE: u32 = 4;
pub const FUTEX_WAKE_OP: u32 = 5;
pub const FUTEX_LOCK_PI: u32 = 6;
pub const FUTEX_UNLOCK_PI: u32 = 7;
pub const FUTEX_WAIT_BITSET: u32 = 9;
pub const FUTEX_WAKE_BITSET: u32 = 10;
pub const FUTEX_WAIT_REQUEUE_PI: u32 = 11;
pub const FUTEX_CMP_REQUEUE_PI: u32 = 12;

pub const FUTEX_PRIVATE_FLAG: u32 = 128;
pub const FUTEX_CLOCK_REALTIME: u32 = 256;
pub const FUTEX_CMD_MASK: u32 = !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME);
pub const FUTEX_BITSET_MATCH_ANY: u32 = 0xffff_ffff;

pub const FUTEX_TID_MASK: u32 = 0x3fff_ffff;
pub const FUTEX_OWNER_DIED: u32 = 0x4000_0000;
pub const FUTEX_WAITERS: u32 = 0x8000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutexCmd {
    Wait,
    Wake,
    WaitBitset,
    WakeBitset,
    LockPi,
    UnlockPi,
    Requeue,
    CmpRequeue,
    WakeOp,
    WaitRequeuePi,
    CmpRequeuePi,
    Unknown(u32),
}

impl FutexCmd {
    pub fn from_raw_op(op: u32) -> (Self, bool) {
        let private = (op & FUTEX_PRIVATE_FLAG) != 0;
        let cmd = match op & FUTEX_CMD_MASK {
            FUTEX_WAIT => FutexCmd::Wait,
            FUTEX_WAKE => FutexCmd::Wake,
            FUTEX_WAIT_BITSET => FutexCmd::WaitBitset,
            FUTEX_WAKE_BITSET => FutexCmd::WakeBitset,
            FUTEX_LOCK_PI => FutexCmd::LockPi,
            FUTEX_UNLOCK_PI => FutexCmd::UnlockPi,
            FUTEX_REQUEUE => FutexCmd::Requeue,
            FUTEX_CMP_REQUEUE => FutexCmd::CmpRequeue,
            FUTEX_WAKE_OP => FutexCmd::WakeOp,
            FUTEX_WAIT_REQUEUE_PI => FutexCmd::WaitRequeuePi,
            FUTEX_CMP_REQUEUE_PI => FutexCmd::CmpRequeuePi,
            other => FutexCmd::Unknown(other),
        };
        (cmd, private)
    }

    pub fn is_wake_family(&self) -> bool {
        matches!(
            self,
            FutexCmd::Wake
                | FutexCmd::WakeBitset
                | FutexCmd::UnlockPi
                | FutexCmd::WakeOp
                | FutexCmd::Requeue
                | FutexCmd::CmpRequeue
        )
    }

    pub fn is_wait_family(&self) -> bool {
        matches!(
            self,
            FutexCmd::Wait
                | FutexCmd::WaitBitset
                | FutexCmd::LockPi
                | FutexCmd::WaitRequeuePi
                | FutexCmd::CmpRequeuePi
        )
    }

    pub fn is_pi(&self) -> bool {
        matches!(
            self,
            FutexCmd::LockPi
                | FutexCmd::UnlockPi
                | FutexCmd::WaitRequeuePi
                | FutexCmd::CmpRequeuePi
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutexScope {
    Private,
    SharedOrUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FutexKey {
    pub scope: FutexScope,
    pub tgid: u32,
    pub uaddr: u64,
}

impl FutexKey {
    pub fn new(private: bool, tgid: u32, uaddr: u64) -> Self {
        Self {
            scope: if private {
                FutexScope::Private
            } else {
                FutexScope::SharedOrUnknown
            },
            tgid,
            uaddr,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FutexEnter {
    pub uaddr: u64,
    pub uaddr2: u64,
    pub cmd: FutexCmd,
    pub private: bool,
    pub val: u32,
    pub val3: u32,
    pub has_timeout: bool,
    pub pi_word: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FutexExit {
    pub ret: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaitOutcome {
    Woken,
    NoSleepValueMismatch, // -EAGAIN (-11)
    Timeout,              // -ETIMEDOUT (-110)
    Interrupted,          // -EINTR (-4)
    InterruptedMayRestart,// -ERESTARTSYS / -ERESTART_RESTARTBLOCK
    EndedByExit,
    Error(i64),
}

impl WaitOutcome {
    pub fn from_ret(ret: i64) -> Self {
        match ret {
            0 => WaitOutcome::Woken,
            -11 => WaitOutcome::NoSleepValueMismatch,
            -110 => WaitOutcome::Timeout,
            -4 => WaitOutcome::Interrupted,
            -512 | -513 | -514 | -516 => WaitOutcome::InterruptedMayRestart,
            other if other < 0 => WaitOutcome::Error(other),
            _ => WaitOutcome::Woken,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitInterval {
    pub thread: ThreadId,
    pub uaddr: u64,
    pub key: FutexKey,
    pub bitset: u32,
    pub enter_ts: u64,
    pub enter_ref: EventRef,
    pub exit_ts: Option<u64>,
    pub exit_ref: Option<EventRef>,
    pub exit_ret: Option<i64>,
    pub outcome: Option<WaitOutcome>,
    pub pi_owner: Option<u32>,
}

impl WaitInterval {
    pub fn new(
        thread: ThreadId,
        uaddr: u64,
        key: FutexKey,
        bitset: u32,
        enter_ts: u64,
        enter_ref: EventRef,
        pi_word: Option<u32>,
    ) -> Self {
        let pi_owner = pi_word.and_then(|w| {
            let tid = w & FUTEX_TID_MASK;
            if tid != 0 {
                Some(tid)
            } else {
                None
            }
        });
        Self {
            thread,
            uaddr,
            key,
            bitset,
            enter_ts,
            enter_ref,
            exit_ts: None,
            exit_ref: None,
            exit_ret: None,
            outcome: None,
            pi_owner,
        }
    }

    pub fn is_open_at(&self, ts: u64) -> bool {
        self.enter_ts <= ts && self.exit_ts.map_or(true, |exit_ts| ts <= exit_ts)
    }

    pub fn close(&mut self, exit_ts: u64, exit_ref: EventRef, ret: i64) {
        self.exit_ts = Some(exit_ts);
        self.exit_ref = Some(exit_ref);
        self.exit_ret = Some(ret);
        self.outcome = Some(WaitOutcome::from_ret(ret));
    }

    pub fn close_by_exit(&mut self, exit_ts: u64, exit_ref: EventRef) {
        self.exit_ts = Some(exit_ts);
        self.exit_ref = Some(exit_ref);
        self.outcome = Some(WaitOutcome::EndedByExit);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeInvocation {
    pub thread: ThreadId,
    pub uaddr: u64,
    pub key: FutexKey,
    pub cmd: FutexCmd,
    pub bitset: u32,
    pub val: u32,
    pub enter_ts: u64,
    pub enter_ref: EventRef,
    pub exit_ts: Option<u64>,
    pub exit_ref: Option<EventRef>,
    pub ret: Option<i64>,
}

impl WakeInvocation {
    pub fn new(
        thread: ThreadId,
        uaddr: u64,
        key: FutexKey,
        cmd: FutexCmd,
        bitset: u32,
        val: u32,
        enter_ts: u64,
        enter_ref: EventRef,
    ) -> Self {
        Self {
            thread,
            uaddr,
            key,
            cmd,
            bitset,
            val,
            enter_ts,
            enter_ref,
            exit_ts: None,
            exit_ref: None,
            ret: None,
        }
    }

    pub fn close(&mut self, exit_ts: u64, exit_ref: EventRef, ret: i64) {
        self.exit_ts = Some(exit_ts);
        self.exit_ref = Some(exit_ref);
        self.ret = Some(ret);
    }
}
