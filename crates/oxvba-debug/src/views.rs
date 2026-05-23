use serde::{Deserialize, Serialize};

/// Editor/source location reported to debugger consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugSourceLocationView {
    pub module: String,
    pub file_line: u32,
    pub runtime_line: Option<u32>,
}

/// Transport-safe stop reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugStopReasonView {
    Entry,
    Breakpoint,
    Step,
    Completed,
}

/// Transport-safe value kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugValueKindView {
    Empty,
    Scalar,
    Object,
    Array,
    Error,
    Unknown,
}

/// Transport-safe value projection. Raw `Variant` values do not cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugValueView {
    pub name: Option<String>,
    pub display_text: String,
    pub type_label: String,
    pub kind: DebugValueKindView,
    pub raw_repr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugFrameView {
    pub id: String,
    pub name: String,
    pub location: Option<DebugSourceLocationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugPauseView {
    pub reason: DebugStopReasonView,
    pub frame_id: String,
    pub current_location: Option<DebugSourceLocationView>,
    pub frames: Vec<DebugFrameView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugBreakpointBindingStatusView {
    Bound,
    Unresolved { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugBreakpointView {
    pub id: String,
    pub module: String,
    pub file_line: u32,
    pub enabled: bool,
    pub binding_status: DebugBreakpointBindingStatusView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugWatchStatusView {
    Pending,
    Evaluated,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugWatchView {
    pub id: String,
    pub expression: String,
    pub status: DebugWatchStatusView,
    pub value: Option<DebugValueView>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugModuleView {
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugExitView {
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugRunResultView {
    Paused(DebugPauseView),
    Exited(DebugExitView),
}
