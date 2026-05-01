use std::path::{Path, PathBuf};

use oxvba_compiler::{ProjectManifest, compile_project};
use oxvba_runtime::Variant;
use thiserror::Error;

use crate::Engine;
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
    pub workspace: EmbeddedWorkspaceSnapshot,
}

impl EmbeddedBuildRequest {
    pub fn new(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self { workspace }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedBuildStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBuildResult {
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub status: EmbeddedBuildStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
}

impl EmbeddedBuildResult {
    pub fn succeeded(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self {
            workspace,
            status: EmbeddedBuildStatus::Succeeded,
            diagnostics: Vec::new(),
        }
    }

    pub fn failed(workspace: EmbeddedWorkspaceSnapshot, diagnostics: Vec<PhaseDiagnostic>) -> Self {
        Self {
            workspace,
            status: EmbeddedBuildStatus::Failed,
            diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRunRequest {
    pub workspace: EmbeddedWorkspaceSnapshot,
}

impl EmbeddedRunRequest {
    pub fn new(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self { workspace }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedRunStatus {
    SessionReady,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedRunResult {
    pub workspace: EmbeddedWorkspaceSnapshot,
    pub status: EmbeddedRunStatus,
    pub diagnostics: Vec<PhaseDiagnostic>,
}

impl EmbeddedRunResult {
    pub fn session_ready(workspace: EmbeddedWorkspaceSnapshot) -> Self {
        Self {
            workspace,
            status: EmbeddedRunStatus::SessionReady,
            diagnostics: Vec::new(),
        }
    }

    pub fn failed(workspace: EmbeddedWorkspaceSnapshot, diagnostics: Vec<PhaseDiagnostic>) -> Self {
        Self {
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
pub enum EmbeddedBuildRunEvent {
    BuildStarted(EmbeddedWorkspaceSnapshot),
    BuildCompleted(EmbeddedBuildResult),
    BuildFailed(EmbeddedBuildResult),
    RunStarted(EmbeddedWorkspaceSnapshot),
    SessionReady(EmbeddedRunResult),
    RunFailed(EmbeddedRunResult),
    RuntimeReset(EmbeddedResetResult),
    InvokeCompleted(EmbeddedInvokeVariantResult),
    InvokeFailed(EmbeddedInvokeVariantResult),
    OutputLine(EmbeddedOutputLine),
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

    pub fn build_workspace(&self, request: &EmbeddedBuildRequest) -> EmbeddedBuildResult {
        match compile_project(&request.workspace.manifest) {
            Ok(_) => EmbeddedBuildResult::succeeded(request.workspace.clone()),
            Err(err) => EmbeddedBuildResult::failed(
                request.workspace.clone(),
                vec![PhaseDiagnostic::compile(err.to_string())],
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn run_project(
        &self,
        request: &EmbeddedRunRequest,
    ) -> Result<EmbeddedRunSession<'engine>, EmbeddedRunResult> {
        match self
            .engine
            .compile_and_prepare_session(&request.workspace.manifest)
        {
            Ok(runtime) => Ok(EmbeddedRunSession {
                engine: self.engine,
                workspace: request.workspace.clone(),
                runtime,
                run_result: EmbeddedRunResult::session_ready(request.workspace.clone()),
            }),
            Err(err) => Err(EmbeddedRunResult::failed(
                request.workspace.clone(),
                vec![err],
            )),
        }
    }
}

pub struct EmbeddedRunSession<'engine> {
    engine: &'engine Engine,
    workspace: EmbeddedWorkspaceSnapshot,
    runtime: crate::ProjectRuntimeSession,
    run_result: EmbeddedRunResult,
}

impl<'engine> EmbeddedRunSession<'engine> {
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

#[cfg(test)]
mod tests {
    use super::{
        EmbeddedBuildRequest, EmbeddedBuildRunEvent, EmbeddedBuildRunHost, EmbeddedBuildStatus,
        EmbeddedExecutionSourcePolicy, EmbeddedInvocationTarget, EmbeddedInvokeEntryPointRequest,
        EmbeddedInvokeProcedureVariantRequest, EmbeddedOutputChannel, EmbeddedOutputLine,
        EmbeddedProcedureTarget, EmbeddedResetKind, EmbeddedResetRequest, EmbeddedRunRequest,
        EmbeddedRunStatus, EmbeddedWorkspaceInput, EmbeddedWorkspaceSnapshot,
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
        let event = EmbeddedBuildRunEvent::BuildStarted(workspace.clone());
        assert_eq!(
            event,
            EmbeddedBuildRunEvent::BuildStarted(workspace.clone())
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

        let build = EmbeddedBuildRequest::new(workspace);
        assert_eq!(
            build.workspace.workspace.source_policy,
            EmbeddedExecutionSourcePolicy::WorkspaceOverlay
        );
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
