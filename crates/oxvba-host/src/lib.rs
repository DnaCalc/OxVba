//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod compat;
pub mod debugger;
pub mod embedded;
pub mod engine;
pub mod events;
pub mod immediate;
pub mod project;
pub mod runner;

pub use debugger::{
    DebugEvaluationRequest, DebugFrameValueKind, DebugFrameVariant, DebugFrameVariantValue,
    DebugSession, DebugSessionError, DebugVariantEvaluationResult, DebugVariantPauseState,
    HostDebugVariantRunResult,
};
pub use embedded::{
    EmbeddedBuildRequest, EmbeddedBuildResult, EmbeddedBuildRunEvent, EmbeddedBuildRunHost,
    EmbeddedBuildStatus, EmbeddedExecutionSourcePolicy, EmbeddedInvocationTarget,
    EmbeddedInvokeEntryPointRequest, EmbeddedInvokeProcedureVariantRequest, EmbeddedInvokeStatus,
    EmbeddedInvokeVariantResult, EmbeddedOutputChannel, EmbeddedOutputLine,
    EmbeddedProcedureTarget, EmbeddedResetKind, EmbeddedResetRequest, EmbeddedResetResult,
    EmbeddedResetStatus, EmbeddedRunRequest, EmbeddedRunResult, EmbeddedRunSession,
    EmbeddedRunSessionError, EmbeddedRunStatus, EmbeddedWorkspaceInput, EmbeddedWorkspaceSnapshot,
};
pub use engine::{
    ComEventCallbackVariantDispatch, DiagnosticPhase, Engine, HostConfig, PhaseDiagnostic,
    ProjectRuntimeSession,
};
pub use immediate::{
    ImmediateDisplayStyle, ImmediateEvaluationRequest, ImmediateInputKind, ImmediateResetKind,
    ImmediateSession, ImmediateSessionError, ImmediateVariantEvaluationOutput,
    ImmediateVariantEvaluationResult, ImmediateVariantValueProjection,
};
pub use project::{
    GraphPublicSymbolResolution, HostExportKind, HostProcedureExport, ModuleAttributes, ModuleKind,
    ModuleNode, Project, ProjectGraph, ProjectKind, ProjectModelError, ProjectNode,
    ProjectReference, PublicSymbolResolution, ReferenceBindingState, ReferenceKind,
    TypeLibraryBindingRecord, TypeLibraryBindingStatus, TypeLibraryCatalogEntry,
};
pub use runner::{
    PolicyOverrides, ResolvedRunnerBootstrap, RunnerBootstrapFallbacks, RunnerBootstrapOptions,
    RuntimeProfileId, resolve_runner_bootstrap, resolve_runner_bootstrap_with_fallbacks,
};
