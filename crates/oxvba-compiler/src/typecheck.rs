use std::collections::{HashMap, HashSet};

use crate::resolve::{BoundCallArg, BoundCond, BoundExpr, BoundModule, BoundParam, BoundStmt};

pub fn check_types(module: BoundModule) -> Result<BoundModule, String> {
    let mut module = module;
    let proc_names: HashSet<String> = module.procedures.iter().map(|p| p.name.clone()).collect();
    let proc_params: HashMap<String, Vec<BoundParam>> = module
        .procedures
        .iter()
        .map(|p| (p.name.clone(), p.params.clone()))
        .collect();

    for procedure in &mut module.procedures {
        let mut declared: HashSet<String> = procedure.declarations.iter().cloned().collect();
        let labels = collect_labels(&procedure.body);
        check_stmt_list(
            &procedure.body,
            module.option_explicit,
            &mut declared,
            &mut procedure.declarations,
            &proc_names,
            &proc_params,
            &labels,
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
    proc_params: &HashMap<String, Vec<BoundParam>>,
    labels: &HashSet<String>,
) -> Result<(), String> {
    for stmt in stmts {
        check_stmt(
            stmt,
            option_explicit,
            declared,
            declarations,
            proc_names,
            proc_params,
            labels,
        )?;
    }
    Ok(())
}

fn check_stmt(
    stmt: &BoundStmt,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
    proc_names: &HashSet<String>,
    proc_params: &HashMap<String, Vec<BoundParam>>,
    labels: &HashSet<String>,
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
                proc_params,
                labels,
            )?;
            check_stmt_list(
                else_body,
                option_explicit,
                declared,
                declarations,
                proc_names,
                proc_params,
                labels,
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
            check_stmt_list(
                body,
                option_explicit,
                declared,
                declarations,
                proc_names,
                proc_params,
                labels,
            )
        }
        BoundStmt::ReDim { .. } => Ok(()),
        BoundStmt::DoWhile { cond, body, .. } => {
            check_condition(cond, option_explicit, declared, declarations)?;
            check_stmt_list(
                body,
                option_explicit,
                declared,
                declarations,
                proc_names,
                proc_params,
                labels,
            )
        }
        BoundStmt::ExitDo => Ok(()),
        BoundStmt::OnErrorResumeNext => Ok(()),
        BoundStmt::OnErrorGoto0 => Ok(()),
        BoundStmt::OnErrorGotoLabel { label } => {
            if labels.contains(label) {
                Ok(())
            } else {
                Err(format!("on error goto target label not found: {label}"))
            }
        }
        BoundStmt::ResumeNext => Ok(()),
        BoundStmt::RaiseError(_) => Ok(()),
        BoundStmt::Label { .. } => Ok(()),
        BoundStmt::GoSub { label } => {
            if labels.contains(label) {
                Ok(())
            } else {
                Err(format!("gosub target label not found: {label}"))
            }
        }
        BoundStmt::Return => Ok(()),
        BoundStmt::Call { name, args } => {
            if !proc_names.contains(name) {
                return Err(format!("call to unknown procedure: {name}"));
            }

            if let Some(params) = proc_params.get(name) {
                let mapped_args = map_call_args_to_params(name, args, params)?;
                for (idx, param) in params.iter().enumerate() {
                    let Some(arg) = mapped_args[idx] else {
                        continue;
                    };

                    if param.by_ref && !matches!(arg.expr, BoundExpr::Var(_)) {
                        return Err(format!(
                            "ByRef parameter {} requires variable argument",
                            param.name
                        ));
                    }
                    check_expr(&arg.expr, option_explicit, declared, declarations)?;
                }
            }
            Ok(())
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            check_expr(expr, option_explicit, declared, declarations)?;
            for (_, body) in arms {
                check_stmt_list(
                    body,
                    option_explicit,
                    declared,
                    declarations,
                    proc_names,
                    proc_params,
                    labels,
                )?;
            }
            check_stmt_list(
                else_body,
                option_explicit,
                declared,
                declarations,
                proc_names,
                proc_params,
                labels,
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
        BoundExpr::IntrinsicCall { args, .. } => {
            for arg in args {
                check_expr(arg, option_explicit, declared, declarations)?;
            }
            Ok(())
        }
    }
}

fn ensure_declared(
    name: &str,
    option_explicit: bool,
    declared: &mut HashSet<String>,
    declarations: &mut Vec<String>,
) -> Result<(), String> {
    if name.eq_ignore_ascii_case("err_number") {
        return Ok(());
    }

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

fn map_call_args_to_params<'a>(
    proc_name: &str,
    args: &'a [BoundCallArg],
    params: &[BoundParam],
) -> Result<Vec<Option<&'a BoundCallArg>>, String> {
    if args.len() > params.len() {
        return Err(format!(
            "procedure {proc_name} expects between {} and {} args, got {}",
            params.iter().filter(|p| !p.optional).count(),
            params.len(),
            args.len()
        ));
    }

    let mut mapped: Vec<Option<&BoundCallArg>> = vec![None; params.len()];
    let mut next_pos = 0usize;
    let mut seen_named = false;

    for arg in args {
        if let Some(name) = &arg.name {
            seen_named = true;
            let Some(param_idx) = params
                .iter()
                .position(|p| p.name.eq_ignore_ascii_case(name))
            else {
                return Err(format!(
                    "procedure {proc_name} has no parameter named {name}"
                ));
            };
            if mapped[param_idx].is_some() {
                return Err(format!(
                    "duplicate argument for parameter {}",
                    params[param_idx].name
                ));
            }
            mapped[param_idx] = Some(arg);
            continue;
        }

        if seen_named {
            return Err("positional argument cannot follow named argument".to_string());
        }

        while next_pos < params.len() && mapped[next_pos].is_some() {
            next_pos += 1;
        }
        if next_pos >= params.len() {
            return Err(format!(
                "procedure {proc_name} expects between {} and {} args, got {}",
                params.iter().filter(|p| !p.optional).count(),
                params.len(),
                args.len()
            ));
        }
        mapped[next_pos] = Some(arg);
        next_pos += 1;
    }

    for (idx, param) in params.iter().enumerate() {
        if !param.optional && mapped[idx].is_none() {
            return Err(format!("missing required argument {}", param.name));
        }
    }

    Ok(mapped)
}

fn collect_labels(stmts: &[BoundStmt]) -> HashSet<String> {
    let mut labels = HashSet::new();
    collect_labels_recursive(stmts, &mut labels);
    labels
}

fn collect_labels_recursive(stmts: &[BoundStmt], labels: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            BoundStmt::Label { name } => {
                labels.insert(name.clone());
            }
            BoundStmt::IfCond {
                then_body,
                else_body,
                ..
            } => {
                collect_labels_recursive(then_body, labels);
                collect_labels_recursive(else_body, labels);
            }
            BoundStmt::ForRange { body, .. } | BoundStmt::DoWhile { body, .. } => {
                collect_labels_recursive(body, labels);
            }
            BoundStmt::SelectCase {
                arms, else_body, ..
            } => {
                for (_, body) in arms {
                    collect_labels_recursive(body, labels);
                }
                collect_labels_recursive(else_body, labels);
            }
            _ => {}
        }
    }
}
