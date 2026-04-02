use oxvba_compiler::ProjectManifest;
use oxvba_runtime::{RuntimeValue, bstr::BStr, runtime_value_to_vba_string};
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
    manifest: ProjectManifest,
    runtime: ProjectRuntimeSession,
    default_target_module: Option<String>,
}

impl<'engine> ImmediateSession<'engine> {
    pub fn new(
        engine: &'engine Engine,
        manifest: ProjectManifest,
        runtime: ProjectRuntimeSession,
    ) -> Self {
        Self {
            engine,
            manifest,
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

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
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

    pub fn reset(
        &mut self,
        kind: ImmediateResetKind,
    ) -> Result<ImmediateEvaluationResult, PhaseDiagnostic> {
        match kind {
            ImmediateResetKind::ClearSessionState | ImmediateResetKind::ReloadProject => {
                self.runtime = self.engine.compile_and_prepare_session(&self.manifest)?;
                Ok(ImmediateEvaluationResult {
                    output: ImmediateEvaluationOutput::Reset,
                    diagnostics: Vec::new(),
                })
            }
        }
    }

    pub fn evaluate(
        &mut self,
        request: &ImmediateEvaluationRequest,
    ) -> Result<ImmediateEvaluationResult, ImmediateSessionError> {
        let trimmed = request.source_text.trim();
        if trimmed.is_empty() {
            return Ok(ImmediateEvaluationResult::empty());
        }

        if trimmed.eq_ignore_ascii_case("reset") {
            return self
                .reset(ImmediateResetKind::ClearSessionState)
                .map_err(ImmediateSessionError::Phase);
        }

        let parsed = match parse_immediate_invocation(trimmed, request.kind) {
            Ok(parsed) => parsed,
            Err(message) => {
                return Ok(ImmediateEvaluationResult {
                    output: ImmediateEvaluationOutput::Empty,
                    diagnostics: vec![PhaseDiagnostic::compile(message)],
                });
            }
        };

        let module_name = parsed
            .module_name
            .or_else(|| self.default_target_module.clone())
            .ok_or_else(|| ImmediateSessionError::UnknownTargetModule {
                module: "<none>".to_string(),
            })?;

        let runtime_value = self
            .engine
            .invoke_procedure(
                &mut self.runtime,
                &module_name,
                &parsed.procedure_name,
                &parsed.args,
            )
            .map_err(ImmediateSessionError::Phase)?;

        let output = if parsed.is_statement {
            ImmediateEvaluationOutput::PrintedLine(format_invoked_statement_line(
                &module_name,
                &parsed.procedure_name,
                &runtime_value,
                request.display_style,
            ))
        } else if matches!(runtime_value, RuntimeValue::Empty) {
            ImmediateEvaluationOutput::Empty
        } else {
            ImmediateEvaluationOutput::Value(ImmediateValueProjection {
                display_text: format_runtime_value_for_immediate(&runtime_value),
                runtime_value,
            })
        };

        Ok(ImmediateEvaluationResult {
            output,
            diagnostics: Vec::new(),
        })
    }
}

impl Engine {
    pub fn prepare_immediate_session(
        &self,
        manifest: &ProjectManifest,
    ) -> Result<ImmediateSession<'_>, PhaseDiagnostic> {
        let runtime = self.compile_and_prepare_session(manifest)?;
        Ok(ImmediateSession::new(self, manifest.clone(), runtime))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedImmediateInvocation {
    module_name: Option<String>,
    procedure_name: String,
    args: Vec<RuntimeValue>,
    is_statement: bool,
}

fn parse_immediate_invocation(
    source_text: &str,
    request_kind: ImmediateInputKind,
) -> Result<ParsedImmediateInvocation, String> {
    let mut text = source_text.trim();
    let mut is_statement = matches!(request_kind, ImmediateInputKind::Statement);
    let mut force_value = matches!(
        request_kind,
        ImmediateInputKind::Expression | ImmediateInputKind::Query
    );

    if let Some(rest) = text.strip_prefix('?') {
        text = rest.trim();
        force_value = true;
        is_statement = false;
    } else if text.len() >= 5 && text[..5].eq_ignore_ascii_case("call ") {
        text = text[5..].trim();
        is_statement = true;
    }

    let (head, args) = if let Some(open_idx) = text.find('(') {
        let close_idx = text
            .rfind(')')
            .ok_or_else(|| "Immediate input is missing closing `)`".to_string())?;
        if close_idx < open_idx {
            return Err("Immediate input has invalid argument list ordering".to_string());
        }
        let head = text[..open_idx].trim();
        let args_text = text[open_idx + 1..close_idx].trim();
        let trailing = text[close_idx + 1..].trim();
        if !trailing.is_empty() {
            return Err("Immediate input has unexpected trailing text".to_string());
        }
        (head, parse_immediate_args(args_text)?)
    } else {
        (text, Vec::new())
    };

    let (module_name, procedure_name) = if let Some(dot_idx) = head.rfind('.') {
        (
            Some(head[..dot_idx].trim().to_string()),
            head[dot_idx + 1..].trim().to_string(),
        )
    } else {
        (None, head.trim().to_string())
    };

    if procedure_name.is_empty() {
        return Err("Immediate input is missing a target procedure name".to_string());
    }

    if force_value {
        is_statement = false;
    }

    Ok(ParsedImmediateInvocation {
        module_name,
        procedure_name,
        args,
        is_statement,
    })
}

fn parse_immediate_args(args_text: &str) -> Result<Vec<RuntimeValue>, String> {
    if args_text.is_empty() {
        return Ok(Vec::new());
    }

    split_immediate_args(args_text)
        .into_iter()
        .map(|token| parse_immediate_literal(&token))
        .collect()
}

fn split_immediate_args(args_text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    for ch in args_text.chars() {
        match ch {
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            ',' if !in_string => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

fn parse_immediate_literal(token: &str) -> Result<RuntimeValue, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err("Immediate argument list contains an empty argument".to_string());
    }

    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(RuntimeValue::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(RuntimeValue::Bool(false));
    }
    if trimmed.eq_ignore_ascii_case("empty") {
        return Ok(RuntimeValue::Empty);
    }
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return Ok(RuntimeValue::String(BStr(
            trimmed[1..trimmed.len() - 1].to_string(),
        )));
    }
    if let Ok(value) = trimmed.parse::<i32>() {
        return Ok(RuntimeValue::I32(value));
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        return Ok(RuntimeValue::I64(value));
    }

    Err(format!(
        "Immediate evaluator currently accepts only string, integer, boolean, and Empty literals; unsupported argument `{trimmed}`"
    ))
}

fn format_runtime_value_for_immediate(value: &RuntimeValue) -> String {
    match runtime_value_to_vba_string(value) {
        Ok(RuntimeValue::String(BStr(text))) => text,
        Ok(other) => format!("{other:?}"),
        Err(_) => format!("{value:?}"),
    }
}

fn format_invoked_statement_line(
    module_name: &str,
    procedure_name: &str,
    runtime_value: &RuntimeValue,
    display_style: ImmediateDisplayStyle,
) -> String {
    let rendered_value = format_runtime_value_for_immediate(runtime_value);
    match display_style {
        ImmediateDisplayStyle::ImmediateWindow | ImmediateDisplayStyle::PrintLike => {
            if matches!(runtime_value, RuntimeValue::Empty) {
                format!("ok: {module_name}.{procedure_name}")
            } else {
                format!("ok: {module_name}.{procedure_name} => {rendered_value}")
            }
        }
        ImmediateDisplayStyle::ValueOnly => rendered_value,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
    use oxvba_runtime::{RuntimeValue, bstr::BStr};

    use super::{
        ImmediateDisplayStyle, ImmediateEvaluationOutput, ImmediateEvaluationRequest,
        ImmediateInputKind, ImmediateSession,
    };
    use crate::{Engine, HostConfig};

    fn make_manifest(source: &str) -> ProjectManifest {
        ProjectManifest {
            project_name: "ImmediateHost".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Module1", ModuleKind::Procedural, source)
                    .expect("module unit"),
            ],
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
        let mut session = ImmediateSession::new(&engine, manifest.clone(), runtime);

        session.set_default_target_module(Some("Module1"));
        assert_eq!(session.default_target_module(), Some("Module1"));

        session.set_default_target_module(Some("UnknownModule"));
        assert_eq!(session.default_target_module(), Some("UnknownModule"));

        session.set_default_target_module(None::<String>);
        assert!(session.default_target_module().is_none());
    }

    #[test]
    fn immediate_session_invokes_live_session_procedures() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Public Function DoubleValue(value As Integer) As Integer
    DoubleValue = value * 2
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");
        session.set_default_target_module(Some("Module1"));

        let result = session
            .evaluate(&ImmediateEvaluationRequest::query("DoubleValue(21)"))
            .expect("evaluate");

        let ImmediateEvaluationOutput::Value(value) = result.output else {
            panic!("expected value result");
        };
        assert_eq!(value.runtime_value, RuntimeValue::I32(42));
        assert_eq!(value.display_text, "42");
    }

    #[test]
    fn immediate_session_preserves_state_across_repeated_calls_and_reset() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Dim counter As Integer

Public Function IncrementCounter() As Integer
    counter = counter + 1
    IncrementCounter = counter
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");
        session.set_default_target_module(Some("Module1"));

        let first = session
            .evaluate(&ImmediateEvaluationRequest::query("IncrementCounter()"))
            .expect("first");
        let second = session
            .evaluate(&ImmediateEvaluationRequest::query("IncrementCounter()"))
            .expect("second");

        let ImmediateEvaluationOutput::Value(first_value) = first.output else {
            panic!("expected first value result");
        };
        let ImmediateEvaluationOutput::Value(second_value) = second.output else {
            panic!("expected second value result");
        };
        assert_eq!(first_value.runtime_value, RuntimeValue::I32(1));
        assert_eq!(second_value.runtime_value, RuntimeValue::I32(2));

        let reset = session
            .reset(super::ImmediateResetKind::ClearSessionState)
            .expect("reset");
        assert!(matches!(reset.output, ImmediateEvaluationOutput::Reset));

        let after_reset = session
            .evaluate(&ImmediateEvaluationRequest::query("IncrementCounter()"))
            .expect("after reset");
        let ImmediateEvaluationOutput::Value(reset_value) = after_reset.output else {
            panic!("expected reset value result");
        };
        assert_eq!(reset_value.runtime_value, RuntimeValue::I32(1));
    }

    #[test]
    fn immediate_session_supports_question_shorthand_and_string_arguments() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Public Function EchoText(value As String) As String
    EchoText = value
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");
        session.set_default_target_module(Some("Module1"));

        let result = session
            .evaluate(&ImmediateEvaluationRequest::new(r#"? EchoText("hello")"#))
            .expect("evaluate");

        let ImmediateEvaluationOutput::Value(value) = result.output else {
            panic!("expected value result");
        };
        assert_eq!(
            value.runtime_value,
            RuntimeValue::String(BStr("hello".to_string()))
        );
        assert_eq!(value.display_text, "hello");
    }

    #[test]
    fn immediate_session_statement_mode_projects_printed_line() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Public Function DoubleValue(value As Integer) As Integer
    DoubleValue = value * 2
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");
        session.set_default_target_module(Some("Module1"));

        let result = session
            .evaluate(&ImmediateEvaluationRequest::statement("Call DoubleValue(21)"))
            .expect("evaluate");

        let ImmediateEvaluationOutput::PrintedLine(line) = result.output else {
            panic!("expected printed-line result");
        };
        assert_eq!(line, "ok: Module1.DoubleValue => 42");
    }

    #[test]
    fn immediate_session_reports_diagnostics_for_unsupported_literal_arguments() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Public Function DoubleValue(value As Integer) As Integer
    DoubleValue = value * 2
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");
        session.set_default_target_module(Some("Module1"));

        let result = session
            .evaluate(&ImmediateEvaluationRequest::new("DoubleValue(counter)"))
            .expect("evaluate");

        assert!(matches!(result.output, ImmediateEvaluationOutput::Empty));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].phase(), crate::DiagnosticPhase::CompileTime);
        assert!(
            result.diagnostics[0]
                .message()
                .contains("unsupported argument `counter`")
        );
    }

    #[test]
    fn immediate_session_requires_target_module_for_unqualified_calls() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Public Function GetValue() As Integer
    GetValue = 42
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");

        let err = session
            .evaluate(&ImmediateEvaluationRequest::new("GetValue()"))
            .expect_err("missing module should fail");

        assert_eq!(
            err,
            super::ImmediateSessionError::UnknownTargetModule {
                module: "<none>".to_string()
            }
        );
    }

    #[test]
    fn immediate_session_reset_command_flows_through_evaluator() {
        let engine = Engine::new(HostConfig::default());
        let manifest = make_manifest(
            r#"
Public Function GetValue() As Integer
    GetValue = 42
End Function
"#,
        );

        let mut session = engine
            .prepare_immediate_session(&manifest)
            .expect("immediate session");
        session.set_default_target_module(Some("Module1"));

        let result = session
            .evaluate(&ImmediateEvaluationRequest::new("reset"))
            .expect("reset should evaluate");

        assert!(matches!(result.output, ImmediateEvaluationOutput::Reset));
        assert!(result.diagnostics.is_empty());
    }
}
