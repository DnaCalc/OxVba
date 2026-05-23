use crossbeam_channel::{Receiver, TryRecvError};
use serde::{Deserialize, Serialize};

use crate::views::{
    DebugBreakpointView, DebugModuleView, DebugSourceLocationView, DebugStopReasonView,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugOutputChannel {
    Stdout,
    Stderr,
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugBreakpointChangeKind {
    Added,
    Changed,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugEvent {
    Stopped {
        seq: u64,
        session_id: String,
        reason: DebugStopReasonView,
        thread_id: Option<u32>,
        frame_id: String,
        location: Option<DebugSourceLocationView>,
    },
    Output {
        seq: u64,
        session_id: String,
        channel: DebugOutputChannel,
        text: String,
    },
    Continued {
        seq: u64,
        session_id: String,
        all_threads_continued: bool,
    },
    Exited {
        seq: u64,
        session_id: String,
        exit_code: Option<i32>,
    },
    BreakpointChanged {
        seq: u64,
        session_id: String,
        change: DebugBreakpointChangeKind,
        breakpoint: DebugBreakpointView,
    },
    ModuleLoaded {
        seq: u64,
        session_id: String,
        module: DebugModuleView,
    },
    ThreadStarted {
        seq: u64,
        session_id: String,
        thread_id: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugEventLag {
    pub dropped: usize,
}

/// Sync event receiver wrapper.
#[derive(Debug, Clone)]
pub struct DebugEventReceiver {
    inner: Receiver<DebugEvent>,
}

impl DebugEventReceiver {
    pub(crate) fn new(inner: Receiver<DebugEvent>) -> Self {
        Self { inner }
    }

    pub fn recv(&self) -> Result<DebugEvent, crossbeam_channel::RecvError> {
        self.inner.recv()
    }

    pub fn try_recv(&self) -> Result<DebugEvent, TryRecvError> {
        self.inner.try_recv()
    }
}
