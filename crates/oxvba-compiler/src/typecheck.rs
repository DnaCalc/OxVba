use std::collections::HashSet;

use crate::resolve::{BoundCond, BoundExpr, BoundModule, BoundStmt};

pub fn check_types(module: BoundModule) -> Result<BoundModule, String> {
    let mut module = module;
    let mut declared: HashSet<String> = module.declarations.iter().cloned().collect();
    check_stmt_list(
        &module.body,
        module.option_explicit,
        &mut declared,
        &mut module.declarations,
    )?;
    Ok(module)
}

fn check_stmt_list(
    stmts: &[BoundStmt],
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
) -> Result<(), String> {
    for stmt in stmts {
        check_stmt(stmt, option_explicit, declared, declarations)?;
    }
    Ok(())
}

fn check_stmt(
    stmt: &BoundStmt,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
) -> Result<(), String> {
    match stmt {
        BoundStmt::Assign { target, expr } => {
            ensure_declared(target, option_explicit, declared, declarations)?;
            check_expr(expr, option_explicit, declared, declarations)
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            check_condition(cond, option_explicit, declared, declarations)?;
            check_stmt_list(then_body, option_explicit, declared, declarations)?;
            check_stmt_list(else_body, option_explicit, declared, declarations)
        }
        BoundStmt::ForRange {
            var,
            start,
            end,
            body,
        } => {
            ensure_declared(var, option_explicit, declared, declarations)?;
            check_expr(start, option_explicit, declared, declarations)?;
            check_expr(end, option_explicit, declared, declarations)?;
            check_stmt_list(body, option_explicit, declared, declarations)
        }
        BoundStmt::DoWhile { cond, body, .. } => {
            check_condition(cond, option_explicit, declared, declarations)?;
            check_stmt_list(body, option_explicit, declared, declarations)
        }
        BoundStmt::ExitDo => Ok(()),
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            check_expr(expr, option_explicit, declared, declarations)?;
            for (_, body) in arms {
                check_stmt_list(body, option_explicit, declared, declarations)?;
            }
            check_stmt_list(else_body, option_explicit, declared, declarations)
        }
        BoundStmt::Unsupported { line } => Err(format!("unsupported statement: {line}")),
    }
}

fn check_condition(
    cond: &BoundCond,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
) -> Result<(), String> {
    match cond {
        BoundCond::Compare { lhs, rhs, .. } => {
            check_expr(lhs, option_explicit, declared, declarations)?;
            check_expr(rhs, option_explicit, declared, declarations)
        }
        BoundCond::Truthy(expr) => check_expr(expr, option_explicit, declared, declarations),
        BoundCond::Not(inner) => check_condition(inner, option_explicit, declared, declarations),
        BoundCond::And(lhs, rhs) | BoundCond::Or(lhs, rhs) => {
            check_condition(lhs, option_explicit, declared, declarations)?;
            check_condition(rhs, option_explicit, declared, declarations)
        }
    }
}

fn check_expr(
    expr: &BoundExpr,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
) -> Result<(), String> {
    match expr {
        BoundExpr::IntConst(_) => Ok(()),
        BoundExpr::Var(name) => ensure_declared(name, option_explicit, declared, declarations),
        BoundExpr::AddConst { var, .. } | BoundExpr::SubConst { var, .. } => {
            ensure_declared(var, option_explicit, declared, declarations)
        }
    }
}

fn ensure_declared(
    name: &str,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
) -> Result<(), String> {
    if declared.contains(name) {
        return Ok(());
    }

    if option_explicit {
        return Err(format!("use of undeclared variable: {name}"));
    }

    declared.insert(name.to_string());
    declarations.push(name.to_string());
    Ok(())
}
