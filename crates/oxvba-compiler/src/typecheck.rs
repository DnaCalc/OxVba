use std::collections::{HashMap, HashSet};

use crate::resolve::{
    AssignmentIntent, BoundCallArg, BoundCond, BoundExpr, BoundModule, BoundParam, BoundStmt,
    BoundType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallMode {
    Early,
    Late,
    Mixed,
}

struct TypecheckProcContext<'a> {
    proc_names: &'a HashSet<String>,
    proc_params: &'a HashMap<String, Vec<BoundParam>>,
    proc_return_types: &'a HashMap<String, BoundType>,
}

fn contains_casefold_name(set: &HashSet<String>, name: &str) -> bool {
    if set.contains(name) {
        return true;
    }
    let folded = name.to_ascii_lowercase();
    set.contains(&folded)
        || set
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

fn lookup_casefold_name<'a, V>(map: &'a HashMap<String, V>, name: &str) -> Option<&'a V> {
    if let Some(value) = map.get(name) {
        return Some(value);
    }
    let folded = name.to_ascii_lowercase();
    map.get(&folded).or_else(|| {
        map.iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    })
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
    let proc_return_types: HashMap<String, BoundType> = module
        .procedures
        .iter()
        .map(|p| (p.name.clone(), p.return_type))
        .collect();
    let proc_context = TypecheckProcContext {
        proc_names: &proc_names,
        proc_params: &proc_params,
        proc_return_types: &proc_return_types,
    };

    for procedure in &mut module.procedures {
        let mut declared: HashSet<String> = procedure.declarations.iter().cloned().collect();
        let mut declared_types: HashMap<String, BoundType> = procedure.declaration_types.clone();
        if let Some(duplicate) = procedure.duplicate_declarations.first() {
            return Err(format!("duplicate declaration: {duplicate}"));
        }

        // VBA allows a local variable to share its name with another procedure
        // in the same module (the variable shadows the proc name within its scope).
        // Only reject true duplicates within the same declaration list.

        let labels = collect_labels(&procedure.body)?;
        check_stmt_list(
            &procedure.body,
            module.option_explicit,
            &default_type_table,
            &mut declared,
            &mut declared_types,
            &mut procedure.declarations,
            &mut procedure.declaration_types,
            &proc_context,
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
        module.array_descriptors = entry.array_descriptors.clone();
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
    proc_context: &TypecheckProcContext<'_>,
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
            proc_context,
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
    proc_context: &TypecheckProcContext<'_>,
    labels: &HashSet<String>,
) -> Result<(), String> {
    match stmt {
        BoundStmt::Assign {
            target,
            expr,
            intent,
        } => {
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
                proc_context,
            )?;
            let expr_ty = infer_expr_type(expr, declared_types);
            let target_ty = *declared_types.get(target).unwrap_or(&BoundType::Variant);
            if is_vbnullstring_expr(expr)
                && !matches!(target_ty, BoundType::String | BoundType::Variant)
            {
                return Err(format!(
                    "type mismatch in assignment: vbNullString requires String or Variant target, got {:?} variable {}",
                    target_ty, target
                ));
            }
            if let Some(message) = validate_assignment_intent(*intent, target_ty, expr_ty, target) {
                Err(message)
            } else if let Some(message) =
                validate_known_object_expr_assignment(*intent, target_ty, expr, target)
            {
                Err(message)
            } else if can_assign_to(target_ty, expr_ty) {
                Ok(())
            } else {
                Err(format!(
                    "type mismatch in assignment: cannot assign {:?} to {:?} variable {}",
                    expr_ty, target_ty, target
                ))
            }
        }
        BoundStmt::AssignRuntimeArrayElement {
            name,
            indices,
            expr,
            ..
        } => {
            ensure_declared(
                name,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            for index in indices {
                check_expr(
                    index,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
                )?;
            }
            check_expr(
                expr,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )
        }
        BoundStmt::UdtAssign {
            target,
            source,
            fields,
        } => {
            ensure_declared(
                target,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            ensure_declared(
                source,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            for field in fields {
                let dst_alias = format!("{target}_{field}");
                let src_alias = format!("{source}_{field}");
                if !declared.contains(&dst_alias) || !declared.contains(&src_alias) {
                    return Err(format!(
                        "udt assignment field mismatch: missing alias pair {dst_alias}/{src_alias}"
                    ));
                }
            }
            Ok(())
        }
        BoundStmt::MidAssign {
            target,
            start,
            count,
            value,
        } => {
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
                start,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            if let Some(count) = count {
                check_expr(
                    count,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
                )?;
            }
            check_expr(
                value,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;

            let target_ty = *declared_types.get(target).unwrap_or(&BoundType::Variant);
            if coercion_result(BoundType::String, target_ty) != CoercionResult::Ok {
                return Err(format!(
                    "type mismatch in Mid assignment target: cannot mutate {:?} variable {}",
                    target_ty, target
                ));
            }

            let start_ty = infer_expr_type(start, declared_types);
            if coercion_result(start_ty, BoundType::Long) != CoercionResult::Ok {
                return Err(format!(
                    "type mismatch in Mid assignment start expression: cannot convert {:?} to Long",
                    start_ty
                ));
            }
            if let Some(count) = count {
                let count_ty = infer_expr_type(count, declared_types);
                if coercion_result(count_ty, BoundType::Long) != CoercionResult::Ok {
                    return Err(format!(
                        "type mismatch in Mid assignment count expression: cannot convert {:?} to Long",
                        count_ty
                    ));
                }
            }
            let value_ty = infer_expr_type(value, declared_types);
            if coercion_result(value_ty, BoundType::String) != CoercionResult::Ok {
                return Err(format!(
                    "type mismatch in Mid assignment value expression: cannot convert {:?} to String",
                    value_ty
                ));
            }
            Ok(())
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
                proc_context,
            )?;
            check_stmt_list(
                then_body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
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
                proc_context,
                labels,
            )
        }
        BoundStmt::ForRange {
            var,
            start,
            end,
            step,
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
                proc_context,
            )?;
            check_expr(
                end,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            check_expr(
                step,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            if matches!(step, BoundExpr::IntConst(0)) {
                return Err("for loop step cannot be zero".to_string());
            }
            check_stmt_list(
                body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
                labels,
            )
        }
        BoundStmt::ForEach {
            var,
            items,
            iterable,
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
            let target_ty = *declared_types.get(var).unwrap_or(&BoundType::Variant);
            if let Some(iterable) = iterable {
                check_expr(
                    iterable,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
                )?;
                let iterable_ty = infer_expr_type(iterable, declared_types);
                if !matches!(
                    iterable_ty,
                    BoundType::Array | BoundType::Object | BoundType::Variant
                ) {
                    return Err(format!(
                        "type mismatch in For Each iterable: expected Array/Object/Variant, got {:?}",
                        iterable_ty
                    ));
                }
            } else {
                for item in items {
                    check_expr(
                        item,
                        option_explicit,
                        default_type_table,
                        declared,
                        declared_types,
                        declarations,
                        declaration_types,
                        proc_context,
                    )?;
                    let item_ty = infer_expr_type(item, declared_types);
                    if !can_assign_to(target_ty, item_ty) {
                        return Err(format!(
                            "type mismatch in For Each item assignment: cannot assign {:?} to {:?} variable {}",
                            item_ty, target_ty, var
                        ));
                    }
                }
            }
            check_stmt_list(
                body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
                labels,
            )
        }
        BoundStmt::ReDim {
            name,
            bounds,
            previous_bounds,
            preserve,
        } => {
            if *preserve {
                let Some(previous_bounds) = previous_bounds else {
                    return Err(format!(
                        "redim preserve requires existing array bounds for {name}"
                    ));
                };
                if !redim_preserve_bounds_are_legal(previous_bounds, bounds) {
                    return Err(format!(
                        "redim preserve only supports resizing the upper bound of the last dimension: {name}"
                    ));
                }
            }
            Ok(())
        }
        BoundStmt::ReDimRuntime { name, bounds, .. } => {
            ensure_declared(
                name,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            if declared_types.get(name).copied() != Some(BoundType::Array) {
                return Err(format!(
                    "runtime ReDim with expression bounds currently requires a dynamically declared array variable: {name}"
                ));
            }
            for bound in bounds {
                check_expr(
                    &bound.upper_bound,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
                )?;
                let upper_ty = infer_expr_type(&bound.upper_bound, declared_types);
                if coercion_result(upper_ty, BoundType::Long) != CoercionResult::Ok {
                    return Err(format!(
                        "runtime ReDim upper bound must be numeric/coercible to Long, got {:?} for {}",
                        upper_ty, name
                    ));
                }
            }
            let element_alias = format!("{name}_0");
            match declaration_types
                .get(element_alias.as_str())
                .copied()
                .unwrap_or(BoundType::Variant)
            {
                BoundType::Object | BoundType::Array | BoundType::Decimal => {
                    return Err(format!(
                        "runtime ReDim does not yet support {:?} element arrays for {}",
                        declaration_types
                            .get(element_alias.as_str())
                            .copied()
                            .unwrap_or(BoundType::Variant),
                        name
                    ));
                }
                _ => {}
            }
            Ok(())
        }
        BoundStmt::Erase { name } => {
            ensure_declared(
                name,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            Ok(())
        }
        BoundStmt::DoWhile { cond, body, .. } => {
            check_condition(
                cond,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            check_stmt_list(
                body,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
                labels,
            )
        }
        BoundStmt::ExitDo => Ok(()),
        BoundStmt::ExitFor => Ok(()),
        BoundStmt::ExitProcedure => Ok(()),
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
        BoundStmt::Resume => Ok(()),
        BoundStmt::ResumeLabel { label } => {
            if labels.contains(label) {
                Ok(())
            } else {
                Err(format!("resume target label not found: {label}"))
            }
        }
        BoundStmt::RaiseError(_) => Ok(()),
        BoundStmt::RaiseEvent { args, .. } => {
            for arg in args {
                check_expr(
                    &arg.expr,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
                )?;
            }
            Ok(())
        }
        BoundStmt::ErrClear => Ok(()),
        BoundStmt::Label { .. } => Ok(()),
        BoundStmt::GoTo { label } => {
            if labels.contains(label) {
                Ok(())
            } else {
                Err(format!("goto target label not found: {label}"))
            }
        }
        BoundStmt::GoSub { label } => {
            if labels.contains(label) {
                Ok(())
            } else {
                Err(format!("gosub target label not found: {label}"))
            }
        }
        BoundStmt::Return => Ok(()),
        BoundStmt::Call { name, args, .. } => validate_call_site(
            name,
            args,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
        )
        .map(|_| ()),
        BoundStmt::AssignFromCall {
            target,
            name,
            args,
            intent,
            ..
        } => {
            ensure_declared(
                target,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            let return_ty = validate_call_site(
                name,
                args,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            let target_ty = *declared_types.get(target).unwrap_or(&BoundType::Variant);
            if let Some(message) = validate_assignment_intent(*intent, target_ty, return_ty, target)
            {
                Err(message)
            } else if let Some(message) =
                validate_known_object_call_assignment(*intent, target_ty, name, target)
            {
                Err(message)
            } else if can_assign_to(target_ty, return_ty) {
                Ok(())
            } else {
                Err(format!(
                    "type mismatch in assignment from call: cannot assign {:?} to {:?} variable {}",
                    return_ty, target_ty, target
                ))
            }
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
                proc_context,
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
                    proc_context,
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
                proc_context,
                labels,
            )
        }
        BoundStmt::FileOpen { .. }
        | BoundStmt::FileClose { .. }
        | BoundStmt::FileKill { .. }
        | BoundStmt::FilePrint { .. }
        | BoundStmt::ConsolePrint { .. }
        | BoundStmt::FileWrite { .. }
        | BoundStmt::FileInput { .. }
        | BoundStmt::ConsoleInput { .. }
        | BoundStmt::FileLineInput { .. }
        | BoundStmt::ConsoleLineInput { .. }
        | BoundStmt::Beep
        | BoundStmt::DebugPrint { .. } => Ok(()),
        BoundStmt::Unsupported { line } => Err(format!("unsupported statement: {line}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_call_site(
    name: &str,
    args: &[BoundCallArg],
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    proc_context: &TypecheckProcContext<'_>,
) -> Result<BoundType, String> {
    if name.eq_ignore_ascii_case("dispatchinvoke")
        || name.eq_ignore_ascii_case("__OxVbaEarlyInvoke")
    {
        return validate_dispatch_invoke_call_site(
            args,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
        );
    }

    // Intrinsic statements that bypass normal procedure lookup
    if name.eq_ignore_ascii_case("randomize") {
        for arg in args {
            check_expr(
                &arg.expr,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
        }
        return Ok(BoundType::Variant);
    }

    let call_mode = classify_call_mode(
        name,
        args,
        proc_context.proc_names,
        proc_context.proc_params,
        declared_types,
    )?;

    if let Some(params) = lookup_casefold_name(proc_context.proc_params, name) {
        let arg_mapping = map_call_args_to_params(name, args, params)?;
        for (idx, param) in params.iter().enumerate() {
            if param.param_array {
                for extra in &arg_mapping.extras {
                    check_expr(
                        &extra.expr,
                        option_explicit,
                        default_type_table,
                        declared,
                        declared_types,
                        declarations,
                        declaration_types,
                        proc_context,
                    )?;
                }
                continue;
            }
            let Some(arg) = arg_mapping.fixed[idx] else {
                continue;
            };

            // VBA allows literals/expressions for ByRef parameters — the callee
            // receives a temporary copy and no copy-back occurs.  The emit
            // layer already handles this by only inserting copy-back for
            // BoundExpr::Var arguments.
            check_expr(
                &arg.expr,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            let arg_ty = infer_expr_type(&arg.expr, declared_types);
            if is_vbnullstring_expr(&arg.expr)
                && !matches!(param.ty, BoundType::String | BoundType::Variant)
            {
                return Err(format!(
                    "argument type mismatch for parameter {}: vbNullString requires String or Variant target, got {:?}",
                    param.name, param.ty
                ));
            }
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
            if call_coercion_result(call_mode, Some(param.ty), arg_ty) != CoercionResult::Ok {
                return Err(format!(
                    "argument type mismatch for parameter {}: cannot pass {:?} to {:?}",
                    param.name, arg_ty, param.ty
                ));
            }
        }
        return Ok(proc_context
            .proc_return_types
            .get(name)
            .or_else(|| lookup_casefold_name(proc_context.proc_return_types, name))
            .copied()
            .unwrap_or(BoundType::Variant));
    }

    if matches!(call_mode, CallMode::Late) {
        let mut named_started = false;
        for arg in args {
            if arg.name.is_some() {
                named_started = true;
            } else if named_started {
                return Err("positional argument cannot follow named argument".to_string());
            }
            check_expr(
                &arg.expr,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            let arg_ty = infer_expr_type(&arg.expr, declared_types);
            if call_coercion_result(CallMode::Late, None, arg_ty) != CoercionResult::Ok {
                return Err(format!(
                    "late-bound call argument coercion mismatch: cannot package {:?} for {}",
                    arg_ty, name
                ));
            }
        }
        return Ok(BoundType::Variant);
    }

    Err(format!("call to unknown procedure: {name}"))
}

#[allow(clippy::too_many_arguments)]
fn validate_dispatch_invoke_call_site(
    args: &[BoundCallArg],
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    proc_context: &TypecheckProcContext<'_>,
) -> Result<BoundType, String> {
    if args.len() < 2 {
        return Err("DispatchInvoke expects at least object and member arguments".to_string());
    }
    if args[0].name.is_some() || args[1].name.is_some() {
        return Err("DispatchInvoke object/member arguments cannot be named".to_string());
    }
    let mut named_started = false;
    for arg in args.iter().skip(2) {
        if arg.name.is_some() {
            named_started = true;
        } else if named_started {
            return Err("positional argument cannot follow named argument".to_string());
        }
    }
    for arg in args {
        check_expr(
            &arg.expr,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
        )?;
    }
    for arg in args.iter().skip(2) {
        let arg_ty = infer_expr_type(&arg.expr, declared_types);
        if call_coercion_result(CallMode::Late, None, arg_ty) != CoercionResult::Ok {
            return Err(format!(
                "late-bound call argument coercion mismatch: cannot package {:?} for DispatchInvoke",
                arg_ty
            ));
        }
    }
    Ok(BoundType::Variant)
}

#[allow(clippy::too_many_arguments)]
fn check_condition(
    cond: &BoundCond,
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    proc_context: &TypecheckProcContext<'_>,
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
                proc_context,
            )?;
            check_expr(
                rhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            let lhs_ty = infer_expr_type(lhs, declared_types);
            let rhs_ty = infer_expr_type(rhs, declared_types);
            if can_compare_types(lhs_ty, rhs_ty) {
                Ok(())
            } else {
                Err(format!(
                    "type mismatch in comparison: cannot compare {:?} with {:?}",
                    lhs_ty, rhs_ty
                ))
            }
        }
        BoundCond::Truthy(expr) => check_expr(
            expr,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
        ),
        BoundCond::Not(inner) => check_condition(
            inner,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
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
                proc_context,
            )?;
            check_condition(
                rhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_expr(
    expr: &BoundExpr,
    option_explicit: bool,
    default_type_table: &[BoundType; 26],
    declared: &mut HashSet<String>,
    declared_types: &mut HashMap<String, BoundType>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    proc_context: &TypecheckProcContext<'_>,
) -> Result<(), String> {
    match expr {
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_) => Ok(()),
        BoundExpr::Var(name) | BoundExpr::VarPtrArrayBuffer(name) => ensure_declared(
            name,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
        ),
        BoundExpr::AddConst { var, .. } | BoundExpr::SubConst { var, .. } => {
            ensure_declared(
                var,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
            )?;
            let lhs_ty = *declared_types.get(var).unwrap_or(&BoundType::Variant);
            match arithmetic_result(lhs_ty, BoundType::Long) {
                ArithmeticResult::Ok(_) => Ok(()),
                ArithmeticResult::TypeMismatch => Err(format!(
                    "type mismatch in arithmetic expression: cannot apply +/- to {:?} and Long",
                    lhs_ty
                )),
            }
        }
        BoundExpr::IntrinsicCall { name, args } => {
            for arg in args {
                check_expr(
                    arg,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
                )?;
            }
            if let Some(target_type) = intrinsic_argument_target_type(name) {
                if args.len() != 1 {
                    return Err(format!(
                        "conversion intrinsic {} expects exactly one argument",
                        name
                    ));
                }
                let arg_type = infer_expr_type(&args[0], declared_types);
                if coercion_result(arg_type, target_type) != CoercionResult::Ok {
                    return Err(format!(
                        "argument type mismatch for conversion intrinsic {}: cannot convert {:?} to {:?}",
                        name, arg_type, target_type
                    ));
                }
            }
            Ok(())
        }
        BoundExpr::ProcCall { name, args } => validate_call_site(
            name,
            args,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
        )
        .map(|_| ()),
        BoundExpr::CompareOp { lhs, rhs, .. } => {
            check_expr(
                lhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            check_expr(
                rhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )
        }
        BoundExpr::BinaryOp { lhs, rhs, .. } | BoundExpr::LogicalBinaryOp { lhs, rhs, .. } => {
            check_expr(
                lhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            check_expr(
                rhs,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )
        }
        BoundExpr::UnaryOp { operand, .. } | BoundExpr::LogicalNot { operand, .. } => check_expr(
            operand,
            option_explicit,
            default_type_table,
            declared,
            declared_types,
            declarations,
            declaration_types,
            proc_context,
        ),
        BoundExpr::Member { receiver, args, .. } => {
            // Late-bound: only the receiver and arguments are statically checkable; the member is
            // resolved dynamically at runtime.
            check_expr(
                receiver,
                option_explicit,
                default_type_table,
                declared,
                declared_types,
                declarations,
                declaration_types,
                proc_context,
            )?;
            for arg in args {
                check_expr(
                    &arg.expr,
                    option_explicit,
                    default_type_table,
                    declared,
                    declared_types,
                    declarations,
                    declaration_types,
                    proc_context,
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
    if is_err_member_alias(name) {
        declared_types
            .entry(name.to_ascii_lowercase())
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

fn is_err_member_alias(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "err_number"
            | "err_description"
            | "err_source"
            | "err_helpcontext"
            | "err_helpfile"
            | "err_lastdllerror"
    )
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
        BoundExpr::BoolConst(_) => BoundType::Boolean,
        BoundExpr::FloatConst(_) => BoundType::Double,
        BoundExpr::StringConst(_) => BoundType::String,
        BoundExpr::Var(name) => *declared_types.get(name).unwrap_or(&BoundType::Variant),
        BoundExpr::VarPtrArrayBuffer(_) => BoundType::LongPtr,
        BoundExpr::AddConst { var, .. } | BoundExpr::SubConst { var, .. } => {
            let lhs_ty = *declared_types.get(var).unwrap_or(&BoundType::Variant);
            match arithmetic_result(lhs_ty, BoundType::Long) {
                ArithmeticResult::Ok(result) => result,
                ArithmeticResult::TypeMismatch => BoundType::Variant,
            }
        }
        BoundExpr::IntrinsicCall { name, .. } => {
            intrinsic_result_type(name).unwrap_or(BoundType::Variant)
        }
        BoundExpr::ProcCall { .. } => BoundType::Variant,
        BoundExpr::CompareOp { .. } => BoundType::Boolean,
        BoundExpr::LogicalBinaryOp { .. } | BoundExpr::LogicalNot { .. } => BoundType::Boolean,
        BoundExpr::BinaryOp { .. } | BoundExpr::UnaryOp { .. } => BoundType::Variant,
        // Late-bound member access: the member's return type is not known statically.
        BoundExpr::Member { .. } => BoundType::Variant,
    }
}

fn is_vbnullstring_expr(expr: &BoundExpr) -> bool {
    matches!(
        expr,
        BoundExpr::IntrinsicCall { name, args } if name == "vbnullstring" && args.is_empty()
    )
}

fn redim_preserve_bounds_are_legal(previous: &[(i32, i32)], next: &[(i32, i32)]) -> bool {
    if previous.len() != next.len() || previous.is_empty() {
        return false;
    }
    if previous.len() == 1 {
        let (prev_lower, _) = previous[0];
        let (next_lower, _) = next[0];
        return prev_lower == next_lower;
    }

    let last = previous.len() - 1;
    for dim in 0..last {
        if previous[dim] != next[dim] {
            return false;
        }
    }

    let (prev_lower, _) = previous[last];
    let (next_lower, _) = next[last];
    prev_lower == next_lower
}

#[cfg(test)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArithmeticResult {
    Ok(BoundType),
    TypeMismatch,
}

fn arithmetic_result(lhs: BoundType, rhs: BoundType) -> ArithmeticResult {
    if lhs == BoundType::Variant || rhs == BoundType::Variant {
        return ArithmeticResult::Ok(BoundType::Variant);
    }
    if lhs == BoundType::Object
        || rhs == BoundType::Object
        || lhs == BoundType::Array
        || rhs == BoundType::Array
    {
        return ArithmeticResult::TypeMismatch;
    }
    if is_numeric_type(lhs) && is_numeric_type(rhs) {
        return ArithmeticResult::Ok(numeric_join(lhs, rhs));
    }
    if is_arithmetic_coercible_type(lhs) && is_arithmetic_coercible_type(rhs) {
        return ArithmeticResult::Ok(BoundType::Variant);
    }
    ArithmeticResult::TypeMismatch
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparisonResult {
    Ok,
    TypeMismatch,
}

fn can_compare_types(lhs: BoundType, rhs: BoundType) -> bool {
    comparison_result(lhs, rhs) == ComparisonResult::Ok
}

fn comparison_result(lhs: BoundType, rhs: BoundType) -> ComparisonResult {
    if lhs == BoundType::Variant || rhs == BoundType::Variant {
        return ComparisonResult::Ok;
    }
    if lhs == BoundType::Object
        || rhs == BoundType::Object
        || lhs == BoundType::Array
        || rhs == BoundType::Array
    {
        return ComparisonResult::TypeMismatch;
    }
    if lhs == rhs || (is_numeric_type(lhs) && is_numeric_type(rhs)) {
        return ComparisonResult::Ok;
    }
    if is_arithmetic_coercible_type(lhs) && is_arithmetic_coercible_type(rhs) {
        return ComparisonResult::Ok;
    }
    ComparisonResult::TypeMismatch
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoercionResult {
    Ok,
    TypeMismatch,
}

fn can_assign_to(target: BoundType, source: BoundType) -> bool {
    coercion_result(source, target) == CoercionResult::Ok
}

fn validate_assignment_intent(
    intent: AssignmentIntent,
    target_ty: BoundType,
    source_ty: BoundType,
    target: &str,
) -> Option<String> {
    match intent {
        AssignmentIntent::Implicit
            if target_ty == BoundType::Object && source_ty == BoundType::Object =>
        {
            Some(format!(
                "type mismatch in assignment: Set required for Object variable {}",
                target
            ))
        }
        AssignmentIntent::Implicit => None,
        AssignmentIntent::Let if target_ty == BoundType::Object => Some(format!(
            "type mismatch in assignment: Let cannot assign to Object variable {}",
            target
        )),
        AssignmentIntent::Set if !matches!(target_ty, BoundType::Object | BoundType::Variant) => {
            Some(format!(
                "type mismatch in assignment: Set requires Object or Variant target, got {:?} variable {}",
                target_ty, target
            ))
        }
        AssignmentIntent::Set if !matches!(source_ty, BoundType::Object | BoundType::Variant) => {
            Some(format!(
                "type mismatch in assignment: Set requires object value for variable {}",
                target
            ))
        }
        _ => None,
    }
}

fn validate_known_object_assignment(
    intent: AssignmentIntent,
    target_ty: BoundType,
    target: &str,
    context: &str,
) -> Option<String> {
    if target_ty == BoundType::Variant {
        return None;
    }
    if target_ty == BoundType::Object {
        return match intent {
            AssignmentIntent::Set => None,
            AssignmentIntent::Implicit => Some(format!(
                "type mismatch in assignment{context}: Set required for Object variable {}",
                target
            )),
            AssignmentIntent::Let => Some(format!(
                "type mismatch in assignment{context}: Let cannot assign to Object variable {}",
                target
            )),
        };
    }
    match intent {
        AssignmentIntent::Implicit | AssignmentIntent::Let => Some(format!(
            "type mismatch in assignment{context}: CreateObject cannot assign Object to {:?} variable {}",
            target_ty, target
        )),
        AssignmentIntent::Set => None,
    }
}

fn validate_known_object_expr_assignment(
    intent: AssignmentIntent,
    target_ty: BoundType,
    expr: &BoundExpr,
    target: &str,
) -> Option<String> {
    if is_known_object_producing_expr(expr) {
        return validate_known_object_assignment(intent, target_ty, target, "");
    }
    None
}

fn validate_known_object_call_assignment(
    intent: AssignmentIntent,
    target_ty: BoundType,
    name: &str,
    target: &str,
) -> Option<String> {
    if is_known_object_producing_call(name) {
        return validate_known_object_assignment(intent, target_ty, target, " from call");
    }
    None
}

fn call_coercion_result(
    call_mode: CallMode,
    param_type: Option<BoundType>,
    arg_type: BoundType,
) -> CoercionResult {
    match call_mode {
        CallMode::Late => coercion_result(arg_type, BoundType::Variant),
        CallMode::Early | CallMode::Mixed => {
            coercion_result(arg_type, param_type.unwrap_or(BoundType::Variant))
        }
    }
}

fn intrinsic_result_type(name: &str) -> Option<BoundType> {
    match name {
        "cint" => Some(BoundType::Integer),
        "clng" => Some(BoundType::Long),
        "cdbl" => Some(BoundType::Double),
        "csng" => Some(BoundType::Single),
        "cbyte" => Some(BoundType::Byte),
        "ccur" => Some(BoundType::Currency),
        "cdec" => Some(BoundType::Decimal),
        "cstr" => Some(BoundType::String),
        "cbool" => Some(BoundType::Boolean),
        "cdate" => Some(BoundType::Date),
        "val" => Some(BoundType::Double),
        "str" => Some(BoundType::String),
        "cverr" => Some(BoundType::Variant),
        "instr" => Some(BoundType::Long),
        "instrrev" => Some(BoundType::Long),
        "strcomp" => Some(BoundType::Long),
        "asc" => Some(BoundType::Long),
        "space" | "string" | "chr" | "format" | "hex" | "oct" | "monthname" => {
            Some(BoundType::String)
        }
        "date" | "time" | "now" => Some(BoundType::Date),
        "timer" => Some(BoundType::Double),
        "year" | "month" | "day" | "weekday" => Some(BoundType::Long),
        "atn" | "tan" | "rate" | "irr" | "mirr" => Some(BoundType::Double),
        "rnd" | "npv" => Some(BoundType::Double),
        "nper" => Some(BoundType::Double),
        "isempty" | "isnull" | "iserror" | "typeofis" | "__oxvba_object_is" => {
            Some(BoundType::Boolean)
        }
        "vbnullstring" => Some(BoundType::String),
        "array" | "__oxvba_array_append" => Some(BoundType::Array),
        "strptr" | "varptr" | "objptr" => Some(BoundType::LongPtr),
        // `New <ProjectClass>` carrier: materialises a reference-counted object reference.
        "__oxvba_project_instance" => Some(BoundType::Object),
        // `Nothing`: the null object reference.
        "__nothing" => Some(BoundType::Object),
        _ => None,
    }
}

fn is_known_object_producing_call(name: &str) -> bool {
    name.eq_ignore_ascii_case("createobject")
}

fn is_known_object_producing_expr(expr: &BoundExpr) -> bool {
    matches!(expr, BoundExpr::IntrinsicCall { name, .. } if is_known_object_producing_call(name))
}

fn intrinsic_argument_target_type(name: &str) -> Option<BoundType> {
    match name {
        "cint" | "clng" | "cdbl" | "csng" | "cbyte" | "ccur" | "cdec" | "cstr" | "cbool"
        | "cdate" | "val" | "str" => intrinsic_result_type(name),
        "cverr" => Some(BoundType::Long),
        _ => None,
    }
}

fn coercion_result(source: BoundType, target: BoundType) -> CoercionResult {
    if target == BoundType::Variant || target == source {
        return CoercionResult::Ok;
    }
    if source == BoundType::Variant {
        return CoercionResult::Ok;
    }
    let allowed = match target {
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
    };
    if allowed {
        CoercionResult::Ok
    } else {
        CoercionResult::TypeMismatch
    }
}

#[cfg(test)]
fn arithmetic_result_label(lhs: BoundType, rhs: BoundType) -> &'static str {
    match arithmetic_result(lhs, rhs) {
        ArithmeticResult::Ok(result) => bound_type_label(result),
        ArithmeticResult::TypeMismatch => "type-mismatch",
    }
}

#[cfg(test)]
fn coercion_result_label(source: BoundType, target: BoundType) -> &'static str {
    match coercion_result(source, target) {
        CoercionResult::Ok => "ok",
        CoercionResult::TypeMismatch => "type-mismatch",
    }
}

#[cfg(test)]
fn call_mode_label(call_mode: CallMode) -> &'static str {
    match call_mode {
        CallMode::Early => "early",
        CallMode::Mixed => "mixed",
        CallMode::Late => "late",
    }
}

#[cfg(test)]
fn call_coercion_result_label(
    call_mode: CallMode,
    param_type: Option<BoundType>,
    arg_type: BoundType,
) -> &'static str {
    match call_coercion_result(call_mode, param_type, arg_type) {
        CoercionResult::Ok => "ok",
        CoercionResult::TypeMismatch => "type-mismatch",
    }
}

#[cfg(test)]
fn comparison_result_label(lhs: BoundType, rhs: BoundType) -> &'static str {
    match comparison_result(lhs, rhs) {
        ComparisonResult::Ok => "ok",
        ComparisonResult::TypeMismatch => "type-mismatch",
    }
}

#[cfg(test)]
fn bound_type_label(ty: BoundType) -> &'static str {
    match ty {
        BoundType::Variant => "variant",
        BoundType::Integer => "integer",
        BoundType::Long => "long",
        BoundType::LongLong => "longlong",
        BoundType::LongPtr => "longptr",
        BoundType::Byte => "byte",
        BoundType::Single => "single",
        BoundType::Double => "double",
        BoundType::Currency => "currency",
        BoundType::Decimal => "decimal",
        BoundType::Date => "date",
        BoundType::String => "string",
        BoundType::Boolean => "boolean",
        BoundType::Object => "object",
        BoundType::Array => "array",
    }
}

fn is_arithmetic_coercible_type(ty: BoundType) -> bool {
    is_numeric_type(ty) || matches!(ty, BoundType::String | BoundType::Date | BoundType::Variant)
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
) -> Result<CallArgMapping<'a>, String> {
    let param_array_idx = params.iter().position(|p| p.param_array);
    let fixed_len = param_array_idx.unwrap_or(params.len());
    let required_fixed = params
        .iter()
        .take(fixed_len)
        .filter(|p| !p.optional)
        .count();

    if param_array_idx.is_none() && args.len() > params.len() {
        return Err(format!(
            "procedure {proc_name} expects between {} and {} args, got {}",
            required_fixed,
            params.len(),
            args.len()
        ));
    }

    let mut mapped: Vec<Option<&BoundCallArg>> = vec![None; params.len()];
    let mut extras: Vec<&BoundCallArg> = Vec::new();
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
            if params[param_idx].param_array {
                return Err(format!(
                    "named arguments are not supported for ParamArray parameter {} in procedure {proc_name}",
                    params[param_idx].name
                ));
            }
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
            if param_array_idx.is_some() {
                extras.push(arg);
                continue;
            }
            return Err("positional argument cannot follow named argument".to_string());
        }

        while next_pos < params.len() && mapped[next_pos].is_some() {
            next_pos += 1;
        }
        if next_pos >= fixed_len {
            if param_array_idx.is_some() {
                extras.push(arg);
                continue;
            }
            return Err(format!(
                "procedure {proc_name} expects between {} and {} args, got {}",
                required_fixed,
                params.len(),
                args.len()
            ));
        }
        mapped[next_pos] = Some(arg);
        next_pos += 1;
    }

    for (idx, param) in params.iter().take(fixed_len).enumerate() {
        if !param.optional && mapped[idx].is_none() {
            return Err(format!("missing required argument {}", param.name));
        }
    }

    Ok(CallArgMapping {
        fixed: mapped,
        extras,
    })
}

struct CallArgMapping<'a> {
    fixed: Vec<Option<&'a BoundCallArg>>,
    extras: Vec<&'a BoundCallArg>,
}

fn classify_call_mode(
    name: &str,
    args: &[BoundCallArg],
    proc_names: &HashSet<String>,
    proc_params: &HashMap<String, Vec<BoundParam>>,
    declared_types: &HashMap<String, BoundType>,
) -> Result<CallMode, String> {
    if contains_casefold_name(proc_names, name) {
        let Some(params) = lookup_casefold_name(proc_params, name) else {
            return Ok(CallMode::Early);
        };
        let arg_mapping = map_call_args_to_params(name, args, params)?;
        let param_array_idx = params.iter().position(|p| p.param_array);
        let dynamic_params = params
            .iter()
            .filter(|param| !param.param_array)
            .any(|param| matches!(param.ty, BoundType::Variant | BoundType::Object));
        let dynamic_args = arg_mapping.fixed.iter().flatten().any(|arg| {
            matches!(
                infer_expr_type(&arg.expr, declared_types),
                BoundType::Variant | BoundType::Object
            )
        }) || param_array_idx.is_some_and(|_| {
            arg_mapping.extras.iter().any(|arg| {
                matches!(
                    infer_expr_type(&arg.expr, declared_types),
                    BoundType::Variant | BoundType::Object
                )
            })
        });
        if dynamic_params || dynamic_args {
            return Ok(CallMode::Mixed);
        }
        return Ok(CallMode::Early);
    }

    if matches!(
        declared_types
            .get(name)
            .or_else(|| lookup_casefold_name(declared_types, name)),
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
            BoundStmt::ForRange { body, .. }
            | BoundStmt::ForEach { body, .. }
            | BoundStmt::DoWhile { body, .. } => {
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

    use super::{
        CallMode, arithmetic_result_label, call_coercion_result_label, call_mode_label,
        can_assign_to, classify_call_mode, coercion_result_label, comparison_result_label,
        intrinsic_argument_target_type, intrinsic_result_type, join_types,
    };
    use crate::resolve::{
        BoundCallArg, BoundExpr, BoundParam, BoundParamSourceMechanism, BoundType,
    };

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
    fn coercion_table_rows_align_with_typecheck_rules() {
        let table_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tables")
            .join("coercion.csv");
        let table =
            std::fs::read_to_string(&table_path).expect("coercion decision table should exist");
        for (line_idx, line) in table.lines().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split(',');
            let source_text = parts
                .next()
                .expect("source column should be present")
                .trim();
            let target_text = parts
                .next()
                .expect("target column should be present")
                .trim();
            let result_text = parts
                .next()
                .expect("result column should be present")
                .trim();
            assert!(
                parts.next().is_none(),
                "coercion table row {} should have exactly 3 columns",
                line_idx + 1
            );

            let source = parse_bound_type(source_text).expect("known source type");
            let target = parse_bound_type(target_text).expect("known target type");
            assert_eq!(
                coercion_result_label(source, target),
                result_text,
                "coercion table mismatch at row {}",
                line_idx + 1
            );
        }
    }

    #[test]
    fn arithmetic_table_rows_align_with_typecheck_rules() {
        let table_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tables")
            .join("arithmetic.csv");
        let table =
            std::fs::read_to_string(&table_path).expect("arithmetic decision table should exist");
        for (line_idx, line) in table.lines().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split(',');
            let lhs_text = parts.next().expect("lhs column should be present").trim();
            let rhs_text = parts.next().expect("rhs column should be present").trim();
            let op_text = parts.next().expect("op column should be present").trim();
            let result_text = parts
                .next()
                .expect("result column should be present")
                .trim();
            assert!(
                parts.next().is_none(),
                "arithmetic table row {} should have exactly 4 columns",
                line_idx + 1
            );
            assert!(
                matches!(op_text, "+" | "-"),
                "arithmetic table row {} has unsupported op {}",
                line_idx + 1,
                op_text
            );

            let lhs = parse_bound_type(lhs_text).expect("known lhs type");
            let rhs = parse_bound_type(rhs_text).expect("known rhs type");
            assert_eq!(
                arithmetic_result_label(lhs, rhs),
                result_text,
                "arithmetic table mismatch at row {}",
                line_idx + 1
            );
        }
    }

    #[test]
    fn comparison_table_rows_align_with_typecheck_rules() {
        let table_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tables")
            .join("comparison.csv");
        let table =
            std::fs::read_to_string(&table_path).expect("comparison decision table should exist");
        for (line_idx, line) in table.lines().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split(',');
            let lhs_text = parts.next().expect("lhs column should be present").trim();
            let rhs_text = parts.next().expect("rhs column should be present").trim();
            let op_text = parts.next().expect("op column should be present").trim();
            let result_text = parts
                .next()
                .expect("result column should be present")
                .trim();
            assert!(
                parts.next().is_none(),
                "comparison table row {} should have exactly 4 columns",
                line_idx + 1
            );
            assert!(
                matches!(op_text, "=" | "<>" | "<" | "<=" | ">" | ">="),
                "comparison table row {} has unsupported op {}",
                line_idx + 1,
                op_text
            );

            let lhs = parse_bound_type(lhs_text).expect("known lhs type");
            let rhs = parse_bound_type(rhs_text).expect("known rhs type");
            assert_eq!(
                comparison_result_label(lhs, rhs),
                result_text,
                "comparison table mismatch at row {}",
                line_idx + 1
            );
        }
    }

    #[test]
    fn call_coercion_table_rows_align_with_typecheck_rules() {
        let table_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tables")
            .join("call_coercion.csv");
        let table = std::fs::read_to_string(&table_path)
            .expect("call coercion decision table should exist");
        for (line_idx, line) in table.lines().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split(',');
            let mode_text = parts.next().expect("mode column should be present").trim();
            let param_text = parts.next().expect("param column should be present").trim();
            let arg_text = parts.next().expect("arg column should be present").trim();
            let result_text = parts
                .next()
                .expect("result column should be present")
                .trim();
            assert!(
                parts.next().is_none(),
                "call coercion table row {} should have exactly 4 columns",
                line_idx + 1
            );

            let mode = parse_call_mode(mode_text).expect("known call mode");
            let param_type = if param_text.is_empty() {
                None
            } else {
                Some(parse_bound_type(param_text).expect("known parameter type"))
            };
            assert!(
                !(matches!(mode, CallMode::Early | CallMode::Mixed) && param_type.is_none()),
                "call coercion table row {} requires param type for early/mixed mode",
                line_idx + 1
            );
            assert!(
                !(matches!(mode, CallMode::Late) && param_type.is_some()),
                "call coercion table row {} must leave param empty for late mode",
                line_idx + 1
            );

            let arg_type = parse_bound_type(arg_text).expect("known argument type");
            assert_eq!(
                call_coercion_result_label(mode, param_type, arg_type),
                result_text,
                "call coercion table mismatch at row {} (mode={})",
                line_idx + 1,
                call_mode_label(mode)
            );
        }
    }

    #[test]
    fn conversion_intrinsic_table_rows_align_with_typecheck_rules() {
        let table_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tables")
            .join("conversion_intrinsics.csv");
        let table = std::fs::read_to_string(&table_path)
            .expect("conversion intrinsic decision table should exist");
        for (line_idx, line) in table.lines().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split(',');
            let intrinsic_name = parts
                .next()
                .expect("intrinsic column should be present")
                .trim();
            let arg_target_text = parts
                .next()
                .expect("arg_target column should be present")
                .trim();
            let result_text = parts
                .next()
                .expect("result column should be present")
                .trim();
            assert!(
                parts.next().is_none(),
                "conversion table row {} should have exactly 3 columns",
                line_idx + 1
            );

            let arg_target = parse_bound_type(arg_target_text).expect("known arg target type");
            let result = parse_bound_type(result_text).expect("known result type");
            assert_eq!(
                intrinsic_argument_target_type(intrinsic_name),
                Some(arg_target),
                "conversion arg target mismatch at row {}",
                line_idx + 1
            );
            assert_eq!(
                intrinsic_result_type(intrinsic_name),
                Some(result),
                "conversion result mismatch at row {}",
                line_idx + 1
            );
        }
    }

    #[test]
    fn string_compare_search_intrinsics_have_long_result_type() {
        assert_eq!(intrinsic_result_type("instr"), Some(BoundType::Long));
        assert_eq!(intrinsic_result_type("instrrev"), Some(BoundType::Long));
        assert_eq!(intrinsic_result_type("strcomp"), Some(BoundType::Long));
        assert_eq!(intrinsic_result_type("array"), Some(BoundType::Array));
    }

    fn parse_bound_type(text: &str) -> Option<BoundType> {
        match text.trim() {
            "Variant" => Some(BoundType::Variant),
            "Integer" => Some(BoundType::Integer),
            "Long" => Some(BoundType::Long),
            "LongLong" => Some(BoundType::LongLong),
            "LongPtr" => Some(BoundType::LongPtr),
            "Byte" => Some(BoundType::Byte),
            "Single" => Some(BoundType::Single),
            "Double" => Some(BoundType::Double),
            "Currency" => Some(BoundType::Currency),
            "Decimal" => Some(BoundType::Decimal),
            "Date" => Some(BoundType::Date),
            "String" => Some(BoundType::String),
            "Boolean" => Some(BoundType::Boolean),
            "Object" => Some(BoundType::Object),
            "Array" => Some(BoundType::Array),
            _ => None,
        }
    }

    fn parse_call_mode(text: &str) -> Option<CallMode> {
        match text.trim() {
            "early" => Some(CallMode::Early),
            "mixed" => Some(CallMode::Mixed),
            "late" => Some(CallMode::Late),
            _ => None,
        }
    }

    #[test]
    fn classify_call_mode_early_for_strict_typed_procedure() {
        let proc_names = HashSet::from(["work".to_string()]);
        let proc_params = HashMap::from([(
            "work".to_string(),
            vec![BoundParam {
                name: "x".to_string(),
                source_mechanism: BoundParamSourceMechanism::ExplicitByVal,
                by_ref: false,
                param_array: false,
                optional: false,
                default_value: None,
                ty: BoundType::Long,
            }],
        )]);
        let declared_types = HashMap::new();
        let args = vec![BoundCallArg {
            name: None,
            expr: BoundExpr::IntConst(1),
            force_byval: false,
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
                source_mechanism: BoundParamSourceMechanism::ExplicitByVal,
                by_ref: false,
                param_array: false,
                optional: false,
                default_value: None,
                ty: BoundType::Variant,
            }],
        )]);
        let declared_types = HashMap::new();
        let args = vec![BoundCallArg {
            name: None,
            expr: BoundExpr::IntConst(1),
            force_byval: false,
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
            force_byval: false,
        }];
        assert_eq!(
            classify_call_mode("obj", &args, &proc_names, &proc_params, &declared_types)
                .expect("classification should succeed"),
            CallMode::Late
        );
    }
}
