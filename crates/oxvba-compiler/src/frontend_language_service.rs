use crate::frontend_hir::HirTypeId;
use crate::frontend_semantic_model::{SemanticDiagnostic, SemanticModel, SemanticNodeKey};
use crate::frontend_symbols::{FrontendSourceSpan, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeSemanticAnswer {
    pub symbol: Option<SymbolId>,
    pub ty: Option<HirTypeId>,
    pub diagnostics: Vec<SemanticDiagnostic>,
}

pub fn answer_ide_query(
    model: &SemanticModel,
    key: &SemanticNodeKey,
    span: FrontendSourceSpan,
) -> IdeSemanticAnswer {
    IdeSemanticAnswer {
        symbol: model.symbol_for_node(key),
        ty: model.type_for_node(key),
        diagnostics: model
            .diagnostics_for_span(span)
            .into_iter()
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_hir::{
        CstBackpointer, HirArenas, HirBuiltinType, HirExpr, HirExprKind, HirType, HirTypeKind,
    };
    use crate::frontend_semantic_model::SemanticModel;
    use crate::frontend_symbols::{SourceProvenance, SymbolModel, SymbolNamespace};

    #[test]
    fn language_service_answers_from_shared_semantic_model_facts() {
        let mut symbols = SymbolModel::default();
        let symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Local,
                "x",
                SourceProvenance {
                    module_name: Some("Module1".to_string()),
                    span: Some(FrontendSourceSpan { start: 0, end: 1 }),
                },
            )
            .expect("symbol");
        let mut hir = HirArenas::default();
        let cst = CstBackpointer {
            syntax_kind: "NameExpr".to_string(),
            span: FrontendSourceSpan { start: 10, end: 11 },
        };
        let expr = hir.alloc_expr(HirExpr {
            cst: cst.clone(),
            kind: HirExprKind::Name(symbol),
        });
        let ty = hir.alloc_type(HirType {
            cst: cst.clone(),
            kind: HirTypeKind::Builtin(HirBuiltinType::Long),
        });
        let mut model = SemanticModel::new(symbols, hir);
        model.bind_expr_node(&cst, expr);
        model.record_expr_type(expr, ty);
        model.push_diagnostic(SemanticDiagnostic {
            span: FrontendSourceSpan { start: 10, end: 11 },
            code: "BIND-I-SHARED".to_string(),
            message: "shared fact".to_string(),
        });

        let answer = answer_ide_query(
            &model,
            &SemanticNodeKey::from(&cst),
            FrontendSourceSpan { start: 10, end: 11 },
        );
        assert_eq!(answer.symbol, Some(symbol));
        assert_eq!(answer.ty, Some(ty));
        assert_eq!(answer.diagnostics[0].code, "BIND-I-SHARED");
    }
}
