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
    DebugEvaluationRequest, DebugEvaluationResult, DebugFrame, DebugFrameValue,
    DebugFrameValueKind, DebugFrameVariant, DebugFrameVariantValue, DebugPauseState, DebugSession,
    DebugSessionError, DebugVariantEvaluationResult, DebugVariantPauseState, HostDebugRunResult,
    HostDebugVariantRunResult,
};
pub use embedded::{
    EmbeddedBuildRequest, EmbeddedBuildResult, EmbeddedBuildRunEvent, EmbeddedBuildRunHost,
    EmbeddedBuildStatus, EmbeddedExecutionSourcePolicy, EmbeddedInvocationTarget,
    EmbeddedInvokeEntryPointRequest, EmbeddedInvokeProcedureRequest,
    EmbeddedInvokeProcedureVariantRequest, EmbeddedInvokeResult, EmbeddedInvokeStatus,
    EmbeddedInvokeVariantResult, EmbeddedOutputChannel, EmbeddedOutputLine,
    EmbeddedProcedureTarget, EmbeddedResetKind, EmbeddedResetRequest, EmbeddedResetResult,
    EmbeddedResetStatus, EmbeddedRunRequest, EmbeddedRunResult, EmbeddedRunSession,
    EmbeddedRunSessionError, EmbeddedRunStatus, EmbeddedWorkspaceInput, EmbeddedWorkspaceSnapshot,
};
pub use engine::{
    ComEventCallbackDispatch, ComEventCallbackVariantDispatch, DiagnosticPhase, Engine, HostConfig,
    PhaseDiagnostic, ProjectRuntimeSession,
};
pub use immediate::{
    ImmediateDisplayStyle, ImmediateEvaluationOutput, ImmediateEvaluationRequest,
    ImmediateEvaluationResult, ImmediateInputKind, ImmediateResetKind, ImmediateSession,
    ImmediateSessionError, ImmediateValueProjection, ImmediateVariantEvaluationOutput,
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
