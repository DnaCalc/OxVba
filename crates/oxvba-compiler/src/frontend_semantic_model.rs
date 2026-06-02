use std::collections::BTreeMap;

use crate::frontend_hir::{
    BoundHirModule, CstBackpointer, HirArenas, HirCaseClause, HirExprId, HirExprKind, HirStmtId,
    HirStmtKind, HirTypeId,
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

    pub fn from_bound_hir_module(module: BoundHirModule) -> Self {
        let BoundHirModule {
            symbols,
            arenas,
            declarations,
        } = module;
        let mut model = Self::new(symbols, arenas);
        for decl in declarations {
            let Some(decl) = model.hir.decl(decl).cloned() else {
                continue;
            };
            if let crate::frontend_hir::HirDeclKind::Procedure { body, .. } = decl.kind {
                for stmt in body {
                    model.index_stmt_tree(stmt);
                }
            }
        }
        model
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

    pub fn expr_for_span(&self, span: FrontendSourceSpan) -> Option<HirExprId> {
        self.exprs_by_node
            .iter()
            .find(|(key, _)| key.span == span)
            .map(|(_, expr)| *expr)
    }

    pub fn stmt_for_span(&self, span: FrontendSourceSpan) -> Option<HirStmtId> {
        self.stmts_by_node
            .iter()
            .find(|(key, _)| key.span == span)
            .map(|(_, stmt)| *stmt)
    }

    pub fn symbol_for_node(&self, key: &SemanticNodeKey) -> Option<SymbolId> {
        let expr = self.expr_for_node(key)?;
        self.symbol_for_expr(expr)
    }

    pub fn symbol_for_span(&self, span: FrontendSourceSpan) -> Option<SymbolId> {
        let expr = self.expr_for_span(span)?;
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

    fn index_stmt_tree(&mut self, stmt: HirStmtId) {
        let Some(stmt_data) = self.hir.stmt(stmt).cloned() else {
            return;
        };
        self.bind_stmt_node(&stmt_data.cst, stmt);
        match stmt_data.kind {
            HirStmtKind::Let { target, value } | HirStmtKind::Set { target, value } => {
                self.index_expr_tree(target);
                self.index_expr_tree(value);
            }
            HirStmtKind::Expr(expr) => self.index_expr_tree(expr),
            HirStmtKind::If {
                condition,
                then_body,
                else_body,
            } => {
                self.index_expr_tree(condition);
                for stmt in then_body.into_iter().chain(else_body) {
                    self.index_stmt_tree(stmt);
                }
            }
            HirStmtKind::DoWhile {
                condition, body, ..
            } => {
                self.index_expr_tree(condition);
                for stmt in body {
                    self.index_stmt_tree(stmt);
                }
            }
            HirStmtKind::SelectCase {
                expr,
                arms,
                else_body,
            } => {
                self.index_expr_tree(expr);
                for (clauses, body) in arms {
                    for clause in clauses {
                        self.index_case_clause(clause);
                    }
                    for stmt in body {
                        self.index_stmt_tree(stmt);
                    }
                }
                for stmt in else_body {
                    self.index_stmt_tree(stmt);
                }
            }
            HirStmtKind::ForRange {
                start,
                end,
                step,
                body,
                ..
            } => {
                self.index_expr_tree(start);
                self.index_expr_tree(end);
                if let Some(step) = step {
                    self.index_expr_tree(step);
                }
                for stmt in body {
                    self.index_stmt_tree(stmt);
                }
            }
            HirStmtKind::ForEach { iterable, body, .. } => {
                self.index_expr_tree(iterable);
                for stmt in body {
                    self.index_stmt_tree(stmt);
                }
            }
            HirStmtKind::ReDim { bounds, .. } => {
                for bound in bounds {
                    if let Some(lower) = bound.lower {
                        self.index_expr_tree(lower);
                    }
                    self.index_expr_tree(bound.upper);
                }
            }
            HirStmtKind::RaiseEvent { args, .. } => {
                for arg in args {
                    self.index_expr_tree(arg);
                }
            }
            HirStmtKind::ConsolePrint { data } | HirStmtKind::DebugPrint { data } => {
                self.index_expr_tree(data);
            }
            HirStmtKind::FileKill { path } => {
                self.index_expr_tree(path);
            }
            HirStmtKind::Block(stmts) => {
                for stmt in stmts {
                    self.index_stmt_tree(stmt);
                }
            }
            HirStmtKind::Empty
            | HirStmtKind::ExitDo
            | HirStmtKind::ExitFor
            | HirStmtKind::ExitProcedure
            | HirStmtKind::OnErrorResumeNext
            | HirStmtKind::OnErrorGoto0
            | HirStmtKind::OnErrorGotoLabel { .. }
            | HirStmtKind::ResumeNext
            | HirStmtKind::Resume
            | HirStmtKind::ResumeLabel { .. }
            | HirStmtKind::Label { .. }
            | HirStmtKind::GoTo { .. }
            | HirStmtKind::GoSub { .. }
            | HirStmtKind::Return
            | HirStmtKind::Erase { .. }
            | HirStmtKind::ConsoleInput { .. } => {}
        }
    }

    fn index_case_clause(&mut self, clause: HirCaseClause) {
        match clause {
            HirCaseClause::Value(expr) => self.index_expr_tree(expr),
            HirCaseClause::Range { start, end } => {
                self.index_expr_tree(start);
                self.index_expr_tree(end);
            }
            HirCaseClause::Is { value, .. } => self.index_expr_tree(value),
        }
    }

    fn index_expr_tree(&mut self, expr: HirExprId) {
        let Some(expr_data) = self.hir.expr(expr).cloned() else {
            return;
        };
        self.bind_expr_node(&expr_data.cst, expr);
        match expr_data.kind {
            HirExprKind::Name(symbol) => self.record_expr_symbol(expr, symbol),
            HirExprKind::Unary { expr: child, .. } => self.index_expr_tree(child),
            HirExprKind::Binary { lhs, rhs, .. } => {
                self.index_expr_tree(lhs);
                self.index_expr_tree(rhs);
            }
            HirExprKind::TypeOfIs { expr: child, .. } => self.index_expr_tree(child),
            HirExprKind::Call(call) => {
                let Some(call_data) = self.hir.call(call).cloned() else {
                    return;
                };
                self.index_expr_tree(call_data.target);
                for arg in call_data.args {
                    self.index_expr_tree(arg.expr);
                }
            }
            HirExprKind::Member(member) => {
                let Some(member_data) = self.hir.member(member).cloned() else {
                    return;
                };
                self.record_expr_symbol(expr, member_data.symbol);
                if let Some(receiver) = member_data.receiver {
                    self.index_expr_tree(receiver);
                }
            }
            HirExprKind::New { .. } | HirExprKind::Missing | HirExprKind::Literal(_) => {}
        }
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
        build_hir_from_source,
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

    #[test]
    fn semantic_model_indexes_symbols_from_cst_backed_hir() {
        let source = "Sub Main(ByVal seed As Long)\n    Dim x As Long\n    x = seed + 1\nEnd Sub\n";
        let hir = build_hir_from_source("Module1", source).expect("HIR module");
        let model = SemanticModel::from_bound_hir_module(hir);

        let seed_start = source.rfind("seed").expect("seed use");
        let seed_span = FrontendSourceSpan {
            start: seed_start,
            end: seed_start + "seed".len(),
        };
        let symbol = model
            .symbol_for_span(seed_span)
            .and_then(|symbol| model.symbols().symbol(symbol))
            .expect("seed symbol");
        let name = model.symbols().name(symbol.name).expect("seed name");
        assert_eq!(name.folded, "seed");
        assert_eq!(symbol.namespace, SymbolNamespace::Parameter);
    }

    #[test]
    fn semantic_model_indexes_statements_from_cst_backed_hir() {
        let source = "Sub Main()\n    Dim total As Long\n    total = 1\nEnd Sub\n";
        let hir = build_hir_from_source("Module1", source).expect("HIR module");
        let model = SemanticModel::from_bound_hir_module(hir);

        let assign_start = source.find("total = 1").expect("assignment");
        let assign_span = FrontendSourceSpan {
            start: assign_start,
            end: assign_start + "total = 1".len(),
        };
        let stmt = model
            .stmt_for_span(assign_span)
            .and_then(|stmt| model.hir().stmt(stmt))
            .expect("assignment statement");
        assert_eq!(stmt.cst.syntax_kind, "AssignStmt");
    }

    #[test]
    fn semantic_model_indexes_raise_event_argument_symbols() {
        let source = "Sub Main(ByVal n As Long)\nRaiseEvent Tick(n)\nEnd Sub\n";
        let hir = build_hir_from_source("Module1", source).expect("HIR module");
        let model = SemanticModel::from_bound_hir_module(hir);

        let arg_start = source.rfind("n)").expect("event argument");
        let arg_span = FrontendSourceSpan {
            start: arg_start,
            end: arg_start + "n".len(),
        };
        let symbol = model
            .symbol_for_span(arg_span)
            .and_then(|symbol| model.symbols().symbol(symbol))
            .expect("RaiseEvent argument symbol");
        let name = model.symbols().name(symbol.name).expect("argument name");
        assert_eq!(name.folded, "n");
        assert_eq!(symbol.namespace, SymbolNamespace::Parameter);
    }
}
