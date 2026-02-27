use crate::resolve::{BoundExpr, BoundModule, BoundStmt};

pub fn optimize_module(mut module: BoundModule) -> BoundModule {
    for procedure in &mut module.procedures {
        procedure.body = optimize_stmt_list(std::mem::take(&mut procedure.body));
    }
    module.body = optimize_stmt_list(std::mem::take(&mut module.body));
    module
}

fn optimize_stmt_list(stmts: Vec<BoundStmt>) -> Vec<BoundStmt> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            BoundStmt::Assign { target, expr } => {
                let is_noop = matches!(
                    &expr,
                    BoundExpr::AddConst { var, delta } if *delta == 0 && var == &target
                ) || matches!(
                    &expr,
                    BoundExpr::SubConst { var, delta } if *delta == 0 && var == &target
                );
                if !is_noop {
                    out.push(BoundStmt::Assign { target, expr });
                }
            }
            BoundStmt::IfCond {
                cond,
                then_body,
                else_body,
            } => out.push(BoundStmt::IfCond {
                cond,
                then_body: optimize_stmt_list(then_body),
                else_body: optimize_stmt_list(else_body),
            }),
            BoundStmt::ForRange {
                var,
                start,
                end,
                body,
            } => out.push(BoundStmt::ForRange {
                var,
                start,
                end,
                body: optimize_stmt_list(body),
            }),
            BoundStmt::DoWhile {
                cond,
                body,
                post_check,
            } => out.push(BoundStmt::DoWhile {
                cond,
                body: optimize_stmt_list(body),
                post_check,
            }),
            BoundStmt::SelectCase {
                expr,
                arms,
                else_body,
            } => {
                let next_arms = arms
                    .into_iter()
                    .map(|(values, body)| (values, optimize_stmt_list(body)))
                    .collect::<Vec<_>>();
                out.push(BoundStmt::SelectCase {
                    expr,
                    arms: next_arms,
                    else_body: optimize_stmt_list(else_body),
                });
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::optimize_module;
    use crate::resolve::{BoundStmt, resolve_symbols};

    #[test]
    fn formal_v19_noop_assignments_removed() {
        let module = resolve_symbols("Sub Main()\nDim x\nx = 1\nx = x + 0\nEnd Sub");
        let optimized = optimize_module(module);
        let has_noop = optimized.body.iter().any(|stmt| {
            matches!(stmt, BoundStmt::Assign { target, expr } if matches!(expr, crate::resolve::BoundExpr::AddConst { var, delta } if var == target && *delta == 0))
        });
        assert!(!has_noop);
    }

    #[test]
    fn formal_v19_optimizer_preserves_non_noop_assignments() {
        let module = resolve_symbols("Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub");
        let optimized = optimize_module(module);
        let add_two = optimized.body.iter().any(|stmt| {
            matches!(stmt, BoundStmt::Assign { expr, .. } if matches!(expr, crate::resolve::BoundExpr::AddConst { delta: 2, .. }))
        });
        assert!(add_two);
    }

    #[test]
    fn formal_v19_nested_blocks_optimized_safely() {
        let module = resolve_symbols(
            "Sub Main()\nDim x\nIf x = 0 Then\nx = x + 0\nElse\nx = x + 1\nEnd If\nEnd Sub",
        );
        let optimized = optimize_module(module);
        let mut seen_else_add = false;
        for stmt in optimized.body {
            if let BoundStmt::IfCond {
                then_body,
                else_body,
                ..
            } = stmt
            {
                assert!(then_body.is_empty());
                seen_else_add = else_body.iter().any(|inner| {
                    matches!(inner, BoundStmt::Assign { expr, .. } if matches!(expr, crate::resolve::BoundExpr::AddConst { delta: 1, .. }))
                });
            }
        }
        assert!(seen_else_add);
    }
}
