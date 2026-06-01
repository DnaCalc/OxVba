use std::collections::BTreeMap;

use crate::frontend_hir::{
    CstBackpointer, HirArenas, HirExprId, HirExprKind, HirStmtId, HirTypeId,
};
use crate::frontend_symbols::{FrontendSourceSpan, SymbolId, SymbolModel};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticNodeKey {
    pub syntax_kind: String,
    pub span: FrontendSourceSpan,
}

impl From<&CstBackpointer> for SemanticNodeKey {
    fn from(cst: &CstBackpointer) -> Self {
        Self {
            syntax_kind: cst.syntax_kind.clone(),
            span: cst.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub span: FrontendSourceSpan,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SemanticModel {
    symbols: SymbolModel,
    hir: HirArenas,
    exprs_by_node: BTreeMap<SemanticNodeKey, HirExprId>,
    stmts_by_node: BTreeMap<SemanticNodeKey, HirStmtId>,
    expr_symbols: BTreeMap<HirExprId, SymbolId>,
    expr_types: BTreeMap<HirExprId, HirTypeId>,
    diagnostics: Vec<SemanticDiagnostic>,
}

impl SemanticModel {
    pub fn new(symbols: SymbolModel, hir: HirArenas) -> Self {
        Self {
            symbols,
            hir,
            exprs_by_node: BTreeMap::new(),
            stmts_by_node: BTreeMap::new(),
            expr_symbols: BTreeMap::new(),
            expr_types: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn symbols(&self) -> &SymbolModel {
        &self.symbols
    }

    pub fn hir(&self) -> &HirArenas {
        &self.hir
    }

    pub fn bind_expr_node(&mut self, cst: &CstBackpointer, expr: HirExprId) {
        self.exprs_by_node.insert(SemanticNodeKey::from(cst), expr);
    }

    pub fn bind_stmt_node(&mut self, cst: &CstBackpointer, stmt: HirStmtId) {
        self.stmts_by_node.insert(SemanticNodeKey::from(cst), stmt);
    }

    pub fn record_expr_symbol(&mut self, expr: HirExprId, symbol: SymbolId) {
        self.expr_symbols.insert(expr, symbol);
    }

    pub fn record_expr_type(&mut self, expr: HirExprId, ty: HirTypeId) {
        self.expr_types.insert(expr, ty);
    }

    pub fn push_diagnostic(&mut self, diagnostic: SemanticDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn expr_for_node(&self, key: &SemanticNodeKey) -> Option<HirExprId> {
        self.exprs_by_node.get(key).copied()
    }

    pub fn stmt_for_node(&self, key: &SemanticNodeKey) -> Option<HirStmtId> {
        self.stmts_by_node.get(key).copied()
    }

    pub fn symbol_for_node(&self, key: &SemanticNodeKey) -> Option<SymbolId> {
        let expr = self.expr_for_node(key)?;
        self.symbol_for_expr(expr)
    }

    pub fn symbol_for_expr(&self, expr: HirExprId) -> Option<SymbolId> {
        self.expr_symbols.get(&expr).copied().or_else(|| {
            self.hir.expr(expr).and_then(|expr| match expr.kind {
                HirExprKind::Name(symbol) => Some(symbol),
                _ => None,
            })
        })
    }

    pub fn type_for_node(&self, key: &SemanticNodeKey) -> Option<HirTypeId> {
        self.expr_for_node(key)
            .and_then(|expr| self.type_for_expr(expr))
    }

    pub fn type_for_expr(&self, expr: HirExprId) -> Option<HirTypeId> {
        self.expr_types.get(&expr).copied()
    }

    pub fn diagnostics_for_span(&self, span: FrontendSourceSpan) -> Vec<&SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| spans_overlap(diagnostic.span, span))
            .collect()
    }

    pub fn diagnostics(&self) -> &[SemanticDiagnostic] {
        &self.diagnostics
    }
}

fn spans_overlap(left: FrontendSourceSpan, right: FrontendSourceSpan) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_hir::{
        CstBackpointer, HirBuiltinType, HirExpr, HirExprKind, HirLiteral, HirType, HirTypeKind,
    };
    use crate::frontend_symbols::{ScopeKind, SourceProvenance, SymbolModel, SymbolNamespace};

    fn cst(kind: &str, start: usize, end: usize) -> CstBackpointer {
        CstBackpointer {
            syntax_kind: kind.to_string(),
            span: FrontendSourceSpan { start, end },
        }
    }

    fn provenance(start: usize, end: usize) -> SourceProvenance {
        SourceProvenance {
            module_name: Some("Module1".to_string()),
            span: Some(FrontendSourceSpan { start, end }),
        }
    }

    #[test]
    fn semantic_model_answers_symbol_and_type_for_cst_node() {
        let mut symbols = SymbolModel::default();
        let scope = symbols
            .add_scope(ScopeKind::Procedure, symbols.global_scope(), Some("Main"))
            .expect("scope");
        let value_symbol = symbols
            .declare_symbol(scope, SymbolNamespace::Local, "x", provenance(10, 11))
            .expect("symbol");

        let mut hir = HirArenas::default();
        let name_cst = cst("NameExpr", 30, 31);
        let expr = hir.alloc_expr(HirExpr {
            cst: name_cst.clone(),
            kind: HirExprKind::Name(value_symbol),
        });
        let ty = hir.alloc_type(HirType {
            cst: cst("TypeExpr", 12, 16),
            kind: HirTypeKind::Builtin(HirBuiltinType::Long),
        });

        let mut model = SemanticModel::new(symbols, hir);
        model.bind_expr_node(&name_cst, expr);
        model.record_expr_type(expr, ty);

        let key = SemanticNodeKey::from(&name_cst);
        assert_eq!(model.symbol_for_node(&key), Some(value_symbol));
        assert_eq!(model.type_for_node(&key), Some(ty));
    }

    #[test]
    fn semantic_model_reuses_explicit_hir_facts_for_non_name_exprs() {
        let mut symbols = SymbolModel::default();
        let function_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Procedure,
                "MakeValue",
                provenance(0, 9),
            )
            .expect("procedure symbol");

        let mut hir = HirArenas::default();
        let literal_cst = cst("IntLiteral", 20, 21);
        let literal = hir.alloc_expr(HirExpr {
            cst: literal_cst.clone(),
            kind: HirExprKind::Literal(HirLiteral::Int(1)),
        });

        let mut model = SemanticModel::new(symbols, hir);
        model.bind_expr_node(&literal_cst, literal);
        model.record_expr_symbol(literal, function_symbol);

        assert_eq!(
            model.symbol_for_node(&SemanticNodeKey::from(&literal_cst)),
            Some(function_symbol)
        );
    }

    #[test]
    fn semantic_model_filters_diagnostics_by_overlapping_span() {
        let mut model = SemanticModel::new(SymbolModel::default(), HirArenas::default());
        model.push_diagnostic(SemanticDiagnostic {
            span: FrontendSourceSpan { start: 10, end: 20 },
            code: "BIND001".to_string(),
            message: "unresolved name".to_string(),
        });
        model.push_diagnostic(SemanticDiagnostic {
            span: FrontendSourceSpan { start: 40, end: 50 },
            code: "BIND002".to_string(),
            message: "type mismatch".to_string(),
        });

        let diagnostics = model.diagnostics_for_span(FrontendSourceSpan { start: 15, end: 16 });
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "BIND001");
    }
}
