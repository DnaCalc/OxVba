#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetirementDisposition {
    Replaced,
    QuarantinedResidual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRetirementRow {
    pub legacy_path: &'static str,
    pub replacement: &'static str,
    pub disposition: RetirementDisposition,
    pub owner: &'static str,
}

pub const LEGACY_RETIREMENT_ROWS: &[LegacyRetirementRow] = &[
    LegacyRetirementRow {
        legacy_path: "resolve::parse_expr_for_syntax_bridge",
        replacement: "frontend_hir + frontend_semantic_model",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.10.5",
    },
    LegacyRetirementRow {
        legacy_path: "project.rs text rewrites for project/class semantics",
        replacement: "frontend_project_symbols/frontend_assignment_semantics/frontend_class_semantics",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.10.5",
    },
    LegacyRetirementRow {
        legacy_path: "stringly structural intrinsic names",
        replacement: "frontend_structural_intrinsics::StructuralIntrinsic",
        disposition: RetirementDisposition::Replaced,
        owner: "bd-aprs.9.1",
    },
];

pub fn residual_retirement_rows() -> Vec<&'static LegacyRetirementRow> {
    LEGACY_RETIREMENT_ROWS
        .iter()
        .filter(|row| row.disposition == RetirementDisposition::QuarantinedResidual)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_inventory_has_owner_for_every_residual() {
        for row in residual_retirement_rows() {
            assert!(!row.owner.is_empty(), "{row:#?}");
            assert!(!row.replacement.is_empty(), "{row:#?}");
        }
    }

    #[test]
    fn retirement_inventory_records_structural_intrinsic_replacement() {
        assert!(LEGACY_RETIREMENT_ROWS.iter().any(|row| {
            row.legacy_path.contains("structural intrinsic")
                && row.disposition == RetirementDisposition::Replaced
        }));
    }
}
