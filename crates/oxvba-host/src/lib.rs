//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod debugger;
pub mod direct_host;
pub mod embedded;
pub mod engine;
pub mod events;
pub mod immediate;
pub mod native_ready_runner;
pub mod project;
pub mod runner;

pub use debugger::{
    DebugBreakpointBindingStatus, DebugBreakpointRecord, DebugBreakpointUnresolvedReason,
    DebugEvaluationRequest, DebugFrameValueKind, DebugFrameVariant, DebugFrameVariantValue,
    DebugSession, DebugSessionCommandStatus, DebugSessionError, DebugVariantEvaluationResult,
    DebugVariantPauseState, DebugWatchEvaluation, DebugWatchEvaluationStatus, DebugWatchRecord,
    HostDebugVariantRunResult,
};
pub use direct_host::{
    DirectHostBreakpointId, DirectHostBuildRequestId, DirectHostCapability,
    DirectHostCapabilityKind, DirectHostCapabilityStatus, DirectHostCommandStatus,
    DirectHostDebugSessionId, DirectHostDiagnostic, DirectHostDocumentId,
    DirectHostImmediateSessionId, DirectHostIssue, DirectHostIssueContext, DirectHostIssueKind,
    DirectHostModuleId, DirectHostProjectId, DirectHostRetryability, DirectHostRunRequestId,
    DirectHostRuntimeSessionId, DirectHostSourceSpan, DirectHostSourceSpanStatus,
    DirectHostSourceUnavailableReason, DirectHostStackFrameId, DirectHostTextPosition,
    DirectHostWatchId, DirectHostWorkspaceId,
};
pub use embedded::{
    EmbeddedBuildArtifactKind, EmbeddedBuildArtifactPlan, EmbeddedBuildPlan, EmbeddedBuildRequest,
    EmbeddedBuildResult, EmbeddedBuildRunEvent, EmbeddedBuildRunHost,
    EmbeddedBuildRunHostCommandStatus, EmbeddedBuildStartedEvent, EmbeddedBuildStatus,
    EmbeddedBuildTarget, EmbeddedComRegistrationScope, EmbeddedComServerCapabilityProfile,
    EmbeddedComServerRegistrationPlan, EmbeddedExecutionSourcePolicy, EmbeddedInvocationTarget,
    EmbeddedInvokeEntryPointRequest, EmbeddedInvokeProcedureVariantRequest, EmbeddedInvokeStatus,
    EmbeddedInvokeVariantResult, EmbeddedOutputChannel, EmbeddedOutputLine,
    EmbeddedProcedureTarget, EmbeddedRequiredTool, EmbeddedResetKind, EmbeddedResetRequest,
    EmbeddedResetResult, EmbeddedResetStatus, EmbeddedRunRequest, EmbeddedRunResult,
    EmbeddedRunSession, EmbeddedRunSessionCommandStatus, EmbeddedRunSessionError,
    EmbeddedRunStartedEvent, EmbeddedRunStatus, EmbeddedWorkspaceInput, EmbeddedWorkspaceSnapshot,
};
pub use engine::{
    ComEventCallbackVariantDispatch, DiagnosticPhase, Engine, HostConfig,
    HostUdfArgumentDescriptor, HostUdfCallContext, HostUdfCallableMetadata,
    HostUdfCapabilityConstraints, HostUdfCatalog, HostUdfFunctionDescriptor,
    HostUdfInvocationTarget, HostUdfInvokeResult, HostUdfRegistrationIdentity,
    HostUdfTypeMapEvidence, HostUdfTypedInvokeResult, HostUdfTypedParameterDescriptor,
    HostUdfTypedSignature, HostUdfTypedValue, PhaseDiagnostic, ProjectRuntimeSession,
};
pub use immediate::{
    ImmediateDisplayStyle, ImmediateEvaluationRequest, ImmediateInputKind, ImmediateResetKind,
    ImmediateSession, ImmediateSessionCommandStatus, ImmediateSessionError,
    ImmediateVariantEvaluationOutput, ImmediateVariantEvaluationResult,
    ImmediateVariantValueProjection,
};
pub use native_ready_runner::{
    NATIVE_READY_RUNNER_SCHEMA_HEADER, NativeReadyRunnerConfig, NativeReadyRunnerRow,
    emit_native_ready_vm_jit_csv, produce_native_ready_vm_jit_rows,
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
