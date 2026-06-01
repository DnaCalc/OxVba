use crate::frontend_semantic_model::SemanticDiagnostic;
use crate::frontend_symbols::FrontendSourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendDiagnosticFamily {
    Parser,
    Binder,
    Type,
    LegacyCompat(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendDiagnostic {
    pub family: FrontendDiagnosticFamily,
    pub severity: FrontendDiagnosticSeverity,
    pub code: String,
    pub legacy_code: Option<String>,
    pub span: FrontendSourceSpan,
    pub message: String,
}

impl FrontendDiagnostic {
    pub fn to_semantic_diagnostic(&self) -> SemanticDiagnostic {
        SemanticDiagnostic {
            span: self.span,
            code: self.code.clone(),
            message: self.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FrontendDiagnosticMapper {
    diagnostics: Vec<FrontendDiagnostic>,
}

impl FrontendDiagnosticMapper {
    pub fn push_parser_error(&mut self, offset: u32, message: impl Into<String>) {
        let start = offset as usize;
        self.diagnostics.push(FrontendDiagnostic {
            family: FrontendDiagnosticFamily::Parser,
            severity: FrontendDiagnosticSeverity::Error,
            code: "PARSE-E-SYNTAX".to_string(),
            legacy_code: None,
            span: FrontendSourceSpan {
                start,
                end: start.saturating_add(1),
            },
            message: message.into(),
        });
    }

    pub fn push_binder_error(
        &mut self,
        span: FrontendSourceSpan,
        code: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push_diagnostic(FrontendDiagnosticFamily::Binder, span, code, None, message);
    }

    pub fn push_type_error(
        &mut self,
        span: FrontendSourceSpan,
        code: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push_diagnostic(FrontendDiagnosticFamily::Type, span, code, None, message);
    }

    pub fn push_legacy_compatible_error(
        &mut self,
        span: FrontendSourceSpan,
        family: impl Into<String>,
        code: impl Into<String>,
        legacy_code: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push_diagnostic(
            FrontendDiagnosticFamily::LegacyCompat(family.into()),
            span,
            code,
            Some(legacy_code.into()),
            message,
        );
    }

    pub fn diagnostics(&self) -> &[FrontendDiagnostic] {
        &self.diagnostics
    }

    pub fn semantic_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .map(FrontendDiagnostic::to_semantic_diagnostic)
            .collect()
    }

    fn push_diagnostic(
        &mut self,
        family: FrontendDiagnosticFamily,
        span: FrontendSourceSpan,
        code: impl Into<String>,
        legacy_code: Option<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(FrontendDiagnostic {
            family,
            severity: FrontendDiagnosticSeverity::Error,
            code: code.into(),
            legacy_code,
            span,
            message: message.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_hir::HirArenas;
    use crate::frontend_semantic_model::SemanticModel;
    use crate::frontend_symbols::SymbolModel;

    #[test]
    fn diagnostic_mapper_routes_parser_error_to_stable_byte_span() {
        let mut mapper = FrontendDiagnosticMapper::default();
        mapper.push_parser_error(12, "expected End If");

        assert_eq!(
            mapper.diagnostics(),
            &[FrontendDiagnostic {
                family: FrontendDiagnosticFamily::Parser,
                severity: FrontendDiagnosticSeverity::Error,
                code: "PARSE-E-SYNTAX".to_string(),
                legacy_code: None,
                span: FrontendSourceSpan { start: 12, end: 13 },
                message: "expected End If".to_string(),
            }]
        );
    }

    #[test]
    fn diagnostic_mapper_preserves_binder_type_and_legacy_family_codes() {
        let mut mapper = FrontendDiagnosticMapper::default();
        mapper.push_binder_error(
            FrontendSourceSpan { start: 1, end: 5 },
            "BIND-E-NAME-UNRESOLVED",
            "unresolved name",
        );
        mapper.push_type_error(
            FrontendSourceSpan { start: 10, end: 20 },
            "TYPE-E-MISMATCH",
            "type mismatch",
        );
        mapper.push_legacy_compatible_error(
            FrontendSourceSpan { start: 30, end: 40 },
            "typelib",
            "BIND-E-TYPELIB-MEMBER-NOT-FOUND",
            "BIND-E-TYPELIB-MEMBER-NOT-FOUND",
            "external member was not found",
        );

        assert!(matches!(
            mapper.diagnostics()[0].family,
            FrontendDiagnosticFamily::Binder
        ));
        assert!(matches!(
            mapper.diagnostics()[1].family,
            FrontendDiagnosticFamily::Type
        ));
        assert_eq!(
            mapper.diagnostics()[2].legacy_code.as_deref(),
            Some("BIND-E-TYPELIB-MEMBER-NOT-FOUND")
        );
    }

    #[test]
    fn diagnostic_mapper_feeds_semantic_model_span_queries() {
        let mut mapper = FrontendDiagnosticMapper::default();
        mapper.push_type_error(
            FrontendSourceSpan { start: 10, end: 15 },
            "TYPE-E-MISMATCH",
            "type mismatch",
        );

        let mut model = SemanticModel::new(SymbolModel::default(), HirArenas::default());
        for diagnostic in mapper.semantic_diagnostics() {
            model.push_diagnostic(diagnostic);
        }

        let diagnostics = model.diagnostics_for_span(FrontendSourceSpan { start: 12, end: 13 });
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TYPE-E-MISMATCH");
    }
}
