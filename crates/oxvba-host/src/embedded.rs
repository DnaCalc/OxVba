use std::path::{Path, PathBuf};

use oxvba_compiler::{ProjectManifest, compile_project};
use oxvba_runtime::Variant;
use thiserror::Error;

use crate::Engine;
use crate::direct_host::{
    DirectHostBuildRequestId, DirectHostCommandStatus, DirectHostIssue, DirectHostIssueKind,
    DirectHostRetryability, DirectHostRunRequestId, DirectHostRuntimeSessionId,
};
use crate::engine::PhaseDiagnostic;

/// Explicit source-of-truth selection for direct embedded build/run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedExecutionSourcePolicy {
    DiskOnly,
    WorkspaceOverlay,
}

/// Canonical workspace target plus the explicit source policy to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedWorkspaceInput {
    pub workspace_target: PathBuf,
    pub source_policy: EmbeddedExecutionSourcePolicy,
}

impl EmbeddedWorkspaceInput {
    pub fn new(path: impl Into<PathBuf>, source_policy: EmbeddedExecutionSourcePolicy) -> Self {
        Self {
            workspace_target: path.into(),
            source_policy,
        }
    }

    pub fn disk_only(path: impl Into<PathBuf>) -> Self {
        Self::new(path, EmbeddedExecutionSourcePolicy::DiskOnly)
    }

    pub fn workspace_overlay(path: impl Into<PathBuf>) -> Self {
        Self::new(path, EmbeddedExecutionSourcePolicy::WorkspaceOverlay)
    }

    pub fn path(&self) -> &Path {
        &self.workspace_target
    }
}

/// Canonical manifest snapshot prepared for embedded build/run consumption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedWorkspaceSnapshot {
    pub workspace: EmbeddedWorkspaceInput,
    pub manifest: ProjectManifest,
}

impl EmbeddedWorkspaceSnapshot {
    pub fn new(workspace: EmbeddedWorkspaceInput, manifest: ProjectManifest) -> Self {
        Self {
            workspace,
            manifest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildRequest {
    pub request_id: DirectHostBuildRequestId,
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub target: EmbeddedBuildTarget,
}

impl EmbeddedBuildRequest {
    pub fn new(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self::with_build_target(workspace, EmbeddedBuildTarget::Bundle)
    }

    pub fn wrapped_com_server(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self::with_build_target(workspace, EmbeddedBuildTarget::WrappedComServer)
    }

    pub fn with_build_target(
        workspace: EmbeddedWorkspaceSnapshot,
        target: EmbeddedBuildTarget,
    ) -> Self {
        let request_id = build_request_id_for_workspace(&workspace, target);
        Self {
            request_id,
            workspace,
            target,
        }
    }

    pub fn with_request_id(
        workspace: EmbeddedWorkspaceSnapshot,
        request_id: impl Into<DirectHostBuildRequestId>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            workspace,
            target: EmbeddedBuildTarget::Bundle,
        }
    }

    pub fn with_request_id_and_target(
        workspace: EmbeddedWorkspaceSnapshot,
        request_id: impl Into<DirectHostBuildRequestId>,
        target: EmbeddedBuildTarget,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            workspace,
            target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedBuildTarget {
    Bundle,
    WrappedComServer,
}

impl EmbeddedBuildTarget {
    pub fn code(self) -> &'static str {
        match self {
            Self::Bundle => "Bundle",
            Self::WrappedComServer => "WrappedComServer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedBuildArtifactKind {
    Bundle,
    DynamicLibrary,
    TypeLibrary,
    RegistrationPlan,
    BuildLog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildArtifactPlan {
    pub kind: EmbeddedBuildArtifactKind,
    pub path: PathBuf,
    pub required: bool,
}

impl EmbeddedBuildArtifactPlan {
    pub fn new(kind: EmbeddedBuildArtifactKind, path: impl Into<PathBuf>, required: bool) -> Self {
        Self {
            kind,
            path: path.into(),
            required,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedComRegistrationScope {
    None,
    PerUser,
    Machine,
    RegistrationFreeManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedComServerRegistrationPlan {
    pub scope: EmbeddedComRegistrationScope,
    pub requires_admin: bool,
    pub command_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRequiredTool {
    pub name: String,
    pub required: bool,
}

impl EmbeddedRequiredTool {
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedComServerCapabilityProfile {
    pub windows: bool,
    pub bitness: String,
    pub toolchain: Vec<EmbeddedRequiredTool>,
    pub registration_scopes: Vec<EmbeddedComRegistrationScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildPlan {
    pub request_id: DirectHostBuildRequestId,
    pub target: EmbeddedBuildTarget,
    pub artifacts: Vec<EmbeddedBuildArtifactPlan>,
    pub required_tools: Vec<EmbeddedRequiredTool>,
    pub warnings: Vec<String>,
    pub com_server_capability: Option<EmbeddedComServerCapabilityProfile>,
    pub registration_plan: Option<EmbeddedComServerRegistrationPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedBuildStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildResult {
    pub request_id: DirectHostBuildRequestId,
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub plan: EmbeddedBuildPlan,
    pub dll_path: Option<PathBuf>,
    pub tlb_path: Option<PathBuf>,
    pub registration_plan: Option<EmbeddedComServerRegistrationPlan>,
    pub status: EmbeddedBuildStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
}

impl EmbeddedBuildResult {
    pub fn succeeded(
        request_id: DirectHostBuildRequestId,
        workspace: EmbeddedWorkspaceSnapshot,
        plan: EmbeddedBuildPlan,
    ) -> Self {
        let dll_path = plan
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == EmbeddedBuildArtifactKind::DynamicLibrary)
            .map(|artifact| artifact.path.clone());
        let tlb_path = plan
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == EmbeddedBuildArtifactKind::TypeLibrary)
            .map(|artifact| artifact.path.clone());
        Self {
            request_id,
            workspace,
            registration_plan: plan.registration_plan.clone(),
            plan,
            dll_path,
            tlb_path,
            status: EmbeddedBuildStatus::Succeeded,
            diagnostics: Vec::new(),
        }
    }

    pub fn failed(
        request_id: DirectHostBuildRequestId,
        workspace: EmbeddedWorkspaceSnapshot,
        plan: EmbeddedBuildPlan,
        diagnostics: Vec<PhaseDiagnostic>,
    ) -> Self {
        Self {
            request_id,
            workspace,
            registration_plan: plan.registration_plan.clone(),
            plan,
            dll_path: None,
            tlb_path: None,
            status: EmbeddedBuildStatus::Failed,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRunRequest {
    pub request_id: DirectHostRunRequestId,
    pub workspace: EmbeddedWorkspaceSnapshot,
}

impl EmbeddedRunRequest {
    pub fn new(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        let request_id = run_request_id_for_workspace(&workspace);
        Self {
            request_id,
            workspace,
        }
    }

    pub fn with_request_id(
        workspace: EmbeddedWorkspaceSnapshot,
        request_id: impl Into<DirectHostRunRequestId>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            workspace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedRunStatus {
    SessionReady,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRunResult {
    pub request_id: DirectHostRunRequestId,
    pub runtime_session_id: Option<DirectHostRuntimeSessionId>,
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub status: EmbeddedRunStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
}

impl EmbeddedRunResult {
    pub fn session_ready(
        request_id: DirectHostRunRequestId,
        runtime_session_id: DirectHostRuntimeSessionId,
        workspace: EmbeddedWorkspaceSnapshot,
    ) -> Self {
        Self {
            request_id,
            runtime_session_id: Some(runtime_session_id),
            workspace,
            status: EmbeddedRunStatus::SessionReady,
            diagnostics: Vec::new(),
        }
    }

    pub fn failed(
        request_id: DirectHostRunRequestId,
        workspace: EmbeddedWorkspaceSnapshot,
        diagnostics: Vec<PhaseDiagnostic>,
    ) -> Self {
        Self {
            request_id,
            runtime_session_id: None,
            workspace,
            status: EmbeddedRunStatus::Failed,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedResetKind {
    ClearSessionState,
    ReloadProject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedResetRequest {
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub kind: EmbeddedResetKind,
}

impl EmbeddedResetRequest {
    pub fn new(workspace: EmbeddedWorkspaceSnapshot, kind: EmbeddedResetKind) -> Self {
        Self { workspace, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedResetStatus {
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedResetResult {
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub kind: EmbeddedResetKind,
    pub status: EmbeddedResetStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
}

impl EmbeddedResetResult {
    pub fn reset(workspace: EmbeddedWorkspaceSnapshot, kind: EmbeddedResetKind) -> Self {
        Self {
            workspace,
            kind,
            status: EmbeddedResetStatus::Reset,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedProcedureTarget {
    pub module_name: String,
    pub procedure_name: String,
}

impl EmbeddedProcedureTarget {
    pub fn new(module_name: impl Into<String>, procedure_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            procedure_name: procedure_name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedInvokeEntryPointRequest {
    pub workspace: EmbeddedWorkspaceSnapshot,
}

impl EmbeddedInvokeEntryPointRequest {
    pub fn new(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self { workspace }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedInvokeProcedureVariantRequest {
    pub target: EmbeddedProcedureTarget,
    /// Retained value-model procedure arguments.
    pub args: Vec<Variant>,
}

impl EmbeddedInvokeProcedureVariantRequest {
    pub fn new(target: EmbeddedProcedureTarget, args: Vec<Variant>) -> Self {
        Self { target, args }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddedInvocationTarget {
    EntryPoint(EmbeddedWorkspaceSnapshot),
    Procedure(EmbeddedProcedureTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedInvokeStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedInvokeVariantResult {
    pub target: EmbeddedInvocationTarget,
    pub status: EmbeddedInvokeStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
    /// Retained value-model return value.
    pub return_value: Option<Variant>,
}

impl EmbeddedInvokeVariantResult {
    pub fn completed(target: EmbeddedInvocationTarget, return_value: Option<Variant>) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedOutputChannel {
    Stdout,
    Stderr,
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedOutputLine {
    pub channel: EmbeddedOutputChannel,
    pub text: String,
}

impl EmbeddedOutputLine {
    pub fn new(channel: EmbeddedOutputChannel, text: impl Into<String>) -> Self {
        Self {
            channel,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildStartedEvent {
    pub request_id: DirectHostBuildRequestId,
    pub workspace: EmbeddedWorkspaceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRunStartedEvent {
    pub request_id: DirectHostRunRequestId,
    pub workspace: EmbeddedWorkspaceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddedBuildRunEvent {
    BuildStarted(EmbeddedBuildStartedEvent),
    BuildCompleted(EmbeddedBuildResult),
    BuildFailed(EmbeddedBuildResult),
    RunStarted(EmbeddedRunStartedEvent),
    SessionReady(EmbeddedRunResult),
    RunFailed(EmbeddedRunResult),
    RuntimeReset(EmbeddedResetResult),
    InvokeCompleted(EmbeddedInvokeVariantResult),
    InvokeFailed(EmbeddedInvokeVariantResult),
    OutputLine(EmbeddedOutputLine),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildRunHostCommandStatus {
    pub build_workspace: DirectHostCommandStatus,
    pub run_project: DirectHostCommandStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRunSessionCommandStatus {
    pub reset_runtime: DirectHostCommandStatus,
    pub invoke_entry_point: DirectHostCommandStatus,
    pub invoke_procedure: DirectHostCommandStatus,
    pub stop_cancel: DirectHostCommandStatus,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EmbeddedRunSessionError {
    #[error(transparent)]
    Phase(PhaseDiagnostic),
    #[error(
        "embedded runtime session target `{requested}` does not match active workspace `{active}`"
    )]
    WorkspaceTargetMismatch { requested: PathBuf, active: PathBuf },
}

impl EmbeddedRunSessionError {
    pub fn direct_host_issue(&self) -> DirectHostIssue {
        match self {
            EmbeddedRunSessionError::Phase(diagnostic) => diagnostic.direct_host_issue(),
            EmbeddedRunSessionError::WorkspaceTargetMismatch { requested, active } => {
                DirectHostIssue::new(DirectHostIssueKind::WorkspaceInvalid)
                    .with_technical_detail(self.to_string())
                    .with_path(requested.clone())
                    .with_workspace_id(active.display().to_string())
            }
        }
    }
}

/// Future direct-host facade for embedded build/run orchestration.
///
/// This currently chooses the owning crate/module and public type family.
/// Later beads add the source-policy handoff and executable methods.
pub struct EmbeddedBuildRunHost<'engine> {
    engine: &'engine Engine,
}

impl<'engine> EmbeddedBuildRunHost<'engine> {
    pub fn new(engine: &'engine Engine) -> Self {
        Self { engine }
    }

    pub fn engine(&self) -> &'engine Engine {
        self.engine
    }

    pub fn command_status(&self) -> EmbeddedBuildRunHostCommandStatus {
        EmbeddedBuildRunHostCommandStatus {
            build_workspace: DirectHostCommandStatus::available(),
            run_project: DirectHostCommandStatus::available(),
        }
    }

    pub fn build_workspace(&self, request: &EmbeddedBuildRequest) -> EmbeddedBuildResult {
        let plan = self.build_plan(request);
        match compile_project(&request.workspace.manifest) {
            Ok(_) => EmbeddedBuildResult::succeeded(
                request.request_id.clone(),
                request.workspace.clone(),
                plan,
            ),
            Err(err) => EmbeddedBuildResult::failed(
                request.request_id.clone(),
                request.workspace.clone(),
                plan,
                vec![PhaseDiagnostic::compile(err.to_string())],
            ),
        }
    }

    pub fn build_plan(&self, request: &EmbeddedBuildRequest) -> EmbeddedBuildPlan {
        let _ = self;
        embedded_build_plan_for_request(request)
    }

    pub fn build_workspace_with_events(
        &self,
        request: &EmbeddedBuildRequest,
    ) -> (EmbeddedBuildResult, Vec<EmbeddedBuildRunEvent>) {
        let started = EmbeddedBuildRunEvent::BuildStarted(EmbeddedBuildStartedEvent {
            request_id: request.request_id.clone(),
            workspace: request.workspace.clone(),
        });
        let result = self.build_workspace(request);
        let completed = match result.status {
            EmbeddedBuildStatus::Succeeded => EmbeddedBuildRunEvent::BuildCompleted(result.clone()),
            EmbeddedBuildStatus::Failed => EmbeddedBuildRunEvent::BuildFailed(result.clone()),
        };
        (result, vec![started, completed])
    }

    #[allow(clippy::result_large_err)]
    pub fn run_project(
        &self,
        request: &EmbeddedRunRequest,
    ) -> Result<EmbeddedRunSession<'engine>, EmbeddedRunResult> {
        let runtime_session_id = runtime_session_id_for_request(&request.request_id);
        match self
            .engine
            .compile_and_prepare_session(&request.workspace.manifest)
        {
            Ok(runtime) => Ok(EmbeddedRunSession {
                engine: self.engine,
                runtime_session_id: runtime_session_id.clone(),
                workspace: request.workspace.clone(),
                runtime,
                run_result: EmbeddedRunResult::session_ready(
                    request.request_id.clone(),
                    runtime_session_id,
                    request.workspace.clone(),
                ),
            }),
            Err(err) => Err(EmbeddedRunResult::failed(
                request.request_id.clone(),
                request.workspace.clone(),
                vec![err],
            )),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn run_project_with_events(
        &self,
        request: &EmbeddedRunRequest,
    ) -> Result<
        (EmbeddedRunSession<'engine>, Vec<EmbeddedBuildRunEvent>),
        (EmbeddedRunResult, Vec<EmbeddedBuildRunEvent>),
    > {
        let started = EmbeddedBuildRunEvent::RunStarted(EmbeddedRunStartedEvent {
            request_id: request.request_id.clone(),
            workspace: request.workspace.clone(),
        });
        match self.run_project(request) {
            Ok(session) => {
                let ready = EmbeddedBuildRunEvent::SessionReady(session.run_result().clone());
                Ok((session, vec![started, ready]))
            }
            Err(result) => {
                let failed = EmbeddedBuildRunEvent::RunFailed(result.clone());
                Err((result, vec![started, failed]))
            }
        }
    }
}

pub struct EmbeddedRunSession<'engine> {
    engine: &'engine Engine,
    runtime_session_id: DirectHostRuntimeSessionId,
    workspace: EmbeddedWorkspaceSnapshot,
    runtime: crate::ProjectRuntimeSession,
    run_result: EmbeddedRunResult,
}

impl<'engine> EmbeddedRunSession<'engine> {
    pub fn runtime_session_id(&self) -> &DirectHostRuntimeSessionId {
        &self.runtime_session_id
    }

    pub fn workspace(&self) -> &EmbeddedWorkspaceSnapshot {
        &self.workspace
    }

    pub fn run_result(&self) -> &EmbeddedRunResult {
        &self.run_result
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.workspace.manifest
    }

    pub fn runtime(&self) -> &crate::ProjectRuntimeSession {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut crate::ProjectRuntimeSession {
        &mut self.runtime
    }

    pub fn command_status(&self) -> EmbeddedRunSessionCommandStatus {
        EmbeddedRunSessionCommandStatus {
            reset_runtime: DirectHostCommandStatus::available(),
            invoke_entry_point: DirectHostCommandStatus::available(),
            invoke_procedure: DirectHostCommandStatus::available(),
            stop_cancel: DirectHostCommandStatus::disabled(
                DirectHostIssue::new(DirectHostIssueKind::RuntimeSessionUnavailable)
                    .with_summary("Stop/cancel is not supported by this embedded runtime session")
                    .with_retryability(DirectHostRetryability::NotRetryable)
                    .with_runtime_session_id(self.runtime_session_id.clone()),
            ),
        }
    }

    pub fn into_immediate_session(self) -> crate::ImmediateSession<'engine> {
        let Self {
            engine,
            runtime_session_id,
            workspace,
            runtime,
            ..
        } = self;
        crate::ImmediateSession::from_embedded_runtime_session(
            engine,
            workspace.manifest,
            runtime,
            runtime_session_id,
        )
    }

    pub fn into_debug_session(self) -> crate::DebugSession<'engine> {
        let Self {
            engine,
            runtime_session_id,
            workspace,
            runtime,
            ..
        } = self;
        crate::DebugSession::from_embedded_runtime_session(
            engine,
            workspace.manifest,
            runtime,
            runtime_session_id,
        )
    }

    pub fn reset_runtime(
        &mut self,
        request: &EmbeddedResetRequest,
    ) -> Result<EmbeddedResetResult, EmbeddedRunSessionError> {
        self.ensure_matching_workspace(&request.workspace)?;
        let runtime = self
            .engine
            .compile_and_prepare_session(&request.workspace.manifest)
            .map_err(EmbeddedRunSessionError::Phase)?;
        self.workspace = request.workspace.clone();
        self.run_result.workspace = request.workspace.clone();
        self.run_result.runtime_session_id = Some(self.runtime_session_id.clone());
        self.runtime = runtime;
        Ok(EmbeddedResetResult::reset(
            request.workspace.clone(),
            request.kind,
        ))
    }

    pub fn invoke_entry_point_variant(
        &mut self,
        request: &EmbeddedInvokeEntryPointRequest,
    ) -> Result<EmbeddedInvokeVariantResult, EmbeddedRunSessionError> {
        self.ensure_matching_workspace(&request.workspace)?;
        let runtime = self
            .engine
            .start_project_runtime_session(&request.workspace.manifest)
            .map_err(EmbeddedRunSessionError::Phase)?;
        self.workspace = request.workspace.clone();
        self.run_result.workspace = request.workspace.clone();
        self.run_result.runtime_session_id = Some(self.runtime_session_id.clone());
        self.runtime = runtime;
        Ok(EmbeddedInvokeVariantResult::completed(
            EmbeddedInvocationTarget::EntryPoint(request.workspace.clone()),
            None,
        ))
    }

    pub fn invoke_procedure_variant(
        &mut self,
        request: &EmbeddedInvokeProcedureVariantRequest,
    ) -> Result<EmbeddedInvokeVariantResult, EmbeddedRunSessionError> {
        let return_value = self
            .engine
            .invoke_procedure_with_variants(
                &mut self.runtime,
                &request.target.module_name,
                &request.target.procedure_name,
                &request.args,
            )
            .map_err(EmbeddedRunSessionError::Phase)?;
        Ok(EmbeddedInvokeVariantResult::completed(
            EmbeddedInvocationTarget::Procedure(request.target.clone()),
            Some(return_value),
        ))
    }

    fn ensure_matching_workspace(
        &self,
        workspace: &EmbeddedWorkspaceSnapshot,
    ) -> Result<(), EmbeddedRunSessionError> {
        if same_workspace_target_path(self.workspace.workspace.path(), workspace.workspace.path()) {
            Ok(())
        } else {
            Err(EmbeddedRunSessionError::WorkspaceTargetMismatch {
                requested: workspace.workspace.workspace_target.clone(),
                active: self.workspace.workspace.workspace_target.clone(),
            })
        }
    }
}

fn build_request_id_for_workspace(
    workspace: &EmbeddedWorkspaceSnapshot,
    target: EmbeddedBuildTarget,
) -> DirectHostBuildRequestId {
    DirectHostBuildRequestId::new(format!(
        "build:{}:{}:{:?}:{}",
        normalize_workspace_target_path(workspace.workspace.path()).display(),
        target.code(),
        workspace.workspace.source_policy,
        workspace.manifest.project_name
    ))
}

fn run_request_id_for_workspace(workspace: &EmbeddedWorkspaceSnapshot) -> DirectHostRunRequestId {
    DirectHostRunRequestId::new(format!(
        "run:{}:{:?}:{}",
        normalize_workspace_target_path(workspace.workspace.path()).display(),
        workspace.workspace.source_policy,
        workspace.manifest.project_name
    ))
}

fn runtime_session_id_for_request(
    request_id: &DirectHostRunRequestId,
) -> DirectHostRuntimeSessionId {
    DirectHostRuntimeSessionId::new(format!("runtime:{}", request_id.as_str()))
}

fn same_workspace_target_path(left: &Path, right: &Path) -> bool {
    normalize_workspace_target_path(left) == normalize_workspace_target_path(right)
}

fn normalize_workspace_target_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir.join(path)
    } else {
        path.to_path_buf()
    };
    absolute.canonicalize().unwrap_or(absolute)
}

fn embedded_build_plan_for_request(request: &EmbeddedBuildRequest) -> EmbeddedBuildPlan {
    let output_dir = embedded_build_output_dir(&request.workspace);
    let project_name = sanitize_artifact_stem(&request.workspace.manifest.project_name);
    match request.target {
        EmbeddedBuildTarget::Bundle => EmbeddedBuildPlan {
            request_id: request.request_id.clone(),
            target: request.target,
            artifacts: vec![EmbeddedBuildArtifactPlan::new(
                EmbeddedBuildArtifactKind::Bundle,
                output_dir.join(format!("{project_name}.oxb")),
                true,
            )],
            required_tools: vec![],
            warnings: vec![],
            com_server_capability: None,
            registration_plan: None,
        },
        EmbeddedBuildTarget::WrappedComServer => {
            let mut warnings = Vec::new();
            if !cfg!(target_os = "windows") {
                warnings.push(
                    "WrappedComServer build output is only available on Windows hosts".to_string(),
                );
            }
            let bitness = if cfg!(target_pointer_width = "64") {
                "x64"
            } else if cfg!(target_pointer_width = "32") {
                "x86"
            } else {
                "unknown"
            }
            .to_string();
            let required_tools = vec![
                EmbeddedRequiredTool::required("rustc"),
                EmbeddedRequiredTool::required("cargo"),
                EmbeddedRequiredTool::required("windows-sdk"),
            ];
            let registration_plan = EmbeddedComServerRegistrationPlan {
                scope: EmbeddedComRegistrationScope::PerUser,
                requires_admin: false,
                command_hint: Some("DllRegisterServer".to_string()),
            };
            EmbeddedBuildPlan {
                request_id: request.request_id.clone(),
                target: request.target,
                artifacts: vec![
                    EmbeddedBuildArtifactPlan::new(
                        EmbeddedBuildArtifactKind::Bundle,
                        output_dir.join(format!("{project_name}.oxb")),
                        true,
                    ),
                    EmbeddedBuildArtifactPlan::new(
                        EmbeddedBuildArtifactKind::DynamicLibrary,
                        output_dir.join(format!("{project_name}.dll")),
                        true,
                    ),
                    EmbeddedBuildArtifactPlan::new(
                        EmbeddedBuildArtifactKind::TypeLibrary,
                        output_dir.join(format!("{project_name}.tlb")),
                        true,
                    ),
                    EmbeddedBuildArtifactPlan::new(
                        EmbeddedBuildArtifactKind::RegistrationPlan,
                        output_dir.join(format!("{project_name}.registration.json")),
                        true,
                    ),
                    EmbeddedBuildArtifactPlan::new(
                        EmbeddedBuildArtifactKind::BuildLog,
                        output_dir.join(format!("{project_name}.build.log")),
                        false,
                    ),
                ],
                required_tools: required_tools.clone(),
                warnings,
                com_server_capability: Some(EmbeddedComServerCapabilityProfile {
                    windows: cfg!(target_os = "windows"),
                    bitness,
                    toolchain: required_tools,
                    registration_scopes: vec![
                        EmbeddedComRegistrationScope::None,
                        EmbeddedComRegistrationScope::PerUser,
                        EmbeddedComRegistrationScope::RegistrationFreeManifest,
                    ],
                }),
                registration_plan: Some(registration_plan),
            }
        }
    }
}

fn embedded_build_output_dir(workspace: &EmbeddedWorkspaceSnapshot) -> PathBuf {
    let path = workspace.workspace.path();
    if path.extension().is_some() {
        path.parent()
            .map(|parent| parent.join("target").join("oxvba"))
            .unwrap_or_else(|| PathBuf::from("target").join("oxvba"))
    } else {
        path.join("target").join("oxvba")
    }
}

fn sanitize_artifact_stem(project_name: &str) -> String {
    let stem = project_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if stem.is_empty() {
        "project".to_string()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmbeddedBuildArtifactKind, EmbeddedBuildRequest, EmbeddedBuildRunEvent,
        EmbeddedBuildRunHost, EmbeddedBuildStartedEvent, EmbeddedBuildStatus, EmbeddedBuildTarget,
        EmbeddedComRegistrationScope, EmbeddedExecutionSourcePolicy, EmbeddedInvocationTarget,
        EmbeddedInvokeEntryPointRequest, EmbeddedInvokeProcedureVariantRequest,
        EmbeddedOutputChannel, EmbeddedOutputLine, EmbeddedProcedureTarget, EmbeddedResetKind,
        EmbeddedResetRequest, EmbeddedRunRequest, EmbeddedRunStatus, EmbeddedWorkspaceInput,
        EmbeddedWorkspaceSnapshot,
    };
    use crate::{Engine, HostConfig};
    use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
    use oxvba_runtime::{VarType, Variant};

    fn make_manifest(source: &str) -> ProjectManifest {
        ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Module1", ModuleKind::Procedural, source).expect("module"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        }
    }

    fn make_startup_manifest(module_source: &str) -> ProjectManifest {
        ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source(
                    "__OxVbaStartupEntryShim",
                    ModuleKind::Procedural,
                    "Attribute VB_Name = \"__OxVbaStartupEntryShim\"\nOption Private Module\nPublic Sub Main()\nCall Module1.Main()\nEnd Sub\n",
                )
                .expect("shim"),
                module_unit_from_source("Module1", ModuleKind::Procedural, module_source)
                    .expect("module"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        }
    }

    #[test]
    fn workspace_input_builders_preserve_explicit_source_policy() {
        let disk = EmbeddedWorkspaceInput::disk_only("App.basproj");
        assert_eq!(disk.source_policy, EmbeddedExecutionSourcePolicy::DiskOnly);
        assert_eq!(disk.path().to_string_lossy(), "App.basproj");

        let overlay = EmbeddedWorkspaceInput::workspace_overlay("App.basproj");
        assert_eq!(
            overlay.source_policy,
            EmbeddedExecutionSourcePolicy::WorkspaceOverlay
        );
    }

    #[test]
    fn invoke_procedure_variant_request_preserves_target_and_args() {
        let request = EmbeddedInvokeProcedureVariantRequest::new(
            EmbeddedProcedureTarget::new("Module1", "Main"),
            vec![Variant::from_i32(42)],
        );

        assert_eq!(request.target.module_name, "Module1");
        assert_eq!(request.target.procedure_name, "Main");
        assert_eq!(request.args, vec![Variant::from_i32(42)]);
    }

    #[test]
    fn invoke_procedure_variant_request_preserves_exact_args() {
        let request = EmbeddedInvokeProcedureVariantRequest::new(
            EmbeddedProcedureTarget::new("Module1", "Main"),
            vec![Variant::from_string("ABC")],
        );

        assert_eq!(request.target.module_name, "Module1");
        assert_eq!(request.target.procedure_name, "Main");
        assert_eq!(request.args[0].vtype(), VarType::String);
        assert_eq!(request.args, vec![Variant::from_string("ABC")]);
    }

    #[test]
    fn embedded_build_event_family_round_trips_through_typed_results() {
        let workspace = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_manifest("Sub Main()\nEnd Sub\n"),
        );
        let build = EmbeddedBuildRequest::new(workspace.clone());
        let event = EmbeddedBuildRunEvent::BuildStarted(EmbeddedBuildStartedEvent {
            request_id: build.request_id.clone(),
            workspace: workspace.clone(),
        });
        assert_eq!(
            event,
            EmbeddedBuildRunEvent::BuildStarted(EmbeddedBuildStartedEvent {
                request_id: build.request_id.clone(),
                workspace: workspace.clone(),
            })
        );

        let invoke_target =
            EmbeddedInvocationTarget::Procedure(EmbeddedProcedureTarget::new("Module1", "Main"));
        let output = EmbeddedBuildRunEvent::OutputLine(EmbeddedOutputLine::new(
            EmbeddedOutputChannel::Host,
            "ready",
        ));
        assert_eq!(
            output,
            EmbeddedBuildRunEvent::OutputLine(EmbeddedOutputLine {
                channel: EmbeddedOutputChannel::Host,
                text: "ready".to_string(),
            })
        );
        assert_eq!(
            invoke_target,
            EmbeddedInvocationTarget::Procedure(EmbeddedProcedureTarget::new("Module1", "Main"))
        );

        assert!(build.request_id.as_str().starts_with("build:"));
        assert_eq!(
            build.workspace.workspace.source_policy,
            EmbeddedExecutionSourcePolicy::WorkspaceOverlay
        );
    }

    #[test]
    fn wrapped_com_server_build_plan_reports_artifacts_and_registration_dtos() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let workspace = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_manifest("Public Function Ping() As Long\nPing = 42\nEnd Function\n"),
        );
        let request = EmbeddedBuildRequest::wrapped_com_server(workspace);

        let plan = host.build_plan(&request);
        assert_eq!(request.target, EmbeddedBuildTarget::WrappedComServer);
        assert_eq!(plan.target, EmbeddedBuildTarget::WrappedComServer);
        assert!(request.request_id.as_str().contains("WrappedComServer"));
        assert!(plan.artifacts.iter().any(|artifact| artifact.kind
            == EmbeddedBuildArtifactKind::Bundle
            && artifact.path.ends_with("App.oxb")
            && artifact.required));
        assert!(plan.artifacts.iter().any(|artifact| artifact.kind
            == EmbeddedBuildArtifactKind::DynamicLibrary
            && artifact.path.ends_with("App.dll")
            && artifact.required));
        assert!(plan.artifacts.iter().any(|artifact| artifact.kind
            == EmbeddedBuildArtifactKind::TypeLibrary
            && artifact.path.ends_with("App.tlb")
            && artifact.required));
        assert!(
            plan.required_tools
                .iter()
                .any(|tool| tool.name == "windows-sdk" && tool.required)
        );
        let registration_plan = plan.registration_plan.as_ref().expect("registration plan");
        assert_eq!(
            registration_plan.scope,
            EmbeddedComRegistrationScope::PerUser
        );
        assert!(!registration_plan.requires_admin);
        let capability = plan
            .com_server_capability
            .as_ref()
            .expect("COM server capability profile");
        assert!(!capability.bitness.is_empty());
        assert!(
            capability
                .registration_scopes
                .contains(&EmbeddedComRegistrationScope::RegistrationFreeManifest)
        );

        let result = host.build_workspace(&request);
        assert_eq!(result.status, EmbeddedBuildStatus::Succeeded);
        assert_eq!(result.plan.target, EmbeddedBuildTarget::WrappedComServer);
        assert!(
            result
                .dll_path
                .as_ref()
                .is_some_and(|path| path.ends_with("App.dll"))
        );
        assert!(
            result
                .tlb_path
                .as_ref()
                .is_some_and(|path| path.ends_with("App.tlb"))
        );
        assert_eq!(
            result
                .registration_plan
                .as_ref()
                .expect("result registration plan")
                .scope,
            EmbeddedComRegistrationScope::PerUser
        );
    }

    #[test]
    fn embedded_build_run_ids_events_and_command_status_are_correlated() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let workspace = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_manifest("Public Sub Main()\nEnd Sub\n"),
        );
        let build_request =
            EmbeddedBuildRequest::with_request_id(workspace.clone(), "build-request:thin-slice");
        let (build_result, build_events) = host.build_workspace_with_events(&build_request);
        assert_eq!(build_result.request_id.as_str(), "build-request:thin-slice");
        assert_eq!(build_result.status, EmbeddedBuildStatus::Succeeded);
        assert_eq!(build_events.len(), 2);
        assert!(matches!(
            &build_events[0],
            EmbeddedBuildRunEvent::BuildStarted(started)
                if started.request_id.as_str() == "build-request:thin-slice"
        ));
        assert!(matches!(
            &build_events[1],
            EmbeddedBuildRunEvent::BuildCompleted(completed)
                if completed.request_id.as_str() == "build-request:thin-slice"
        ));

        let run_request = EmbeddedRunRequest::with_request_id(workspace, "run-request:thin-slice");
        let (run_session, run_events) = host
            .run_project_with_events(&run_request)
            .expect("run session");
        assert_eq!(
            run_session.runtime_session_id().as_str(),
            "runtime:run-request:thin-slice"
        );
        assert_eq!(
            run_session
                .run_result()
                .runtime_session_id
                .as_ref()
                .unwrap()
                .as_str(),
            "runtime:run-request:thin-slice"
        );
        assert_eq!(run_events.len(), 2);
        assert!(matches!(
            &run_events[0],
            EmbeddedBuildRunEvent::RunStarted(started)
                if started.request_id.as_str() == "run-request:thin-slice"
        ));
        assert!(matches!(
            &run_events[1],
            EmbeddedBuildRunEvent::SessionReady(result)
                if result.request_id.as_str() == "run-request:thin-slice"
        ));

        let host_status = host.command_status();
        assert!(host_status.build_workspace.is_available());
        assert!(host_status.run_project.is_available());
        let session_status = run_session.command_status();
        assert!(session_status.reset_runtime.is_available());
        assert!(session_status.invoke_procedure.is_available());
        assert!(!session_status.stop_cancel.is_available());
        assert_eq!(
            session_status
                .stop_cancel
                .disabled_reason()
                .expect("disabled reason")
                .stable_code,
            "DH-RUNTIME-SESSION-UNAVAILABLE"
        );
    }

    #[test]
    fn embedded_run_session_attaches_immediate_and_debug_with_stable_ids() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let workspace = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_manifest(
                "Public Sub Main()\nCall Foo(1)\nEnd Sub\nPublic Sub Foo(ByVal value As Long)\nEnd Sub\n",
            ),
        );

        let immediate_run = host
            .run_project(&EmbeddedRunRequest::with_request_id(
                workspace.clone(),
                "run:immediate",
            ))
            .expect("immediate run");
        let runtime_id = immediate_run.runtime_session_id().clone();
        let immediate = immediate_run.into_immediate_session();
        assert_eq!(immediate.runtime_session_id(), Some(&runtime_id));
        assert_eq!(
            immediate.immediate_session_id().as_str(),
            "immediate:runtime:run:immediate"
        );
        assert!(immediate.command_status().evaluate.is_available());

        let debug_run = host
            .run_project(&EmbeddedRunRequest::with_request_id(workspace, "run:debug"))
            .expect("debug run");
        let runtime_id = debug_run.runtime_session_id().clone();
        let mut debug = debug_run.into_debug_session();
        assert_eq!(debug.runtime_session_id(), Some(&runtime_id));
        assert_eq!(debug.debug_session_id().as_str(), "debug:runtime:run:debug");
        assert!(!debug.command_status().evaluate.is_available());
        let _ = debug.start_variants().expect("debug start");
        assert!(debug.command_status().evaluate.is_available());
    }

    #[test]
    fn embedded_host_wrapper_exposes_engine_reference() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        assert_eq!(host.engine().host_policy(), engine.host_policy());
    }

    #[test]
    fn embedded_host_build_workspace_projects_compile_success() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let request = EmbeddedBuildRequest::new(EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::disk_only("App.basproj"),
            make_manifest("Sub Main()\nEnd Sub\n"),
        ));

        let result = host.build_workspace(&request);
        assert_eq!(result.status, EmbeddedBuildStatus::Succeeded);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn embedded_host_build_workspace_projects_compile_failure_as_typed_diagnostics() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let mut manifest = make_manifest("Sub Main()\nEnd Sub\n");
        manifest.modules.push(
            module_unit_from_source("Module1", ModuleKind::Procedural, "Sub Other()\nEnd Sub\n")
                .expect("duplicate module"),
        );
        let request = EmbeddedBuildRequest::new(EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::disk_only("Broken.basproj"),
            manifest,
        ));

        let result = host.build_workspace(&request);
        assert_eq!(result.status, EmbeddedBuildStatus::Failed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].phase(),
            crate::DiagnosticPhase::CompileTime
        );
    }

    #[test]
    fn embedded_host_run_project_returns_live_runtime_session() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let request = EmbeddedRunRequest::new(EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::disk_only("App.basproj"),
            make_manifest(
                "Public Function GetValue() As Integer\n    GetValue = 42\nEnd Function\n",
            ),
        ));

        let mut session = host.run_project(&request).expect("run session");
        assert_eq!(session.run_result().status, EmbeddedRunStatus::SessionReady);

        let result = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "GetValue"),
                Vec::new(),
            ))
            .expect("invoke");
        assert_eq!(result.return_value, Some(Variant::from_i32(42)));
    }

    #[test]
    fn embedded_run_session_invokes_procedure_with_variant_args() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let request = EmbeddedRunRequest::new(EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::disk_only("App.basproj"),
            make_manifest(
                "Public Function Echo(ByVal value As String) As String\n    Echo = value\nEnd Function\n",
            ),
        ));

        let mut session = host.run_project(&request).expect("run session");
        let result = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "Echo"),
                vec![Variant::from_string("ABC")],
            ))
            .expect("invoke");

        let return_value = result.return_value.expect("return value");
        assert_eq!(return_value.vtype(), VarType::String);
        assert_eq!(return_value.as_bstr(), Some("ABC".into()));
    }

    #[test]
    fn embedded_run_session_reset_restores_live_runtime_state() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let snapshot = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::disk_only("App.basproj"),
            make_manifest(
                "Dim counter As Integer\n\
                 Public Function IncrementCounter() As Integer\n\
                     counter = counter + 1\n\
                     IncrementCounter = counter\n\
                 End Function\n",
            ),
        );
        let request = EmbeddedRunRequest::new(snapshot.clone());
        let mut session = host.run_project(&request).expect("run session");

        let first = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "IncrementCounter"),
                Vec::new(),
            ))
            .expect("first");
        let second = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "IncrementCounter"),
                Vec::new(),
            ))
            .expect("second");
        assert_eq!(first.return_value, Some(Variant::from_i32(1)));
        assert_eq!(second.return_value, Some(Variant::from_i32(2)));

        let reset = session
            .reset_runtime(&EmbeddedResetRequest::new(
                snapshot.clone(),
                EmbeddedResetKind::ClearSessionState,
            ))
            .expect("reset");
        assert_eq!(reset.kind, EmbeddedResetKind::ClearSessionState);

        let after_reset = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "IncrementCounter"),
                Vec::new(),
            ))
            .expect("after reset");
        assert_eq!(after_reset.return_value, Some(Variant::from_i32(1)));
    }

    #[test]
    fn embedded_run_session_invokes_entry_point_through_startup_shim() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let snapshot = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_startup_manifest(
                "Dim counter As Integer\n\
                 Public Sub Main()\n\
                     counter = counter + 1\n\
                 End Sub\n\
                 Public Function GetCounter() As Integer\n\
                     GetCounter = counter\n\
                 End Function\n",
            ),
        );
        let request = EmbeddedRunRequest::new(snapshot.clone());
        let mut session = host.run_project(&request).expect("run session");

        let before = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "GetCounter"),
                Vec::new(),
            ))
            .expect("before invoke");
        assert_eq!(before.return_value, Some(Variant::from_i32(0)));

        let invoked = session
            .invoke_entry_point_variant(&EmbeddedInvokeEntryPointRequest::new(snapshot))
            .expect("invoke entry point");
        assert_eq!(invoked.status, super::EmbeddedInvokeStatus::Completed);

        let after = session
            .invoke_procedure_variant(&EmbeddedInvokeProcedureVariantRequest::new(
                EmbeddedProcedureTarget::new("Module1", "GetCounter"),
                Vec::new(),
            ))
            .expect("after invoke");
        assert_eq!(after.return_value, Some(Variant::from_i32(1)));
    }

    #[test]
    fn embedded_run_session_keeps_run_result_workspace_in_sync_after_reset() {
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);
        let first_snapshot = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_manifest(
                "Public Function GetValue() As Integer\n    GetValue = 1\nEnd Function\n",
            ),
        );
        let second_snapshot = EmbeddedWorkspaceSnapshot::new(
            EmbeddedWorkspaceInput::workspace_overlay("App.basproj"),
            make_manifest(
                "Public Function GetValue() As Integer\n    GetValue = 2\nEnd Function\n",
            ),
        );

        let mut session = host
            .run_project(&EmbeddedRunRequest::new(first_snapshot.clone()))
            .expect("run session");
        assert_eq!(session.run_result().workspace, first_snapshot);

        session
            .reset_runtime(&EmbeddedResetRequest::new(
                second_snapshot.clone(),
                EmbeddedResetKind::ReloadProject,
            ))
            .expect("reset");

        assert_eq!(session.workspace(), &second_snapshot);
        assert_eq!(session.run_result().workspace, second_snapshot);
    }
}
