use crate::frontend_legacy_route_audit::{
    LegacyRouteAuditDisposition, LegacyRouteAuditReport, run_production_legacy_route_audit,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarMatrixRouteRow {
    pub production: &'static str,
    pub category: &'static str,
    pub audit_area: &'static str,
    pub disposition: LegacyRouteAuditDisposition,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarMatrixRouteReport {
    pub rows: Vec<GrammarMatrixRouteRow>,
}

impl GrammarMatrixRouteReport {
    pub fn terminal_gate_passed(&self) -> bool {
        self.rows
            .iter()
            .all(|row| row.disposition == LegacyRouteAuditDisposition::HirProduction)
    }

    pub fn residuals(&self) -> Vec<&GrammarMatrixRouteRow> {
        self.rows
            .iter()
            .filter(|row| row.disposition != LegacyRouteAuditDisposition::HirProduction)
            .collect()
    }
}

pub fn run_grammar_matrix_route_audit() -> GrammarMatrixRouteReport {
    let legacy_report = run_production_legacy_route_audit();
    let rows = GRAMMAR_MATRIX_AUDIT_MAP
        .iter()
        .map(|mapping| route_row_for_mapping(&legacy_report, mapping))
        .collect();
    GrammarMatrixRouteReport { rows }
}

#[derive(Debug, Clone, Copy)]
struct GrammarMatrixAuditMapping {
    production: &'static str,
    category: &'static str,
    audit_area_contains: &'static str,
}

const GRAMMAR_MATRIX_AUDIT_MAP: &[GrammarMatrixAuditMapping] = &[
    mapping("source_file", "top-level", "scoped procedure/local"),
    mapping("option_line", "top-level", "option explicit"),
    mapping("attribute_line", "top-level", "module attribute"),
    mapping("const_decl", "declaration", "const statement"),
    mapping("dim_decl", "declaration", "dim declaration"),
    mapping("enum_decl", "declaration", "enum member constant"),
    mapping("type_decl", "declaration", "UDT layout descriptor"),
    mapping("declare_decl", "declaration", "declared external call"),
    mapping("sub_decl", "procedure", "procedure call statement"),
    mapping("function_decl", "procedure", "function declaration"),
    mapping("property_get", "procedure", "indexed property get"),
    mapping("property_let", "procedure", "indexed property let"),
    mapping("property_set", "procedure", "indexed property set"),
    mapping("parameter_decl", "procedure", "optional parameter"),
    mapping("assignment_stmt", "statement", "assignment/arithmetic"),
    mapping("call_stmt", "statement", "procedure call statement"),
    mapping("inline_if_stmt", "statement", "single-line if statement"),
    mapping("block_if_stmt", "statement", "elseif statement"),
    mapping("select_stmt", "statement", "select case statement"),
    mapping("case_selector", "statement", "select case range"),
    mapping("for_stmt", "statement", "for statement"),
    mapping("for_each_stmt", "statement", "for each statement"),
    mapping("do_loop_stmt", "statement", "do until statement"),
    mapping("while_wend_stmt", "statement", "while wend statement"),
    mapping("with_stmt", "statement", "with member read"),
    mapping("on_error_stmt", "statement", "on error goto label"),
    mapping("resume_stmt", "statement", "on error resume next"),
    mapping("goto_stmt", "statement", "goto label statement"),
    mapping("gosub_stmt", "statement", "gosub return statement"),
    mapping("exit_stmt", "statement", "exit for statement"),
    mapping("erase_stmt", "statement", "erase statement"),
    mapping("redim_stmt", "statement", "redim runtime statement"),
    mapping("raise_event_stmt", "statement", "raise event statement"),
    mapping("comparison_expr", "expression", "TypeOf Is expression"),
    mapping("concat_expr", "expression", "concat expression"),
    mapping("additive_expr", "expression", "assignment/arithmetic"),
    mapping("multiplicative_expr", "expression", "assignment/arithmetic"),
    mapping("unary_expr", "expression", "unary Not expression"),
    mapping("postfix_expr", "expression", "value-side member expression"),
    mapping("type_of_expr", "expression", "TypeOf Is expression"),
    mapping(
        "argument_list",
        "expression",
        "statement-form procedure call arguments",
    ),
    mapping("named_argument", "expression", "named indexed property let"),
    mapping("builtin_type", "lexical", "builtin type declaration"),
    mapping("literal", "lexical", "const statement"),
];

const fn mapping(
    production: &'static str,
    category: &'static str,
    audit_area_contains: &'static str,
) -> GrammarMatrixAuditMapping {
    GrammarMatrixAuditMapping {
        production,
        category,
        audit_area_contains,
    }
}

fn route_row_for_mapping(
    legacy_report: &LegacyRouteAuditReport,
    mapping: &GrammarMatrixAuditMapping,
) -> GrammarMatrixRouteRow {
    let finding = legacy_report
        .findings
        .iter()
        .find(|finding| finding.area.contains(mapping.audit_area_contains));
    match finding {
        Some(finding) => GrammarMatrixRouteRow {
            production: mapping.production,
            category: mapping.category,
            audit_area: finding.area,
            disposition: finding.disposition,
            evidence: finding.evidence.clone(),
        },
        None => GrammarMatrixRouteRow {
            production: mapping.production,
            category: mapping.category,
            audit_area: mapping.audit_area_contains,
            disposition: LegacyRouteAuditDisposition::StaticResidual,
            evidence: "grammar matrix row has no matching legacy-route audit finding".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_matrix_route_audit_maps_broad_anchored_rows_to_hir_production() {
        let report = run_grammar_matrix_route_audit();
        assert!(report.terminal_gate_passed(), "{report:#?}");
        assert_eq!(report.rows.len(), 44, "{report:#?}");
        assert!(report.residuals().is_empty(), "{report:#?}");
        for production in [
            "source_file",
            "dim_decl",
            "declare_decl",
            "property_get",
            "do_loop_stmt",
            "redim_stmt",
            "concat_expr",
            "unary_expr",
            "postfix_expr",
            "named_argument",
            "builtin_type",
        ] {
            assert!(
                report.rows.iter().any(|row| {
                    row.production == production
                        && row.disposition == LegacyRouteAuditDisposition::HirProduction
                }),
                "{production} missing from grammar matrix route report: {report:#?}"
            );
        }
    }
}
