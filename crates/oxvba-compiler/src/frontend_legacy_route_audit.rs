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

    findings.push(LegacyRouteAuditFinding {
        area: "project.rs source-text rewrite bridge",
        evidence: "production project compilation selects ModuleAwareBindPlan unconditionally; RewriteBridge remains only as an internal parity-test strategy".to_string(),
        disposition: LegacyRouteAuditDisposition::HirProduction,
        owner: "bd-aprs.8.*",
    });
    findings.push(LegacyRouteAuditFinding {
        area: "language-service legacy BoundModule compatibility",
        evidence: "oxvba-languageservice SemanticSnapshot no longer retains/exposes BoundModule or uses it for signature help, but semantic.rs still builds a transient BoundModule for fallback correlation and resolution diagnostics".to_string(),
        disposition: LegacyRouteAuditDisposition::StaticResidual,
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
    fn audit_records_static_residuals_before_terminal_gate() {
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
                finding.area.contains("project.rs")
                    && finding.disposition == LegacyRouteAuditDisposition::HirProduction
            }),
            "{report:#?}"
        );
        assert!(
            !report.terminal_gate_passed(),
            "terminal gate must not pass while residuals exist: {report:#?}"
        );
    }
}
