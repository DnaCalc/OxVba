use std::collections::{HashMap, HashSet};

use crate::resolve::{
    BoundCallArg, BoundCond, BoundExpr, BoundModule, BoundParam, BoundStmt, BoundType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallMode {
    Early,
    Late,
    Mixed,
}

pub fn check_types(module: BoundModule) -> Result<BoundModule, String> {
    let mut module = module;
    let default_type_table = module.default_type_table;
    let proc_names: HashSet<String> = module.procedures.iter().map(|p| p.name.clone()).collect();
    let proc_params: HashMap<String, Vec<BoundParam>> = module
        .procedures
        .iter()
        .map(|p| (p.name.clone(), p.params.clone()))
        .collect();

    for procedure in &mut module.procedures {
        let mut declared: HashSet<String> = procedure.declarations.iter().cloned().collect();
        let mut declared_types: HashMap<String, BoundType> = procedure.declaration_types.clone();
        if let Some(duplicate) = procedure.duplicate_declarations.first() {
            return Err(format!("duplicate declaration: {duplicate}"));
        }

        for declared_name in &procedure.declarations {
            if declared_name.eq_ignore_ascii_case(&procedure.name) {
                continue;
            }
            if proc_names.contains(declared_name) {
                return Err(format!(
                    "name collision between variable and procedure: {declared_name}"
                ));
            }
        }

        let labels = collect_labels(&procedure.body)?;
        check_stmt_list(
            &procedure.body,
            module.option_explicit,
            &default_type_table,
            &mut declared,
            &mut declared_types,
            &mut procedure.declarations,
            &mut procedure.declaration_types,
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
        module.declaration_types = entry.declaration_types.clone();
        module.body = entry.body.clone();
    }

    Ok(module)
}

#[allow(clippy::too_many_arguments)]
fn check_stmt_list(
    stmts: &[BoundStmt],
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    proc_names: &HashSet<String>,
    proc_params: &HashMap<String, Vec<BoundParam>>,
    labels: &HashSet<String>,
) -> Result<(), String> {
    for stmt in stmts {
        check_stmt(
            stmt,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_names,
            proc_params,
            labels,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_stmt(
    stmt: &BoundStmt,
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    proc_names: &HashSet<String>,
    proc_params: &HashMap<String, Vec<BoundParam>>,
    labels: &HashSet<String>,
) -> Result<(), String> {
    match stmt {
        BoundStmt::Assign { target, expr } => {
            ensure_declared(
                target,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_expr(
                expr,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            let expr_ty = infer_expr_type(expr, declared_types);
            let target_ty = *declared_types.get(target).unwrap_or(&BoundType::Variant);
            if can_assign_to(target_ty, expr_ty) {
                Ok(())
            } else {
                Err(format!(
                    "type mismatch in assignment: cannot assign {:?} to {:?} variable {}",
                    expr_ty, target_ty, target
                ))
            }
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            check_condition(
                cond,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_stmt_list(
                then_body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_names,
                proc_params,
                labels,
            )?;
            check_stmt_list(
                else_body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
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
            ensure_declared(
                var,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_expr(
                start,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_expr(
                end,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_stmt_list(
                body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_names,
                proc_params,
                labels,
            )
        }
        BoundStmt::ReDim { .. } => Ok(()),
        BoundStmt::DoWhile { cond, body, .. } => {
            check_condition(
                cond,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_stmt_list(
                body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
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
            let call_mode =
                classify_call_mode(name, args, proc_names, proc_params, declared_types)?;
            if matches!(call_mode, CallMode::Late) {
                for arg in args {
                    check_expr(
                        &arg.expr,
                        option_explicit,
                        default_type_table,
                        declared,
                        declared_types,
                        declarations,
                        declaration_types,
                    )?;
                }
                return Err(format!(
                    "late-bound default-member call is not yet executable: {name}"
                ));
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
                    check_expr(
                        &arg.expr,
                        option_explicit,
                        default_type_table,
                        declared,
                        declared_types,
                        declarations,
                        declaration_types,
                    )?;
                    let arg_ty = infer_expr_type(&arg.expr, declared_types);
                    if param.by_ref
                        && param.ty != BoundType::Variant
                        && arg_ty != BoundType::Variant
                        && arg_ty != param.ty
                    {
                        return Err(format!(
                            "ByRef parameter {} requires exact type match: expected {:?}, got {:?}",
                            param.name, param.ty, arg_ty
                        ));
                    }
                    if !can_assign_to(param.ty, arg_ty) {
                        return Err(format!(
                            "argument type mismatch for parameter {}: cannot pass {:?} to {:?}",
                            param.name, arg_ty, param.ty
                        ));
                    }
                }
            }
            Ok(())
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            check_expr(
                expr,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            for (_, body) in arms {
                check_stmt_list(
                    body,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_names,
                    proc_params,
                    labels,
                )?;
            }
            check_stmt_list(
                else_body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
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
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
) -> Result<(), String> {
    match cond {
        BoundCond::Compare { lhs, rhs, .. } => {
            check_expr(
                lhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_expr(
                rhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )
        }
        BoundCond::Truthy(expr) => check_expr(
            expr,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
        ),
        BoundCond::Not(inner) => check_condition(
            inner,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
        ),
        BoundCond::And(lhs, rhs) | BoundCond::Or(lhs, rhs) => {
            check_condition(
                lhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            check_condition(
                rhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )
        }
    }
}

fn check_expr(
    expr: &BoundExpr,
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
) -> Result<(), String> {
    match expr {
        BoundExpr::IntConst(_) => Ok(()),
        BoundExpr::Var(name) => ensure_declared(
            name,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
        ),
        BoundExpr::AddConst { var, .. } | BoundExpr::SubConst { var, .. } => ensure_declared(
            var,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
        ),
        BoundExpr::IntrinsicCall { args, .. } => {
            for arg in args {
                check_expr(
                    arg,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                )?;
            }
            Ok(())
        }
    }
}

fn ensure_declared(
    name: &str,
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
) -> Result<(), String> {
    if name.eq_ignore_ascii_case("err_number") {
        declared_types
            .entry("err_number".to_string())
            .or_insert(BoundType::Long);
        return Ok(());
    }

    if declared.contains(name) {
        if !declared_types.contains_key(name) {
            declared_types.insert(
                name.to_string(),
                default_type_for_name(name, default_type_table),
            );
        }
        if !declaration_types.contains_key(name) {
            declaration_types.insert(
                name.to_string(),
                default_type_for_name(name, default_type_table),
            );
        }
        return Ok(());
    }

    if option_explicit {
        return Err(format!("use of undeclared variable: {name}"));
    }

    declared.insert(name.to_string());
    declarations.push(name.to_string());
    let default_ty = default_type_for_name(name, default_type_table);
    declared_types.insert(name.to_string(), default_ty);
    declaration_types.insert(name.to_string(), default_ty);
    Ok(())
}

fn default_type_for_name(name: &str, default_type_table: &[BoundType; 26]) -> BoundType {
    let Some(first) = name.chars().next() else {
        return BoundType::Variant;
    };
    if !first.is_ascii_alphabetic() {
        return BoundType::Variant;
    }
    let idx = (first.to_ascii_lowercase() as u8 - b'a') as usize;
    default_type_table
        .get(idx)
        .copied()
        .unwrap_or(BoundType::Variant)
}

fn infer_expr_type(expr: &BoundExpr, declared_types: &HashMap<String, BoundType>) -> BoundType {
    match expr {
        BoundExpr::IntConst(_) => BoundType::Long,
        BoundExpr::Var(name) => *declared_types.get(name).unwrap_or(&BoundType::Variant),
        BoundExpr::AddConst { var, .. } | BoundExpr::SubConst { var, .. } => join_types(
            *declared_types.get(var).unwrap_or(&BoundType::Variant),
            BoundType::Long,
        ),
        BoundExpr::IntrinsicCall { .. } => BoundType::Variant,
    }
}

fn join_types(lhs: BoundType, rhs: BoundType) -> BoundType {
    if lhs == rhs {
        return lhs;
    }
    if lhs == BoundType::Variant || rhs == BoundType::Variant {
        return BoundType::Variant;
    }
    if lhs == BoundType::Array || rhs == BoundType::Array {
        return if lhs == BoundType::Array && rhs == BoundType::Array {
            BoundType::Array
        } else {
            BoundType::Variant
        };
    }
    if lhs == BoundType::Object || rhs == BoundType::Object {
        return if lhs == BoundType::Object && rhs == BoundType::Object {
            BoundType::Object
        } else {
            BoundType::Variant
        };
    }
    if is_numeric_type(lhs) && is_numeric_type(rhs) {
        return numeric_join(lhs, rhs);
    }
    if lhs == BoundType::String && rhs == BoundType::String {
        return BoundType::String;
    }
    BoundType::Variant
}

fn can_assign_to(target: BoundType, source: BoundType) -> bool {
    if target == BoundType::Variant || target == source {
        return true;
    }
    if source == BoundType::Variant {
        return true;
    }
    match target {
        BoundType::Object => source == BoundType::Object,
        BoundType::Array => source == BoundType::Array,
        BoundType::String => source != BoundType::Object && source != BoundType::Array,
        BoundType::Boolean => {
            source == BoundType::Boolean
                || is_numeric_type(source)
                || source == BoundType::String
                || source == BoundType::Date
        }
        BoundType::Date | BoundType::Currency | BoundType::Decimal => {
            is_numeric_type(source) || source == BoundType::String || source == BoundType::Date
        }
        _ => {
            if is_numeric_type(target) {
                is_numeric_type(source)
                    || source == BoundType::Boolean
                    || source == BoundType::Date
                    || source == BoundType::String
            } else {
                false
            }
        }
    }
}

fn is_numeric_type(ty: BoundType) -> bool {
    matches!(
        ty,
        BoundType::Byte
            | BoundType::Integer
            | BoundType::Long
            | BoundType::LongLong
            | BoundType::LongPtr
            | BoundType::Single
            | BoundType::Double
            | BoundType::Currency
            | BoundType::Decimal
            | BoundType::Boolean
    )
}

fn numeric_join(lhs: BoundType, rhs: BoundType) -> BoundType {
    if lhs == BoundType::Double || rhs == BoundType::Double {
        return BoundType::Double;
    }
    if lhs == BoundType::Single || rhs == BoundType::Single {
        return BoundType::Single;
    }
    if lhs == BoundType::Decimal || rhs == BoundType::Decimal {
        return BoundType::Decimal;
    }
    if lhs == BoundType::Currency || rhs == BoundType::Currency {
        return BoundType::Currency;
    }
    if lhs == BoundType::LongLong || rhs == BoundType::LongLong {
        return BoundType::LongLong;
    }
    if lhs == BoundType::LongPtr && rhs == BoundType::LongPtr {
        return BoundType::LongPtr;
    }
    if lhs == BoundType::Long || rhs == BoundType::Long {
        return BoundType::Long;
    }
    if lhs == BoundType::Integer || rhs == BoundType::Integer {
        return BoundType::Integer;
    }
    BoundType::Long
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

fn classify_call_mode(
    name: &str,
    args: &[BoundCallArg],
    proc_names: &HashSet<String>,
    proc_params: &HashMap<String, Vec<BoundParam>>,
    declared_types: &HashMap<String, BoundType>,
) -> Result<CallMode, String> {
    if proc_names.contains(name) {
        let Some(params) = proc_params.get(name) else {
            return Ok(CallMode::Early);
        };
        let mapped_args = map_call_args_to_params(name, args, params)?;
        let dynamic_params = params
            .iter()
            .any(|param| matches!(param.ty, BoundType::Variant | BoundType::Object));
        let dynamic_args = mapped_args.iter().flatten().any(|arg| {
            matches!(
                infer_expr_type(&arg.expr, declared_types),
                BoundType::Variant | BoundType::Object
            )
        });
        if dynamic_params || dynamic_args {
            return Ok(CallMode::Mixed);
        }
        return Ok(CallMode::Early);
    }

    if matches!(
        declared_types.get(name),
        Some(BoundType::Variant) | Some(BoundType::Object)
    ) {
        return Ok(CallMode::Late);
    }

    Err(format!("call to unknown procedure: {name}"))
}

fn collect_labels(stmts: &[BoundStmt]) -> Result<HashSet<String>, String> {
    let mut labels = HashSet::new();
    let mut duplicates = Vec::new();
    collect_labels_recursive(stmts, &mut labels, &mut duplicates);
    if let Some(name) = duplicates.first() {
        return Err(format!("duplicate label declaration: {name}"));
    }
    Ok(labels)
}

fn collect_labels_recursive(
    stmts: &[BoundStmt],
    labels: &mut HashSet<String>,
    duplicates: &mut Vec<String>,
) {
    for stmt in stmts {
        match stmt {
            BoundStmt::Label { name } => {
                if !labels.insert(name.clone()) {
                    duplicates.push(name.clone());
                }
            }
            BoundStmt::IfCond {
                then_body,
                else_body,
                ..
            } => {
                collect_labels_recursive(then_body, labels, duplicates);
                collect_labels_recursive(else_body, labels, duplicates);
            }
            BoundStmt::ForRange { body, .. } | BoundStmt::DoWhile { body, .. } => {
                collect_labels_recursive(body, labels, duplicates);
            }
            BoundStmt::SelectCase {
                arms, else_body, ..
            } => {
                for (_, body) in arms {
                    collect_labels_recursive(body, labels, duplicates);
                }
                collect_labels_recursive(else_body, labels, duplicates);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{CallMode, can_assign_to, classify_call_mode, join_types};
    use crate::resolve::{BoundCallArg, BoundExpr, BoundParam, BoundType};

    #[test]
    fn join_numeric_promotes_to_wider_type() {
        assert_eq!(
            join_types(BoundType::Integer, BoundType::Double),
            BoundType::Double
        );
        assert_eq!(
            join_types(BoundType::Long, BoundType::LongLong),
            BoundType::LongLong
        );
    }

    #[test]
    fn join_object_with_non_object_yields_variant() {
        assert_eq!(
            join_types(BoundType::Object, BoundType::String),
            BoundType::Variant
        );
    }

    #[test]
    fn assignability_numeric_and_variant_are_permitted() {
        assert!(can_assign_to(BoundType::Long, BoundType::Integer));
        assert!(can_assign_to(BoundType::String, BoundType::Variant));
    }

    #[test]
    fn assignability_array_to_scalar_is_rejected() {
        assert!(!can_assign_to(BoundType::Long, BoundType::Array));
        assert!(!can_assign_to(BoundType::Object, BoundType::Array));
    }

    #[test]
    fn classify_call_mode_early_for_strict_typed_procedure() {
        let proc_names = HashSet::from(["work".to_string()]);
        let proc_params = HashMap::from([(
            "work".to_string(),
            vec![BoundParam {
                name: "x".to_string(),
                by_ref: false,
                optional: false,
                default_value: None,
                ty: BoundType::Long,
            }],
        )]);
        let declared_types = HashMap::new();
        let args = vec![BoundCallArg {
            name: None,
            expr: BoundExpr::IntConst(1),
        }];
        assert_eq!(
            classify_call_mode("work", &args, &proc_names, &proc_params, &declared_types)
                .expect("classification should succeed"),
            CallMode::Early
        );
    }

    #[test]
    fn classify_call_mode_mixed_for_variant_signature() {
        let proc_names = HashSet::from(["work".to_string()]);
        let proc_params = HashMap::from([(
            "work".to_string(),
            vec![BoundParam {
                name: "x".to_string(),
                by_ref: false,
                optional: false,
                default_value: None,
                ty: BoundType::Variant,
            }],
        )]);
        let declared_types = HashMap::new();
        let args = vec![BoundCallArg {
            name: None,
            expr: BoundExpr::IntConst(1),
        }];
        assert_eq!(
            classify_call_mode("work", &args, &proc_names, &proc_params, &declared_types)
                .expect("classification should succeed"),
            CallMode::Mixed
        );
    }

    #[test]
    fn classify_call_mode_late_for_object_default_member_target() {
        let proc_names = HashSet::new();
        let proc_params = HashMap::new();
        let declared_types = HashMap::from([("obj".to_string(), BoundType::Object)]);
        let args = vec![BoundCallArg {
            name: None,
            expr: BoundExpr::IntConst(1),
        }];
        assert_eq!(
            classify_call_mode("obj", &args, &proc_names, &proc_params, &declared_types)
                .expect("classification should succeed"),
            CallMode::Late
        );
    }
}
