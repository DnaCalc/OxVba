//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod debugger;
pub mod engine;
pub mod events;
pub mod immediate;
pub mod project;
pub mod runner;

pub use debugger::{
    DebugEvaluationRequest, DebugEvaluationResult, DebugFrame, DebugFrameValue,
    DebugFrameValueKind, DebugPauseState, DebugSession, DebugSessionError, HostDebugRunResult,
};
pub use engine::{
    ComEventCallbackDispatch, DiagnosticPhase, Engine, HostConfig, PhaseDiagnostic,
    ProjectRuntimeSession,
};
pub use immediate::{
    ImmediateDisplayStyle, ImmediateEvaluationOutput, ImmediateEvaluationRequest,
    ImmediateEvaluationResult, ImmediateInputKind, ImmediateResetKind, ImmediateSession,
    ImmediateSessionError, ImmediateValueProjection,
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
