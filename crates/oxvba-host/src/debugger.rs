use oxvba_compiler::{
    ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind, ProcedureRuntimeSlotMetadata,
    ProjectManifest,
};
use oxvba_runtime::{Variant, variant_to_vba_string};
use oxvba_vm::{DebugBreakpoint, DebugRunResult, DebugRuntimeSnapshot, DebugStop};
use thiserror::Error;

use crate::direct_host::{
    DirectHostBreakpointId, DirectHostCommandStatus, DirectHostDebugSessionId, DirectHostIssue,
    DirectHostIssueKind, DirectHostRuntimeSessionId, DirectHostStackFrameId, DirectHostWatchId,
};
use crate::engine::PhaseDiagnostic;
use crate::{Engine, ProjectRuntimeSession};

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
    /// Retained value-model frame values.
    pub values: Vec<DebugFrameVariantValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariantPauseState {
    pub stop: DebugStop,
    pub frames: Vec<DebugFrameVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostDebugVariantRunResult {
    Paused(DebugVariantPauseState),
    Completed,
}

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
}

pub struct DebugSession<'engine> {
    engine: &'engine Engine,
    debug_session_id: DirectHostDebugSessionId,
    runtime_session_id: Option<DirectHostRuntimeSessionId>,
    manifest: ProjectManifest,
    runtime: ProjectRuntimeSession,
    watch_records: Vec<DebugWatchRecord>,
    breakpoint_records: Vec<DebugBreakpointRecord>,
}

impl<'engine> DebugSession<'engine> {
    pub fn new(
        engine: &'engine Engine,
        manifest: ProjectManifest,
        runtime: ProjectRuntimeSession,
    ) -> Self {
        let debug_session_id = default_debug_session_id(&manifest, None);
        Self::new_with_ids(engine, manifest, runtime, debug_session_id, None)
    }

    pub fn new_with_ids(
        engine: &'engine Engine,
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
            runtime,
            watch_records: Vec::new(),
            breakpoint_records: Vec::new(),
        }
    }

    pub fn from_embedded_runtime_session(
        engine: &'engine Engine,
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

    pub fn engine(&self) -> &'engine Engine {
        self.engine
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

    pub fn command_status(&self) -> DebugSessionCommandStatus {
        let paused = self
            .runtime
            .vm()
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
        self.watch_records
            .iter()
            .map(|record| {
                let status = match self
                    .evaluate_variant(&DebugEvaluationRequest::new(record.expression_text.clone()))
                {
                    Ok(result) => DebugWatchEvaluationStatus::Value(result.value),
                    Err(DebugSessionError::NotPaused) => DebugWatchEvaluationStatus::Unavailable(
                        DirectHostIssue::new(DirectHostIssueKind::NotPaused)
                            .with_debug_session_id(self.debug_session_id.clone())
                            .with_watch_id(record.watch_id.clone()),
                    ),
                    Err(err) => DebugWatchEvaluationStatus::Error(
                        err.direct_host_issue()
                            .with_watch_id(record.watch_id.clone()),
                    ),
                };
                DebugWatchEvaluation {
                    watch_id: record.watch_id.clone(),
                    expression_text: record.expression_text.clone(),
                    status,
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
        let breakpoint_id = DirectHostBreakpointId::new(format!(
            "{}:breakpoint:{}",
            self.debug_session_id.as_str(),
            self.breakpoint_records.len() + 1
        ));
        let (binding_status, unresolved_reason) =
            self.bind_source_breakpoint(&module_name, line_number);
        let record = DebugBreakpointRecord {
            breakpoint_id,
            module_name,
            line_number,
            enabled: true,
            binding_status,
            unresolved_reason,
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
        self.runtime.vm_mut().debug_set_breakpoints(breakpoints);
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
        self.runtime.vm_mut().debug_set_breakpoints(breakpoints);
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

    pub fn start_variants(&mut self) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_start(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Continue execution and retain debugger frame values as `Variant`
    /// carriers.
    pub fn continue_execution_variants(
        &mut self,
    ) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_continue(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Step into and retain debugger frame values as `Variant` carriers.
    pub fn step_into_variants(&mut self) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_step_into(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Step over and retain debugger frame values as `Variant` carriers.
    pub fn step_over_variants(&mut self) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_step_over(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Step out and retain debugger frame values as `Variant` carriers.
    pub fn step_out_variants(&mut self) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_step_out(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    /// Current paused state with retained `Variant` frame values.
    pub fn current_variant_pause_state(
        &self,
    ) -> Result<Option<DebugVariantPauseState>, DebugSessionError> {
        let Some(snapshot) = self.runtime.vm().debug_snapshot() else {
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
    ) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        match result {
            DebugRunResult::Completed => Ok(HostDebugVariantRunResult::Completed),
            DebugRunResult::Paused(stop) => {
                self.record_breakpoint_hit(&stop);
                let snapshot = self
                    .runtime
                    .vm()
                    .debug_snapshot()
                    .ok_or(DebugSessionError::NotPaused)?;
                Ok(HostDebugVariantRunResult::Paused(
                    self.project_variant_pause_state(stop, &snapshot)?,
                ))
            }
        }
    }

    fn project_variant_pause_state(
        &self,
        stop: DebugStop,
        snapshot: &DebugRuntimeSnapshot,
    ) -> Result<DebugVariantPauseState, DebugSessionError> {
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
                values: metadata
                    .slots
                    .iter()
                    .map(|slot| self.project_frame_variant_value(slot))
                    .collect(),
            });
        }
        Ok(DebugVariantPauseState { stop, frames })
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

impl Engine {
    pub fn prepare_debug_session(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<DebugSession<'_>, PhaseDiagnostic> {
        let runtime = self.compile_and_prepare_session(manifest)?;
        Ok(DebugSession::new(self, manifest.clone(), runtime))
    }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
    use oxvba_runtime::VarType;
    use oxvba_vm::DebugStopReason;

    use super::{
        DebugBreakpointBindingStatus, DebugBreakpointUnresolvedReason, DebugEvaluationRequest,
        DebugFrameValueKind, DebugWatchEvaluationStatus, HostDebugVariantRunResult,
    };
    use crate::{Engine, HostConfig};

    fn make_manifest(source: &str) -> ProjectManifest {
        ProjectManifest {
            project_name: "DebugHost".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Module1", ModuleKind::Procedural, source)
                    .expect("module unit"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        }
    }

    #[test]
    fn prepare_debug_session_wraps_live_runtime_session() {
        let manifest = make_manifest("Sub Main()\nEnd Sub");
        let engine = Engine::new(HostConfig::default());
        let session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");
        assert_eq!(session.manifest().project_name, "DebugHost");
        assert!(
            session
                .runtime()
                .procedure_metadata()
                .keys()
                .any(|name| name.ends_with("_main") || name.eq_ignore_ascii_case("main"))
        );
    }

    #[test]
    fn debug_session_projects_frames_and_bounded_identifier_evaluation() {
        let manifest = make_manifest(
            "Sub Main()\n\
             Call Foo(4)\n\
             End Sub\n\
             \n\
             Sub Foo(ByVal y As Long)\n\
             Dim z As Long\n\
             z = y + 1\n\
             End Sub",
        );
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");

        let HostDebugVariantRunResult::Paused(entry_pause) =
            session.start_variants().expect("debug start should pause")
        else {
            panic!("expected entry pause");
        };
        assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
        assert_eq!(entry_pause.frames.len(), 1);
        let HostDebugVariantRunResult::Paused(callee_pause) = session
            .step_into_variants()
            .expect("step into should pause in callee")
        else {
            panic!("expected callee pause");
        };
        assert_eq!(callee_pause.stop.reason, DebugStopReason::Step);
        assert_eq!(callee_pause.frames.len(), 2);
        let current = callee_pause.frames.last().expect("current frame");
        assert!(current.procedure_name.eq_ignore_ascii_case("Foo"));
        let y = session
            .evaluate_variant(&DebugEvaluationRequest::new("y"))
            .expect("y should be visible in callee");
        assert_eq!(y.value.variant_value.as_i32(), Some(4));
        assert_eq!(y.value.kind, DebugFrameValueKind::Parameter);
        let y_slot = current
            .values
            .iter()
            .find(|value| value.name.eq_ignore_ascii_case("y"))
            .expect("y frame value")
            .slot;
        let y_variant = session.runtime().read_variant_slot(y_slot);
        assert_eq!(y_variant.vtype(), VarType::Long);
        assert_eq!(y_variant.as_i32(), Some(4));
    }

    #[test]
    fn debug_session_exposes_variant_frames_and_identifier_evaluation_before_projection() {
        let manifest = make_manifest(
            "Sub Main()\n\
             Call Foo(4)\n\
             End Sub\n\
             \n\
             Sub Foo(ByVal y As Long)\n\
             Dim z As Long\n\
             z = y + 1\n\
             End Sub",
        );
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");

        let HostDebugVariantRunResult::Paused(entry_pause) = session
            .start_variants()
            .expect("debug variant start should pause")
        else {
            panic!("expected entry pause");
        };
        assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
        let HostDebugVariantRunResult::Paused(callee_pause) = session
            .step_into_variants()
            .expect("variant step into should pause in callee")
        else {
            panic!("expected callee pause");
        };
        let current = callee_pause.frames.last().expect("current frame");
        let y = session
            .evaluate_variant(&DebugEvaluationRequest::new("y"))
            .expect("y should be visible in callee");
        assert_eq!(y.value.variant_value.as_i32(), Some(4));
        assert_eq!(y.value.kind, DebugFrameValueKind::Parameter);
        assert!(current.values.iter().any(|value| {
            value.name.eq_ignore_ascii_case("y") && value.variant_value.as_i32() == Some(4)
        }));
    }

    #[test]
    fn debug_session_watch_registry_reports_unavailable_error_and_value_states() {
        let manifest = make_manifest(
            "Sub Main()\n\
             Call Foo(4)\n\
             End Sub\n\
             \n\
             Sub Foo(ByVal y As Long)\n\
             Dim z As Long\n\
             z = y + 1\n\
             End Sub",
        );
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");
        let watch = session.add_watch("y");
        assert!(watch.watch_id.as_str().contains(":watch:1"));

        let before_start = session.evaluate_watches();
        assert_eq!(before_start.len(), 1);
        assert!(matches!(
            &before_start[0].status,
            DebugWatchEvaluationStatus::Unavailable(issue)
                if issue.stable_code == "DH-NOT-PAUSED"
        ));

        let HostDebugVariantRunResult::Paused(_) = session.start_variants().expect("entry pause")
        else {
            panic!("expected entry pause");
        };
        let entry_eval = session.evaluate_watches();
        assert!(matches!(
            &entry_eval[0].status,
            DebugWatchEvaluationStatus::Error(issue)
                if issue.stable_code == "DH-WATCH-EVALUATION-FAILED"
        ));

        let HostDebugVariantRunResult::Paused(_) =
            session.step_into_variants().expect("callee pause")
        else {
            panic!("expected callee pause");
        };
        let values = session.evaluate_watches();
        assert!(matches!(
            &values[0].status,
            DebugWatchEvaluationStatus::Value(value)
                if value.name.eq_ignore_ascii_case("y")
                    && value.variant_value.as_i32() == Some(4)
        ));

        let updated = session
            .update_watch(&watch.watch_id, "missing")
            .expect("update watch");
        assert_eq!(updated.expression_text, "missing");
        assert!(matches!(
            &session.evaluate_watches()[0].status,
            DebugWatchEvaluationStatus::Error(issue)
                if issue.stable_code == "DH-WATCH-EVALUATION-FAILED"
        ));
        let removed = session.remove_watch(&watch.watch_id).expect("remove watch");
        assert_eq!(removed.watch_id, watch.watch_id);
        assert!(session.watches().is_empty());
    }

    #[test]
    fn debug_session_breakpoint_records_bind_disable_clear_and_count_hits() {
        let manifest = make_manifest(
            "Sub Main()\n\
             Dim x As Long\n\
             x = 1\n\
             End Sub",
        );
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");

        let missing = session.set_source_breakpoint("Missing", 2);
        assert_eq!(
            missing.binding_status,
            DebugBreakpointBindingStatus::Unbound
        );
        assert_eq!(
            missing.unresolved_reason,
            Some(DebugBreakpointUnresolvedReason::NoMatchingModule)
        );
        let invalid_line = session.set_source_breakpoint("Module1", 99);
        assert_eq!(
            invalid_line.binding_status,
            DebugBreakpointBindingStatus::Unbound
        );
        assert_eq!(
            invalid_line.unresolved_reason,
            Some(DebugBreakpointUnresolvedReason::NoExecutableStatementOnLine)
        );

        let bound = session.set_source_breakpoint("Module1", 3);
        assert_eq!(bound.binding_status, DebugBreakpointBindingStatus::Bound);
        assert!(bound.unresolved_reason.is_none());
        assert!(bound.enabled);
        assert!(bound.breakpoint_id.as_str().contains(":breakpoint:3"));

        let HostDebugVariantRunResult::Paused(entry_pause) =
            session.start_variants().expect("entry pause")
        else {
            panic!("expected entry pause");
        };
        assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
        assert!(
            entry_pause.frames[0]
                .frame_id
                .as_str()
                .contains(":frame:1:")
        );
        let _ = session
            .continue_execution_variants()
            .expect("continuing with a bound breakpoint should be valid");
        let bound_after_continue = session
            .source_breakpoints()
            .iter()
            .find(|record| record.breakpoint_id == bound.breakpoint_id)
            .expect("bound breakpoint");
        assert_eq!(
            bound_after_continue.binding_status,
            DebugBreakpointBindingStatus::Bound
        );

        let disabled = session
            .set_breakpoint_enabled(&bound.breakpoint_id, false)
            .expect("disable breakpoint");
        assert!(!disabled.enabled);
        let cleared = session
            .clear_source_breakpoint(&bound.breakpoint_id)
            .expect("clear breakpoint");
        assert_eq!(cleared.breakpoint_id, bound.breakpoint_id);
    }

    #[test]
    fn debug_session_pause_state_is_absent_before_start_and_after_completion() {
        let manifest = make_manifest("Sub Main()\nEnd Sub");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");
        assert_eq!(
            session
                .current_variant_pause_state()
                .expect("pause query should succeed"),
            None
        );
        assert!(matches!(
            session
                .start_variants()
                .expect("debug start should complete"),
            HostDebugVariantRunResult::Completed
        ));
        assert_eq!(
            session
                .current_variant_pause_state()
                .expect("pause query should succeed"),
            None
        );
    }

    #[test]
    fn debug_session_rejects_non_identifier_and_unknown_name_evaluation() {
        let manifest = make_manifest("Sub Main()\nDim answer As Long\nanswer = 42\nEnd Sub");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");
        let HostDebugVariantRunResult::Paused(_) =
            session.start_variants().expect("debug start should pause")
        else {
            panic!("expected entry pause");
        };
        let unsupported = session
            .evaluate_variant(&DebugEvaluationRequest::new("answer + 1"))
            .expect_err("non-identifier expression should be rejected");
        assert!(matches!(
            unsupported,
            super::DebugSessionError::UnsupportedEvaluation { .. }
        ));
        let unknown = session
            .evaluate_variant(&DebugEvaluationRequest::new("missingValue"))
            .expect_err("unknown name should be rejected");
        assert!(matches!(
            unknown,
            super::DebugSessionError::UnknownVisibleName { .. }
        ));
    }
}
