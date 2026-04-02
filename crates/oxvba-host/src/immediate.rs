use oxvba_compiler::ProjectManifest;
use oxvba_runtime::RuntimeValue;
use thiserror::Error;

use crate::engine::PhaseDiagnostic;
use crate::{Engine, ProjectRuntimeSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmediateInputKind {
    Auto,
    Expression,
    Statement,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmediateDisplayStyle {
    ImmediateWindow,
    PrintLike,
    ValueOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmediateEvaluationRequest {
    pub kind: ImmediateInputKind,
    pub source_text: String,
    pub target_module: Option<String>,
    pub display_style: ImmediateDisplayStyle,
}

impl ImmediateEvaluationRequest {
    pub fn new(source_text: impl Into<String>) -> Self {
        Self {
            kind: ImmediateInputKind::Auto,
            source_text: source_text.into(),
            target_module: None,
            display_style: ImmediateDisplayStyle::ImmediateWindow,
        }
    }

    pub fn expression(source_text: impl Into<String>) -> Self {
        let mut request = Self::new(source_text);
        request.kind = ImmediateInputKind::Expression;
        request
    }

    pub fn statement(source_text: impl Into<String>) -> Self {
        let mut request = Self::new(source_text);
        request.kind = ImmediateInputKind::Statement;
        request
    }

    pub fn query(source_text: impl Into<String>) -> Self {
        let mut request = Self::new(source_text);
        request.kind = ImmediateInputKind::Query;
        request
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmediateResetKind {
    ClearSessionState,
    ReloadProject,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ImmediateSessionError {
    #[error(transparent)]
    Phase(PhaseDiagnostic),
    #[error("unknown default target module `{module}`")]
    UnknownTargetModule { module: String },
}

pub struct ImmediateSession<'engine> {
    engine: &'engine Engine,
    runtime: ProjectRuntimeSession,
    default_target_module: Option<String>,
}

impl<'engine> ImmediateSession<'engine> {
    pub fn new(engine: &'engine Engine, runtime: ProjectRuntimeSession) -> Self {
        Self {
            engine,
            runtime,
            default_target_module: None,
        }
    }

    pub fn engine(&self) -> &'engine Engine {
        self.engine
    }

    pub fn runtime(&self) -> &ProjectRuntimeSession {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut ProjectRuntimeSession {
        &mut self.runtime
    }

    pub fn default_target_module(&self) -> Option<&str> {
        self.default_target_module.as_deref()
    }

    pub fn set_default_target_module(&mut self, module: Option<impl Into<String>>) {
        self.default_target_module = module.map(|value| value.into());
    }

    pub fn snapshot(&self) -> Vec<RuntimeValue> {
        self.runtime.snapshot()
    }
}

impl Engine {
    pub fn prepare_immediate_session(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<ImmediateSession<'_>, PhaseDiagnostic> {
        let runtime = self.compile_and_prepare_session(manifest)?;
        Ok(ImmediateSession::new(self, runtime))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};

    use super::{
        ImmediateDisplayStyle, ImmediateEvaluationRequest, ImmediateInputKind, ImmediateSession,
    };
    use crate::{Engine, HostConfig};

    fn make_manifest(source: &str) -> ProjectManifest {
        ProjectManifest {
            project_name: "ImmediateHost".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                source,
            )
            .expect("module unit")],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        }
    }

    #[test]
    fn immediate_request_builders_preserve_bounded_defaults() {
        let expression = ImmediateEvaluationRequest::expression("? answer");
        assert_eq!(expression.kind, ImmediateInputKind::Expression);
        assert_eq!(
            expression.display_style,
            ImmediateDisplayStyle::ImmediateWindow
        );

        let statement = ImmediateEvaluationRequest::statement("x = 1");
        assert_eq!(statement.kind, ImmediateInputKind::Statement);

        let query = ImmediateEvaluationRequest::query("answer");
        assert_eq!(query.kind, ImmediateInputKind::Query);
    }

    #[test]
    fn prepare_immediate_session_wraps_live_runtime_session() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Sub Main()
End Sub
"#,
        );

        let session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");

        assert!(!session.runtime().procedure_metadata().is_empty());
        assert!(session.default_target_module().is_none());
    }

    #[test]
    fn immediate_session_tracks_default_target_module() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Sub Main()
End Sub
"#,
        );

        let runtime = engine
            .compile_and_prepare_session(&manifest)
            .expect("runtime session");
        let mut session = ImmediateSession::new(&engine, runtime);

        session.set_default_target_module(Some("Module1"));
        assert_eq!(session.default_target_module(), Some("Module1"));

        session.set_default_target_module(Some("UnknownModule"));
        assert_eq!(session.default_target_module(), Some("UnknownModule"));

        session.set_default_target_module(None::<String>);
        assert!(session.default_target_module().is_none());
    }
}
