//! Explicit compatibility adapters for legacy host observation surfaces.
//!
//! Host execution and debugger/immediate observation should prefer retained
//! `Variant` values. This module contains the deliberate projections needed by
//! older callers that still consume `RuntimeValue` snapshots or legacy slot
//! dumps.

use oxvba_com::{ComCallbackToken, ComMemberToken, ComSubscriptionToken};
use oxvba_compiler::{OxBundle, ProjectManifest};
use oxvba_runtime::{ObjectRef, Variant, compat::RuntimeValue};
use oxvba_vm::DebugStop;

use crate::debugger::{
    DebugEvaluationRequest, DebugFrameValueKind, DebugFrameVariant, DebugFrameVariantValue,
    DebugSession, DebugSessionError, DebugVariantEvaluationResult, DebugVariantPauseState,
    HostDebugVariantRunResult,
};
use crate::embedded::{
    EmbeddedInvocationTarget, EmbeddedInvokeEntryPointRequest,
    EmbeddedInvokeProcedureVariantRequest, EmbeddedInvokeStatus, EmbeddedInvokeVariantResult,
    EmbeddedRunSession, EmbeddedRunSessionError,
};
use crate::immediate::{
    ImmediateEvaluationRequest, ImmediateSessionError, ImmediateVariantEvaluationOutput,
    ImmediateVariantEvaluationResult, ImmediateVariantValueProjection,
};
use crate::{
    ComEventCallbackVariantDispatch, Engine, ImmediateSession, PhaseDiagnostic,
    ProjectRuntimeSession,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventCallbackDispatch {
    pub callback_token: ComCallbackToken,
    pub subscription_token: ComSubscriptionToken,
    pub object: ObjectRef,
    pub event: ComMemberToken,
    pub handler_symbol: String,
    pub args: Vec<RuntimeValue>,
}

/// Explicit compatibility projection for COM callback payloads.
pub trait RuntimeValueCompatComEventCallbackExt {
    fn to_runtime_dispatch(&self) -> Result<ComEventCallbackDispatch, String>;
}

impl RuntimeValueCompatComEventCallbackExt for ComEventCallbackVariantDispatch {
    fn to_runtime_dispatch(&self) -> Result<ComEventCallbackDispatch, String> {
        Ok(ComEventCallbackDispatch {
            callback_token: self.callback_token,
            subscription_token: self.subscription_token,
            object: self.object.clone(),
            event: self.event,
            handler_symbol: self.handler_symbol.clone(),
            args: self
                .args
                .iter()
                .map(Variant::to_runtime_value)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmediateValueProjection {
    pub runtime_value: RuntimeValue,
    pub display_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImmediateEvaluationOutput {
    Empty,
    Value(ImmediateValueProjection),
    PrintedLine(String),
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmediateEvaluationResult {
    pub output: ImmediateEvaluationOutput,
    pub diagnostics: Vec<PhaseDiagnostic>,
}

impl ImmediateEvaluationResult {
    pub fn empty() -> Self {
        Self {
            output: ImmediateEvaluationOutput::Empty,
            diagnostics: Vec::new(),
        }
    }
}

pub trait RuntimeValueCompatImmediateValueProjectionExt {
    fn to_runtime_value(&self) -> Result<ImmediateValueProjection, String>;
}

impl RuntimeValueCompatImmediateValueProjectionExt for ImmediateVariantValueProjection {
    fn to_runtime_value(&self) -> Result<ImmediateValueProjection, String> {
        let runtime_value = self.variant_value.to_runtime_value()?;
        Ok(ImmediateValueProjection {
            runtime_value,
            display_text: self.display_text.clone(),
        })
    }
}

pub trait RuntimeValueCompatImmediateOutputExt {
    fn to_runtime_output(&self) -> Result<ImmediateEvaluationOutput, String>;
}

impl RuntimeValueCompatImmediateOutputExt for ImmediateVariantEvaluationOutput {
    fn to_runtime_output(&self) -> Result<ImmediateEvaluationOutput, String> {
        match self {
            Self::Empty => Ok(ImmediateEvaluationOutput::Empty),
            Self::Value(value) => value
                .to_runtime_value()
                .map(ImmediateEvaluationOutput::Value),
            Self::PrintedLine(line) => Ok(ImmediateEvaluationOutput::PrintedLine(line.clone())),
            Self::Reset => Ok(ImmediateEvaluationOutput::Reset),
        }
    }
}

pub trait RuntimeValueCompatImmediateResultExt {
    fn to_runtime_result(&self) -> Result<ImmediateEvaluationResult, String>;
}

impl RuntimeValueCompatImmediateResultExt for ImmediateVariantEvaluationResult {
    fn to_runtime_result(&self) -> Result<ImmediateEvaluationResult, String> {
        Ok(ImmediateEvaluationResult {
            output: self.output.to_runtime_output()?,
            diagnostics: self.diagnostics.clone(),
        })
    }
}

pub trait RuntimeValueCompatImmediateSessionExt {
    fn snapshot(&self) -> Vec<RuntimeValue>;
    fn snapshot_compat_values(&self) -> Vec<RuntimeValue>;
    fn evaluate(
        &mut self,
        request: &ImmediateEvaluationRequest,
    ) -> Result<ImmediateEvaluationResult, ImmediateSessionError>;
}

impl RuntimeValueCompatImmediateSessionExt for ImmediateSession<'_> {
    fn snapshot(&self) -> Vec<RuntimeValue> {
        immediate_session_snapshot_values(self)
    }

    fn snapshot_compat_values(&self) -> Vec<RuntimeValue> {
        immediate_session_snapshot_values(self)
    }

    fn evaluate(
        &mut self,
        request: &ImmediateEvaluationRequest,
    ) -> Result<ImmediateEvaluationResult, ImmediateSessionError> {
        self.evaluate_variant(request)?
            .to_runtime_result()
            .map_err(|message| ImmediateSessionError::Phase(PhaseDiagnostic::runtime(message)))
    }
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
pub struct DebugEvaluationResult {
    pub value: DebugFrameValue,
}

pub trait RuntimeValueCompatDebugFrameValueExt {
    fn to_runtime_value(&self) -> Result<DebugFrameValue, String>;
}

impl RuntimeValueCompatDebugFrameValueExt for DebugFrameVariantValue {
    fn to_runtime_value(&self) -> Result<DebugFrameValue, String> {
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

pub trait RuntimeValueCompatDebugFrameExt {
    fn to_runtime_frame(&self) -> Result<DebugFrame, String>;
}

impl RuntimeValueCompatDebugFrameExt for DebugFrameVariant {
    fn to_runtime_frame(&self) -> Result<DebugFrame, String> {
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

pub trait RuntimeValueCompatDebugPauseStateExt {
    fn to_runtime_pause_state(&self) -> Result<DebugPauseState, String>;
}

impl RuntimeValueCompatDebugPauseStateExt for DebugVariantPauseState {
    fn to_runtime_pause_state(&self) -> Result<DebugPauseState, String> {
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

pub trait RuntimeValueCompatHostDebugRunResultExt {
    fn to_runtime_run_result(&self) -> Result<HostDebugRunResult, String>;
}

impl RuntimeValueCompatHostDebugRunResultExt for HostDebugVariantRunResult {
    fn to_runtime_run_result(&self) -> Result<HostDebugRunResult, String> {
        match self {
            Self::Completed => Ok(HostDebugRunResult::Completed),
            Self::Paused(state) => state
                .to_runtime_pause_state()
                .map(HostDebugRunResult::Paused),
        }
    }
}

pub trait RuntimeValueCompatDebugEvaluationResultExt {
    fn to_runtime_result(&self) -> Result<DebugEvaluationResult, String>;
}

impl RuntimeValueCompatDebugEvaluationResultExt for DebugVariantEvaluationResult {
    fn to_runtime_result(&self) -> Result<DebugEvaluationResult, String> {
        Ok(DebugEvaluationResult {
            value: self.value.to_runtime_value()?,
        })
    }
}

pub trait RuntimeValueCompatDebugSessionExt {
    fn start(&mut self) -> Result<HostDebugRunResult, DebugSessionError>;
    fn continue_execution(&mut self) -> Result<HostDebugRunResult, DebugSessionError>;
    fn step_into(&mut self) -> Result<HostDebugRunResult, DebugSessionError>;
    fn step_over(&mut self) -> Result<HostDebugRunResult, DebugSessionError>;
    fn step_out(&mut self) -> Result<HostDebugRunResult, DebugSessionError>;
    fn current_pause_state(&self) -> Result<Option<DebugPauseState>, DebugSessionError>;
    fn evaluate(
        &self,
        request: &DebugEvaluationRequest,
    ) -> Result<DebugEvaluationResult, DebugSessionError>;
}

impl RuntimeValueCompatDebugSessionExt for DebugSession<'_> {
    fn start(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.start_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
    }

    fn continue_execution(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.continue_execution_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
    }

    fn step_into(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.step_into_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
    }

    fn step_over(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.step_over_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
    }

    fn step_out(&mut self) -> Result<HostDebugRunResult, DebugSessionError> {
        self.step_out_variants()?
            .to_runtime_run_result()
            .map_err(DebugSessionError::Runtime)
    }

    fn current_pause_state(&self) -> Result<Option<DebugPauseState>, DebugSessionError> {
        self.current_variant_pause_state()?
            .map(|state| {
                state
                    .to_runtime_pause_state()
                    .map_err(DebugSessionError::Runtime)
            })
            .transpose()
    }

    fn evaluate(
        &self,
        request: &DebugEvaluationRequest,
    ) -> Result<DebugEvaluationResult, DebugSessionError> {
        self.evaluate_variant(request)?
            .to_runtime_result()
            .map_err(DebugSessionError::Runtime)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedInvokeProcedureRequest {
    pub target: crate::embedded::EmbeddedProcedureTarget,
    pub args: Vec<RuntimeValue>,
}

impl EmbeddedInvokeProcedureRequest {
    pub fn new(target: crate::embedded::EmbeddedProcedureTarget, args: Vec<RuntimeValue>) -> Self {
        Self { target, args }
    }

    pub fn to_variant_request(&self) -> Result<EmbeddedInvokeProcedureVariantRequest, String> {
        let args = self
            .args
            .iter()
            .map(RuntimeValue::to_variant)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EmbeddedInvokeProcedureVariantRequest::new(
            self.target.clone(),
            args,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedInvokeResult {
    pub target: EmbeddedInvocationTarget,
    pub status: EmbeddedInvokeStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
    pub return_value: Option<RuntimeValue>,
}

impl EmbeddedInvokeResult {
    pub fn completed(target: EmbeddedInvocationTarget, return_value: Option<RuntimeValue>) -> Self {
        Self {
            target,
            status: EmbeddedInvokeStatus::Completed,
            diagnostics: Vec::new(),
            return_value,
        }
    }

    pub fn failed(target: EmbeddedInvocationTarget, diagnostics: Vec<PhaseDiagnostic>) -> Self {
        Self {
            target,
            status: EmbeddedInvokeStatus::Failed,
            diagnostics,
            return_value: None,
        }
    }
}

pub trait RuntimeValueCompatEmbeddedProcedureVariantRequestExt {
    fn to_runtime_request(&self) -> Result<EmbeddedInvokeProcedureRequest, String>;
}

impl RuntimeValueCompatEmbeddedProcedureVariantRequestExt
    for EmbeddedInvokeProcedureVariantRequest
{
    fn to_runtime_request(&self) -> Result<EmbeddedInvokeProcedureRequest, String> {
        let args = self
            .args
            .iter()
            .map(Variant::to_runtime_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EmbeddedInvokeProcedureRequest::new(
            self.target.clone(),
            args,
        ))
    }
}

pub trait RuntimeValueCompatEmbeddedVariantResultExt {
    fn to_runtime_result(&self) -> Result<EmbeddedInvokeResult, String>;
}

impl RuntimeValueCompatEmbeddedVariantResultExt for EmbeddedInvokeVariantResult {
    fn to_runtime_result(&self) -> Result<EmbeddedInvokeResult, String> {
        let return_value = self
            .return_value
            .as_ref()
            .map(Variant::to_runtime_value)
            .transpose()?;
        Ok(EmbeddedInvokeResult {
            target: self.target.clone(),
            status: self.status,
            diagnostics: self.diagnostics.clone(),
            return_value,
        })
    }
}

pub trait RuntimeValueCompatEmbeddedRunSessionExt {
    fn invoke_entry_point(
        &mut self,
        request: &EmbeddedInvokeEntryPointRequest,
    ) -> Result<EmbeddedInvokeResult, EmbeddedRunSessionError>;

    fn invoke_procedure(
        &mut self,
        request: &EmbeddedInvokeProcedureRequest,
    ) -> Result<EmbeddedInvokeResult, EmbeddedRunSessionError>;
}

impl RuntimeValueCompatEmbeddedRunSessionExt for EmbeddedRunSession<'_> {
    fn invoke_entry_point(
        &mut self,
        request: &EmbeddedInvokeEntryPointRequest,
    ) -> Result<EmbeddedInvokeResult, EmbeddedRunSessionError> {
        self.invoke_entry_point_variant(request)?
            .to_runtime_result()
            .map_err(|message| EmbeddedRunSessionError::Phase(PhaseDiagnostic::runtime(message)))
    }

    fn invoke_procedure(
        &mut self,
        request: &EmbeddedInvokeProcedureRequest,
    ) -> Result<EmbeddedInvokeResult, EmbeddedRunSessionError> {
        let request = request
            .to_variant_request()
            .map_err(|message| EmbeddedRunSessionError::Phase(PhaseDiagnostic::runtime(message)))?;
        self.invoke_procedure_variant(&request)?
            .to_runtime_result()
            .map_err(|message| EmbeddedRunSessionError::Phase(PhaseDiagnostic::runtime(message)))
    }
}

/// Explicit compatibility extension for legacy project-session observations.
///
/// Normal host code should use the retained `Variant` inherent methods on
/// `ProjectRuntimeSession`. Import this trait only at call sites that are
/// intentionally testing or serving the temporary `RuntimeValue` compatibility
/// boundary.
pub trait RuntimeValueCompatProjectSessionExt {
    fn snapshot(&self) -> Vec<RuntimeValue>;
    fn snapshot_compat_values(&self) -> Vec<RuntimeValue>;
    fn snapshot_values(&self) -> Vec<RuntimeValue>;
    fn read_slot(&self, slot: usize) -> RuntimeValue;
    fn read_slot_value(&self, slot: usize) -> RuntimeValue;
}

impl RuntimeValueCompatProjectSessionExt for ProjectRuntimeSession {
    fn snapshot(&self) -> Vec<RuntimeValue> {
        project_session_snapshot_values(self)
    }

    fn snapshot_compat_values(&self) -> Vec<RuntimeValue> {
        project_session_snapshot_values(self)
    }

    fn snapshot_values(&self) -> Vec<RuntimeValue> {
        project_session_snapshot_values(self)
    }

    fn read_slot(&self, slot: usize) -> RuntimeValue {
        project_session_read_slot(self, slot)
    }

    fn read_slot_value(&self, slot: usize) -> RuntimeValue {
        project_session_read_slot(self, slot)
    }
}

/// Explicit compatibility extension for legacy `Engine` RuntimeValue surfaces.
///
/// Retained host execution uses the inherent `*_variant*` methods. Import this
/// trait only for temporary compatibility callers while phase-2 cleanout moves
/// public surfaces to `Variant` or explicit DTOs.
pub trait RuntimeValueCompatEngineExt {
    fn dispatch_host_event_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        project_name: &str,
        module_name: &str,
        event_name: &str,
        source_instance: ObjectRef,
        args: &[RuntimeValue],
    ) -> Result<bool, PhaseDiagnostic>;

    fn poll_com_event_callback(&self) -> Result<Option<ComEventCallbackDispatch>, PhaseDiagnostic>;

    fn invoke_procedure(
        &self,
        session: &mut ProjectRuntimeSession,
        module: &str,
        procedure: &str,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, PhaseDiagnostic>;

    fn invoke_member_on_object(
        &self,
        session: &mut ProjectRuntimeSession,
        object: ObjectRef,
        member: &str,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, PhaseDiagnostic>;

    fn poll_and_dispatch_next_com_event_callback(
        &self,
        runtime: &mut ProjectRuntimeSession,
    ) -> Result<bool, PhaseDiagnostic>;

    fn dispatch_com_event_callback_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        callback: &ComEventCallbackDispatch,
    ) -> Result<(), PhaseDiagnostic>;

    fn execute_source_with_snapshot(&self, source: &str) -> Result<Vec<RuntimeValue>, String>;

    fn execute_source_with_value_snapshot(&self, source: &str)
    -> Result<Vec<RuntimeValue>, String>;

    fn execute_source_with_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic>;

    fn execute_source_with_value_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic>;

    fn execute_project_with_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic>;

    fn execute_project_with_value_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic>;

    fn execute_bundle_with_snapshot(
        &self,
        bundle: &OxBundle,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic>;
}

impl RuntimeValueCompatEngineExt for Engine {
    fn dispatch_host_event_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        project_name: &str,
        module_name: &str,
        event_name: &str,
        source_instance: ObjectRef,
        args: &[RuntimeValue],
    ) -> Result<bool, PhaseDiagnostic> {
        let args = args
            .iter()
            .map(RuntimeValue::to_variant)
            .collect::<Result<Vec<_>, _>>()
            .map_err(PhaseDiagnostic::runtime)?;
        self.dispatch_host_event_variants_into_runtime(
            runtime,
            project_name,
            module_name,
            event_name,
            source_instance,
            &args,
        )
    }

    fn poll_com_event_callback(&self) -> Result<Option<ComEventCallbackDispatch>, PhaseDiagnostic> {
        self.poll_com_event_callback_variants()?
            .map(|callback| {
                callback
                    .to_runtime_dispatch()
                    .map_err(PhaseDiagnostic::runtime)
            })
            .transpose()
    }

    fn invoke_procedure(
        &self,
        session: &mut ProjectRuntimeSession,
        module: &str,
        procedure: &str,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, PhaseDiagnostic> {
        let variants = args
            .iter()
            .map(RuntimeValue::to_variant)
            .collect::<Result<Vec<_>, _>>()
            .map_err(PhaseDiagnostic::runtime)?;
        self.invoke_procedure_with_variants(session, module, procedure, &variants)
            .and_then(|value| value.to_runtime_value().map_err(PhaseDiagnostic::runtime))
    }

    fn invoke_member_on_object(
        &self,
        session: &mut ProjectRuntimeSession,
        object: ObjectRef,
        member: &str,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, PhaseDiagnostic> {
        let variants = args
            .iter()
            .map(RuntimeValue::to_variant)
            .collect::<Result<Vec<_>, _>>()
            .map_err(PhaseDiagnostic::runtime)?;
        self.invoke_member_on_object_with_variants(session, object, member, &variants)
            .and_then(|value| value.to_runtime_value().map_err(PhaseDiagnostic::runtime))
    }

    fn poll_and_dispatch_next_com_event_callback(
        &self,
        runtime: &mut ProjectRuntimeSession,
    ) -> Result<bool, PhaseDiagnostic> {
        let Some(callback) = self.poll_com_event_callback()? else {
            return Ok(false);
        };
        self.dispatch_com_event_callback_into_runtime(runtime, &callback)?;
        Ok(true)
    }

    fn dispatch_com_event_callback_into_runtime(
        &self,
        runtime: &mut ProjectRuntimeSession,
        callback: &ComEventCallbackDispatch,
    ) -> Result<(), PhaseDiagnostic> {
        let variant_args = callback
            .args
            .iter()
            .map(RuntimeValue::to_variant)
            .collect::<Result<Vec<_>, _>>()
            .map_err(PhaseDiagnostic::runtime)?;
        let callback = ComEventCallbackVariantDispatch {
            callback_token: callback.callback_token,
            subscription_token: callback.subscription_token,
            object: callback.object.clone(),
            event: callback.event,
            handler_symbol: callback.handler_symbol.clone(),
            args: variant_args,
        };
        self.dispatch_com_event_callback_variants_into_runtime(runtime, &callback)
    }

    fn execute_source_with_snapshot(&self, source: &str) -> Result<Vec<RuntimeValue>, String> {
        crate::compat::execute_source_with_snapshot(self, source)
    }

    fn execute_source_with_value_snapshot(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, String> {
        crate::compat::execute_source_with_snapshot(self, source)
    }

    fn execute_source_with_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        crate::compat::execute_source_with_snapshot_phased(self, source)
    }

    fn execute_source_with_value_snapshot_phased(
        &self,
        source: &str,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        crate::compat::execute_source_with_snapshot_phased(self, source)
    }

    fn execute_project_with_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        crate::compat::execute_project_with_snapshot_phased(self, manifest)
    }

    fn execute_project_with_value_snapshot_phased(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        crate::compat::execute_project_with_snapshot_phased(self, manifest)
    }

    fn execute_bundle_with_snapshot(
        &self,
        bundle: &OxBundle,
    ) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
        crate::compat::execute_bundle_with_snapshot(self, bundle)
    }
}

pub fn project_session_snapshot_values(session: &ProjectRuntimeSession) -> Vec<RuntimeValue> {
    project_variants_to_runtime_values(session.snapshot_variants())
        .expect("project runtime session VARIANT snapshot should project")
}

pub fn project_session_read_slot(session: &ProjectRuntimeSession, slot: usize) -> RuntimeValue {
    session
        .read_variant_slot(slot)
        .to_runtime_value()
        .unwrap_or(RuntimeValue::Empty)
}

pub fn immediate_session_snapshot_values(session: &ImmediateSession<'_>) -> Vec<RuntimeValue> {
    project_session_snapshot_values(session.runtime())
}

pub fn execute_source_with_snapshot(
    engine: &Engine,
    source: &str,
) -> Result<Vec<RuntimeValue>, String> {
    execute_source_with_snapshot_phased(engine, source)
        .map_err(|diagnostic| diagnostic.message().to_string())
}

pub fn execute_source_with_snapshot_phased(
    engine: &Engine,
    source: &str,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    project_variants_to_runtime_values(engine.execute_source_with_variant_snapshot_phased(source)?)
}

pub fn execute_project_with_snapshot_phased(
    engine: &Engine,
    manifest: &ProjectManifest,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    project_variants_to_runtime_values(
        engine.execute_project_with_variant_snapshot_phased(manifest)?,
    )
}

pub fn execute_bundle_with_snapshot(
    engine: &Engine,
    bundle: &OxBundle,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    project_variants_to_runtime_values(engine.execute_bundle_with_variant_snapshot(bundle)?)
}

pub fn project_variants_to_runtime_values(
    values: Vec<Variant>,
) -> Result<Vec<RuntimeValue>, PhaseDiagnostic> {
    values
        .into_iter()
        .map(|value| value.to_runtime_value().map_err(PhaseDiagnostic::runtime))
        .collect()
}
