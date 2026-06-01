use crate::syntax_bridge::{SyntaxBridgeProductionRoute, production_route_for_source};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyRouteAuditDisposition {
    HirProduction,
    LegacyFallbackResidual,
    StaticResidual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRouteAuditFinding {
    pub area: &'static str,
    pub evidence: String,
    pub disposition: LegacyRouteAuditDisposition,
    pub owner: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRouteAuditReport {
    pub findings: Vec<LegacyRouteAuditFinding>,
}

impl LegacyRouteAuditReport {
    pub fn terminal_gate_passed(&self) -> bool {
        self.findings
            .iter()
            .all(|finding| finding.disposition == LegacyRouteAuditDisposition::HirProduction)
    }

    pub fn residuals(&self) -> Vec<&LegacyRouteAuditFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.disposition != LegacyRouteAuditDisposition::HirProduction)
            .collect()
    }
}

pub fn run_production_legacy_route_audit() -> LegacyRouteAuditReport {
    let mut findings = Vec::new();

    let scoped_assignment = "Sub Main()\nDim x As Long\nx = 1 + 2\nEnd Sub\n";
    findings.push(route_finding(
        "scoped procedure/local/assignment/arithmetic fixture",
        scoped_assignment,
        "bd-aprs.9.5",
    ));

    let call_statement = "Sub Main()\nCall Worker()\nEnd Sub\nSub Worker()\nEnd Sub\n";
    findings.push(route_finding(
        "procedure call statement fixture",
        call_statement,
        "bd-aprs.9.5",
    ));

    let statement_form_call = "Sub Use(ByVal a, ByVal b)\nEnd Sub\nSub Main()\nUse 1, 2\nEnd Sub\n";
    findings.push(route_finding(
        "statement-form procedure call arguments fixture",
        statement_form_call,
        "bd-aprs.9.5",
    ));

    let function_statement =
        "Function Alpha() As Long\nAlpha = 1\nEnd Function\nSub Main()\nEnd Sub\n";
    findings.push(route_finding(
        "function declaration fixture",
        function_statement,
        "bd-aprs.9.5",
    ));

    let if_statement = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nEnd If\nEnd Sub\n";
    findings.push(route_finding(
        "if statement fixture",
        if_statement,
        "bd-aprs.9.5",
    ));

    let if_else_statement =
        "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nElse\nx = 2\nEnd If\nEnd Sub\n";
    findings.push(route_finding(
        "if else statement fixture",
        if_else_statement,
        "bd-aprs.9.5",
    ));

    let elseif_statement = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nElseIf x = 1 Then\nx = 2\nElse\nx = 3\nEnd If\nEnd Sub\n";
    findings.push(route_finding(
        "elseif statement fixture",
        elseif_statement,
        "bd-aprs.9.5",
    ));

    let single_line_if_statement =
        "Sub Main()\nDim x As Long\nIf x = 0 Then x = 1 Else x = 2\nEnd Sub\n";
    findings.push(route_finding(
        "single-line if statement fixture",
        single_line_if_statement,
        "bd-aprs.9.5",
    ));

    let do_while_statement =
        "Sub Main()\nDim x As Long\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub\n";
    findings.push(route_finding(
        "do while statement fixture",
        do_while_statement,
        "bd-aprs.9.5",
    ));

    let select_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase 1\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case statement fixture",
        select_statement,
        "bd-aprs.9.5",
    ));

    let do_until_statement =
        "Sub Main()\nDim x As Long\nDo Until x = 3\nx = x + 1\nLoop\nEnd Sub\n";
    findings.push(route_finding(
        "do until statement fixture",
        do_until_statement,
        "bd-aprs.9.5",
    ));

    let post_check_loop_statement =
        "Sub Main()\nDim x As Long\nDo\nx = x + 1\nLoop Until x = 3\nEnd Sub\n";
    findings.push(route_finding(
        "post-check loop statement fixture",
        post_check_loop_statement,
        "bd-aprs.9.5",
    ));

    let while_wend_statement = "Sub Main()\nDim x As Long\nWhile x < 3\nx = x + 1\nWend\nEnd Sub\n";
    findings.push(route_finding(
        "while wend statement fixture",
        while_wend_statement,
        "bd-aprs.9.5",
    ));

    let for_statement = "Sub Main()\nDim i As Long\nFor i = 1 To 3\ni = i + 1\nNext\nEnd Sub\n";
    findings.push(route_finding(
        "for statement fixture",
        for_statement,
        "bd-aprs.9.5",
    ));

    let select_range_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case range fixture",
        select_range_statement,
        "bd-aprs.9.5",
    ));

    let select_case_is_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase Is < 0\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case is fixture",
        select_case_is_statement,
        "bd-aprs.9.5",
    ));

    let select_multi_statement =
        "Sub Main()\nDim x As Long\nSelect Case x\nCase 1, 2\nx = 2\nEnd Select\nEnd Sub\n";
    findings.push(route_finding(
        "select case multi-value fixture",
        select_multi_statement,
        "bd-aprs.9.5",
    ));

    let for_each_statement =
        "Sub Main()\nDim item As Variant\nFor Each item In item\nitem = item\nNext\nEnd Sub\n";
    findings.push(route_finding(
        "for each statement fixture",
        for_each_statement,
        "bd-aprs.9.5",
    ));

    let exit_do_statement = "Sub Main()\nDim x As Long\nDo While x < 3\nExit Do\nLoop\nEnd Sub\n";
    findings.push(route_finding(
        "exit do statement fixture",
        exit_do_statement,
        "bd-aprs.9.5",
    ));

    let exit_for_statement = "Sub Main()\nDim i As Long\nFor i = 1 To 3\nExit For\nNext\nEnd Sub\n";
    findings.push(route_finding(
        "exit for statement fixture",
        exit_for_statement,
        "bd-aprs.9.5",
    ));

    let exit_sub_statement = "Sub Main()\nExit Sub\nEnd Sub\n";
    findings.push(route_finding(
        "exit sub statement fixture",
        exit_sub_statement,
        "bd-aprs.9.5",
    ));

    let on_error_resume_next_statement = "Sub Main()\nOn Error Resume Next\nResume Next\nEnd Sub\n";
    findings.push(route_finding(
        "on error resume next statement fixture",
        on_error_resume_next_statement,
        "bd-aprs.9.5",
    ));

    let on_error_goto_zero_statement = "Sub Main()\nOn Error GoTo 0\nResume\nEnd Sub\n";
    findings.push(route_finding(
        "on error goto zero statement fixture",
        on_error_goto_zero_statement,
        "bd-aprs.9.5",
    ));

    let on_error_goto_label_statement =
        "Sub Main()\nOn Error GoTo handler\nhandler:\nResume done\ndone:\nEnd Sub\n";
    findings.push(route_finding(
        "on error goto label statement fixture",
        on_error_goto_label_statement,
        "bd-aprs.9.5",
    ));

    let goto_label_statement = "Sub Main()\nGoTo done\ndone:\nEnd Sub\n";
    findings.push(route_finding(
        "goto label statement fixture",
        goto_label_statement,
        "bd-aprs.9.5",
    ));

    let goto_numeric_label_statement = "Sub Main()\nGoTo 100\n100:\nEnd Sub\n";
    findings.push(route_finding(
        "goto numeric label statement fixture",
        goto_numeric_label_statement,
        "bd-aprs.9.5",
    ));

    let gosub_return_statement = "Sub Main()\nGoSub helper\nhelper:\nReturn\nEnd Sub\n";
    findings.push(route_finding(
        "gosub return statement fixture",
        gosub_return_statement,
        "bd-aprs.9.5",
    ));

    let erase_statement = "Sub Main()\nDim a\nErase a\nEnd Sub\n";
    findings.push(route_finding(
        "erase statement fixture",
        erase_statement,
        "bd-aprs.9.5",
    ));

    let redim_statement =
        "Sub Main()\nDim length As Long\nDim buf() As Byte\nReDim buf(length - 1)\nEnd Sub\n";
    findings.push(route_finding(
        "redim runtime statement fixture",
        redim_statement,
        "bd-aprs.9.5",
    ));

    let raise_event_statement = "Sub Main()\nRaiseEvent Tick(1)\nEnd Sub\n";
    findings.push(route_finding(
        "raise event statement fixture",
        raise_event_statement,
        "bd-aprs.9.5",
    ));

    let const_statement = "Const CBase = 7, CName = \"a,b\"\nSub Main()\nDim x\nDim y\nx = CBase\ny = CName\nEnd Sub\n";
    findings.push(route_finding(
        "const statement fixture",
        const_statement,
        "bd-aprs.9.5",
    ));

    let member_expression =
        "Sub Main()\nDim obj\nDim x\nDim y\nx = obj.Value\ny = obj.Method(1)\nEnd Sub\n";
    findings.push(route_finding(
        "value-side member expression fixture",
        member_expression,
        "bd-aprs.9.5",
    ));

    let statement_form_member_call = "Sub Main()\nDim obj\nobj.Method 1, 2\nEnd Sub\n";
    findings.push(route_finding(
        "statement-form member call arguments fixture",
        statement_form_member_call,
        "bd-aprs.9.5",
    ));

    findings.push(LegacyRouteAuditFinding {
        area: "project.rs source-text rewrite bridge",
        evidence: "production project compilation selects ModuleAwareBindPlan unconditionally; RewriteBridge remains only as an internal parity-test strategy".to_string(),
        disposition: LegacyRouteAuditDisposition::HirProduction,
        owner: "bd-aprs.8.*",
    });
    findings.push(LegacyRouteAuditFinding {
        area: "language-service legacy BoundModule compatibility",
        evidence: "oxvba-languageservice SemanticSnapshot no longer retains/exposes or builds a legacy BoundModule; unsupported HIR snapshots report front-end diagnostics instead of rebuilding legacy symbol/callable correlation".to_string(),
        disposition: LegacyRouteAuditDisposition::HirProduction,
        owner: "bd-aprs.10.4",
    });

    LegacyRouteAuditReport { findings }
}

fn route_finding(
    area: &'static str,
    source: &'static str,
    owner: &'static str,
) -> LegacyRouteAuditFinding {
    match production_route_for_source(source) {
        Ok(SyntaxBridgeProductionRoute::HirProduction) => LegacyRouteAuditFinding {
            area,
            evidence: "classified as HIR production".to_string(),
            disposition: LegacyRouteAuditDisposition::HirProduction,
            owner,
        },
        Ok(SyntaxBridgeProductionRoute::HirUnsupportedResidual) => LegacyRouteAuditFinding {
            area,
            evidence: "classified as HIR Unsupported residual; outer default policy may still fall back to legacy"
                .to_string(),
            disposition: LegacyRouteAuditDisposition::LegacyFallbackResidual,
            owner,
        },
        Err(err) => LegacyRouteAuditFinding {
            area,
            evidence: format!("route classification failed: {err}"),
            disposition: LegacyRouteAuditDisposition::LegacyFallbackResidual,
            owner,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_records_hir_production_for_completed_scoped_fixture() {
        let report = run_production_legacy_route_audit();
        let scoped = report
            .findings
            .iter()
            .find(|finding| finding.area.contains("assignment"))
            .expect("scoped fixture finding");
        assert_eq!(
            scoped.disposition,
            LegacyRouteAuditDisposition::HirProduction,
            "{report:#?}"
        );
    }

    #[test]
    fn audit_terminal_gate_passes_after_audited_residuals_retire() {
        let report = run_production_legacy_route_audit();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.area.contains("call statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("statement-form procedure call arguments")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("function declaration")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("project.rs")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("if statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("if else statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("elseif statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("single-line if statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.findings.iter().any(|finding| {
                finding.area.contains("do until statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("post-check loop statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("while wend statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("for statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("select case range")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("select case multi-value")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("select case is")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("for each statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("exit do statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("exit for statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("exit sub statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("on error resume next")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("on error goto zero")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("on error goto label")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("goto label statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("goto numeric label")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("gosub return")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("erase statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("redim runtime statement")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding.area.contains("value-side member expression")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }) && report.findings.iter().any(|finding| {
                finding
                    .area
                    .contains("statement-form member call arguments")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            report.terminal_gate_passed(),
            "terminal gate should pass when audited residuals are retired: {report:#?}"
        );
    }
}
