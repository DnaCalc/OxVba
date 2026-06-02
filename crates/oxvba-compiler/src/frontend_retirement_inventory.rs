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
    pub partial_work: &'static str,
    pub closure_condition: &'static str,
}

pub const LEGACY_RETIREMENT_ROWS: &[LegacyRetirementRow] = &[
    LegacyRetirementRow {
        legacy_path: "resolve::parse_expr_for_syntax_bridge",
        replacement: "frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir for scoped production assignment/expression lowering",
        disposition: RetirementDisposition::Replaced,
        owner: "bd-aprs.10.2",
        partial_work: "FE-4.1 first replaced the ad-hoc bridge hook with CST expression lowering; FE-8.5 then moved the completed scoped production route to HIR lowering.",
        closure_condition: "scoped assignment/expression fixtures route to HIR production before any legacy compile fallback",
    },
    LegacyRetirementRow {
        legacy_path: "compile_with_options default legacy fallback after HIR Unsupported",
        replacement: "construct-by-construct HIR production lowering owned by FE-6/FE-7/FE-8 follow-up beads",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.9.6",
        partial_work: "The default compile options now try HIR production lowering directly; unsupported constructs still compile by falling back to the existing resolver/lowering path outside explicit frontend_v2 mode.",
        closure_condition: "every construct in the claimed scoped surface either routes to HIR production or has an explicit out-of-scope residual row before terminal closure",
    },
    LegacyRetirementRow {
        legacy_path: "resolve::parse_expr substring splitting inside production resolver",
        replacement: "oxvba-syntax Pratt parser -> binder/HIR -> HIR production lowering",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.9.6",
        partial_work: "Operator normalization removed parser-produced AddConst/SubConst fast paths and scoped HIR lowering bypasses parse_expr for completed assignment/expression fixtures.",
        closure_condition: "no scoped production fixture reaches compile(source)/resolve::resolve_symbols as its authoritative expression parser",
    },
    LegacyRetirementRow {
        legacy_path: "bundle.rs module fact resolve_symbols fallback",
        replacement: "HIR BoundModule facts from frontend_type_hooks plus frontend_hir_lowering",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.10.8",
        partial_work: "Bundle module context fact extraction now tries HIR BoundModule construction before falling back to resolve_symbols for unsupported residual modules; the route decision is test-visible so HIR-owned facts and residual fallback cannot be conflated.",
        closure_condition: "bundle context facts for every accepted module are proven to come from HIR facts, with legacy resolver fallback deleted or marked comparison-only",
    },
    LegacyRetirementRow {
        legacy_path: "project.rs text rewrites for project/class/COM/default-member semantics",
        replacement: "frontend_project_symbols/frontend_member_dispatch/frontend_assignment_semantics/frontend_class_semantics plus HIR lowering",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.7.*, bd-aprs.9.6",
        partial_work: "Project symbol, member-dispatch, class, event, and external-reference indices now classify several routes, but project.rs rewrites remain load-bearing for broad project semantics.",
        closure_condition: "migrated project/class/COM/default-member fixture rows have route proof plus deletion or compatibility quarantine of the corresponding rewrite",
    },
    LegacyRetirementRow {
        legacy_path: "syntax_bridge::lower_cst_expr CST-to-legacy expression bridge",
        replacement: "typed HIR expression facts lowered through frontend_hir_lowering",
        disposition: RetirementDisposition::QuarantinedResidual,
        owner: "bd-aprs.9.6",
        partial_work: "CST expression lowering still exists as a compatibility bridge for bridge-specific tests and unsupported HIR constructs.",
        closure_condition: "terminal route audit proves the expression bridge is test-only/compatibility-only or deletes it",
    },
    LegacyRetirementRow {
        legacy_path: "stringly structural intrinsic names",
        replacement: "frontend_structural_intrinsics::StructuralIntrinsic",
        disposition: RetirementDisposition::Replaced,
        owner: "bd-aprs.9.1",
        partial_work: "Structural intrinsics for Null, Nothing, omitted arguments, project instances, pointer helpers, WithEvents helpers, and invoke helpers now use typed enum variants.",
        closure_condition: "remaining magic names are genuine library/runtime helper names, not compiler structural concepts",
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
            assert!(!row.partial_work.is_empty(), "{row:#?}");
            assert!(!row.closure_condition.is_empty(), "{row:#?}");
        }
    }

    #[test]
    fn retirement_inventory_records_structural_intrinsic_replacement() {
        assert!(LEGACY_RETIREMENT_ROWS.iter().any(|row| {
            row.legacy_path.contains("structural intrinsic")
                && row.disposition == RetirementDisposition::Replaced
        }));
    }

    #[test]
    fn retirement_inventory_distinguishes_hir_route_from_legacy_fallback() {
        use crate::syntax_bridge::{SyntaxBridgeProductionRoute, production_route_for_source};

        let scoped_source = "Sub Main()\nDim x As Long\nx = 1 + 2\nEnd Sub\n";
        assert_eq!(
            production_route_for_source(scoped_source).expect("route classification"),
            SyntaxBridgeProductionRoute::HirProduction
        );

        let fallback_source = "Sub Main()\nDim x\nx = 1 Xor 2\nEnd Sub\n";
        assert_eq!(
            production_route_for_source(fallback_source).expect("route classification"),
            SyntaxBridgeProductionRoute::HirUnsupportedResidual
        );

        assert!(LEGACY_RETIREMENT_ROWS.iter().any(|row| {
            row.legacy_path.contains("fallback")
                && row.disposition == RetirementDisposition::QuarantinedResidual
        }));
    }
}
