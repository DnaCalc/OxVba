use std::collections::{HashMap, HashSet};

use crate::bytecode::Bytecode;
use crate::emit::{ProcedureRuntimeMetadata, emit_bytecode_with_runtime_metadata};
use crate::frontend_hir::{
    HirBinaryOp, HirCaseClause, HirDeclId, HirDeclKind, HirExprId, HirExprKind, HirLiteral,
    HirStmtId, HirStmtKind, HirUnaryOp,
};
use crate::frontend_structural_intrinsics::StructuralIntrinsic;
use crate::frontend_symbols::{SymbolId, SymbolNamespace};
use crate::frontend_type_hooks::{
    HirAssignmentIntent, TypedHirModule, collect_type_hooks_from_source,
};
use crate::optimize::optimize_module;
use crate::resolve::{
    ArithOp, AssignmentIntent, BoundArrayDescriptor, BoundCallArg, BoundCallSyntax,
    BoundCaseClause, BoundCompareMode, BoundCond, BoundExpr, BoundModule, BoundParam,
    BoundParamSourceMechanism, BoundProcedure, BoundStmt, BoundType, CompareOp, LogicalBinOp,
    RuntimeArrayDimExpr, collect_option_base,
};
use crate::typecheck::check_types;
use crate::{CompileError, VbaTypeId};
use oxvba_syntax::SyntaxKind;

#[derive(Debug, thiserror::Error)]
pub enum HirProductionLoweringError {
    #[error("unsupported HIR lowering shape: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Compile(#[from] CompileError),
}

pub fn compile_source_with_runtime_metadata_via_hir(
    source: &str,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    HirProductionLoweringError,
> {
    reject_unsupported_production_syntax(source)?;
    let typed_hir = collect_type_hooks_from_source("Main", source)
        .map_err(|err| HirProductionLoweringError::Unsupported(err.to_string()))?;
    validate_hir_assignment_diagnostics(&typed_hir)?;
    let bound = lower_typed_hir_to_bound_module(source, &typed_hir)?;
    let checked = check_types(bound).map_err(CompileError::TypeError)?;
    let optimized = if std::env::var("OXVBA_DISABLE_OPT").ok().as_deref() == Some("1") {
        checked
    } else {
        optimize_module(checked)
    };
    Ok(emit_bytecode_with_runtime_metadata(&optimized))
}

fn validate_hir_assignment_diagnostics(
    typed_hir: &TypedHirModule,
) -> Result<(), HirProductionLoweringError> {
    for (stmt, intent) in typed_hir.hooks.assignment_intents() {
        let Some(stmt_data) = typed_hir.module.arenas.stmt(stmt) else {
            continue;
        };
        let (HirStmtKind::Let { target, value } | HirStmtKind::Set { target, value }) =
            stmt_data.kind
        else {
            continue;
        };
        let Some(target_name) = hir_name_expr(typed_hir, target)? else {
            continue;
        };
        let target_type = hir_expr_bound_type(typed_hir, target).unwrap_or(BoundType::Variant);
        let value_type = hir_expr_bound_type(typed_hir, value).unwrap_or(BoundType::Variant);
        let explicit_let = stmt_data.cst.syntax_kind == "LetStmt";
        if matches!(intent, HirAssignmentIntent::Let)
            && explicit_let
            && target_type == BoundType::Object
        {
            return Err(HirProductionLoweringError::Compile(
                CompileError::TypeError(format!(
                    "type mismatch in assignment: Let cannot assign to Object variable {target_name}"
                )),
            ));
        }
        if matches!(intent, HirAssignmentIntent::Let)
            && !explicit_let
            && target_type == BoundType::Object
            && value_type == BoundType::Object
        {
            return Err(HirProductionLoweringError::Compile(
                CompileError::TypeError(format!(
                    "type mismatch in assignment: Set required for Object variable {target_name}"
                )),
            ));
        }
        if matches!(intent, HirAssignmentIntent::Let)
            && !explicit_let
            && target_type == BoundType::Object
            && !matches!(value_type, BoundType::Object | BoundType::Variant)
        {
            return Err(HirProductionLoweringError::Compile(
                CompileError::TypeError(format!(
                    "type mismatch in assignment: cannot assign {value_type:?} to Object variable {target_name}"
                )),
            ));
        }
        if matches!(intent, HirAssignmentIntent::Set)
            && !matches!(target_type, BoundType::Object | BoundType::Variant)
        {
            return Err(HirProductionLoweringError::Compile(
                CompileError::TypeError(format!(
                    "type mismatch in assignment: Set requires Object or Variant target, got {target_type:?} variable {target_name}"
                )),
            ));
        }
        if matches!(intent, HirAssignmentIntent::Set)
            && !matches!(value_type, BoundType::Object | BoundType::Variant)
        {
            return Err(HirProductionLoweringError::Compile(
                CompileError::TypeError(format!(
                    "type mismatch in assignment: Set requires object value for variable {target_name}"
                )),
            ));
        }
    }
    Ok(())
}

fn reject_unsupported_production_syntax(source: &str) -> Result<(), HirProductionLoweringError> {
    let parsed = oxvba_syntax::parse(source);
    if !parsed.errors().is_empty() {
        return Err(HirProductionLoweringError::Unsupported(format!(
            "{:?}",
            parsed.errors()
        )));
    }
    if let Some(kind) = first_unsupported_production_syntax(parsed.syntax()) {
        return Err(HirProductionLoweringError::Unsupported(format!(
            "syntax kind {kind:?}"
        )));
    }
    if let Some(text) = first_unsupported_const_stmt(parsed.syntax()) {
        return Err(HirProductionLoweringError::Unsupported(format!(
            "const statement {text:?}"
        )));
    }
    Ok(())
}

fn first_unsupported_production_syntax(node: oxvba_syntax::SyntaxNode<'_>) -> Option<SyntaxKind> {
    if matches!(
        node.kind(),
        SyntaxKind::DeclareStmt
            | SyntaxKind::TypeBlock
            | SyntaxKind::EnumBlock
            | SyntaxKind::ImplementsStmt
            | SyntaxKind::NewExpr
    ) {
        return Some(node.kind());
    }
    node.child_nodes()
        .into_iter()
        .find_map(first_unsupported_production_syntax)
}

fn first_unsupported_const_stmt(node: oxvba_syntax::SyntaxNode<'_>) -> Option<String> {
    if node.kind() == SyntaxKind::ConstStmt {
        let text = node.text();
        if !const_stmt_is_supported(&text) {
            return Some(text.trim().to_string());
        }
    }
    node.child_nodes()
        .into_iter()
        .find_map(first_unsupported_const_stmt)
}

fn const_stmt_is_supported(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let Some(pos) = lower.find("const") else {
        return false;
    };
    let payload = text[pos + "const".len()..].trim();
    let declarators = split_const_declarators(payload);
    !declarators.is_empty()
        && declarators.iter().all(|declarator| {
            let Some((_, rhs)) = declarator.split_once('=') else {
                return false;
            };
            parse_const_literal(rhs.trim()).is_some()
        })
}

pub fn lower_typed_hir_to_bound_module(
    source: &str,
    typed_hir: &TypedHirModule,
) -> Result<BoundModule, HirProductionLoweringError> {
    let const_values = collect_const_values(source, typed_hir);
    let option_base = collect_option_base(&source.lines().map(str::to_string).collect::<Vec<_>>());
    let mut procedures = Vec::new();
    for decl in &typed_hir.module.declarations {
        if let Some(proc) = lower_procedure(source, typed_hir, &const_values, option_base, *decl)? {
            procedures.push(proc);
        }
    }
    if procedures.is_empty() {
        return Err(HirProductionLoweringError::Unsupported(
            "source contains no HIR procedures".to_string(),
        ));
    }
    Ok(BoundModule {
        source: source.to_string(),
        option_explicit: false,
        is_class_module: false,
        compare_mode: BoundCompareMode::Binary,
        default_type_table: [BoundType::Variant; 26],
        resolution_diagnostics: Vec::new(),
        declarations: Vec::new(),
        declaration_types: HashMap::new(),
        array_descriptors: HashMap::new(),
        enum_descriptors: Vec::new(),
        external_declarations: HashMap::new(),
        body: Vec::new(),
        procedures,
    })
}

fn lower_procedure(
    source: &str,
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    option_base: i32,
    decl_id: HirDeclId,
) -> Result<Option<BoundProcedure>, HirProductionLoweringError> {
    let Some(decl) = typed_hir.module.arenas.decl(decl_id) else {
        return Ok(None);
    };
    let HirDeclKind::Procedure { params, body, .. } = &decl.kind else {
        return Ok(None);
    };
    let name = symbol_name(typed_hir, decl.symbol)?;
    let is_function = decl.cst.syntax_kind == "FunctionDecl";
    let mut declarations = Vec::new();
    let mut declaration_types = HashMap::new();
    let mut array_descriptors = HashMap::new();
    let mut dynamic_array_names = HashSet::new();
    let mut bound_params = Vec::new();

    for param in params {
        let param_name = symbol_name(typed_hir, *param)?;
        let ty = declared_bound_type(typed_hir, *param).unwrap_or(BoundType::Variant);
        let source_mechanism = parameter_source_mechanism(source, typed_hir, *param);
        let by_ref = !matches!(source_mechanism, BoundParamSourceMechanism::ExplicitByVal);
        declarations.push(param_name.clone());
        declaration_types.insert(param_name.clone(), ty);
        bound_params.push(BoundParam {
            name: param_name,
            source_mechanism,
            by_ref,
            param_array: false,
            optional: false,
            default_value: None,
            ty,
        });
    }

    for symbol in typed_hir.module.symbols.symbols() {
        if symbol.namespace != SymbolNamespace::Local
            || !symbol_belongs_to_decl_span(typed_hir, symbol.id, decl_id)
            || is_declaration_modifier_symbol(typed_hir, symbol.id)
            || const_values.contains_key(&symbol.id)
        {
            continue;
        }
        let local_name = symbol_name(typed_hir, symbol.id)?;
        if !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&local_name))
        {
            declarations.push(local_name.clone());
        }
        if let Some(element_type) = dynamic_array_element_type(source, symbol.id, typed_hir) {
            dynamic_array_names.insert(local_name.to_ascii_lowercase());
            declaration_types.insert(local_name.clone(), BoundType::Array);
            declaration_types.insert(format!("{local_name}_0"), element_type);
            array_descriptors.insert(
                local_name,
                BoundArrayDescriptor {
                    element_type,
                    rank: 1,
                    bounds: Vec::new(),
                    dynamic: true,
                    option_base,
                },
            );
        } else {
            declaration_types.insert(
                local_name,
                declared_bound_type(typed_hir, symbol.id).unwrap_or(BoundType::Variant),
            );
        }
    }

    if is_function {
        let return_type = declared_bound_type(typed_hir, decl.symbol).unwrap_or(BoundType::Variant);
        declarations.push(name.clone());
        declaration_types.insert(name.clone(), return_type);
    }

    let mut stmts = Vec::new();
    for stmt in body {
        lower_stmt(
            typed_hir,
            const_values,
            option_base,
            &dynamic_array_names,
            *stmt,
            &mut stmts,
        )?;
    }
    let (source_line_start, source_line_end) =
        span_lines(source, decl.cst.span.start, decl.cst.span.end);
    let mut statement_line_numbers = typed_hir
        .module
        .symbols
        .symbols()
        .iter()
        .filter(|symbol| {
            symbol.namespace == SymbolNamespace::Local
                && symbol_belongs_to_decl_span(typed_hir, symbol.id, decl_id)
                && !is_declaration_modifier_symbol(typed_hir, symbol.id)
                && !const_values.contains_key(&symbol.id)
        })
        .filter_map(|symbol| symbol.provenance.span)
        .map(|span| line_number_at(source, span.start))
        .collect::<Vec<_>>();
    statement_line_numbers.extend(
        body.iter()
            .filter_map(|stmt| typed_hir.module.arenas.stmt(*stmt))
            .map(|stmt| line_number_at(source, stmt.cst.span.start)),
    );
    statement_line_numbers.sort_unstable();
    statement_line_numbers.dedup();

    Ok(Some(BoundProcedure {
        name,
        source_line_start,
        source_line_end,
        statement_line_numbers,
        return_type: if is_function {
            declared_bound_type(typed_hir, decl.symbol).unwrap_or(BoundType::Variant)
        } else {
            BoundType::Variant
        },
        params: bound_params,
        module_scope_names: Vec::new(),
        declarations,
        declaration_types,
        array_descriptors,
        udt_descriptors: Vec::new(),
        duplicate_declarations: Vec::new(),
        body: stmts,
    }))
}

fn lower_stmt(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    option_base: i32,
    dynamic_array_names: &HashSet<String>,
    stmt: HirStmtId,
    out: &mut Vec<BoundStmt>,
) -> Result<(), HirProductionLoweringError> {
    let Some(stmt_data) = typed_hir.module.arenas.stmt(stmt) else {
        return Ok(());
    };
    match &stmt_data.kind {
        HirStmtKind::Let { target, value } => {
            let target = lower_assignment_target(typed_hir, *target)?;
            let expr = lower_expr(typed_hir, const_values, *value)?;
            let intent = if stmt_data.cst.syntax_kind == "LetStmt" {
                AssignmentIntent::Let
            } else {
                AssignmentIntent::Implicit
            };
            match expr {
                BoundExpr::ProcCall { name, args } => out.push(BoundStmt::AssignFromCall {
                    target,
                    name,
                    args,
                    intent,
                    syntax: BoundCallSyntax::ExpressionCall,
                }),
                expr => out.push(BoundStmt::Assign {
                    target,
                    expr,
                    intent,
                }),
            }
        }
        HirStmtKind::Set { target, value } => {
            let target = lower_assignment_target(typed_hir, *target)?;
            let expr = lower_expr(typed_hir, const_values, *value)?;
            match expr {
                BoundExpr::ProcCall { name, args } => out.push(BoundStmt::AssignFromCall {
                    target,
                    name,
                    args,
                    intent: AssignmentIntent::Set,
                    syntax: BoundCallSyntax::ExpressionCall,
                }),
                expr => out.push(BoundStmt::Assign {
                    target,
                    expr,
                    intent: AssignmentIntent::Set,
                }),
            }
        }
        HirStmtKind::Block(children) => {
            for child in children {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *child,
                    out,
                )?;
            }
        }
        HirStmtKind::Expr(expr) => match lower_expr(typed_hir, const_values, *expr)? {
            BoundExpr::ProcCall { name, args } => out.push(BoundStmt::Call {
                name,
                args,
                syntax: if stmt_data.cst.syntax_kind == "CallStmtNoCallKeyword" {
                    BoundCallSyntax::StatementNoCall
                } else {
                    BoundCallSyntax::StatementCallKeyword
                },
            }),
            BoundExpr::Member {
                receiver,
                member,
                args,
            } => out.push(BoundStmt::Expr {
                expr: BoundExpr::Member {
                    receiver,
                    member,
                    args,
                },
            }),
            other => {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "expression statement {other:?}"
                )));
            }
        },
        HirStmtKind::If {
            condition,
            then_body,
            else_body,
        } => {
            let mut lowered_then = Vec::new();
            for stmt in then_body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *stmt,
                    &mut lowered_then,
                )?;
            }
            let mut lowered_else = Vec::new();
            for stmt in else_body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *stmt,
                    &mut lowered_else,
                )?;
            }
            out.push(BoundStmt::IfCond {
                cond: lower_condition(typed_hir, const_values, *condition)?,
                then_body: lowered_then,
                else_body: lowered_else,
            });
        }
        HirStmtKind::DoWhile {
            condition,
            body,
            post_check,
            until,
        } => {
            let mut lowered_body = Vec::new();
            for stmt in body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *stmt,
                    &mut lowered_body,
                )?;
            }
            let mut cond = lower_condition(typed_hir, const_values, *condition)?;
            if *until {
                cond = BoundCond::Not(Box::new(cond));
            }
            out.push(BoundStmt::DoWhile {
                cond,
                body: lowered_body,
                post_check: *post_check,
            });
        }
        HirStmtKind::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            let mut lowered_arms = Vec::new();
            for (clauses, body) in arms {
                let clauses = clauses
                    .iter()
                    .map(|clause| lower_case_clause(typed_hir, const_values, clause))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut lowered_body = Vec::new();
                for stmt in body {
                    lower_stmt(
                        typed_hir,
                        const_values,
                        option_base,
                        dynamic_array_names,
                        *stmt,
                        &mut lowered_body,
                    )?;
                }
                lowered_arms.push((clauses, lowered_body));
            }
            let mut lowered_else = Vec::new();
            for stmt in else_body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *stmt,
                    &mut lowered_else,
                )?;
            }
            out.push(BoundStmt::SelectCase {
                expr: lower_expr(typed_hir, const_values, *expr)?,
                arms: lowered_arms,
                else_body: lowered_else,
            });
        }
        HirStmtKind::ForRange {
            var,
            start,
            end,
            step,
            body,
        } => {
            let mut lowered_body = Vec::new();
            for stmt in body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *stmt,
                    &mut lowered_body,
                )?;
            }
            out.push(BoundStmt::ForRange {
                var: symbol_name(typed_hir, *var)?,
                start: lower_expr(typed_hir, const_values, *start)?,
                end: lower_expr(typed_hir, const_values, *end)?,
                step: match step {
                    Some(step) => lower_expr(typed_hir, const_values, *step)?,
                    None => BoundExpr::IntConst(1),
                },
                body: lowered_body,
            });
        }
        HirStmtKind::ForEach {
            var,
            iterable,
            body,
        } => {
            let mut lowered_body = Vec::new();
            for stmt in body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    option_base,
                    dynamic_array_names,
                    *stmt,
                    &mut lowered_body,
                )?;
            }
            out.push(BoundStmt::ForEach {
                var: symbol_name(typed_hir, *var)?,
                items: Vec::new(),
                iterable: Some(lower_expr(typed_hir, const_values, *iterable)?),
                body: lowered_body,
            });
        }
        HirStmtKind::ExitDo => out.push(BoundStmt::ExitDo),
        HirStmtKind::ExitFor => out.push(BoundStmt::ExitFor),
        HirStmtKind::ExitProcedure => out.push(BoundStmt::ExitProcedure),
        HirStmtKind::OnErrorResumeNext => out.push(BoundStmt::OnErrorResumeNext),
        HirStmtKind::OnErrorGoto0 => out.push(BoundStmt::OnErrorGoto0),
        HirStmtKind::OnErrorGotoLabel { label } => out.push(BoundStmt::OnErrorGotoLabel {
            label: label.clone(),
        }),
        HirStmtKind::ResumeNext => out.push(BoundStmt::ResumeNext),
        HirStmtKind::Resume => out.push(BoundStmt::Resume),
        HirStmtKind::ResumeLabel { label } => out.push(BoundStmt::ResumeLabel {
            label: label.clone(),
        }),
        HirStmtKind::Label { name } => out.push(BoundStmt::Label { name: name.clone() }),
        HirStmtKind::GoTo { label } => out.push(BoundStmt::GoTo {
            label: label.clone(),
        }),
        HirStmtKind::GoSub { label } => out.push(BoundStmt::GoSub {
            label: label.clone(),
        }),
        HirStmtKind::Return => out.push(BoundStmt::Return),
        HirStmtKind::ReDim {
            name,
            bounds,
            preserve,
        } => {
            if !dynamic_array_names.contains(&name.to_ascii_lowercase()) {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "ReDim production lowering requires a dynamic array declaration for {name}"
                )));
            }
            if bounds.len() != 1 {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "ReDim production lowering currently supports one runtime bound, got {} for {name}",
                    bounds.len()
                )));
            }
            let bounds = bounds
                .iter()
                .map(|bound| {
                    Ok(RuntimeArrayDimExpr {
                        lower_bound: option_base,
                        upper_bound: lower_expr(typed_hir, const_values, *bound)?,
                    })
                })
                .collect::<Result<Vec<_>, HirProductionLoweringError>>()?;
            out.push(BoundStmt::ReDimRuntime {
                name: name.clone(),
                bounds,
                preserve: *preserve,
            });
        }
        HirStmtKind::Erase { name } => out.push(BoundStmt::Erase { name: name.clone() }),
        HirStmtKind::RaiseEvent { name, args } => out.push(BoundStmt::RaiseEvent {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| {
                    Ok(BoundCallArg {
                        name: None,
                        expr: lower_expr(typed_hir, const_values, *arg)?,
                        force_byval: false,
                    })
                })
                .collect::<Result<Vec<_>, HirProductionLoweringError>>()?,
        }),
        HirStmtKind::Empty => {}
    }
    Ok(())
}

fn dynamic_array_element_type(
    source: &str,
    symbol: SymbolId,
    typed_hir: &TypedHirModule,
) -> Option<BoundType> {
    let span = typed_hir
        .module
        .symbols
        .symbol(symbol)
        .and_then(|symbol| symbol.provenance.span)?;
    let suffix = source.get(span.end..)?;
    let line_end = suffix.find('\n').unwrap_or(suffix.len());
    let segment = source.get(span.end..span.end + line_end)?.trim_start();
    if !segment.starts_with("()") {
        return None;
    }
    let lower = segment.to_ascii_lowercase();
    let as_pos = lower.find(" as ")?;
    let ty_text = segment[as_pos + " as ".len()..]
        .trim_start()
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .next()?;
    bound_type_name(ty_text)
}

fn bound_type_name(text: &str) -> Option<BoundType> {
    match text.to_ascii_lowercase().as_str() {
        "boolean" => Some(BoundType::Boolean),
        "byte" => Some(BoundType::Byte),
        "integer" => Some(BoundType::Integer),
        "long" => Some(BoundType::Long),
        "longlong" => Some(BoundType::LongLong),
        "longptr" => Some(BoundType::LongPtr),
        "single" => Some(BoundType::Single),
        "double" => Some(BoundType::Double),
        "currency" => Some(BoundType::Currency),
        "date" => Some(BoundType::Date),
        "string" => Some(BoundType::String),
        "object" => Some(BoundType::Object),
        "variant" => Some(BoundType::Variant),
        _ => None,
    }
}

fn lower_assignment_target(
    typed_hir: &TypedHirModule,
    target: HirExprId,
) -> Result<String, HirProductionLoweringError> {
    let Some(expr) = typed_hir.module.arenas.expr(target) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing assignment target".to_string(),
        ));
    };
    match expr.kind {
        HirExprKind::Name(symbol) => symbol_name(typed_hir, symbol),
        _ => Err(HirProductionLoweringError::Unsupported(format!(
            "assignment target {:?}",
            expr.kind
        ))),
    }
}

fn lower_expr(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    expr: HirExprId,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let Some(expr_data) = typed_hir.module.arenas.expr(expr) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing expression".to_string(),
        ));
    };
    match &expr_data.kind {
        HirExprKind::Literal(HirLiteral::Empty) => Ok(BoundExpr::IntrinsicCall {
            name: "__empty".to_string(),
            args: Vec::new(),
        }),
        HirExprKind::Literal(HirLiteral::Null) => Ok(BoundExpr::StructuralIntrinsicCall {
            intrinsic: StructuralIntrinsic::NullLiteral,
            args: Vec::new(),
        }),
        HirExprKind::Literal(HirLiteral::Nothing) => Ok(BoundExpr::StructuralIntrinsicCall {
            intrinsic: StructuralIntrinsic::NothingLiteral,
            args: Vec::new(),
        }),
        HirExprKind::Literal(HirLiteral::Bool(value)) => Ok(BoundExpr::BoolConst(*value)),
        HirExprKind::Literal(HirLiteral::Int(value)) => {
            let value = i32::try_from(*value).map_err(|_| {
                HirProductionLoweringError::Unsupported(format!("integer literal {value}"))
            })?;
            Ok(BoundExpr::IntConst(value))
        }
        HirExprKind::Literal(HirLiteral::String(value)) => {
            Ok(BoundExpr::StringConst(value.clone()))
        }
        HirExprKind::Name(symbol) => {
            if let Some(value) = const_values.get(symbol) {
                Ok(value.clone())
            } else {
                symbol_name(typed_hir, *symbol).map(BoundExpr::Var)
            }
        }
        HirExprKind::Unary { op, expr } => match op {
            HirUnaryOp::Negate => Ok(BoundExpr::UnaryOp {
                op: ArithOp::Neg,
                operand: Box::new(lower_expr(typed_hir, const_values, *expr)?),
            }),
            HirUnaryOp::Not => Ok(BoundExpr::LogicalNot {
                operand: Box::new(lower_expr(typed_hir, const_values, *expr)?),
            }),
        },
        HirExprKind::Binary { op, lhs, rhs } => {
            lower_binary_expr(typed_hir, const_values, *op, *lhs, *rhs)
        }
        HirExprKind::Call(call) => lower_call_expr(typed_hir, const_values, *call),
        HirExprKind::Member(member) => lower_member_expr(typed_hir, const_values, *member),
        other => Err(HirProductionLoweringError::Unsupported(format!(
            "expression {other:?}"
        ))),
    }
}

fn lower_binary_expr(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    op: HirBinaryOp,
    lhs: HirExprId,
    rhs: HirExprId,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let lhs = lower_expr(typed_hir, const_values, lhs)?;
    let rhs = lower_expr(typed_hir, const_values, rhs)?;
    match op {
        HirBinaryOp::Add => binary(ArithOp::Add, lhs, rhs),
        HirBinaryOp::Sub => binary(ArithOp::Sub, lhs, rhs),
        HirBinaryOp::Mul => binary(ArithOp::Mul, lhs, rhs),
        HirBinaryOp::Div => binary(ArithOp::Div, lhs, rhs),
        HirBinaryOp::Pow => binary(ArithOp::Pow, lhs, rhs),
        HirBinaryOp::Concat => binary(ArithOp::Concat, lhs, rhs),
        HirBinaryOp::Eq => compare(CompareOp::Eq, lhs, rhs),
        HirBinaryOp::Ne => compare(CompareOp::Ne, lhs, rhs),
        HirBinaryOp::Lt => compare(CompareOp::Lt, lhs, rhs),
        HirBinaryOp::Le => compare(CompareOp::Le, lhs, rhs),
        HirBinaryOp::Gt => compare(CompareOp::Gt, lhs, rhs),
        HirBinaryOp::Ge => compare(CompareOp::Ge, lhs, rhs),
        HirBinaryOp::Is => compare(CompareOp::Is, lhs, rhs),
        HirBinaryOp::And => logical(LogicalBinOp::And, lhs, rhs),
        HirBinaryOp::Or => logical(LogicalBinOp::Or, lhs, rhs),
    }
}

fn lower_call_expr(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    call: crate::frontend_hir::HirCallId,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let Some(call_data) = typed_hir.module.arenas.call(call) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing call".to_string(),
        ));
    };
    let target = lower_expr(typed_hir, const_values, call_data.target)?;
    let args = call_data
        .args
        .iter()
        .map(|arg| {
            lower_expr(typed_hir, const_values, arg.expr).map(|expr| BoundCallArg {
                name: None,
                expr,
                force_byval: arg.force_byval,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    match target {
        BoundExpr::Var(name) => {
            let target_symbol = typed_hir
                .module
                .arenas
                .expr(call_data.target)
                .and_then(|expr| match expr.kind {
                    crate::frontend_hir::HirExprKind::Name(symbol) => {
                        typed_hir.module.symbols.symbol(symbol)
                    }
                    _ => None,
                });
            if !target_symbol.is_some_and(|symbol| {
                symbol.namespace == crate::frontend_symbols::SymbolNamespace::Procedure
            }) {
                return Err(HirProductionLoweringError::Unsupported(
                    "call target is not a procedure symbol".to_string(),
                ));
            }
            Ok(BoundExpr::ProcCall { name, args })
        }
        BoundExpr::Member {
            receiver, member, ..
        } => Ok(BoundExpr::Member {
            receiver,
            member,
            args,
        }),
        other => Err(HirProductionLoweringError::Unsupported(format!(
            "call target {other:?}"
        ))),
    }
}

fn lower_member_expr(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    member: crate::frontend_hir::HirMemberId,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let Some(member_data) = typed_hir.module.arenas.member(member) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing member expression".to_string(),
        ));
    };
    let Some(receiver) = member_data.receiver else {
        return Err(HirProductionLoweringError::Unsupported(
            "member expression without receiver".to_string(),
        ));
    };
    Ok(BoundExpr::Member {
        receiver: Box::new(lower_expr(typed_hir, const_values, receiver)?),
        member: symbol_name(typed_hir, member_data.symbol)?,
        args: Vec::new(),
    })
}

fn lower_case_clause(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    clause: &HirCaseClause,
) -> Result<BoundCaseClause, HirProductionLoweringError> {
    match clause {
        HirCaseClause::Value(expr) => {
            lower_case_value(typed_hir, const_values, *expr).map(BoundCaseClause::Value)
        }
        HirCaseClause::Range { start, end } => Ok(BoundCaseClause::Range {
            start: lower_case_value(typed_hir, const_values, *start)?,
            end: lower_case_value(typed_hir, const_values, *end)?,
        }),
        HirCaseClause::Is { op, value } => {
            let Some(op) = compare_op_from_hir(*op) else {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "Select Case Is operator {op:?}"
                )));
            };
            Ok(BoundCaseClause::Is {
                op,
                value: lower_case_value(typed_hir, const_values, *value)?,
            })
        }
    }
}

fn lower_case_value(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    expr: HirExprId,
) -> Result<i32, HirProductionLoweringError> {
    if let BoundExpr::IntConst(value) = lower_expr(typed_hir, const_values, expr)? {
        return Ok(value);
    }
    let Some(expr_data) = typed_hir.module.arenas.expr(expr) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing Select Case clause expression".to_string(),
        ));
    };
    match expr_data.kind {
        HirExprKind::Literal(HirLiteral::Int(value)) => {
            let value = i32::try_from(value).map_err(|_| {
                HirProductionLoweringError::Unsupported(format!(
                    "Select Case integer literal {value}"
                ))
            })?;
            Ok(value)
        }
        _ => Err(HirProductionLoweringError::Unsupported(format!(
            "Select Case clause {:?}",
            expr_data.kind
        ))),
    }
}

fn lower_condition(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    expr: crate::frontend_hir::HirExprId,
) -> Result<BoundCond, HirProductionLoweringError> {
    let Some(expr_data) = typed_hir.module.arenas.expr(expr) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing condition expression".to_string(),
        ));
    };
    match &expr_data.kind {
        HirExprKind::Unary {
            op: HirUnaryOp::Not,
            expr,
        } => Ok(BoundCond::Not(Box::new(lower_condition(
            typed_hir,
            const_values,
            *expr,
        )?))),
        HirExprKind::Binary { op, lhs, rhs } => match op {
            HirBinaryOp::Eq
            | HirBinaryOp::Ne
            | HirBinaryOp::Lt
            | HirBinaryOp::Le
            | HirBinaryOp::Gt
            | HirBinaryOp::Ge
            | HirBinaryOp::Is => {
                let Some(compare_op) = compare_op_from_hir(*op) else {
                    unreachable!("comparison operator covered by match arm");
                };
                Ok(BoundCond::Compare {
                    op: compare_op,
                    lhs: lower_expr(typed_hir, const_values, *lhs)?,
                    rhs: lower_expr(typed_hir, const_values, *rhs)?,
                })
            }
            HirBinaryOp::And => Ok(BoundCond::And(
                Box::new(lower_condition(typed_hir, const_values, *lhs)?),
                Box::new(lower_condition(typed_hir, const_values, *rhs)?),
            )),
            HirBinaryOp::Or => Ok(BoundCond::Or(
                Box::new(lower_condition(typed_hir, const_values, *lhs)?),
                Box::new(lower_condition(typed_hir, const_values, *rhs)?),
            )),
            _ => Ok(BoundCond::Truthy(lower_expr(
                typed_hir,
                const_values,
                expr,
            )?)),
        },
        _ => Ok(BoundCond::Truthy(lower_expr(
            typed_hir,
            const_values,
            expr,
        )?)),
    }
}

fn compare_op_from_hir(op: HirBinaryOp) -> Option<CompareOp> {
    match op {
        HirBinaryOp::Eq => Some(CompareOp::Eq),
        HirBinaryOp::Ne => Some(CompareOp::Ne),
        HirBinaryOp::Lt => Some(CompareOp::Lt),
        HirBinaryOp::Le => Some(CompareOp::Le),
        HirBinaryOp::Gt => Some(CompareOp::Gt),
        HirBinaryOp::Ge => Some(CompareOp::Ge),
        HirBinaryOp::Is => Some(CompareOp::Is),
        _ => None,
    }
}

fn binary(
    op: ArithOp,
    lhs: BoundExpr,
    rhs: BoundExpr,
) -> Result<BoundExpr, HirProductionLoweringError> {
    Ok(BoundExpr::BinaryOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn compare(
    op: CompareOp,
    lhs: BoundExpr,
    rhs: BoundExpr,
) -> Result<BoundExpr, HirProductionLoweringError> {
    Ok(BoundExpr::CompareOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn logical(
    op: LogicalBinOp,
    lhs: BoundExpr,
    rhs: BoundExpr,
) -> Result<BoundExpr, HirProductionLoweringError> {
    Ok(BoundExpr::LogicalBinaryOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

fn symbol_name(
    typed_hir: &TypedHirModule,
    symbol: SymbolId,
) -> Result<String, HirProductionLoweringError> {
    let symbol = typed_hir
        .module
        .symbols
        .symbol(symbol)
        .ok_or_else(|| HirProductionLoweringError::Unsupported("missing symbol".to_string()))?;
    Ok(typed_hir
        .module
        .symbols
        .name(symbol.name)
        .ok_or_else(|| HirProductionLoweringError::Unsupported("missing symbol name".to_string()))?
        .folded
        .clone())
}

fn collect_const_values(source: &str, typed_hir: &TypedHirModule) -> HashMap<SymbolId, BoundExpr> {
    typed_hir
        .module
        .symbols
        .symbols()
        .iter()
        .filter_map(|symbol| {
            if symbol.namespace != SymbolNamespace::Local {
                return None;
            }
            let span = symbol.provenance.span?;
            let value = const_literal_after_span(source, span)?;
            Some((symbol.id, value))
        })
        .collect()
}

fn const_literal_after_span(
    source: &str,
    span: crate::frontend_symbols::FrontendSourceSpan,
) -> Option<BoundExpr> {
    let line_start = source[..span.start].rfind('\n').map_or(0, |idx| idx + 1);
    let line_end = span.end
        + source[span.end..]
            .find('\n')
            .unwrap_or(source.len() - span.end);
    let prefix = source[line_start..span.start].to_ascii_lowercase();
    if !prefix.contains("const") {
        return None;
    }
    let suffix = source.get(span.end..line_end)?;
    let suffix = first_const_declarator_tail(suffix);
    let (_, rhs) = suffix.split_once('=')?;
    parse_const_literal(rhs.trim())
}

fn first_const_declarator_tail(text: &str) -> &str {
    let mut in_string = false;
    let mut chars = text.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '"' => {
                if in_string && matches!(chars.peek(), Some((_, '"'))) {
                    chars.next();
                } else {
                    in_string = !in_string;
                }
            }
            ',' if !in_string => return &text[..idx],
            _ => {}
        }
    }
    text
}

fn split_const_declarators(text: &str) -> Vec<&str> {
    let mut declarators = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut chars = text.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '"' => {
                if in_string && matches!(chars.peek(), Some((_, '"'))) {
                    chars.next();
                } else {
                    in_string = !in_string;
                }
            }
            ',' if !in_string => {
                let part = text[start..idx].trim();
                if !part.is_empty() {
                    declarators.push(part);
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let part = text[start..].trim();
    if !part.is_empty() {
        declarators.push(part);
    }
    declarators
}

fn parse_const_literal(text: &str) -> Option<BoundExpr> {
    if let Ok(value) = text.parse::<i32>() {
        return Some(BoundExpr::IntConst(value));
    }
    if text.eq_ignore_ascii_case("true") {
        return Some(BoundExpr::BoolConst(true));
    }
    if text.eq_ignore_ascii_case("false") {
        return Some(BoundExpr::BoolConst(false));
    }
    let text = text.trim();
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        return Some(BoundExpr::StringConst(
            text[1..text.len() - 1].replace("\"\"", "\""),
        ));
    }
    None
}

fn declared_bound_type(typed_hir: &TypedHirModule, symbol: SymbolId) -> Option<BoundType> {
    typed_hir
        .hooks
        .declared_type(symbol)
        .map(|hook| bound_type_from_vba_type_id(hook.runtime_type))
}

fn hir_name_expr(
    typed_hir: &TypedHirModule,
    expr: HirExprId,
) -> Result<Option<String>, HirProductionLoweringError> {
    match typed_hir.module.arenas.expr(expr).map(|expr| &expr.kind) {
        Some(HirExprKind::Name(symbol)) => symbol_name(typed_hir, *symbol).map(Some),
        _ => Ok(None),
    }
}

fn hir_expr_bound_type(typed_hir: &TypedHirModule, expr: HirExprId) -> Option<BoundType> {
    match typed_hir.module.arenas.expr(expr).map(|expr| &expr.kind)? {
        HirExprKind::Name(symbol) => declared_bound_type(typed_hir, *symbol),
        HirExprKind::Literal(HirLiteral::Bool(_)) => Some(BoundType::Boolean),
        HirExprKind::Literal(HirLiteral::Int(_)) => Some(BoundType::Long),
        HirExprKind::Literal(HirLiteral::String(_)) => Some(BoundType::String),
        HirExprKind::Literal(HirLiteral::Nothing) => Some(BoundType::Object),
        HirExprKind::Literal(HirLiteral::Empty | HirLiteral::Null) => Some(BoundType::Variant),
        HirExprKind::Binary { .. } | HirExprKind::Unary { .. } => Some(BoundType::Variant),
        _ => None,
    }
}

fn bound_type_from_vba_type_id(ty: VbaTypeId) -> BoundType {
    match ty {
        VbaTypeId::Integer => BoundType::Integer,
        VbaTypeId::Long => BoundType::Long,
        VbaTypeId::LongLong => BoundType::LongLong,
        VbaTypeId::LongPtr => BoundType::LongPtr,
        VbaTypeId::Byte => BoundType::Byte,
        VbaTypeId::Single => BoundType::Single,
        VbaTypeId::Double => BoundType::Double,
        VbaTypeId::Currency => BoundType::Currency,
        VbaTypeId::Date => BoundType::Date,
        VbaTypeId::String => BoundType::String,
        VbaTypeId::Boolean => BoundType::Boolean,
        VbaTypeId::Object => BoundType::Object,
        VbaTypeId::Array => BoundType::Array,
        _ => BoundType::Variant,
    }
}

fn symbol_belongs_to_decl_span(
    typed_hir: &TypedHirModule,
    symbol: SymbolId,
    decl: HirDeclId,
) -> bool {
    let Some(symbol_span) = typed_hir
        .module
        .symbols
        .symbol(symbol)
        .and_then(|symbol| symbol.provenance.span)
    else {
        return false;
    };
    let Some(decl_span) = typed_hir.module.arenas.decl(decl).map(|decl| decl.cst.span) else {
        return false;
    };
    symbol_span.start >= decl_span.start && symbol_span.end <= decl_span.end
}

fn is_declaration_modifier_symbol(typed_hir: &TypedHirModule, symbol: SymbolId) -> bool {
    matches!(
        symbol_name(typed_hir, symbol).ok().as_deref(),
        Some("withevents" | "optional" | "byval" | "byref" | "paramarray")
    )
}

fn parameter_source_mechanism(
    source: &str,
    typed_hir: &TypedHirModule,
    symbol: SymbolId,
) -> BoundParamSourceMechanism {
    let Some(span) = typed_hir
        .module
        .symbols
        .symbol(symbol)
        .and_then(|symbol| symbol.provenance.span)
    else {
        return BoundParamSourceMechanism::Omitted;
    };
    let prefix = source.get(..span.start).unwrap_or_default();
    let start = prefix
        .rfind(|ch| ['(', ',', '\n', '\r'].contains(&ch))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let segment = source.get(start..span.start).unwrap_or_default();
    if contains_ascii_word(segment, "byval") {
        BoundParamSourceMechanism::ExplicitByVal
    } else if contains_ascii_word(segment, "byref") {
        BoundParamSourceMechanism::ExplicitByRef
    } else {
        BoundParamSourceMechanism::Omitted
    }
}

fn contains_ascii_word(text: &str, needle: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| word.eq_ignore_ascii_case(needle))
}

fn span_lines(source: &str, start: usize, end: usize) -> (usize, usize) {
    (line_number_at(source, start), line_number_at(source, end))
}

fn line_number_at(source: &str, offset: usize) -> usize {
    source
        .char_indices()
        .take_while(|(idx, _)| *idx < offset)
        .filter(|(_, ch)| *ch == '\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instruction, ParameterPassingMode, SourceParameterMechanism};

    #[test]
    fn hir_production_lowering_emits_bytecode_and_metadata_for_scoped_assignment() {
        let source = "Sub Main()\nDim x As Long\nx = 1 + 2\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(bytecode.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::AddSlots { .. } | Instruction::AddConstI32 { .. }
            )
        }));
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| {
            slot.name == "x"
                && slot.kind == crate::ProcedureRuntimeSlotKind::Local
                && slot.declared_type == VbaTypeId::Long
        }));
    }

    #[test]
    fn hir_production_lowering_emits_same_module_call_statement() {
        let source = "Sub Main()\nDim v As Variant\nv = 5\nCall Use(v)\nEnd Sub\nSub Use(ByVal x As Long)\nDim sink\nsink = x\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CallProc { .. })),
            "expected CallProc bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        let use_metadata = metadata.get("use").expect("use metadata");
        assert_eq!(
            use_metadata.signature.parameters[0].source_mechanism,
            SourceParameterMechanism::ExplicitByVal
        );
        assert_eq!(
            use_metadata.signature.parameters[0].passing_mode,
            ParameterPassingMode::ByVal
        );
    }

    #[test]
    fn hir_production_lowering_projects_function_return_type() {
        let source = "Function Alpha() As Long\nAlpha = 1\nEnd Function\nSub Main()\nEnd Sub\n";
        let (_, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        let alpha = metadata.get("alpha").expect("alpha metadata");
        assert_eq!(
            alpha.return_slot.map(|slot| {
                alpha.legacy_declared_type_for_slot(
                    slot,
                    crate::ProcedureRuntimeSlotKind::ReturnValue,
                )
            }),
            Some(VbaTypeId::Long),
            "{alpha:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_emits_branch_bytecode_for_multiline_if() {
        let source = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nEnd If\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected conditional branch bytecode: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| {
            slot.name == "x"
                && slot.kind == crate::ProcedureRuntimeSlotKind::Local
                && slot.declared_type == VbaTypeId::Long
        }));
    }

    #[test]
    fn hir_production_lowering_preserves_logical_if_conditions() {
        let source = "Sub Main()\nDim x As Long\nDim y As Long\nIf x = 0 And y = 0 Then\nx = 1\nEnd If\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected conditional branch bytecode: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            main.slots.iter().any(|slot| slot.name == "x")
                && main.slots.iter().any(|slot| slot.name == "y"),
            "{main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_emits_nested_branches_for_elseif() {
        let source = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nElseIf x = 1 Then\nx = 2\nElse\nx = 3\nEnd If\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        let branch_count = bytecode
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::JumpIfZero { .. }))
            .count();
        assert!(
            branch_count >= 2,
            "expected branch bytecode for If and ElseIf: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_branch_bytecode_for_single_line_if() {
        let source = "Sub Main()\nDim x As Long\nIf x = 0 Then x = 1 Else x = 2\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected conditional branch bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_loop_bytecode_for_do_while() {
        let source = "Sub Main()\nDim x As Long\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected loop exit branch bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Jump { .. })),
            "expected loop backedge bytecode: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| {
            slot.name == "x"
                && slot.kind == crate::ProcedureRuntimeSlotKind::Local
                && slot.declared_type == VbaTypeId::Long
        }));
    }

    #[test]
    fn hir_production_lowering_emits_loop_bytecode_for_until_and_post_check_loops() {
        let source = "Sub Main()\nDim x As Long\nDo Until x = 3\nx = x + 1\nLoop\nDo\nx = x + 1\nLoop Until x = 7\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        let branch_count = bytecode
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::JumpIfZero { .. }))
            .count();
        assert!(
            branch_count >= 2,
            "expected branch bytecode for both loops: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| {
            slot.name == "x"
                && slot.kind == crate::ProcedureRuntimeSlotKind::Local
                && slot.declared_type == VbaTypeId::Long
        }));
    }

    #[test]
    fn hir_production_lowering_emits_loop_bytecode_for_while_wend() {
        let source = "Sub Main()\nDim x As Long\nWhile x < 3\nx = x + 1\nWend\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected loop exit branch bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Jump { .. })),
            "expected loop backedge bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_loop_bytecode_for_for_range() {
        let source = "Sub Main()\nDim i As Long\nFor i = 1 To 3\ni = i + 1\nNext\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected for-loop exit branch bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Jump { .. })),
            "expected for-loop backedge bytecode: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| {
            slot.name == "i"
                && slot.kind == crate::ProcedureRuntimeSlotKind::Local
                && slot.declared_type == VbaTypeId::Long
        }));
    }

    #[test]
    fn hir_production_lowering_emits_loop_bytecode_for_for_each() {
        let source =
            "Sub Main()\nDim item As Variant\nFor Each item In item\nitem = item\nNext\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::IntrinsicForEachInit { .. })),
            "expected For Each init bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::IntrinsicForEachNext { .. })),
            "expected For Each next bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_exit_statements() {
        let source = "Sub Main()\nDim i As Long\nDo While i < 3\nExit Do\nLoop\nFor i = 1 To 3\nExit For\nNext\nExit Sub\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        let jump_count = bytecode
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::Jump { .. }))
            .count();
        assert!(
            jump_count >= 3,
            "expected loop/procedure exit jumps: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_basic_error_control_statements() {
        let source =
            "Sub Main()\nOn Error Resume Next\nResume Next\nOn Error GoTo 0\nResume\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::SetOnErrorResumeNext)),
            "expected On Error Resume Next bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::SetOnErrorGoto0)),
            "expected On Error GoTo 0 bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::ResumeNext)),
            "expected Resume Next bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Resume)),
            "expected Resume bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_label_error_control_statements() {
        let source = "Sub Main()\nOn Error GoTo handler\nhandler:\nResume done\ndone:\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::SetOnErrorGotoLabel { .. })),
            "expected On Error GoTo label bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::ResumeLabel { .. })),
            "expected Resume label bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_goto_label_jumps() {
        let source = "Sub Main()\nGoTo done\ndone:\nGoTo 100\n100:\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Jump { .. })),
            "expected GoTo jump bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_gosub_and_return() {
        let source = "Sub Main()\nGoSub helper\nhelper:\nReturn\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CallProc { .. })),
            "expected GoSub call bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Return)),
            "expected Return bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_erase_statement() {
        let source = "Sub Main()\nDim a\nErase a\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(!bytecode.instructions.is_empty());
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_runtime_redim_for_dynamic_array() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nlength = 3\nReDim Preserve buf(length - 1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArrayResizePreserve {
                    lower_bounds,
                    element_type: crate::bytecode::RuntimeArrayElementType::Byte,
                    ..
                } if lower_bounds == &vec![0]
            )),
            "expected runtime ReDim Preserve bytecode: {:?}",
            bytecode.instructions
        );
        let proc = metadata.get("main").expect("main metadata");
        let shape = proc
            .array_shapes
            .iter()
            .find(|shape| shape.name == "buf")
            .expect("dynamic array shape");
        assert_eq!(shape.element_type, VbaTypeId::Byte);
        assert_eq!(shape.rank, 1);
    }

    #[test]
    fn hir_production_lowering_rejects_fixed_array_redim_for_fallback() {
        let source = "Sub Main()\nDim a(1)\nReDim a(3)\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("fixed-array ReDim remains a residual");

        assert!(
            matches!(err, HirProductionLoweringError::Unsupported(_)),
            "fixed-array ReDim must remain fallback-eligible, got {err:?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_raise_event_statement() {
        let source = "Sub Main()\nRaiseEvent Tick(1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LoadConstI32 { .. })),
            "{bytecode:#?}"
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_event_declaration_with_raise_event() {
        let source = "Event Tick(ByVal value)\nSub Main()\nRaiseEvent Tick(1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            !bytecode.instructions.is_empty(),
            "expected declared-event fixture to emit argument evaluation bytecode"
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_value_side_member_expressions() {
        let source =
            "Sub Main()\nDim obj\nDim x\nDim y\nx = obj.Value\ny = obj.Method(1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            !bytecode.instructions.is_empty(),
            "expected member expression bytecode"
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| slot.name == "obj"));
        assert!(main.slots.iter().any(|slot| slot.name == "x"));
        assert!(main.slots.iter().any(|slot| slot.name == "y"));
    }

    #[test]
    fn hir_production_lowering_accepts_statement_form_procedure_call_arguments() {
        let source = "Sub Use(ByVal a, ByVal b)\nEnd Sub\nSub Main()\nUse 1, 2\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        let main = metadata.get("main").expect("main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| call_site
                .target_name
                .eq_ignore_ascii_case("use")
                && call_site.arguments.len() == 2
                && call_site.invocation_syntax
                    == crate::emit::CallInvocationSyntaxDescriptor::StatementNoCall),
            "expected statement-form call to preserve two call-site args: {main:#?}"
        );
        assert!(
            !bytecode.instructions.is_empty(),
            "expected statement-form call bytecode"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_statement_form_member_call_arguments() {
        let source = "Sub Main()\nDim obj\nobj.Method 1, 2\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost { args, .. }
                    if args.len() == 2
                )
            }),
            "expected statement-form member call to preserve dispatch args: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_with_member_reads() {
        let source = "Sub Main()\nDim obj\nDim x\nWith obj\nx = .Value\nEnd With\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost { args, .. }
                    if args.is_empty()
                )
            }),
            "expected With member read to emit dispatch invoke: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_rejects_with_member_assignment_target_for_fallback() {
        let source = "Sub Main()\nDim obj\nWith obj\n.Value = 1\nEnd With\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("With member assignment target remains residual");

        assert!(
            matches!(err, HirProductionLoweringError::Unsupported(_)),
            "With member assignment target must remain fallback-eligible, got {err:?}"
        );
    }

    #[test]
    fn hir_production_lowering_rejects_member_assignment_target_for_fallback() {
        let source = "Sub Main()\nDim obj\nobj.Value = 1\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("member assignment target remains residual");

        assert!(
            matches!(err, HirProductionLoweringError::Unsupported(_)),
            "member assignment target must remain fallback-eligible, got {err:?}"
        );
    }

    #[test]
    fn hir_production_lowering_rejects_bang_member_access_for_fallback() {
        let source = "Sub Main()\nDim obj\nDim x\nx = obj!Value\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("bang member access remains residual");

        assert!(
            matches!(err, HirProductionLoweringError::Unsupported(_)),
            "bang member access must remain fallback-eligible, got {err:?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_const_statement() {
        let source = "Const CBase = 7\nSub Main()\nDim x\nx = CBase\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "{bytecode:#?}"
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            !main
                .slots
                .iter()
                .any(|slot| slot.name.eq_ignore_ascii_case("cbase")),
            "{main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_multi_literal_const_statement() {
        let source = "Const CBase = 7, CName = \"a,b\"\nSub Main()\nDim x\nDim y\nx = CBase\ny = CName\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "{bytecode:#?}"
        );
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstString { value, .. } if value == "a,b"
            )),
            "{bytecode:#?}"
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            !main.slots.iter().any(|slot| {
                slot.name.eq_ignore_ascii_case("cbase") || slot.name.eq_ignore_ascii_case("cname")
            }),
            "{main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_rejects_expression_const_statement() {
        let source = "Const CBase = 1 + 2\nSub Main()\nDim x\nx = CBase\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("expression constants remain a tracked residual");
        assert!(matches!(err, HirProductionLoweringError::Unsupported(_)));
    }

    #[test]
    fn hir_production_lowering_emits_branch_bytecode_for_select_case() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase 1\nx = 2\nEnd Select\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected case branch bytecode: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| {
            slot.name == "x"
                && slot.kind == crate::ProcedureRuntimeSlotKind::Local
                && slot.declared_type == VbaTypeId::Long
        }));
    }

    #[test]
    fn hir_production_lowering_emits_branch_bytecode_for_select_case_range() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected case branch bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_branch_bytecode_for_select_case_multi_value() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase 1, 2\nx = 2\nEnd Select\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::BoolOr { .. })),
            "expected aggregate case match bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_emits_branch_bytecode_for_select_case_is() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase Is < 0\nx = 2\nEnd Select\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::JumpIfZero { .. })),
            "expected case branch bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }
}
