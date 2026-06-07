use serde::{Deserialize, Serialize};

use crate::core::{
    DebugBreakpointBindingStatus, DebugBreakpointRecord, DebugCoreRunResult, DebugFrameValueKind,
    DebugFrameVariant, DebugFrameVariantValue, DebugPauseState, DebugWatchEvaluation,
    DebugWatchEvaluationStatus,
};

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

pub fn value_view_from_core(value: &DebugFrameVariantValue) -> DebugValueView {
    DebugValueView {
        name: Some(value.name.clone()),
        display_text: value.display_text.clone(),
        type_label: format!("{:?}", value.variant_value.vtype()),
        kind: value_kind_from_core(value),
        raw_repr: Some(hex_bytes(&value.variant_value.to_wire_bytes())),
    }
}

pub fn frame_view_from_core(frame: &DebugFrameVariant) -> DebugFrameView {
    DebugFrameView {
        id: frame.frame_id.as_str().to_string(),
        name: format!("{}::{}", frame.module_name, frame.procedure_name),
        location: source_location_from_status(&frame.source),
    }
}

pub fn pause_view_from_core(pause: &DebugPauseState) -> DebugPauseView {
    let current_frame = pause.frames.last();
    DebugPauseView {
        reason: match pause.stop.reason {
            oxvba_vm::DebugStopReason::Entry => DebugStopReasonView::Entry,
            oxvba_vm::DebugStopReason::Breakpoint => DebugStopReasonView::Breakpoint,
            oxvba_vm::DebugStopReason::Step => DebugStopReasonView::Step,
        },
        frame_id: current_frame
            .map(|frame| frame.frame_id.as_str().to_string())
            .unwrap_or_default(),
        current_location: source_location_from_status(&pause.current_source),
        frames: pause.frames.iter().map(frame_view_from_core).collect(),
    }
}

pub fn breakpoint_view_from_core(record: &DebugBreakpointRecord) -> DebugBreakpointView {
    DebugBreakpointView {
        id: record.breakpoint_id.as_str().to_string(),
        module: record.module_name.clone(),
        file_line: record
            .source
            .source_span()
            .map(|span| span.start.line)
            .unwrap_or_else(|| u32::try_from(record.line_number).unwrap_or(u32::MAX)),
        enabled: record.enabled,
        binding_status: match record.binding_status {
            DebugBreakpointBindingStatus::Bound => DebugBreakpointBindingStatusView::Bound,
            DebugBreakpointBindingStatus::Unbound => DebugBreakpointBindingStatusView::Unresolved {
                reason: record
                    .unresolved_reason
                    .map(|reason| format!("{reason:?}"))
                    .unwrap_or_else(|| "Unbound".to_string()),
            },
        },
    }
}

pub fn watch_view_from_core(evaluation: &DebugWatchEvaluation) -> DebugWatchView {
    match &evaluation.status {
        DebugWatchEvaluationStatus::Value(value) => DebugWatchView {
            id: evaluation.watch_id.as_str().to_string(),
            expression: evaluation.expression_text.clone(),
            status: DebugWatchStatusView::Evaluated,
            value: Some(value_view_from_core(value)),
            error: None,
        },
        DebugWatchEvaluationStatus::Unavailable(issue) => DebugWatchView {
            id: evaluation.watch_id.as_str().to_string(),
            expression: evaluation.expression_text.clone(),
            status: DebugWatchStatusView::Pending,
            value: None,
            error: Some(issue.stable_code.to_string()),
        },
        DebugWatchEvaluationStatus::Error(issue) => DebugWatchView {
            id: evaluation.watch_id.as_str().to_string(),
            expression: evaluation.expression_text.clone(),
            status: DebugWatchStatusView::Error,
            value: None,
            error: Some(issue.stable_code.to_string()),
        },
    }
}

pub fn run_result_view_from_core(result: &DebugCoreRunResult) -> DebugRunResultView {
    match result {
        DebugCoreRunResult::Paused(pause) => {
            DebugRunResultView::Paused(pause_view_from_core(pause))
        }
        DebugCoreRunResult::Completed => {
            DebugRunResultView::Exited(DebugExitView { exit_code: None })
        }
    }
}

fn value_kind_from_core(value: &DebugFrameVariantValue) -> DebugValueKindView {
    match value.kind {
        DebugFrameValueKind::Parameter
        | DebugFrameValueKind::Local
        | DebugFrameValueKind::ReturnValue => {}
    }
    match value.variant_value.vtype() {
        oxvba_runtime::VarType::Empty | oxvba_runtime::VarType::Null => DebugValueKindView::Empty,
        oxvba_runtime::VarType::Object => DebugValueKindView::Object,
        oxvba_runtime::VarType::ArrayVariant => DebugValueKindView::Array,
        oxvba_runtime::VarType::Error => DebugValueKindView::Error,
        _ => DebugValueKindView::Scalar,
    }
}

fn source_location_from_status(
    status: &oxvba_host::DirectHostSourceSpanStatus,
) -> Option<DebugSourceLocationView> {
    let span = status.source_span()?;
    Some(DebugSourceLocationView {
        module: span.document_id.as_str().to_string(),
        file_line: span.start.line,
        runtime_line: Some(span.start.line),
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
