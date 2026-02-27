use std::collections::HashSet;

use crate::resolve::{BoundCond, BoundExpr, BoundModule, BoundStmt};

pub fn check_types(module: BoundModule) -> Result<BoundModule, String> {
    let mut module = module;
    let proc_names: HashSet<String> = module.procedures.iter().map(|p| p.name.clone()).collect();

    for procedure in &mut module.procedures {
        let mut declared: HashSet<String> = procedure.declarations.iter().cloned().collect();
        check_stmt_list(
            &procedure.body,
            module.option_explicit,
            &mut declared,
            &mut procedure.declarations,
            &proc_names,
        )?;
    }

    if let Some(entry) = module
        .procedures
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("main"))
        .or_else(|| module.procedures.first())
    {
        module.declarations = entry.declarations.clone();
        module.body = entry.body.clone();
    }

    Ok(module)
}

fn check_stmt_list(
    stmts: &[BoundStmt],
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
    proc_names: &HashSet<String>,
) -> Result<(), String> {
    for stmt in stmts {
        check_stmt(stmt, option_explicit, declared, declarations, proc_names)?;
    }
    Ok(())
}

fn check_stmt(
    stmt: &BoundStmt,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
    proc_names: &HashSet<String>,
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
            check_stmt_list(
                then_body,
                option_explicit,
                declared,
                declarations,
                proc_names,
            )?;
            check_stmt_list(
                else_body,
                option_explicit,
                declared,
                declarations,
                proc_names,
            )
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
            check_stmt_list(body, option_explicit, declared, declarations, proc_names)
        }
        BoundStmt::DoWhile { cond, body, .. } => {
            check_condition(cond, option_explicit, declared, declarations)?;
            check_stmt_list(body, option_explicit, declared, declarations, proc_names)
        }
        BoundStmt::ExitDo => Ok(()),
        BoundStmt::Call { name } => {
            if proc_names.contains(name) {
                Ok(())
            } else {
                Err(format!("call to unknown procedure: {name}"))
            }
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            check_expr(expr, option_explicit, declared, declarations)?;
            for (_, body) in arms {
                check_stmt_list(body, option_explicit, declared, declarations, proc_names)?;
            }
            check_stmt_list(
                else_body,
                option_explicit,
                declared,
                declarations,
                proc_names,
            )
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
