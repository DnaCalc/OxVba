use oxvba_host::{
    DebugFrame, DebugFrameValue, DebugFrameValueKind, DebugPauseState, ImmediateDisplayStyle,
    ImmediateEvaluationOutput, ImmediateEvaluationRequest, ImmediateInputKind,
    ImmediateValueProjection,
};
use oxvba_languageservice::{
    DiagnosticSeverity, HostWorkspaceDocument, SemanticProvenance, SpannedDiagnostic,
    SymbolProvenanceKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebHostCommand {
    LoadWorkspace { path: String },
    ReloadWorkspace,
    ListDocuments,
    SetDocumentText { document_id: String, source: String },
    CloseDocument { document_id: String },
    RunProject,
    ResetRuntime,
    ImmediateEvaluate(WebImmediateRequest),
    DebugStart,
    DebugContinue,
    DebugStepInto,
    DebugStepOver,
    DebugStepOut,
    DebugEvaluate { expression_text: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebHostEvent {
    WorkspaceLoaded(WebWorkspaceSummary),
    DiagnosticsUpdated {
        document_id: String,
        diagnostics: Vec<WebDiagnostic>,
    },
    OutputLine {
        stream: WebOutputStream,
        text: String,
    },
    ImmediateResult(WebImmediateResult),
    DebugPaused(WebDebugPauseState),
    RunStateChanged(WebRunState),
    Error {
        operation: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebRunState {
    Idle,
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebWorkspaceSummary {
    pub workspace_target: String,
    pub documents: Vec<WebWorkspaceDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebWorkspaceDocument {
    pub document_id: String,
    pub project_name: Option<String>,
    pub provenance_kind: WebProvenanceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebProvenanceKind {
    SourceModule,
    ProjectReference,
    ImportedTypeLibraryProjection,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebDiagnostic {
    pub start: u32,
    pub end: u32,
    pub severity: WebDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebImmediateRequest {
    pub source_text: String,
    pub kind: WebImmediateInputKind,
    pub display_style: WebImmediateDisplayStyle,
    pub target_module: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebImmediateInputKind {
    Auto,
    Expression,
    Statement,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebImmediateDisplayStyle {
    ImmediateWindow,
    PrintLike,
    ValueOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebImmediateResult {
    pub output: WebImmediateOutput,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebImmediateOutput {
    Empty,
    Value { display_text: String },
    PrintedLine { text: String },
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebDebugPauseState {
    pub stop_kind: String,
    pub frames: Vec<WebDebugFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebDebugFrame {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
    pub values: Vec<WebDebugValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebDebugValue {
    pub name: String,
    pub slot: usize,
    pub kind: WebDebugValueKind,
    pub display_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebDebugValueKind {
    Parameter,
    Local,
    ReturnValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSemanticProvenance {
    pub project_name: Option<String>,
    pub document_id: String,
    pub snapshot_version: u64,
    pub kind: WebProvenanceKind,
}

pub fn project_workspace_loaded(
    workspace_target: impl Into<String>,
    documents: &[HostWorkspaceDocument],
) -> WebHostEvent {
    WebHostEvent::WorkspaceLoaded(WebWorkspaceSummary {
        workspace_target: workspace_target.into(),
        documents: documents
            .iter()
            .cloned()
            .map(WebWorkspaceDocument::from)
            .collect(),
    })
}

impl From<HostWorkspaceDocument> for WebWorkspaceDocument {
    fn from(value: HostWorkspaceDocument) -> Self {
        Self {
            document_id: value.id.0,
            project_name: value.project_name,
            provenance_kind: value.provenance_kind.into(),
        }
    }
}

impl From<SymbolProvenanceKind> for WebProvenanceKind {
    fn from(value: SymbolProvenanceKind) -> Self {
        match value {
            SymbolProvenanceKind::SourceModule => Self::SourceModule,
            SymbolProvenanceKind::ProjectReference => Self::ProjectReference,
            SymbolProvenanceKind::ImportedTypeLibraryProjection => {
                Self::ImportedTypeLibraryProjection
            }
            SymbolProvenanceKind::Generated => Self::Generated,
        }
    }
}

impl From<DiagnosticSeverity> for WebDiagnosticSeverity {
    fn from(value: DiagnosticSeverity) -> Self {
        match value {
            DiagnosticSeverity::Error => Self::Error,
            DiagnosticSeverity::Warning => Self::Warning,
        }
    }
}

impl From<SpannedDiagnostic> for WebDiagnostic {
    fn from(value: SpannedDiagnostic) -> Self {
        Self {
            start: value.span.start,
            end: value.span.end,
            severity: value.severity.into(),
            message: value.message,
        }
    }
}

impl From<ImmediateInputKind> for WebImmediateInputKind {
    fn from(value: ImmediateInputKind) -> Self {
        match value {
            ImmediateInputKind::Auto => Self::Auto,
            ImmediateInputKind::Expression => Self::Expression,
            ImmediateInputKind::Statement => Self::Statement,
            ImmediateInputKind::Query => Self::Query,
        }
    }
}

impl From<WebImmediateInputKind> for ImmediateInputKind {
    fn from(value: WebImmediateInputKind) -> Self {
        match value {
            WebImmediateInputKind::Auto => Self::Auto,
            WebImmediateInputKind::Expression => Self::Expression,
            WebImmediateInputKind::Statement => Self::Statement,
            WebImmediateInputKind::Query => Self::Query,
        }
    }
}

impl From<ImmediateDisplayStyle> for WebImmediateDisplayStyle {
    fn from(value: ImmediateDisplayStyle) -> Self {
        match value {
            ImmediateDisplayStyle::ImmediateWindow => Self::ImmediateWindow,
            ImmediateDisplayStyle::PrintLike => Self::PrintLike,
            ImmediateDisplayStyle::ValueOnly => Self::ValueOnly,
        }
    }
}

impl From<WebImmediateDisplayStyle> for ImmediateDisplayStyle {
    fn from(value: WebImmediateDisplayStyle) -> Self {
        match value {
            WebImmediateDisplayStyle::ImmediateWindow => Self::ImmediateWindow,
            WebImmediateDisplayStyle::PrintLike => Self::PrintLike,
            WebImmediateDisplayStyle::ValueOnly => Self::ValueOnly,
        }
    }
}

impl From<WebImmediateRequest> for ImmediateEvaluationRequest {
    fn from(value: WebImmediateRequest) -> Self {
        Self {
            kind: value.kind.into(),
            source_text: value.source_text,
            target_module: value.target_module,
            display_style: value.display_style.into(),
        }
    }
}

impl From<ImmediateEvaluationRequest> for WebImmediateRequest {
    fn from(value: ImmediateEvaluationRequest) -> Self {
        Self {
            source_text: value.source_text,
            kind: value.kind.into(),
            display_style: value.display_style.into(),
            target_module: value.target_module,
        }
    }
}

impl From<ImmediateValueProjection> for WebImmediateOutput {
    fn from(value: ImmediateValueProjection) -> Self {
        Self::Value {
            display_text: value.display_text,
        }
    }
}

pub fn project_immediate_result(
    output: &ImmediateEvaluationOutput,
    diagnostics: &[impl ToString],
) -> WebImmediateResult {
    let output = match output {
        ImmediateEvaluationOutput::Empty => WebImmediateOutput::Empty,
        ImmediateEvaluationOutput::Value(value) => value.clone().into(),
        ImmediateEvaluationOutput::PrintedLine(line) => {
            WebImmediateOutput::PrintedLine { text: line.clone() }
        }
        ImmediateEvaluationOutput::Reset => WebImmediateOutput::Reset,
    };
    WebImmediateResult {
        output,
        diagnostics: diagnostics.iter().map(ToString::to_string).collect(),
    }
}

pub fn project_debug_pause_state(value: &DebugPauseState) -> WebDebugPauseState {
    WebDebugPauseState {
        stop_kind: format!("{:?}", value.stop),
        frames: value.frames.iter().map(project_debug_frame).collect(),
    }
}

fn project_debug_frame(value: &DebugFrame) -> WebDebugFrame {
    WebDebugFrame {
        module_name: value.module_name.clone(),
        procedure_name: value.procedure_name.clone(),
        entry_pc: value.entry_pc,
        source_line_start: value.source_line_start,
        source_line_end: value.source_line_end,
        values: value.values.iter().map(project_debug_value).collect(),
    }
}

fn project_debug_value(value: &DebugFrameValue) -> WebDebugValue {
    WebDebugValue {
        name: value.name.clone(),
        slot: value.slot,
        kind: match value.kind {
            DebugFrameValueKind::Parameter => WebDebugValueKind::Parameter,
            DebugFrameValueKind::Local => WebDebugValueKind::Local,
            DebugFrameValueKind::ReturnValue => WebDebugValueKind::ReturnValue,
        },
        display_text: value.display_text.clone(),
    }
}

impl From<SemanticProvenance> for WebSemanticProvenance {
    fn from(value: SemanticProvenance) -> Self {
        Self {
            project_name: value.project_name,
            document_id: value.document_id,
            snapshot_version: value.snapshot_version,
            kind: value.kind.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use oxvba_host::{
        DebugFrame, DebugFrameValue, DebugFrameValueKind, DebugPauseState,
        ImmediateEvaluationOutput,
    };
    use oxvba_languageservice::{
        DiagnosticSeverity, DocumentId, HostWorkspaceDocument, SpannedDiagnostic, TextSpan,
    };
    use oxvba_vm::{DebugSourceLocation, DebugStop, DebugStopReason};

    use super::{
        WebDiagnosticSeverity, WebHostCommand, WebHostEvent, WebImmediateDisplayStyle,
        WebImmediateInputKind, WebImmediateOutput, WebProvenanceKind, WebRunState,
        project_debug_pause_state, project_immediate_result, project_workspace_loaded,
    };

    #[test]
    fn workspace_loaded_event_projects_documents() {
        let docs = vec![HostWorkspaceDocument {
            id: DocumentId("Module1.bas".to_string()),
            project_name: Some("App".to_string()),
            provenance_kind: oxvba_languageservice::SymbolProvenanceKind::SourceModule,
        }];

        let event = project_workspace_loaded("C:\\temp\\App", &docs);
        let json = serde_json::to_string(&event).expect("serialize event");
        let restored: WebHostEvent = serde_json::from_str(&json).expect("deserialize event");

        match restored {
            WebHostEvent::WorkspaceLoaded(summary) => {
                assert_eq!(summary.workspace_target, "C:\\temp\\App");
                assert_eq!(summary.documents.len(), 1);
                assert_eq!(summary.documents[0].document_id, "Module1.bas");
                assert_eq!(
                    summary.documents[0].provenance_kind,
                    WebProvenanceKind::SourceModule
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn immediate_request_round_trips_through_contract() {
        let command = WebHostCommand::ImmediateEvaluate(super::WebImmediateRequest {
            source_text: "? Foo(1)".to_string(),
            kind: WebImmediateInputKind::Query,
            display_style: WebImmediateDisplayStyle::ImmediateWindow,
            target_module: Some("Module1".to_string()),
        });

        let json = serde_json::to_string(&command).expect("serialize");
        let restored: WebHostCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, command);
    }

    #[test]
    fn immediate_result_projects_display_text() {
        let result = project_immediate_result(
            &ImmediateEvaluationOutput::PrintedLine("42".to_string()),
            &["note"],
        );
        assert_eq!(
            result.output,
            WebImmediateOutput::PrintedLine {
                text: "42".to_string()
            }
        );
        assert_eq!(result.diagnostics, vec!["note".to_string()]);
    }

    #[test]
    fn debug_pause_projection_projects_frame_values() {
        let pause = DebugPauseState {
            stop: DebugStop {
                reason: DebugStopReason::Step,
                location: DebugSourceLocation {
                    module_name: "Module1".to_string(),
                    procedure_name: "Main".to_string(),
                    entry_pc: 4,
                    statement_pc: 7,
                    line_number: Some(12),
                },
                call_stack_depth: 1,
            },
            frames: vec![DebugFrame {
                module_name: "Module1".to_string(),
                procedure_name: "Main".to_string(),
                entry_pc: 4,
                source_line_start: 10,
                source_line_end: 20,
                values: vec![DebugFrameValue {
                    name: "x".to_string(),
                    slot: 1,
                    kind: DebugFrameValueKind::Local,
                    runtime_value: oxvba_runtime::RuntimeValue::I32(42),
                    display_text: "42".to_string(),
                }],
            }],
        };

        let projected = project_debug_pause_state(&pause);
        assert_eq!(projected.frames.len(), 1);
        assert_eq!(projected.frames[0].values.len(), 1);
        assert_eq!(projected.frames[0].values[0].display_text, "42");
    }

    #[test]
    fn spanned_diagnostic_projects_to_web_shape() {
        let diagnostic = super::WebDiagnostic::from(SpannedDiagnostic {
            span: TextSpan::new(3, 8),
            message: "bad".to_string(),
            severity: DiagnosticSeverity::Warning,
        });

        assert_eq!(diagnostic.start, 3);
        assert_eq!(diagnostic.end, 8);
        assert_eq!(diagnostic.severity, WebDiagnosticSeverity::Warning);
    }

    #[test]
    fn run_state_is_serializable() {
        let json = serde_json::to_string(&WebRunState::Paused).expect("serialize");
        let restored: WebRunState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, WebRunState::Paused);
    }
}
