//! Shared structured diagnostics for OxVba.
//!
//! Error-producing crates own their semantic error types and expose conversions
//! into this leaf model at crate boundaries. That keeps the source of truth local
//! while giving hosts, the CLI, tests, and future tooling one stable surface.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&'static str> for DiagnosticCode {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DiagnosticCode {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl DiagnosticSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticPhase {
    Syntax,
    ProjectLoad,
    Symbol,
    Bind,
    Bundle,
    Runtime,
    Hal,
    Com,
    Host,
}

impl DiagnosticPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticPhase::Syntax => "syntax",
            DiagnosticPhase::ProjectLoad => "project-load",
            DiagnosticPhase::Symbol => "symbol",
            DiagnosticPhase::Bind => "bind",
            DiagnosticPhase::Bundle => "bundle",
            DiagnosticPhase::Runtime => "runtime",
            DiagnosticPhase::Hal => "hal",
            DiagnosticPhase::Com => "com",
            DiagnosticPhase::Host => "host",
        }
    }

    pub fn is_runtime(self) -> bool {
        matches!(
            self,
            DiagnosticPhase::Runtime
                | DiagnosticPhase::Hal
                | DiagnosticPhase::Com
                | DiagnosticPhase::Host
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: u32,
    pub end: u32,
}

impl SourceSpan {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn point(offset: u32) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

impl DiagnosticSource {
    pub fn new() -> Self {
        Self {
            file: None,
            module: None,
            span: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
            snippet: None,
        }
    }

    pub fn module(module: impl Into<String>) -> Self {
        Self {
            module: Some(module.into()),
            ..Self::new()
        }
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_source_text(mut self, source: &str) -> Self {
        if let Some(span) = self.span {
            let start = byte_offset_to_line_column(source, span.start);
            let end = byte_offset_to_line_column(source, span.end);
            self.line = Some(start.line);
            self.column = Some(start.column);
            self.end_line = Some(end.line);
            self.end_column = Some(end.column);
            self.snippet = line_at(source, start.line).map(str::to_string);
        }
        self
    }

    fn display_location(&self) -> String {
        let base = self
            .file
            .as_deref()
            .or(self.module.as_deref())
            .unwrap_or("<unknown>");
        match (self.line, self.column) {
            (Some(line), Some(column)) => format!("{base}:{line}:{column}"),
            _ => base.to_string(),
        }
    }
}

impl Default for DiagnosticSource {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLabel {
    pub source: DiagnosticSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCause {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticCode>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub phase: DiagnosticPhase,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<DiagnosticSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<DiagnosticLabel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<DiagnosticCause>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vba_error_number: Option<i32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl Diagnostic {
    pub fn error(
        code: impl Into<DiagnosticCode>,
        phase: DiagnosticPhase,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            phase,
            message: message.into(),
            source: None,
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
            causes: Vec::new(),
            vba_error_number: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_source(mut self, source: DiagnosticSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_cause(mut self, cause: DiagnosticCause) -> Self {
        self.causes.push(cause);
        self
    }

    pub fn with_vba_error_number(mut self, number: i32) -> Self {
        self.vba_error_number = Some(number);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn render_human(&self) -> String {
        let mut out = format!(
            "{} [{}:{}]: {}",
            self.severity.as_str(),
            self.phase.as_str(),
            self.code,
            self.message
        );
        if let Some(number) = self.vba_error_number {
            out.push_str(&format!(" (VBA Err.Number {number})"));
        }
        if let Some(source) = &self.source {
            out.push_str(&format!("\n  --> {}", source.display_location()));
            if let Some(snippet) = &source.snippet {
                out.push_str(&format!("\n   | {snippet}"));
            }
        }
        for note in &self.notes {
            out.push_str(&format!("\n  note: {note}"));
        }
        if let Some(help) = &self.help {
            out.push_str(&format!("\n  help: {help}"));
        }
        for cause in &self.causes {
            match &cause.code {
                Some(code) => out.push_str(&format!("\n  caused by [{code}]: {}", cause.message)),
                None => out.push_str(&format!("\n  caused by: {}", cause.message)),
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    pub fn single(diagnostic: Diagnostic) -> Self {
        Self {
            diagnostics: vec![diagnostic],
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn extract_prefixed_code(message: &str, family_prefix: &str) -> Option<String> {
    let trimmed = message.trim_start();
    let (candidate, _) = trimmed.split_once(':')?;
    if candidate.starts_with(family_prefix)
        && candidate
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
    {
        Some(candidate.to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineColumn {
    line: u32,
    column: u32,
}

fn byte_offset_to_line_column(source: &str, offset: u32) -> LineColumn {
    let target = offset as usize;
    let mut line = 1;
    let mut column = 1;
    for (idx, ch) in source.char_indices() {
        if idx >= target {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    LineColumn { line, column }
}

fn line_at(source: &str, target_line: u32) -> Option<&str> {
    source
        .lines()
        .nth(target_line.saturating_sub(1) as usize)
        .map(str::trim_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_text_adds_line_column_and_snippet() {
        let source = "Sub Main()\n  Dim x\nEnd Sub\n";
        let diagnostic_source = DiagnosticSource::module("Main")
            .with_span(SourceSpan::point(13))
            .with_source_text(source);
        assert_eq!(diagnostic_source.line, Some(2));
        assert_eq!(diagnostic_source.column, Some(3));
        assert_eq!(diagnostic_source.snippet.as_deref(), Some("  Dim x"));
    }

    #[test]
    fn prefixed_code_extraction_is_strict() {
        assert_eq!(
            extract_prefixed_code("COM-E-STATE-POISONED: bad", "COM-E-").as_deref(),
            Some("COM-E-STATE-POISONED")
        );
        assert!(extract_prefixed_code("COM state poisoned", "COM-E-").is_none());
    }

    #[test]
    fn json_report_contains_stable_code() {
        let json = DiagnosticReport::single(Diagnostic::error(
            "SYN-E-PARSE",
            DiagnosticPhase::Syntax,
            "expected identifier",
        ))
        .to_json_pretty()
        .expect("diagnostic JSON should serialize");
        assert!(json.contains("\"code\": \"SYN-E-PARSE\""));
    }
}
