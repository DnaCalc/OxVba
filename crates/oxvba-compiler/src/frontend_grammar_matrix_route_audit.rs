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
    mapping("file_preamble", "top-level", "option explicit"),
    mapping("module_body", "top-level", "const statement"),
    mapping("module_item", "top-level", "const statement"),
    mapping("option_line", "top-level", "option explicit"),
    mapping("option_kind", "top-level", "option explicit"),
    mapping("option_value", "top-level", "option compare database"),
    mapping("attribute_line", "top-level", "module attribute"),
    mapping("statement_line", "top-level", "assignment/arithmetic"),
    mapping("logical_line", "top-level", "assignment/arithmetic"),
    mapping("statement_list", "top-level", "single-line if statement"),
    mapping("line_comment", "top-level", "trivia comment and blank-line"),
    mapping("line_prefix", "top-level", "goto numeric label"),
    mapping("blank_line", "top-level", "trivia comment and blank-line"),
    mapping("comment_line", "top-level", "trivia comment and blank-line"),
    mapping("trivia", "top-level", "trivia comment and blank-line"),
    mapping("declaration", "declaration", "const statement"),
    mapping("const_decl", "declaration", "const statement"),
    mapping("const_item", "declaration", "const statement"),
    mapping("dim_decl", "declaration", "dim declaration"),
    mapping("variable_decl_stmt", "declaration", "dim declaration"),
    mapping("variable_decl_list", "declaration", "dim declaration"),
    mapping("variable_decl", "declaration", "dim declaration"),
    mapping("array_rank", "declaration", "fixed array element alias"),
    mapping("bound_list", "declaration", "fixed array element alias"),
    mapping("bound", "declaration", "fixed array element alias"),
    mapping("type_ascription", "declaration", "dim declaration"),
    mapping("new_modifier", "declaration", "active-project construction"),
    mapping("enum_decl", "declaration", "enum member constant"),
    mapping("enum_member", "declaration", "enum member constant"),
    mapping("type_decl", "declaration", "UDT layout descriptor"),
    mapping("field_decl", "declaration", "UDT layout descriptor"),
    mapping("declare_decl", "declaration", "declared external call"),
    mapping("lib_clause", "declaration", "declared external call"),
    mapping("alias_clause", "declaration", "declared external call"),
    mapping("event_decl", "declaration", "event declaration"),
    mapping("implements_decl", "declaration", "single-source implements"),
    mapping("procedure_decl", "procedure", "function declaration"),
    mapping("sub_decl", "procedure", "procedure call statement"),
    mapping("function_decl", "procedure", "function declaration"),
    mapping("property_decl", "procedure", "simple property"),
    mapping("property_get", "procedure", "indexed property get"),
    mapping("property_let", "procedure", "indexed property let"),
    mapping("property_set", "procedure", "indexed property set"),
    mapping("parameter_list", "procedure", "optional parameter"),
    mapping("parameter_decl_list", "procedure", "optional parameter"),
    mapping("parameter_decl", "procedure", "optional parameter"),
    mapping("parameter_modifier", "procedure", "optional parameter"),
    mapping(
        "default_value",
        "procedure",
        "optional integer default expression",
    ),
    mapping("statement_block", "procedure", "if statement"),
    mapping("visibility", "procedure", "enum member constant"),
    mapping("statement", "statement", "assignment/arithmetic"),
    mapping("assignment_stmt", "statement", "assignment/arithmetic"),
    mapping("call_stmt", "statement", "procedure call statement"),
    mapping("if_stmt", "statement", "elseif statement"),
    mapping("inline_if_stmt", "statement", "single-line if statement"),
    mapping("block_if_stmt", "statement", "elseif statement"),
    mapping("elseif_clause", "statement", "elseif statement"),
    mapping("else_clause", "statement", "if else statement"),
    mapping("select_stmt", "statement", "select case statement"),
    mapping("case_clause", "statement", "select case statement"),
    mapping("case_selector_list", "statement", "select case multi-value"),
    mapping("case_selector", "statement", "select case range"),
    mapping("case_compare_op", "statement", "select case is"),
    mapping("for_stmt", "statement", "for statement"),
    mapping("for_each_stmt", "statement", "for each statement"),
    mapping("do_loop_stmt", "statement", "do until statement"),
    mapping("loop_condition", "statement", "post-check loop statement"),
    mapping("while_wend_stmt", "statement", "while wend statement"),
    mapping("with_stmt", "statement", "with member read"),
    mapping("on_error_stmt", "statement", "on error goto label"),
    mapping("resume_stmt", "statement", "on error resume next"),
    mapping("goto_stmt", "statement", "goto numeric label"),
    mapping("gosub_stmt", "statement", "gosub return statement"),
    mapping("return_stmt", "statement", "gosub return statement"),
    mapping("exit_stmt", "statement", "exit for statement"),
    mapping("erase_stmt", "statement", "erase statement"),
    mapping("redim_stmt", "statement", "redim runtime statement"),
    mapping("redim_item", "statement", "redim runtime statement"),
    mapping("raise_event_stmt", "statement", "raise event statement"),
    mapping("local_decl_stmt", "statement", "dim declaration"),
    mapping("expression", "expression", "assignment/arithmetic"),
    mapping("comparison_expr", "expression", "TypeOf Is expression"),
    mapping("compare_op", "expression", "select case is"),
    mapping("concat_expr", "expression", "concat expression"),
    mapping("additive_expr", "expression", "assignment/arithmetic"),
    mapping("multiplicative_expr", "expression", "assignment/arithmetic"),
    mapping("unary_expr", "expression", "unary Not expression"),
    mapping("postfix_expr", "expression", "value-side member expression"),
    mapping("postfix_part", "expression", "value-side member expression"),
    mapping("member_part", "expression", "value-side member expression"),
    mapping(
        "bang_part",
        "expression",
        "value-side bang member expression",
    ),
    mapping("index_or_call_part", "expression", "indexed property get"),
    mapping("primary_expr", "expression", "assignment/arithmetic"),
    mapping(
        "parenthesized_expr",
        "expression",
        "optional integer default expression",
    ),
    mapping("type_of_expr", "expression", "TypeOf Is expression"),
    mapping(
        "argument_syntax",
        "expression",
        "statement-form procedure call arguments",
    ),
    mapping(
        "argument_list",
        "expression",
        "statement-form procedure call arguments",
    ),
    mapping(
        "argument",
        "expression",
        "statement-form procedure call arguments",
    ),
    mapping("named_argument", "expression", "named indexed property let"),
    mapping("callable_expr", "expression", "procedure call statement"),
    mapping("lvalue", "expression", "assignment/arithmetic"),
    mapping("identifier", "lexical", "assignment/arithmetic"),
    mapping("type_name", "lexical", "builtin type declaration"),
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
        assert_eq!(report.rows.len(), 106, "{report:#?}");
        assert!(report.residuals().is_empty(), "{report:#?}");
        for production in [
            "source_file",
            "dim_decl",
            "line_comment",
            "variable_decl",
            "declare_decl",
            "property_get",
            "case_selector_list",
            "do_loop_stmt",
            "redim_stmt",
            "concat_expr",
            "unary_expr",
            "argument_syntax",
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
