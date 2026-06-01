use crate::frontend_hir::{HirBinaryOp, HirExpr, HirExprId, HirExprKind, HirLiteral};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorNormalization {
    AlreadyUniform,
    FoldedConst {
        op: HirBinaryOp,
        lhs: i64,
        rhs: i64,
        result: i64,
    },
}

pub fn normalize_parser_operator(expr: &HirExpr) -> OperatorNormalization {
    match &expr.kind {
        HirExprKind::Binary { .. } => OperatorNormalization::AlreadyUniform,
        _ => OperatorNormalization::AlreadyUniform,
    }
}

pub fn fold_const_binary(
    op: HirBinaryOp,
    lhs_id: HirExprId,
    lhs: &HirExpr,
    rhs_id: HirExprId,
    rhs: &HirExpr,
) -> Option<OperatorNormalization> {
    let (
        HirExprKind::Literal(HirLiteral::Int(lhs_value)),
        HirExprKind::Literal(HirLiteral::Int(rhs_value)),
    ) = (&lhs.kind, &rhs.kind)
    else {
        return None;
    };
    let result = match op {
        HirBinaryOp::Add => lhs_value + rhs_value,
        HirBinaryOp::Sub => lhs_value - rhs_value,
        HirBinaryOp::Mul => lhs_value * rhs_value,
        _ => return None,
    };
    let _ = (lhs_id, rhs_id);
    Some(OperatorNormalization::FoldedConst {
        op,
        lhs: *lhs_value,
        rhs: *rhs_value,
        result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_hir::CstBackpointer;
    use crate::frontend_symbols::FrontendSourceSpan;

    fn expr(kind: HirExprKind) -> HirExpr {
        HirExpr {
            cst: CstBackpointer {
                syntax_kind: "Expr".to_string(),
                span: FrontendSourceSpan { start: 0, end: 1 },
            },
            kind,
        }
    }

    #[test]
    fn operator_normalization_accepts_uniform_binary_ops() {
        let binary = expr(HirExprKind::Binary {
            op: HirBinaryOp::Add,
            lhs: HirExprId(1),
            rhs: HirExprId(2),
        });
        assert_eq!(
            normalize_parser_operator(&binary),
            OperatorNormalization::AlreadyUniform
        );
    }

    #[test]
    fn operator_normalization_folds_constants_in_optimizer_step() {
        let lhs = expr(HirExprKind::Literal(HirLiteral::Int(2)));
        let rhs = expr(HirExprKind::Literal(HirLiteral::Int(3)));
        assert_eq!(
            fold_const_binary(HirBinaryOp::Add, HirExprId(1), &lhs, HirExprId(2), &rhs),
            Some(OperatorNormalization::FoldedConst {
                op: HirBinaryOp::Add,
                lhs: 2,
                rhs: 3,
                result: 5,
            })
        );
    }

    #[test]
    fn operator_normalization_does_not_fold_non_arithmetic_ops() {
        let lhs = expr(HirExprKind::Literal(HirLiteral::Int(2)));
        let rhs = expr(HirExprKind::Literal(HirLiteral::Int(3)));
        assert_eq!(
            fold_const_binary(HirBinaryOp::Eq, HirExprId(1), &lhs, HirExprId(2), &rhs),
            None
        );
    }
}
