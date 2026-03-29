use crate::resolve::{
    BoundCaseClause, BoundCond, BoundExpr, BoundModule, BoundStmt, CompareOp, IntrinsicSurface,
    intrinsic_surface,
};

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
            BoundStmt::Assign {
                target,
                expr,
                intent,
            } => {
                let is_noop = matches!(
                    &expr,
                    BoundExpr::AddConst { var, delta } if *delta == 0 && var == &target
                ) || matches!(
                    &expr,
                    BoundExpr::SubConst { var, delta } if *delta == 0 && var == &target
                ) || matches!(
                    &expr,
                    BoundExpr::Var(var) if var == &target
                );

                if let Some(BoundStmt::Assign {
                    target: prev_target,
                    expr: prev_expr,
                    ..
                }) = out.last()
                    && prev_target == &target
                    && !expr_uses_var(&expr, &target)
                    && !expr_has_observable_effect(prev_expr)
                {
                    out.pop();
                }

                if !is_noop {
                    out.push(BoundStmt::Assign {
                        target,
                        expr,
                        intent,
                    });
                }
            }
            BoundStmt::IfCond {
                cond,
                then_body,
                else_body,
            } => {
                let then_body = optimize_stmt_list(then_body);
                let else_body = optimize_stmt_list(else_body);
                if let Some(always_true) = eval_const_cond(&cond) {
                    if always_true {
                        out.extend(then_body);
                    } else {
                        out.extend(else_body);
                    }
                } else {
                    out.push(BoundStmt::IfCond {
                        cond,
                        then_body,
                        else_body,
                    });
                }
            }
            BoundStmt::ForRange {
                var,
                start,
                end,
                step,
                body,
            } => {
                let body = optimize_stmt_list(body);
                out.push(BoundStmt::ForRange {
                    var,
                    start,
                    end,
                    step,
                    body,
                });
            }
            BoundStmt::ForEach {
                var,
                items,
                iterable,
                body,
            } => {
                let body = optimize_stmt_list(body);
                out.push(BoundStmt::ForEach {
                    var,
                    items,
                    iterable,
                    body,
                });
            }
            BoundStmt::DoWhile {
                cond,
                body,
                post_check,
            } => {
                let body = optimize_stmt_list(body);
                if !post_check && matches!(eval_const_cond(&cond), Some(false)) {
                    continue;
                }
                out.push(BoundStmt::DoWhile {
                    cond,
                    body,
                    post_check,
                });
            }
            BoundStmt::SelectCase {
                expr,
                arms,
                else_body,
            } => {
                let next_arms = arms
                    .into_iter()
                    .map(|(values, body)| (values, optimize_stmt_list(body)))
                    .collect::<Vec<_>>();
                let else_body = optimize_stmt_list(else_body);

                if let Some(value) = eval_const_expr(&expr) {
                    let mut selected = None;
                    for (clauses, body) in next_arms {
                        if clauses
                            .iter()
                            .any(|clause| case_clause_matches_const(clause, value))
                        {
                            selected = Some(body);
                            break;
                        }
                    }
                    out.extend(selected.unwrap_or(else_body));
                } else {
                    out.push(BoundStmt::SelectCase {
                        expr,
                        arms: next_arms,
                        else_body,
                    });
                }
            }
            BoundStmt::AssignFromCall {
                target,
                name,
                args,
                intent,
            } => {
                out.push(BoundStmt::AssignFromCall {
                    target,
                    name,
                    args,
                    intent,
                });
            }
            other => out.push(other),
        }
    }
    out
}

fn eval_const_expr(expr: &BoundExpr) -> Option<i32> {
    match expr {
        BoundExpr::IntConst(v) => Some(*v),
        BoundExpr::BoolConst(v) => Some(if *v { -1 } else { 0 }),
        BoundExpr::FloatConst(_) => None,
        _ => None,
    }
}

fn expr_uses_var(expr: &BoundExpr, var: &str) -> bool {
    match expr {
        BoundExpr::Var(name) => name == var,
        BoundExpr::AddConst { var: name, .. } => name == var,
        BoundExpr::SubConst { var: name, .. } => name == var,
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_) => false,
        BoundExpr::IntrinsicCall { args, .. } => args.iter().any(|arg| expr_uses_var(arg, var)),
        BoundExpr::ProcCall { args, .. } => args.iter().any(|arg| expr_uses_var(&arg.expr, var)),
        BoundExpr::BinaryOp { lhs, rhs, .. } => expr_uses_var(lhs, var) || expr_uses_var(rhs, var),
        BoundExpr::UnaryOp { operand, .. } => expr_uses_var(operand, var),
    }
}

fn expr_has_observable_effect(expr: &BoundExpr) -> bool {
    match expr {
        BoundExpr::IntrinsicCall { name, args } => {
            let call_has_effect = name.eq_ignore_ascii_case("__oxvba_withevents_set")
                || name.eq_ignore_ascii_case("__oxvba_withevents_clear_owner")
                || name.eq_ignore_ascii_case("__oxvba_withevents_first_owner")
                || name.eq_ignore_ascii_case("__oxvba_withevents_next_owner")
                || matches!(
                    intrinsic_surface(name.as_str()),
                    Some(IntrinsicSurface::HostSensitive)
                );
            call_has_effect || args.iter().any(expr_has_observable_effect)
        }
        BoundExpr::ProcCall { .. } => true,
        BoundExpr::Var(_)
        | BoundExpr::AddConst { .. }
        | BoundExpr::SubConst { .. }
        | BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_) => false,
        BoundExpr::BinaryOp { lhs, rhs, .. } => {
            expr_has_observable_effect(lhs) || expr_has_observable_effect(rhs)
        }
        BoundExpr::UnaryOp { operand, .. } => expr_has_observable_effect(operand),
    }
}

fn eval_const_cond(cond: &BoundCond) -> Option<bool> {
    match cond {
        BoundCond::Compare { op, lhs, rhs } => {
            let lhs = eval_const_expr(lhs)?;
            let rhs = eval_const_expr(rhs)?;
            Some(match op {
                CompareOp::Eq => lhs == rhs,
                CompareOp::Ne => lhs != rhs,
                CompareOp::Lt => lhs < rhs,
                CompareOp::Le => lhs <= rhs,
                CompareOp::Gt => lhs > rhs,
                CompareOp::Ge => lhs >= rhs,
                CompareOp::Like => lhs == rhs,
            })
        }
        BoundCond::Truthy(expr) => Some(eval_const_expr(expr)? != 0),
        BoundCond::Not(inner) => Some(!eval_const_cond(inner)?),
        BoundCond::And(lhs, rhs) => Some(eval_const_cond(lhs)? && eval_const_cond(rhs)?),
        BoundCond::Or(lhs, rhs) => Some(eval_const_cond(lhs)? || eval_const_cond(rhs)?),
    }
}

fn case_clause_matches_const(clause: &BoundCaseClause, value: i32) -> bool {
    match clause {
        BoundCaseClause::Value(exact) => value == *exact,
        BoundCaseClause::Is { op, value: rhs } => match op {
            CompareOp::Eq => value == *rhs,
            CompareOp::Ne => value != *rhs,
            CompareOp::Lt => value < *rhs,
            CompareOp::Le => value <= *rhs,
            CompareOp::Gt => value > *rhs,
            CompareOp::Ge => value >= *rhs,
            CompareOp::Like => value == *rhs,
        },
        BoundCaseClause::Range { start, end } => value >= *start && value <= *end,
    }
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
            matches!(stmt, BoundStmt::Assign { target, expr, .. } if matches!(expr, crate::resolve::BoundExpr::AddConst { var, delta } if var == target && *delta == 0))
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

    #[test]
    fn formal_v19_constant_if_eliminates_unreachable_branch() {
        let module = resolve_symbols(
            "Sub Main()\nDim x\nIf 1 = 1 Then\nx = 3\nElse\nx = 4\nEnd If\nEnd Sub",
        );
        let optimized = optimize_module(module);
        assert_eq!(optimized.body.len(), 1);
        assert!(matches!(optimized.body[0], BoundStmt::Assign { .. }));
    }

    #[test]
    fn formal_v19_constant_select_case_is_folded() {
        let module = resolve_symbols(
            "Sub Main()\nDim x\nSelect Case 2\nCase 1\nx = 10\nCase 2\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub",
        );
        let optimized = optimize_module(module);
        assert!(
            optimized
                .body
                .iter()
                .all(|stmt| !matches!(stmt, BoundStmt::SelectCase { .. }))
        );
        assert!(
            optimized
                .body
                .iter()
                .any(|stmt| matches!(stmt, BoundStmt::Assign { .. }))
        );
    }

    #[test]
    fn formal_v19_for_loop_semantics_preserved_with_optimized_body() {
        let module =
            resolve_symbols("Sub Main()\nDim i\nFor i = 3 To 1\ni = i + 0\nNext i\nEnd Sub");
        let optimized = optimize_module(module);
        let mut saw_loop = false;
        for stmt in optimized.body {
            if let BoundStmt::ForRange { body, .. } = stmt {
                saw_loop = true;
                assert!(body.is_empty());
            }
        }
        assert!(saw_loop);
    }

    #[test]
    fn formal_v19_consecutive_dead_store_is_eliminated() {
        let module = resolve_symbols("Sub Main()\nDim x\nx = 1\nx = 2\nEnd Sub");
        let optimized = optimize_module(module);
        let assigns = optimized
            .body
            .iter()
            .filter(|stmt| matches!(stmt, BoundStmt::Assign { .. }))
            .count();
        assert_eq!(assigns, 1);
    }

    #[test]
    fn formal_v19_dead_store_preserves_previous_assignment_with_observable_effect() {
        let module = resolve_symbols(
            "Sub Main()\nDim x\nx = __oxvba_withevents_set(0, 7, 11)\nx = __oxvba_withevents_get(0, 7)\nEnd Sub",
        );
        let optimized = optimize_module(module);
        let intrinsic_set_count = optimized
            .body
            .iter()
            .filter(|stmt| {
                matches!(
                    stmt,
                    BoundStmt::Assign {
                        expr: crate::resolve::BoundExpr::IntrinsicCall { name, .. },
                        ..
                    } if name == "__oxvba_withevents_set"
                )
            })
            .count();
        assert_eq!(intrinsic_set_count, 1);
    }

    #[test]
    fn formal_v19_dead_store_preserves_withevents_owner_iteration_intrinsics() {
        let module = resolve_symbols(
            "Sub Main()\nDim x\nx = __oxvba_withevents_first_owner(1, 7)\nx = __oxvba_withevents_next_owner()\nEnd Sub",
        );
        let optimized = optimize_module(module);
        let iterator_calls = optimized
            .body
            .iter()
            .filter(|stmt| {
                matches!(
                    stmt,
                    BoundStmt::Assign {
                        expr: crate::resolve::BoundExpr::IntrinsicCall { name, .. },
                        ..
                    } if name == "__oxvba_withevents_first_owner"
                        || name == "__oxvba_withevents_next_owner"
                )
            })
            .count();
        assert_eq!(iterator_calls, 2);
    }

    #[test]
    fn formal_v19_variant_source_assignment_intent_is_not_constant_folded() {
        let module = resolve_symbols(
            "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nSet v = src\nEnd Sub",
        );
        let optimized = optimize_module(module);
        assert!(optimized.body.iter().any(|stmt| {
            matches!(
                stmt,
                BoundStmt::Assign {
                    target,
                    expr: crate::resolve::BoundExpr::Var(source),
                    intent: crate::resolve::AssignmentIntent::Set,
                } if target == "v" && source == "src"
            )
        }));
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::optimize_module;
    use crate::resolve::{BoundExpr, BoundModule, BoundStmt};

    #[kani::proof]
    fn zero_delta_self_add_assignment_is_removed() {
        let delta: i32 = kani::any();
        let module = BoundModule {
            source: "Sub Main()\nEnd Sub".to_string(),
            option_explicit: false,
            compare_mode: crate::resolve::BoundCompareMode::Binary,
            default_type_table: [crate::resolve::BoundType::Variant; 26],
            resolution_diagnostics: Vec::new(),
            declarations: vec!["x".to_string()],
            declaration_types: std::collections::HashMap::from([(
                "x".to_string(),
                crate::resolve::BoundType::Variant,
            )]),
            array_descriptors: std::collections::HashMap::new(),
            external_declarations: std::collections::HashMap::new(),
            body: vec![BoundStmt::Assign {
                target: "x".to_string(),
                expr: BoundExpr::AddConst {
                    var: "x".to_string(),
                    delta,
                },
            }],
            procedures: Vec::new(),
        };

        let optimized = optimize_module(module);
        if delta == 0 {
            assert!(optimized.body.is_empty());
        } else {
            assert_eq!(optimized.body.len(), 1);
        }
    }
}
