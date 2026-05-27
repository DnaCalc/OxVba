use std::path::PathBuf;

macro_rules! direct_host_id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

direct_host_id_type!(DirectHostWorkspaceId);
direct_host_id_type!(DirectHostProjectId);
direct_host_id_type!(DirectHostDocumentId);
direct_host_id_type!(DirectHostModuleId);
direct_host_id_type!(DirectHostBuildRequestId);
direct_host_id_type!(DirectHostRunRequestId);
direct_host_id_type!(DirectHostRuntimeSessionId);
direct_host_id_type!(DirectHostImmediateSessionId);
direct_host_id_type!(DirectHostDebugSessionId);
direct_host_id_type!(DirectHostBreakpointId);
direct_host_id_type!(DirectHostStackFrameId);
direct_host_id_type!(DirectHostWatchId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectHostRetryability {
    Retryable,
    NotRetryable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectHostCapabilityKind {
    Workspace,
    LanguageService,
    ProjectAuthoring,
    BuildCheck,
    RuntimeSession,
    ImmediateWindow,
    Debugger,
    WatchEvaluation,
    Breakpoints,
    ComReferenceDiscovery,
    ComRuntimeInvocation,
    NativeService,
    SidecarService,
}

impl DirectHostCapabilityKind {
    pub fn code(self) -> &'static str {
        match self {
            DirectHostCapabilityKind::Workspace => "workspace",
            DirectHostCapabilityKind::LanguageService => "language-service",
            DirectHostCapabilityKind::ProjectAuthoring => "project-authoring",
            DirectHostCapabilityKind::BuildCheck => "build-check",
            DirectHostCapabilityKind::RuntimeSession => "runtime-session",
            DirectHostCapabilityKind::ImmediateWindow => "immediate-window",
            DirectHostCapabilityKind::Debugger => "debugger",
            DirectHostCapabilityKind::WatchEvaluation => "watch-evaluation",
            DirectHostCapabilityKind::Breakpoints => "breakpoints",
            DirectHostCapabilityKind::ComReferenceDiscovery => "com-reference-discovery",
            DirectHostCapabilityKind::ComRuntimeInvocation => "com-runtime-invocation",
            DirectHostCapabilityKind::NativeService => "native-service",
            DirectHostCapabilityKind::SidecarService => "sidecar-service",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectHostIssueKind {
    BrowserUnsupported,
    BrowserResponseFamilyUnsupported,
    NonWindowsUnsupported,
    NativeServiceMissing,
    NativeServiceUnhealthy,
    HostPolicyDenied,
    WorkspaceInvalid,
    ProjectInvalid,
    DocumentNotFound,
    WorkspaceVersionStale,
    DocumentVersionStale,
    CompileOptionsInvalid,
    BuildFailed,
    RunTargetMissing,
    RuntimeStartupFailed,
    RuntimeSessionUnavailable,
    RuntimeBusy,
    RuntimeAlreadyRunning,
    RuntimeNotRunning,
    RuntimeStopped,
    ImmediateUnavailable,
    ImmediateSessionUnavailable,
    ImmediateEvaluationFailed,
    DebugUnavailable,
    DebugSessionUnavailable,
    NotPaused,
    WatchEvaluationFailed,
    BreakpointUnresolved,
    ComDiscoveryUnavailable,
    ComReferenceMissingOrBroken,
    ComRuntimeUnavailable,
    ComInvocationUnavailable,
    ComBitnessOrApartmentIncompatible,
}

impl DirectHostIssueKind {
    pub fn stable_code(self) -> &'static str {
        match self {
            DirectHostIssueKind::BrowserUnsupported => "DH-BROWSER-UNSUPPORTED",
            DirectHostIssueKind::BrowserResponseFamilyUnsupported => {
                "DH-BROWSER-RESPONSE-FAMILY-UNSUPPORTED"
            }
            DirectHostIssueKind::NonWindowsUnsupported => "DH-NONWINDOWS-UNSUPPORTED",
            DirectHostIssueKind::NativeServiceMissing => "DH-NATIVE-SERVICE-MISSING",
            DirectHostIssueKind::NativeServiceUnhealthy => "DH-NATIVE-SERVICE-UNHEALTHY",
            DirectHostIssueKind::HostPolicyDenied => "DH-HOST-POLICY-DENIED",
            DirectHostIssueKind::WorkspaceInvalid => "DH-WORKSPACE-INVALID",
            DirectHostIssueKind::ProjectInvalid => "DH-PROJECT-INVALID",
            DirectHostIssueKind::DocumentNotFound => "DH-DOCUMENT-NOT-FOUND",
            DirectHostIssueKind::WorkspaceVersionStale => "DH-WORKSPACE-VERSION-STALE",
            DirectHostIssueKind::DocumentVersionStale => "DH-DOCUMENT-VERSION-STALE",
            DirectHostIssueKind::CompileOptionsInvalid => "DH-COMPILE-OPTIONS-INVALID",
            DirectHostIssueKind::BuildFailed => "DH-BUILD-FAILED",
            DirectHostIssueKind::RunTargetMissing => "DH-RUN-TARGET-MISSING",
            DirectHostIssueKind::RuntimeStartupFailed => "DH-RUNTIME-STARTUP-FAILED",
            DirectHostIssueKind::RuntimeSessionUnavailable => "DH-RUNTIME-SESSION-UNAVAILABLE",
            DirectHostIssueKind::RuntimeBusy => "DH-RUNTIME-BUSY",
            DirectHostIssueKind::RuntimeAlreadyRunning => "DH-RUNTIME-ALREADY-RUNNING",
            DirectHostIssueKind::RuntimeNotRunning => "DH-RUNTIME-NOT-RUNNING",
            DirectHostIssueKind::RuntimeStopped => "DH-RUNTIME-STOPPED",
            DirectHostIssueKind::ImmediateUnavailable => "DH-IMMEDIATE-UNAVAILABLE",
            DirectHostIssueKind::ImmediateSessionUnavailable => "DH-IMMEDIATE-SESSION-UNAVAILABLE",
            DirectHostIssueKind::ImmediateEvaluationFailed => "DH-IMMEDIATE-EVALUATION-FAILED",
            DirectHostIssueKind::DebugUnavailable => "DH-DEBUG-UNAVAILABLE",
            DirectHostIssueKind::DebugSessionUnavailable => "DH-DEBUG-SESSION-UNAVAILABLE",
            DirectHostIssueKind::NotPaused => "DH-NOT-PAUSED",
            DirectHostIssueKind::WatchEvaluationFailed => "DH-WATCH-EVALUATION-FAILED",
            DirectHostIssueKind::BreakpointUnresolved => "DH-BREAKPOINT-UNRESOLVED",
            DirectHostIssueKind::ComDiscoveryUnavailable => "DH-COM-DISCOVERY-UNAVAILABLE",
            DirectHostIssueKind::ComReferenceMissingOrBroken => "DH-COM-REFERENCE-MISSING-BROKEN",
            DirectHostIssueKind::ComRuntimeUnavailable => "DH-COM-RUNTIME-UNAVAILABLE",
            DirectHostIssueKind::ComInvocationUnavailable => "DH-COM-INVOCATION-UNAVAILABLE",
            DirectHostIssueKind::ComBitnessOrApartmentIncompatible => {
                "DH-COM-BITNESS-APARTMENT-INCOMPATIBLE"
            }
        }
    }

    pub fn default_summary(self) -> &'static str {
        match self {
            DirectHostIssueKind::BrowserUnsupported => {
                "Capability is not available in browser profiles"
            }
            DirectHostIssueKind::BrowserResponseFamilyUnsupported => {
                "Response family is not available in browser profiles"
            }
            DirectHostIssueKind::NonWindowsUnsupported => "Capability is only available on Windows",
            DirectHostIssueKind::NativeServiceMissing => "Native service is not available",
            DirectHostIssueKind::NativeServiceUnhealthy => "Native service is unhealthy",
            DirectHostIssueKind::HostPolicyDenied => "Host policy denied the operation",
            DirectHostIssueKind::WorkspaceInvalid => "Workspace is invalid",
            DirectHostIssueKind::ProjectInvalid => "Project is invalid",
            DirectHostIssueKind::DocumentNotFound => "Document is not part of the loaded workspace",
            DirectHostIssueKind::WorkspaceVersionStale => {
                "Workspace revision is stale for the requested operation"
            }
            DirectHostIssueKind::DocumentVersionStale => {
                "Document version is stale for the requested operation"
            }
            DirectHostIssueKind::CompileOptionsInvalid => "Compile options are invalid",
            DirectHostIssueKind::BuildFailed => "Build failed",
            DirectHostIssueKind::RunTargetMissing => "Run target is missing",
            DirectHostIssueKind::RuntimeStartupFailed => "Runtime startup failed",
            DirectHostIssueKind::RuntimeSessionUnavailable => "Runtime session is unavailable",
            DirectHostIssueKind::RuntimeBusy => "Runtime is busy",
            DirectHostIssueKind::RuntimeAlreadyRunning => "Runtime is already running",
            DirectHostIssueKind::RuntimeNotRunning => "Runtime is not running",
            DirectHostIssueKind::RuntimeStopped => "Runtime is stopped",
            DirectHostIssueKind::ImmediateUnavailable => "Immediate Window is unavailable",
            DirectHostIssueKind::ImmediateSessionUnavailable => "Immediate session is unavailable",
            DirectHostIssueKind::ImmediateEvaluationFailed => "Immediate evaluation failed",
            DirectHostIssueKind::DebugUnavailable => "Debugger is unavailable",
            DirectHostIssueKind::DebugSessionUnavailable => "Debug session is unavailable",
            DirectHostIssueKind::NotPaused => "Debug session is not paused",
            DirectHostIssueKind::WatchEvaluationFailed => "Watch evaluation failed",
            DirectHostIssueKind::BreakpointUnresolved => "Breakpoint is unresolved",
            DirectHostIssueKind::ComDiscoveryUnavailable => "COM discovery is unavailable",
            DirectHostIssueKind::ComReferenceMissingOrBroken => {
                "COM reference is missing or broken"
            }
            DirectHostIssueKind::ComRuntimeUnavailable => "COM runtime invocation is unavailable",
            DirectHostIssueKind::ComInvocationUnavailable => "COM invocation is unavailable",
            DirectHostIssueKind::ComBitnessOrApartmentIncompatible => {
                "COM bitness or apartment model is incompatible"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectHostTextPosition {
    pub line: u32,
    pub character: u32,
}

impl DirectHostTextPosition {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectHostSourceSpan {
    pub document_id: DirectHostDocumentId,
    pub start: DirectHostTextPosition,
    pub end: DirectHostTextPosition,
}

impl DirectHostSourceSpan {
    pub fn new(
        document_id: impl Into<DirectHostDocumentId>,
        start: DirectHostTextPosition,
        end: DirectHostTextPosition,
    ) -> Self {
        Self {
            document_id: document_id.into(),
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectHostSourceUnavailableReason {
    NoSourceLocation,
    NoMatchingDocument,
    SourceMapUnavailable,
    GeneratedOrSyntheticSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectHostSourceSpanStatus {
    Known(DirectHostSourceSpan),
    Unavailable(DirectHostSourceUnavailableReason),
}

impl DirectHostSourceSpanStatus {
    pub fn known(span: DirectHostSourceSpan) -> Self {
        Self::Known(span)
    }

    pub fn unavailable(reason: DirectHostSourceUnavailableReason) -> Self {
        Self::Unavailable(reason)
    }

    pub fn source_span(&self) -> Option<&DirectHostSourceSpan> {
        match self {
            Self::Known(span) => Some(span),
            Self::Unavailable(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectHostDiagnostic {
    pub issue: DirectHostIssue,
    pub source: DirectHostSourceSpanStatus,
}

impl DirectHostDiagnostic {
    pub fn new(issue: DirectHostIssue, source: DirectHostSourceSpanStatus) -> Self {
        Self { issue, source }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectHostIssueContext {
    pub workspace_id: Option<DirectHostWorkspaceId>,
    pub project_id: Option<DirectHostProjectId>,
    pub document_id: Option<DirectHostDocumentId>,
    pub build_request_id: Option<DirectHostBuildRequestId>,
    pub runtime_session_id: Option<DirectHostRuntimeSessionId>,
    pub immediate_session_id: Option<DirectHostImmediateSessionId>,
    pub debug_session_id: Option<DirectHostDebugSessionId>,
    pub breakpoint_id: Option<DirectHostBreakpointId>,
    pub stack_frame_id: Option<DirectHostStackFrameId>,
    pub watch_id: Option<DirectHostWatchId>,
    pub source_span: Option<DirectHostSourceSpan>,
    pub expected_workspace_revision: Option<u64>,
    pub actual_workspace_revision: Option<u64>,
    pub expected_document_version: Option<u64>,
    pub actual_document_version: Option<u64>,
    pub response_family: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectHostIssue {
    pub kind: DirectHostIssueKind,
    pub stable_code: &'static str,
    pub summary: String,
    pub technical_detail: Option<String>,
    pub retryability: DirectHostRetryability,
    pub context: DirectHostIssueContext,
}

impl DirectHostIssue {
    pub fn new(kind: DirectHostIssueKind) -> Self {
        Self {
            kind,
            stable_code: kind.stable_code(),
            summary: kind.default_summary().to_string(),
            technical_detail: None,
            retryability: DirectHostRetryability::Unknown,
            context: DirectHostIssueContext::default(),
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn with_technical_detail(mut self, detail: impl Into<String>) -> Self {
        self.technical_detail = Some(detail.into());
        self
    }

    pub fn with_retryability(mut self, retryability: DirectHostRetryability) -> Self {
        self.retryability = retryability;
        self
    }

    pub fn with_context(mut self, context: DirectHostIssueContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.context.path = Some(path.into());
        self
    }

    pub fn with_workspace_id(mut self, workspace_id: impl Into<DirectHostWorkspaceId>) -> Self {
        self.context.workspace_id = Some(workspace_id.into());
        self
    }

    pub fn with_project_id(mut self, project_id: impl Into<DirectHostProjectId>) -> Self {
        self.context.project_id = Some(project_id.into());
        self
    }

    pub fn with_document_id(mut self, document_id: impl Into<DirectHostDocumentId>) -> Self {
        self.context.document_id = Some(document_id.into());
        self
    }

    pub fn with_runtime_session_id(
        mut self,
        runtime_session_id: impl Into<DirectHostRuntimeSessionId>,
    ) -> Self {
        self.context.runtime_session_id = Some(runtime_session_id.into());
        self
    }

    pub fn with_debug_session_id(
        mut self,
        debug_session_id: impl Into<DirectHostDebugSessionId>,
    ) -> Self {
        self.context.debug_session_id = Some(debug_session_id.into());
        self
    }

    pub fn with_immediate_session_id(
        mut self,
        immediate_session_id: impl Into<DirectHostImmediateSessionId>,
    ) -> Self {
        self.context.immediate_session_id = Some(immediate_session_id.into());
        self
    }

    pub fn with_breakpoint_id(mut self, breakpoint_id: impl Into<DirectHostBreakpointId>) -> Self {
        self.context.breakpoint_id = Some(breakpoint_id.into());
        self
    }

    pub fn with_stack_frame_id(
        mut self,
        stack_frame_id: impl Into<DirectHostStackFrameId>,
    ) -> Self {
        self.context.stack_frame_id = Some(stack_frame_id.into());
        self
    }

    pub fn with_watch_id(mut self, watch_id: impl Into<DirectHostWatchId>) -> Self {
        self.context.watch_id = Some(watch_id.into());
        self
    }

    pub fn with_workspace_revision(mut self, expected: u64, actual: u64) -> Self {
        self.context.expected_workspace_revision = Some(expected);
        self.context.actual_workspace_revision = Some(actual);
        self
    }

    pub fn with_document_version(mut self, expected: u64, actual: u64) -> Self {
        self.context.expected_document_version = Some(expected);
        self.context.actual_document_version = Some(actual);
        self
    }

    pub fn with_response_family(mut self, response_family: impl Into<String>) -> Self {
        self.context.response_family = Some(response_family.into());
        self
    }

    pub fn stale_workspace_version(expected: u64, actual: u64) -> Self {
        Self::new(DirectHostIssueKind::WorkspaceVersionStale)
            .with_retryability(DirectHostRetryability::Retryable)
            .with_workspace_revision(expected, actual)
    }

    pub fn stale_document_version(
        document_id: impl Into<DirectHostDocumentId>,
        expected: u64,
        actual: u64,
    ) -> Self {
        Self::new(DirectHostIssueKind::DocumentVersionStale)
            .with_retryability(DirectHostRetryability::Retryable)
            .with_document_id(document_id)
            .with_document_version(expected, actual)
    }

    pub fn browser_response_family_unsupported(response_family: impl Into<String>) -> Self {
        Self::new(DirectHostIssueKind::BrowserResponseFamilyUnsupported)
            .with_retryability(DirectHostRetryability::NotRetryable)
            .with_response_family(response_family)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum DirectHostCommandStatus {
    Available,
    Disabled { reason: DirectHostIssue },
}

impl DirectHostCommandStatus {
    pub fn available() -> Self {
        Self::Available
    }

    pub fn disabled(reason: DirectHostIssue) -> Self {
        Self::Disabled { reason }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn disabled_reason(&self) -> Option<&DirectHostIssue> {
        match self {
            Self::Available => None,
            Self::Disabled { reason } => Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectHostCapabilityStatus {
    Available,
    Unavailable { reason: DirectHostIssue },
    Degraded { reason: DirectHostIssue },
}

impl DirectHostCapabilityStatus {
    pub fn available() -> Self {
        Self::Available
    }

    pub fn unavailable(reason: DirectHostIssue) -> Self {
        Self::Unavailable { reason }
    }

    pub fn degraded(reason: DirectHostIssue) -> Self {
        Self::Degraded { reason }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectHostCapability {
    pub kind: DirectHostCapabilityKind,
    pub status: DirectHostCapabilityStatus,
}

impl DirectHostCapability {
    pub fn available(kind: DirectHostCapabilityKind) -> Self {
        Self {
            kind,
            status: DirectHostCapabilityStatus::Available,
        }
    }

    pub fn unavailable(kind: DirectHostCapabilityKind, reason: DirectHostIssue) -> Self {
        Self {
            kind,
            status: DirectHostCapabilityStatus::Unavailable { reason },
        }
    }

    pub fn degraded(kind: DirectHostCapabilityKind, reason: DirectHostIssue) -> Self {
        Self {
            kind,
            status: DirectHostCapabilityStatus::Degraded { reason },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DirectHostCapability, DirectHostCapabilityKind, DirectHostCommandStatus,
        DirectHostDebugSessionId, DirectHostDocumentId, DirectHostImmediateSessionId,
        DirectHostIssue, DirectHostIssueKind, DirectHostModuleId, DirectHostRetryability,
        DirectHostRuntimeSessionId,
    };

    #[test]
    fn direct_host_issue_codes_are_stable() {
        let expected = [
            (
                DirectHostIssueKind::BrowserUnsupported,
                "DH-BROWSER-UNSUPPORTED",
            ),
            (
                DirectHostIssueKind::BrowserResponseFamilyUnsupported,
                "DH-BROWSER-RESPONSE-FAMILY-UNSUPPORTED",
            ),
            (
                DirectHostIssueKind::NonWindowsUnsupported,
                "DH-NONWINDOWS-UNSUPPORTED",
            ),
            (
                DirectHostIssueKind::NativeServiceMissing,
                "DH-NATIVE-SERVICE-MISSING",
            ),
            (
                DirectHostIssueKind::NativeServiceUnhealthy,
                "DH-NATIVE-SERVICE-UNHEALTHY",
            ),
            (
                DirectHostIssueKind::HostPolicyDenied,
                "DH-HOST-POLICY-DENIED",
            ),
            (
                DirectHostIssueKind::WorkspaceInvalid,
                "DH-WORKSPACE-INVALID",
            ),
            (DirectHostIssueKind::ProjectInvalid, "DH-PROJECT-INVALID"),
            (
                DirectHostIssueKind::DocumentNotFound,
                "DH-DOCUMENT-NOT-FOUND",
            ),
            (
                DirectHostIssueKind::WorkspaceVersionStale,
                "DH-WORKSPACE-VERSION-STALE",
            ),
            (
                DirectHostIssueKind::DocumentVersionStale,
                "DH-DOCUMENT-VERSION-STALE",
            ),
            (
                DirectHostIssueKind::CompileOptionsInvalid,
                "DH-COMPILE-OPTIONS-INVALID",
            ),
            (DirectHostIssueKind::BuildFailed, "DH-BUILD-FAILED"),
            (
                DirectHostIssueKind::RunTargetMissing,
                "DH-RUN-TARGET-MISSING",
            ),
            (
                DirectHostIssueKind::RuntimeStartupFailed,
                "DH-RUNTIME-STARTUP-FAILED",
            ),
            (
                DirectHostIssueKind::RuntimeSessionUnavailable,
                "DH-RUNTIME-SESSION-UNAVAILABLE",
            ),
            (DirectHostIssueKind::RuntimeBusy, "DH-RUNTIME-BUSY"),
            (
                DirectHostIssueKind::RuntimeAlreadyRunning,
                "DH-RUNTIME-ALREADY-RUNNING",
            ),
            (
                DirectHostIssueKind::RuntimeNotRunning,
                "DH-RUNTIME-NOT-RUNNING",
            ),
            (DirectHostIssueKind::RuntimeStopped, "DH-RUNTIME-STOPPED"),
            (
                DirectHostIssueKind::ImmediateUnavailable,
                "DH-IMMEDIATE-UNAVAILABLE",
            ),
            (
                DirectHostIssueKind::ImmediateSessionUnavailable,
                "DH-IMMEDIATE-SESSION-UNAVAILABLE",
            ),
            (
                DirectHostIssueKind::ImmediateEvaluationFailed,
                "DH-IMMEDIATE-EVALUATION-FAILED",
            ),
            (
                DirectHostIssueKind::DebugUnavailable,
                "DH-DEBUG-UNAVAILABLE",
            ),
            (
                DirectHostIssueKind::DebugSessionUnavailable,
                "DH-DEBUG-SESSION-UNAVAILABLE",
            ),
            (DirectHostIssueKind::NotPaused, "DH-NOT-PAUSED"),
            (
                DirectHostIssueKind::WatchEvaluationFailed,
                "DH-WATCH-EVALUATION-FAILED",
            ),
            (
                DirectHostIssueKind::BreakpointUnresolved,
                "DH-BREAKPOINT-UNRESOLVED",
            ),
            (
                DirectHostIssueKind::ComDiscoveryUnavailable,
                "DH-COM-DISCOVERY-UNAVAILABLE",
            ),
            (
                DirectHostIssueKind::ComReferenceMissingOrBroken,
                "DH-COM-REFERENCE-MISSING-BROKEN",
            ),
            (
                DirectHostIssueKind::ComRuntimeUnavailable,
                "DH-COM-RUNTIME-UNAVAILABLE",
            ),
            (
                DirectHostIssueKind::ComInvocationUnavailable,
                "DH-COM-INVOCATION-UNAVAILABLE",
            ),
            (
                DirectHostIssueKind::ComBitnessOrApartmentIncompatible,
                "DH-COM-BITNESS-APARTMENT-INCOMPATIBLE",
            ),
        ];

        for (kind, code) in expected {
            assert_eq!(kind.stable_code(), code);
            assert_eq!(DirectHostIssue::new(kind).stable_code, code);
        }
    }

    #[test]
    fn direct_host_disabled_command_preserves_typed_reason_and_context() {
        let reason = DirectHostIssue::new(DirectHostIssueKind::NotPaused)
            .with_retryability(DirectHostRetryability::Retryable)
            .with_debug_session_id(DirectHostDebugSessionId::new("debug:1"))
            .with_document_id(DirectHostDocumentId::new("Module1"));
        let status = DirectHostCommandStatus::disabled(reason.clone());
        let module_id = DirectHostModuleId::new("project:App::Module1");

        assert!(!status.is_available());
        let disabled = status.disabled_reason().expect("disabled reason");
        assert_eq!(disabled.kind, DirectHostIssueKind::NotPaused);
        assert_eq!(disabled.stable_code, "DH-NOT-PAUSED");
        assert_eq!(disabled.retryability, DirectHostRetryability::Retryable);
        assert_eq!(
            disabled.context.debug_session_id.as_ref().unwrap().as_str(),
            "debug:1"
        );
        assert_eq!(
            disabled.context.document_id.as_ref().unwrap().as_str(),
            "Module1"
        );
        assert_eq!(module_id.as_str(), "project:App::Module1");
    }

    #[test]
    fn direct_host_capability_exposes_stable_kind_codes() {
        let capability = DirectHostCapability::unavailable(
            DirectHostCapabilityKind::ComRuntimeInvocation,
            DirectHostIssue::new(DirectHostIssueKind::ComInvocationUnavailable),
        );
        assert_eq!(capability.kind.code(), "com-runtime-invocation");
    }

    #[test]
    fn direct_host_command_status_classifies_runtime_and_no_session_states() {
        for (kind, code) in [
            (DirectHostIssueKind::RuntimeBusy, "DH-RUNTIME-BUSY"),
            (
                DirectHostIssueKind::RuntimeAlreadyRunning,
                "DH-RUNTIME-ALREADY-RUNNING",
            ),
            (
                DirectHostIssueKind::RuntimeNotRunning,
                "DH-RUNTIME-NOT-RUNNING",
            ),
            (DirectHostIssueKind::RuntimeStopped, "DH-RUNTIME-STOPPED"),
        ] {
            let status = DirectHostCommandStatus::disabled(
                DirectHostIssue::new(kind)
                    .with_runtime_session_id(DirectHostRuntimeSessionId::new("runtime:1")),
            );
            let reason = status.disabled_reason().expect("disabled reason");
            assert_eq!(reason.kind, kind);
            assert_eq!(reason.stable_code, code);
            assert_eq!(
                reason.context.runtime_session_id.as_ref().unwrap().as_str(),
                "runtime:1"
            );
        }

        let immediate = DirectHostCommandStatus::disabled(
            DirectHostIssue::new(DirectHostIssueKind::ImmediateSessionUnavailable)
                .with_immediate_session_id(DirectHostImmediateSessionId::new("immediate:missing")),
        );
        assert_eq!(
            immediate.disabled_reason().unwrap().stable_code,
            "DH-IMMEDIATE-SESSION-UNAVAILABLE"
        );

        let debug = DirectHostCommandStatus::disabled(
            DirectHostIssue::new(DirectHostIssueKind::DebugSessionUnavailable)
                .with_debug_session_id(DirectHostDebugSessionId::new("debug:missing")),
        );
        assert_eq!(
            debug.disabled_reason().unwrap().stable_code,
            "DH-DEBUG-SESSION-UNAVAILABLE"
        );
    }

    #[test]
    fn direct_host_taxonomy_carries_stale_version_context() {
        let workspace = DirectHostIssue::stale_workspace_version(10, 12);
        assert_eq!(workspace.kind, DirectHostIssueKind::WorkspaceVersionStale);
        assert_eq!(workspace.retryability, DirectHostRetryability::Retryable);
        assert_eq!(workspace.context.expected_workspace_revision, Some(10));
        assert_eq!(workspace.context.actual_workspace_revision, Some(12));

        let document = DirectHostIssue::stale_document_version("Module1", 4, 5);
        assert_eq!(document.kind, DirectHostIssueKind::DocumentVersionStale);
        assert_eq!(
            document.context.document_id.as_ref().unwrap().as_str(),
            "Module1"
        );
        assert_eq!(document.context.expected_document_version, Some(4));
        assert_eq!(document.context.actual_document_version, Some(5));
    }

    #[test]
    fn direct_host_taxonomy_classifies_browser_response_family() {
        let issue = DirectHostIssue::browser_response_family_unsupported("debug");
        assert_eq!(
            issue.kind,
            DirectHostIssueKind::BrowserResponseFamilyUnsupported
        );
        assert_eq!(issue.stable_code, "DH-BROWSER-RESPONSE-FAMILY-UNSUPPORTED");
        assert_eq!(issue.retryability, DirectHostRetryability::NotRetryable);
        assert_eq!(issue.context.response_family.as_deref(), Some("debug"));
    }
}
