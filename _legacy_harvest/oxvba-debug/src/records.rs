use oxvba_host::{DirectHostBreakpointId, DirectHostWatchId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugBreakpointBindingStatus {
    Bound {
        runtime_line: u32,
    },
    Unresolved {
        reason: DebugUnresolvedBreakpointReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugUnresolvedBreakpointReason {
    NonExecutableLine,
    UnknownModule,
    NoNearestExecutableLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugBreakpointRecord {
    pub id: DirectHostBreakpointId,
    pub module: String,
    pub file_line: u32,
    pub enabled: bool,
    pub binding_status: DebugBreakpointBindingStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugWatchStatus {
    Pending,
    Evaluated,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugWatchRecord {
    pub id: DirectHostWatchId,
    pub expression: String,
    pub status: DebugWatchStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugEvaluationRequest {
    pub frame_id: Option<String>,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugSessionCommandStatus {
    Accepted,
    Rejected,
    Completed,
}
