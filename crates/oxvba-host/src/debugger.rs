use oxvba_compiler::{
    ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind, ProcedureRuntimeSlotMetadata,
    ProjectManifest,
};
use oxvba_runtime::{RuntimeValue, Variant, variant_to_vba_string};
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
    /// Compatibility projection for debugger clients that still consume
    /// semantic values. Retained frame reads start from `DebugFrameVariantValue`.
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
    /// Project a retained frame value into the legacy debugger value shape.
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
    /// Compatibility frame values projected from retained `Variant` slot reads.
    pub values: Vec<DebugFrameValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFrameVariant {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
    /// Retained value-model frame values.
    pub values: Vec<DebugFrameVariantValue>,
}

impl DebugFrameVariant {
    /// Project retained frame values into the legacy debugger frame shape.
    pub fn to_runtime_frame(&self) -> Result<DebugFrame, String> {
        let values = self
            .values
            .iter()
            .map(DebugFrameVariantValue::to_runtime_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DebugFrame {
            module_name: self.module_name.clone(),
            procedure_name: self.procedure_name.clone(),
            entry_pc: self.entry_pc,
            source_line_start: self.source_line_start,
            source_line_end: self.source_line_end,
            values,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugPauseState {
    pub stop: DebugStop,
    pub frames: Vec<DebugFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariantPauseState {
    pub stop: DebugStop,
    pub frames: Vec<DebugFrameVariant>,
}

impl DebugVariantPauseState {
    /// Project retained debugger pause state into the legacy debugger shape.
    pub fn to_runtime_pause_state(&self) -> Result<DebugPauseState, String> {
        let frames = self
            .frames
            .iter()
            .map(DebugFrameVariant::to_runtime_frame)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DebugPauseState {
            stop: self.stop.clone(),
            frames,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostDebugRunResult {
    Paused(DebugPauseState),
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostDebugVariantRunResult {
    Paused(DebugVariantPauseState),
    Completed,
}

impl HostDebugVariantRunResult {
    /// Project retained debugger run state into the legacy debugger shape.
    pub fn to_runtime_run_result(&self) -> Result<HostDebugRunResult, String> {
        match self {
            Self::Completed => Ok(HostDebugRunResult::Completed),
            Self::Paused(state) => state
                .to_runtime_pause_state()
                .map(HostDebugRunResult::Paused),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariantEvaluationResult {
    pub value: DebugFrameVariantValue,
}

impl DebugVariantEvaluationResult {
    /// Project a retained debugger evaluation into the legacy debugger shape.
    pub fn to_runtime_result(&self) -> Result<DebugEvaluationResult, String> {
        Ok(DebugEvaluationResult {
            value: self.value.to_runtime_value()?,
        })
    }
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
        self.start_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
    }

    /// Start execution and retain debugger frame values as `Variant` carriers.
    pub fn start_variants(&mut self) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        let bytecode = self.runtime.compiled().bytecode.clone();
        let result = self
            .runtime
            .vm_mut()
            .debug_start(&bytecode)
            .map_err(DebugSessionError::Runtime)?;
        self.project_variant_run_result(result)
    }

    pub fn continue_execution(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.continue_execution_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
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

    pub fn step_into(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.step_into_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
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

    pub fn step_over(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.step_over_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
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

    pub fn step_out(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.step_out_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
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

    pub fn current_pause_state(&self) -> Result<Option<DebugPauseState>, DebugSessionError> {
        self.current_variant_pause_state()?
            .map(|state| {
                state
                    .to_runtime_pause_state()
                    .map_err(DebugSessionError::Runtime)
            })
            .transpose()
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

    pub fn evaluate(
        &self,
        request: &DebugEvaluationRequest,
    ) -> Result<DebugEvaluationResult, DebugSessionError> {
        self.evaluate_variant(request)?
            .to_runtime_result()
            .map_err(DebugSessionError::Runtime)
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
        &self,
        result: DebugRunResult,
    ) -> Result<HostDebugVariantRunResult, DebugSessionError> {
        match result {
            DebugRunResult::Completed => Ok(HostDebugVariantRunResult::Completed),
            DebugRunResult::Paused(stop) => {
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
        for entry_pc in &snapshot.activation_entry_pcs {
            let metadata = self.metadata_for_entry_pc(*entry_pc).ok_or(
                DebugSessionError::MissingFrameMetadata {
                    entry_pc: *entry_pc,
                },
            )?;
            frames.push(DebugFrameVariant {
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
    use oxvba_runtime::{RuntimeValue, VarType};
    use oxvba_vm::DebugStopReason;

    use super::{
        DebugEvaluationRequest, DebugFrameValueKind, HostDebugRunResult, HostDebugVariantRunResult,
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
