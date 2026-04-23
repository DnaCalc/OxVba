use oxvba_compiler::{
    ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind, ProcedureRuntimeSlotMetadata,
    ProjectManifest,
};
use oxvba_runtime::{RuntimeValue, Variant, runtime_value_to_vba_string};
use oxvba_vm::{DebugBreakpoint, DebugRunResult, DebugRuntimeSnapshot, DebugStop};
use thiserror::Error;

use crate::engine::PhaseDiagnostic;
use crate::{Engine, ProjectRuntimeSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugFrameValueKind {
    Parameter,
    Local,
    ReturnValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFrameValue {
    pub name: String,
    pub slot: usize,
    pub kind: DebugFrameValueKind,
    pub runtime_value: RuntimeValue,
    pub display_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFrameVariantValue {
    pub name: String,
    pub slot: usize,
    pub kind: DebugFrameValueKind,
    pub variant_value: Variant,
    pub display_text: String,
}

impl DebugFrameVariantValue {
    pub fn to_runtime_value(&self) -> Result<DebugFrameValue, String> {
        let runtime_value = self.variant_value.to_runtime_value()?;
        Ok(DebugFrameValue {
            name: self.name.clone(),
            slot: self.slot,
            kind: self.kind,
            display_text: self.display_text.clone(),
            runtime_value,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFrame {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
    pub values: Vec<DebugFrameValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugPauseState {
    pub stop: DebugStop,
    pub frames: Vec<DebugFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostDebugRunResult {
    Paused(DebugPauseState),
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
pub struct DebugEvaluationResult {
    pub value: DebugFrameValue,
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

pub struct DebugSession<'engine> {
    engine: &'engine Engine,
    manifest: ProjectManifest,
    runtime: ProjectRuntimeSession,
}

impl<'engine> DebugSession<'engine> {
    pub fn new(
        engine: &'engine Engine,
        manifest: ProjectManifest,
        runtime: ProjectRuntimeSession,
    ) -> Self {
        Self {
            engine,
            manifest,
            runtime,
        }
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

    pub fn set_breakpoints(&mut self, breakpoints: Vec<DebugBreakpoint>) {
        self.runtime.vm_mut().debug_set_breakpoints(breakpoints);
    }

    pub fn start(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_start(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_run_result(result)
    }

    pub fn continue_execution(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_continue(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_run_result(result)
    }

    pub fn step_into(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_step_into(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_run_result(result)
    }

    pub fn step_over(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_step_over(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_run_result(result)
    }

    pub fn step_out(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_step_out(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_run_result(result)
    }

    pub fn current_pause_state(&self) -> Result<Option<DebugPauseState>, DebugSessionError> {
        let Some(snapshot) = self.runtime.vm().debug_snapshot() else {
            return Ok(None);
        };
        let Some(stop) = snapshot.last_pause.clone() else {
            return Ok(None);
        };
        Ok(Some(self.project_pause_state(stop, &snapshot)?))
    }

    pub fn evaluate(
        &self,
        request: &DebugEvaluationRequest,
    ) -> Result<DebugEvaluationResult, DebugSessionError> {
        let pause = self
            .current_pause_state()?
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
        Ok(DebugEvaluationResult { value })
    }

    fn project_run_result(
        &self,
        result: DebugRunResult,
    ) -> Result<HostDebugRunResult, DebugSessionError> {
        match result {
            DebugRunResult::Completed => Ok(HostDebugRunResult::Completed),
            DebugRunResult::Paused(stop) => {
                let snapshot = self
                    .runtime
                    .vm()
                    .debug_snapshot()
                    .ok_or(DebugSessionError::NotPaused)?;
                Ok(HostDebugRunResult::Paused(
                    self.project_pause_state(stop, &snapshot)?,
                ))
            }
        }
    }

    fn project_pause_state(
        &self,
        stop: DebugStop,
        snapshot: &DebugRuntimeSnapshot,
    ) -> Result<DebugPauseState, DebugSessionError> {
        let mut frames = Vec::with_capacity(snapshot.activation_entry_pcs.len());
        for entry_pc in &snapshot.activation_entry_pcs {
            let metadata = self.metadata_for_entry_pc(*entry_pc).ok_or(
                DebugSessionError::MissingFrameMetadata {
                    entry_pc: *entry_pc,
                },
            )?;
            frames.push(DebugFrame {
                module_name: metadata.module_name.clone(),
                procedure_name: metadata.procedure_name.clone(),
                entry_pc: metadata.entry_pc,
                source_line_start: metadata.source_line_start,
                source_line_end: metadata.source_line_end,
                values: metadata
                    .slots
                    .iter()
                    .map(|slot| self.project_frame_value(slot))
                    .collect(),
            });
        }
        Ok(DebugPauseState { stop, frames })
    }

    fn metadata_for_entry_pc(&self, entry_pc: usize) -> Option<&ProcedureRuntimeMetadata> {
        self.runtime
            .compiled()
            .procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
    }

    fn project_frame_value(&self, slot: &ProcedureRuntimeSlotMetadata) -> DebugFrameValue {
        self.project_frame_variant_value(slot)
            .to_runtime_value()
            .unwrap_or_else(|_| DebugFrameValue {
                name: slot.name.clone(),
                slot: slot.slot,
                kind: project_slot_kind(slot.kind.clone()),
                runtime_value: RuntimeValue::Empty,
                display_text: String::new(),
            })
    }

    fn project_frame_variant_value(
        &self,
        slot: &ProcedureRuntimeSlotMetadata,
    ) -> DebugFrameVariantValue {
        let variant_value = self.runtime.read_variant_slot(slot.slot);
        let display_text = variant_value
            .to_runtime_value()
            .map(|runtime_value| format_runtime_value_for_debug(&runtime_value))
            .unwrap_or_else(|_| format!("{variant_value:?}"));
        DebugFrameVariantValue {
            name: slot.name.clone(),
            slot: slot.slot,
            kind: project_slot_kind(slot.kind.clone()),
            display_text,
            variant_value,
        }
    }
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

fn format_runtime_value_for_debug(value: &RuntimeValue) -> String {
    match runtime_value_to_vba_string(value) {
        Ok(RuntimeValue::String(text)) => text.into_string(),
        Ok(other) => format!("{other:?}"),
        Err(_) => format!("{value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
    use oxvba_runtime::{RuntimeValue, VarType};
    use oxvba_vm::DebugStopReason;

    use super::{DebugEvaluationRequest, DebugFrameValueKind, HostDebugRunResult};
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

        let HostDebugRunResult::Paused(entry_pause) =
            session.start().expect("debug start should pause")
        else {
            panic!("expected entry pause");
        };
        assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
        assert_eq!(entry_pause.frames.len(), 1);
        let HostDebugRunResult::Paused(callee_pause) = session
            .step_into()
            .expect("step into should pause in callee")
        else {
            panic!("expected callee pause");
        };
        assert_eq!(callee_pause.stop.reason, DebugStopReason::Step);
        assert_eq!(callee_pause.frames.len(), 2);
        let current = callee_pause.frames.last().expect("current frame");
        assert!(current.procedure_name.eq_ignore_ascii_case("Foo"));
        let y = session
            .evaluate(&DebugEvaluationRequest::new("y"))
            .expect("y should be visible in callee");
        assert_eq!(y.value.runtime_value, RuntimeValue::I32(4));
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
    fn debug_session_pause_state_is_absent_before_start_and_after_completion() {
        let manifest = make_manifest("Sub Main()\nEnd Sub");
        let engine = Engine::new(HostConfig::default());
        let mut session = engine
            .prepare_debug_session(&manifest)
            .expect("debug session should prepare");
        assert_eq!(
            session
                .current_pause_state()
                .expect("pause query should succeed"),
            None
        );
        assert!(matches!(
            session.start().expect("debug start should complete"),
            HostDebugRunResult::Completed
        ));
        assert_eq!(
            session
                .current_pause_state()
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
        let HostDebugRunResult::Paused(_) = session.start().expect("debug start should pause")
        else {
            panic!("expected entry pause");
        };
        let unsupported = session
            .evaluate(&DebugEvaluationRequest::new("answer + 1"))
            .expect_err("non-identifier expression should be rejected");
        assert!(matches!(
            unsupported,
            super::DebugSessionError::UnsupportedEvaluation { .. }
        ));
        let unknown = session
            .evaluate(&DebugEvaluationRequest::new("missingValue"))
            .expect_err("unknown name should be rejected");
        assert!(matches!(
            unknown,
            super::DebugSessionError::UnknownVisibleName { .. }
        ));
    }
}
