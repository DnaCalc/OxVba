use std::{marker::PhantomData, rc::Rc, sync::Arc};

use oxvba_compiler::{
    ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind, ProcedureRuntimeSlotMetadata,
    ProjectManifest,
};
use oxvba_runtime::{Variant, variant_to_vba_string};
use oxvba_vm::{DebugBreakpoint, DebugRunResult, DebugRuntimeSnapshot, DebugStop};
use thiserror::Error;

use oxvba_host::{
    DirectHostBreakpointId, DirectHostCommandStatus, DirectHostDebugSessionId, DirectHostIssue,
    DirectHostIssueKind, DirectHostRuntimeSessionId, DirectHostSourceSpan,
    DirectHostSourceSpanStatus, DirectHostSourceUnavailableReason, DirectHostStackFrameId,
    DirectHostTextPosition, DirectHostWatchId,
};
use oxvba_host::{Engine, PhaseDiagnostic, ProjectRuntimeSession};

use crate::source_map::DebugSourceMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugFrameValueKind {
    Parameter,
    Local,
    ReturnValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFrameVariantValue {
    pub name: String,
    pub slot: usize,
    pub kind: DebugFrameValueKind,
    pub variant_value: Variant,
    pub display_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFrameVariant {
    pub frame_id: DirectHostStackFrameId,
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
    pub source: DirectHostSourceSpanStatus,
    /// Retained value-model frame values.
    pub values: Vec<DebugFrameVariantValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugPauseState {
    pub stop: DebugStop,
    pub current_source: DirectHostSourceSpanStatus,
    pub frames: Vec<DebugFrameVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugCoreRunResult {
    Paused(DebugPauseState),
    Completed,
}

pub type DebugVariantPauseState = DebugPauseState;
pub type HostDebugVariantRunResult = DebugCoreRunResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugEvaluationRequest {
    pub expression_text: String,
}

impl DebugEvaluationRequest {
    pub fn new(expression_text: impl Into<String>) -> Self {
        Self {
            expression_text: expression_text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariantEvaluationResult {
    pub value: DebugFrameVariantValue,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DebugSessionError {
    #[error(transparent)]
    Phase(PhaseDiagnostic),
    #[error("debug runtime error: {0}")]
    Runtime(String),
    #[error("debug session is not paused")]
    NotPaused,
    #[error("debug metadata missing for entry pc {entry_pc}")]
    MissingFrameMetadata { entry_pc: usize },
    #[error("bounded debug evaluation only supports current-frame identifiers; got `{expression}`")]
    UnsupportedEvaluation { expression: String },
    #[error("name `{name}` is not visible in the current frame")]
    UnknownVisibleName { name: String },
}

impl DebugSessionError {
    pub fn direct_host_issue(&self) -> DirectHostIssue {
        match self {
            DebugSessionError::Phase(diagnostic) => {
                DirectHostIssue::new(DirectHostIssueKind::DebugUnavailable)
                    .with_technical_detail(diagnostic.to_string())
            }
            DebugSessionError::Runtime(message) => {
                DirectHostIssue::new(DirectHostIssueKind::DebugUnavailable)
                    .with_technical_detail(message.clone())
            }
            DebugSessionError::NotPaused => DirectHostIssue::new(DirectHostIssueKind::NotPaused)
                .with_technical_detail(self.to_string()),
            DebugSessionError::MissingFrameMetadata { .. } => {
                DirectHostIssue::new(DirectHostIssueKind::DebugUnavailable)
                    .with_technical_detail(self.to_string())
            }
            DebugSessionError::UnsupportedEvaluation { .. }
            | DebugSessionError::UnknownVisibleName { .. } => {
                DirectHostIssue::new(DirectHostIssueKind::WatchEvaluationFailed)
                    .with_technical_detail(self.to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSessionCommandStatus {
    pub continue_execution: DirectHostCommandStatus,
    pub step_into: DirectHostCommandStatus,
    pub step_over: DirectHostCommandStatus,
    pub step_out: DirectHostCommandStatus,
    pub evaluate: DirectHostCommandStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugBreakpointBindingStatus {
    Bound,
    Unbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugBreakpointUnresolvedReason {
    NoMatchingModule,
    NoExecutableStatementOnLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugBreakpointRecord {
    pub breakpoint_id: DirectHostBreakpointId,
    pub module_name: String,
    pub line_number: usize,
    pub enabled: bool,
    pub binding_status: DebugBreakpointBindingStatus,
    pub unresolved_reason: Option<DebugBreakpointUnresolvedReason>,
    pub source: DirectHostSourceSpanStatus,
    pub hit_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugWatchRecord {
    pub watch_id: DirectHostWatchId,
    pub expression_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugWatchEvaluationStatus {
    Value(DebugFrameVariantValue),
    Unavailable(DirectHostIssue),
    Error(DirectHostIssue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugWatchEvaluation {
    pub watch_id: DirectHostWatchId,
    pub expression_text: String,
    pub status: DebugWatchEvaluationStatus,
    pub source: DirectHostSourceSpanStatus,
}

pub struct DebugSessionCore {
    engine: Arc<Engine>,
    debug_session_id: DirectHostDebugSessionId,
    runtime_session_id: Option<DirectHostRuntimeSessionId>,
    manifest: ProjectManifest,
    runtime: ProjectRuntimeSession,
    source_map: DebugSourceMap,
    watch_records: Vec<DebugWatchRecord>,
    breakpoint_records: Vec<DebugBreakpointRecord>,
    not_send_sync: PhantomData<Rc<()>>,
}

impl DebugSessionCore {
    pub fn new(
        engine: Arc<Engine>,
        manifest: ProjectManifest,
        runtime: ProjectRuntimeSession,
    ) -> Self {
        let debug_session_id = default_debug_session_id(&manifest, None);
        Self::new_with_ids(engine, manifest, runtime, debug_session_id, None)
    }

    pub fn new_with_ids(
        engine: Arc<Engine>,
        manifest: ProjectManifest,
        runtime: ProjectRuntimeSession,
        debug_session_id: impl Into<DirectHostDebugSessionId>,
        runtime_session_id: Option<DirectHostRuntimeSessionId>,
    ) -> Self {
        Self {
            engine,
            debug_session_id: debug_session_id.into(),
            runtime_session_id,
            manifest,
            source_map: DebugSourceMap::from_compiled_project(runtime.compiled()),
            runtime,
            watch_records: Vec::new(),
            breakpoint_records: Vec::new(),
            not_send_sync: PhantomData,
        }
    }

    pub fn from_embedded_runtime_session(
        engine: Arc<Engine>,
        manifest: ProjectManifest,
        runtime: ProjectRuntimeSession,
        runtime_session_id: DirectHostRuntimeSessionId,
    ) -> Self {
        let debug_session_id = default_debug_session_id(&manifest, Some(&runtime_session_id));
        Self::new_with_ids(
            engine,
            manifest,
            runtime,
            debug_session_id,
            Some(runtime_session_id),
        )
    }

    pub fn debug_session_id(&self) -> &DirectHostDebugSessionId {
        &self.debug_session_id
    }

    pub fn runtime_session_id(&self) -> Option<&DirectHostRuntimeSessionId> {
        self.runtime_session_id.as_ref()
    }

    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn runtime(&self) -> &ProjectRuntimeSession {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut ProjectRuntimeSession {
        &mut self.runtime
    }

    pub fn source_map(&self) -> &DebugSourceMap {
        &self.source_map
    }

    pub fn command_status(&self) -> DebugSessionCommandStatus {
        let paused = self
            .runtime
            .debug_vm()
            .debug_snapshot()
            .map(|snapshot| snapshot.last_pause.is_some())
            .unwrap_or(false);
        let evaluate = if paused {
            DirectHostCommandStatus::available()
        } else {
            DirectHostCommandStatus::disabled(
                DirectHostIssue::new(DirectHostIssueKind::NotPaused)
                    .with_debug_session_id(self.debug_session_id.clone()),
            )
        };
        DebugSessionCommandStatus {
            continue_execution: DirectHostCommandStatus::available(),
            step_into: DirectHostCommandStatus::available(),
            step_over: DirectHostCommandStatus::available(),
            step_out: DirectHostCommandStatus::available(),
            evaluate,
        }
    }

    pub fn add_watch(&mut self, expression_text: impl Into<String>) -> DebugWatchRecord {
        let expression_text = expression_text.into();
        let watch_id = DirectHostWatchId::new(format!(
            "{}:watch:{}",
            self.debug_session_id.as_str(),
            self.watch_records.len() + 1
        ));
        let record = DebugWatchRecord {
            watch_id,
            expression_text,
        };
        self.watch_records.push(record.clone());
        record
    }

    pub fn update_watch(
        &mut self,
        watch_id: &DirectHostWatchId,
        expression_text: impl Into<String>,
    ) -> Option<DebugWatchRecord> {
        let record = self
            .watch_records
            .iter_mut()
            .find(|record| &record.watch_id == watch_id)?;
        record.expression_text = expression_text.into();
        Some(record.clone())
    }

    pub fn remove_watch(&mut self, watch_id: &DirectHostWatchId) -> Option<DebugWatchRecord> {
        let index = self
            .watch_records
            .iter()
            .position(|record| &record.watch_id == watch_id)?;
        Some(self.watch_records.remove(index))
    }

    pub fn watches(&self) -> &[DebugWatchRecord] {
        &self.watch_records
    }

    pub fn evaluate_watches(&self) -> Vec<DebugWatchEvaluation> {
        let paused_source = self
            .current_variant_pause_state()
            .ok()
            .flatten()
            .map(|pause| pause.current_source)
            .unwrap_or_else(|| {
                DirectHostSourceSpanStatus::unavailable(
                    DirectHostSourceUnavailableReason::NoSourceLocation,
                )
            });
        self.watch_records
            .iter()
            .map(|record| {
                let (status, source) = match self
                    .evaluate_variant(&DebugEvaluationRequest::new(record.expression_text.clone()))
                {
                    Ok(result) => (
                        DebugWatchEvaluationStatus::Value(result.value),
                        paused_source.clone(),
                    ),
                    Err(DebugSessionError::NotPaused) => (
                        DebugWatchEvaluationStatus::Unavailable(
                            DirectHostIssue::new(DirectHostIssueKind::NotPaused)
                                .with_debug_session_id(self.debug_session_id.clone())
                                .with_watch_id(record.watch_id.clone()),
                        ),
                        DirectHostSourceSpanStatus::unavailable(
                            DirectHostSourceUnavailableReason::NoSourceLocation,
                        ),
                    ),
                    Err(err) => (
                        DebugWatchEvaluationStatus::Error(
                            err.direct_host_issue()
                                .with_watch_id(record.watch_id.clone()),
                        ),
                        paused_source.clone(),
                    ),
                };
                DebugWatchEvaluation {
                    watch_id: record.watch_id.clone(),
                    expression_text: record.expression_text.clone(),
                    status,
                    source,
                }
            })
            .collect()
    }

    pub fn set_source_breakpoint(
        &mut self,
        module_name: impl Into<String>,
        line_number: usize,
    ) -> DebugBreakpointRecord {
        let module_name = module_name.into();
        let runtime_line_number = self
            .source_map
            .file_to_runtime(&module_name, u32::try_from(line_number).unwrap_or(u32::MAX))
            .map(|line| line as usize)
            .unwrap_or(line_number);
        let breakpoint_id = DirectHostBreakpointId::new(format!(
            "{}:breakpoint:{}",
            self.debug_session_id.as_str(),
            self.breakpoint_records.len() + 1
        ));
        let (binding_status, unresolved_reason) =
            self.bind_source_breakpoint(&module_name, runtime_line_number);
        let record = DebugBreakpointRecord {
            breakpoint_id,
            module_name: module_name.clone(),
            line_number: runtime_line_number,
            enabled: true,
            binding_status,
            unresolved_reason,
            source: self.source_span_for_module_line(&module_name, Some(runtime_line_number)),
            hit_count: 0,
        };
        self.breakpoint_records.push(record.clone());
        self.refresh_vm_breakpoints();
        record
    }

    pub fn set_breakpoint_enabled(
        &mut self,
        breakpoint_id: &DirectHostBreakpointId,
        enabled: bool,
    ) -> Option<DebugBreakpointRecord> {
        let record = self
            .breakpoint_records
            .iter_mut()
            .find(|record| &record.breakpoint_id == breakpoint_id)?;
        record.enabled = enabled;
        let cloned = record.clone();
        self.refresh_vm_breakpoints();
        Some(cloned)
    }

    pub fn clear_source_breakpoint(
        &mut self,
        breakpoint_id: &DirectHostBreakpointId,
    ) -> Option<DebugBreakpointRecord> {
        let index = self
            .breakpoint_records
            .iter()
            .position(|record| &record.breakpoint_id == breakpoint_id)?;
        let removed = self.breakpoint_records.remove(index);
        self.refresh_vm_breakpoints();
        Some(removed)
    }

    pub fn source_breakpoints(&self) -> &[DebugBreakpointRecord] {
        &self.breakpoint_records
    }

    pub fn set_breakpoints(&mut self, breakpoints: Vec<DebugBreakpoint>) {
        self.runtime
            .debug_vm_mut()
            .debug_set_breakpoints(breakpoints);
    }

    fn bind_source_breakpoint(
        &self,
        module_name: &str,
        line_number: usize,
    ) -> (
        DebugBreakpointBindingStatus,
        Option<DebugBreakpointUnresolvedReason>,
    ) {
        let mut module_exists = false;
        for metadata in self.runtime.compiled().procedure_runtime_metadata.values() {
            if metadata.module_name.eq_ignore_ascii_case(module_name) {
                module_exists = true;
                if metadata.statement_line_numbers.contains(&line_number) {
                    return (DebugBreakpointBindingStatus::Bound, None);
                }
            }
        }

        if module_exists {
            (
                DebugBreakpointBindingStatus::Unbound,
                Some(DebugBreakpointUnresolvedReason::NoExecutableStatementOnLine),
            )
        } else {
            (
                DebugBreakpointBindingStatus::Unbound,
                Some(DebugBreakpointUnresolvedReason::NoMatchingModule),
            )
        }
    }

    fn refresh_vm_breakpoints(&mut self) {
        let breakpoints = self
            .breakpoint_records
            .iter()
            .filter(|record| {
                record.enabled && record.binding_status == DebugBreakpointBindingStatus::Bound
            })
            .map(|record| DebugBreakpoint {
                module_name: record.module_name.clone(),
                line_number: record.line_number,
            })
            .collect();
        self.runtime
            .debug_vm_mut()
            .debug_set_breakpoints(breakpoints);
    }

    fn record_breakpoint_hit(&mut self, stop: &DebugStop) {
        if stop.reason != oxvba_vm::DebugStopReason::Breakpoint {
            return;
        }
        let Some(line_number) = stop.location.line_number else {
            return;
        };
        for record in &mut self.breakpoint_records {
            if record.enabled
                && record.binding_status == DebugBreakpointBindingStatus::Bound
                && record.line_number == line_number
                && record
                    .module_name
                    .eq_ignore_ascii_case(&stop.location.module_name)
            {
                record.hit_count += 1;
            }
        }
    }

    pub fn start_variants(&mut self) -> Result<DebugCoreRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .debug_vm_mut()
            .debug_start(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Continue execution and retain debugger frame values as `Variant`
    /// carriers.
    pub fn continue_execution_variants(&mut self) -> Result<DebugCoreRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .debug_vm_mut()
            .debug_continue(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Step into and retain debugger frame values as `Variant` carriers.
    pub fn step_into_variants(&mut self) -> Result<DebugCoreRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .debug_vm_mut()
            .debug_step_into(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Step over and retain debugger frame values as `Variant` carriers.
    pub fn step_over_variants(&mut self) -> Result<DebugCoreRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .debug_vm_mut()
            .debug_step_over(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Step out and retain debugger frame values as `Variant` carriers.
    pub fn step_out_variants(&mut self) -> Result<DebugCoreRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .debug_vm_mut()
            .debug_step_out(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Current paused state with retained `Variant` frame values.
    pub fn current_variant_pause_state(
        &self,
    ) -> Result<Option<DebugPauseState>, DebugSessionError> {
        let Some(snapshot) = self.runtime.debug_vm().debug_snapshot() else {
            return Ok(None);
        };
        let Some(stop) = snapshot.last_pause.clone() else {
            return Ok(None);
        };
        Ok(Some(self.project_variant_pause_state(stop, &snapshot)?))
    }

    /// Evaluate a visible frame identifier and retain the result as a
    /// `Variant`.
    pub fn evaluate_variant(
        &self,
        request: &DebugEvaluationRequest,
    ) -> Result<DebugVariantEvaluationResult, DebugSessionError> {
        let pause = self
            .current_variant_pause_state()?
            .ok_or(DebugSessionError::NotPaused)?;
        let current_frame = pause.frames.last().ok_or(DebugSessionError::NotPaused)?;
        let expression = request
            .expression_text
            .trim()
            .trim_start_matches('?')
            .trim();
        if expression.is_empty()
            || !expression
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            return Err(DebugSessionError::UnsupportedEvaluation {
                expression: request.expression_text.clone(),
            });
        }
        let value = current_frame
            .values
            .iter()
            .find(|value| value.name.eq_ignore_ascii_case(expression))
            .cloned()
            .ok_or_else(|| DebugSessionError::UnknownVisibleName {
                name: expression.to_string(),
            })?;
        Ok(DebugVariantEvaluationResult { value })
    }

    fn project_variant_run_result(
        &mut self,
        result: DebugRunResult,
    ) -> Result<DebugCoreRunResult, DebugSessionError> {
        match result {
            DebugRunResult::Completed => Ok(DebugCoreRunResult::Completed),
            DebugRunResult::Paused(stop) => {
                self.record_breakpoint_hit(&stop);
                let snapshot = self
                    .runtime
                    .debug_vm()
                    .debug_snapshot()
                    .ok_or(DebugSessionError::NotPaused)?;
                Ok(DebugCoreRunResult::Paused(
                    self.project_variant_pause_state(stop, &snapshot)?,
                ))
            }
        }
    }

    fn project_variant_pause_state(
        &self,
        stop: DebugStop,
        snapshot: &DebugRuntimeSnapshot,
    ) -> Result<DebugPauseState, DebugSessionError> {
        let mut frames = Vec::with_capacity(snapshot.activation_entry_pcs.len());
        for (index, entry_pc) in snapshot.activation_entry_pcs.iter().enumerate() {
            let metadata = self.metadata_for_entry_pc(*entry_pc).ok_or(
                DebugSessionError::MissingFrameMetadata {
                    entry_pc: *entry_pc,
                },
            )?;
            frames.push(DebugFrameVariant {
                frame_id: DirectHostStackFrameId::new(format!(
                    "{}:frame:{}:{}",
                    self.debug_session_id.as_str(),
                    index + 1,
                    metadata.entry_pc
                )),
                module_name: metadata.module_name.clone(),
                procedure_name: metadata.procedure_name.clone(),
                entry_pc: metadata.entry_pc,
                source_line_start: metadata.source_line_start,
                source_line_end: metadata.source_line_end,
                source: self.source_span_for_module_range(
                    &metadata.module_name,
                    metadata.source_line_start,
                    metadata.source_line_end,
                ),
                values: metadata
                    .slots
                    .iter()
                    .map(|slot| self.project_frame_variant_value(slot))
                    .collect(),
            });
        }
        let current_source =
            self.source_span_for_module_line(&stop.location.module_name, stop.location.line_number);
        Ok(DebugPauseState {
            stop,
            current_source,
            frames,
        })
    }

    fn metadata_for_entry_pc(&self, entry_pc: usize) -> Option<&ProcedureRuntimeMetadata> {
        self.runtime
            .compiled()
            .procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
    }

    fn project_frame_variant_value(
        &self,
        slot: &ProcedureRuntimeSlotMetadata,
    ) -> DebugFrameVariantValue {
        // Retained debugger frame value read.
        let variant_value = self.runtime.read_variant_slot(slot.slot);
        let display_text = format_variant_for_debug(&variant_value);
        DebugFrameVariantValue {
            name: slot.name.clone(),
            slot: slot.slot,
            kind: project_slot_kind(slot.kind.clone()),
            display_text,
            variant_value,
        }
    }

    fn source_span_for_module_line(
        &self,
        module_name: &str,
        line_number: Option<usize>,
    ) -> DirectHostSourceSpanStatus {
        let Some(line_number) = line_number else {
            return DirectHostSourceSpanStatus::unavailable(
                DirectHostSourceUnavailableReason::NoSourceLocation,
            );
        };
        if line_number == 0 {
            return DirectHostSourceSpanStatus::unavailable(
                DirectHostSourceUnavailableReason::NoSourceLocation,
            );
        }
        self.source_span_for_module_range(module_name, line_number, line_number)
    }

    fn source_span_for_module_range(
        &self,
        module_name: &str,
        source_line_start: usize,
        source_line_end: usize,
    ) -> DirectHostSourceSpanStatus {
        let Some(module) = self
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name.eq_ignore_ascii_case(module_name))
        else {
            return DirectHostSourceSpanStatus::unavailable(
                DirectHostSourceUnavailableReason::NoMatchingDocument,
            );
        };
        if source_line_start == 0 || source_line_end == 0 {
            return DirectHostSourceSpanStatus::unavailable(
                DirectHostSourceUnavailableReason::NoSourceLocation,
            );
        }
        let start_line = source_line_start.min(source_line_end);
        let end_line = source_line_start.max(source_line_end);
        let mapped_start = self
            .source_map
            .runtime_to_file(module_name, u32::try_from(start_line).unwrap_or(u32::MAX))
            .unwrap_or_else(|| u32::try_from(start_line).unwrap_or(u32::MAX));
        let mapped_end = self
            .source_map
            .runtime_to_file(module_name, u32::try_from(end_line).unwrap_or(u32::MAX))
            .unwrap_or_else(|| u32::try_from(end_line).unwrap_or(u32::MAX));
        let end_exclusive = mapped_end.saturating_add(1);
        let (start_line, end_line) = (mapped_start, end_exclusive);
        if start_line == u32::MAX || end_line == u32::MAX {
            return DirectHostSourceSpanStatus::unavailable(
                DirectHostSourceUnavailableReason::NoSourceLocation,
            );
        };
        DirectHostSourceSpanStatus::known(DirectHostSourceSpan::new(
            module.module_name.clone(),
            DirectHostTextPosition::new(start_line, 0),
            DirectHostTextPosition::new(end_line, 0),
        ))
    }
}

fn default_debug_session_id(
    manifest: &ProjectManifest,
    runtime_session_id: Option<&DirectHostRuntimeSessionId>,
) -> DirectHostDebugSessionId {
    let suffix = runtime_session_id
        .map(|id| id.as_str().to_string())
        .unwrap_or_else(|| manifest.project_name.clone());
    DirectHostDebugSessionId::new(format!("debug:{suffix}"))
}

fn project_slot_kind(kind: ProcedureRuntimeSlotKind) -> DebugFrameValueKind {
    match kind {
        ProcedureRuntimeSlotKind::Parameter => DebugFrameValueKind::Parameter,
        ProcedureRuntimeSlotKind::Local => DebugFrameValueKind::Local,
        ProcedureRuntimeSlotKind::ReturnValue => DebugFrameValueKind::ReturnValue,
    }
}

fn format_variant_for_debug(value: &Variant) -> String {
    match variant_to_vba_string(value) {
        Ok(text) => text.into_string(),
        Err(_) => format!("{value:?}"),
    }
}
