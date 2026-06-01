use std::collections::HashMap;

use crate::bytecode::Bytecode;
use crate::emit::{ProcedureRuntimeMetadata, emit_bytecode_with_runtime_metadata};
use crate::frontend_hir::{
    HirBinaryOp, HirCaseClause, HirDeclId, HirDeclKind, HirExprId, HirExprKind, HirLiteral,
    HirStmtId, HirStmtKind, HirUnaryOp,
};
use crate::frontend_structural_intrinsics::StructuralIntrinsic;
use crate::frontend_symbols::{SymbolId, SymbolNamespace};
use crate::frontend_type_hooks::{TypedHirModule, collect_type_hooks_from_source};
use crate::optimize::optimize_module;
use crate::resolve::{
    ArithOp, AssignmentIntent, BoundCallArg, BoundCallSyntax, BoundCaseClause, BoundCompareMode,
    BoundCond, BoundExpr, BoundModule, BoundParam, BoundParamSourceMechanism, BoundProcedure,
    BoundStmt, BoundType, CompareOp, LogicalBinOp,
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
    let bound = lower_typed_hir_to_bound_module(source, &typed_hir)?;
    let checked = check_types(bound).map_err(CompileError::TypeError)?;
    let optimized = if std::env::var("OXVBA_DISABLE_OPT").ok().as_deref() == Some("1") {
        checked
    } else {
        optimize_module(checked)
    };
    Ok(emit_bytecode_with_runtime_metadata(&optimized))
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
    Ok(())
}

fn first_unsupported_production_syntax(node: oxvba_syntax::SyntaxNode<'_>) -> Option<SyntaxKind> {
    if matches!(
        node.kind(),
        SyntaxKind::DeclareStmt
            | SyntaxKind::ConstStmt
            | SyntaxKind::TypeBlock
            | SyntaxKind::EnumBlock
            | SyntaxKind::ForEachStmt
            | SyntaxKind::WithStmt
            | SyntaxKind::OnErrorStmt
            | SyntaxKind::ResumeStmt
            | SyntaxKind::ReDimStmt
            | SyntaxKind::EraseStmt
            | SyntaxKind::ExitStmt
            | SyntaxKind::GoToStmt
            | SyntaxKind::GoSubStmt
            | SyntaxKind::ReturnStmt
            | SyntaxKind::RaiseEventStmt
            | SyntaxKind::ImplementsStmt
            | SyntaxKind::EventDecl
            | SyntaxKind::MemberExpr
            | SyntaxKind::NewExpr
    ) {
        return Some(node.kind());
    }
    node.child_nodes()
        .into_iter()
        .find_map(first_unsupported_production_syntax)
}

pub fn lower_typed_hir_to_bound_module(
    source: &str,
    typed_hir: &TypedHirModule,
) -> Result<BoundModule, HirProductionLoweringError> {
    let mut procedures = Vec::new();
    for decl in &typed_hir.module.declarations {
        if let Some(proc) = lower_procedure(source, typed_hir, *decl)? {
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
        declaration_types.insert(
            local_name,
            declared_bound_type(typed_hir, symbol.id).unwrap_or(BoundType::Variant),
        );
    }

    if is_function {
        declarations.push(name.clone());
        declaration_types.insert(name.clone(), BoundType::Variant);
    }

    let mut stmts = Vec::new();
    for stmt in body {
        lower_stmt(typed_hir, *stmt, &mut stmts)?;
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
        return_type: BoundType::Variant,
        params: bound_params,
        module_scope_names: Vec::new(),
        declarations,
        declaration_types,
        array_descriptors: HashMap::new(),
        udt_descriptors: Vec::new(),
        duplicate_declarations: Vec::new(),
        body: stmts,
    }))
}

fn lower_stmt(
    typed_hir: &TypedHirModule,
    stmt: HirStmtId,
    out: &mut Vec<BoundStmt>,
) -> Result<(), HirProductionLoweringError> {
    let Some(stmt_data) = typed_hir.module.arenas.stmt(stmt) else {
        return Ok(());
    };
    match &stmt_data.kind {
        HirStmtKind::Let { target, value } => out.push(BoundStmt::Assign {
            target: lower_assignment_target(typed_hir, *target)?,
            expr: lower_expr(typed_hir, *value)?,
            intent: if stmt_data.cst.syntax_kind == "LetStmt" {
                AssignmentIntent::Let
            } else {
                AssignmentIntent::Implicit
            },
        }),
        HirStmtKind::Set { target, value } => out.push(BoundStmt::Assign {
            target: lower_assignment_target(typed_hir, *target)?,
            expr: lower_expr(typed_hir, *value)?,
            intent: AssignmentIntent::Set,
        }),
        HirStmtKind::Block(children) => {
            for child in children {
                lower_stmt(typed_hir, *child, out)?;
            }
        }
        HirStmtKind::Expr(expr) => match lower_expr(typed_hir, *expr)? {
            BoundExpr::ProcCall { name, args } => out.push(BoundStmt::Call {
                name,
                args,
                syntax: BoundCallSyntax::StatementCallKeyword,
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
                lower_stmt(typed_hir, *stmt, &mut lowered_then)?;
            }
            let mut lowered_else = Vec::new();
            for stmt in else_body {
                lower_stmt(typed_hir, *stmt, &mut lowered_else)?;
            }
            out.push(BoundStmt::IfCond {
                cond: lower_condition(typed_hir, *condition)?,
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
                lower_stmt(typed_hir, *stmt, &mut lowered_body)?;
            }
            let mut cond = lower_condition(typed_hir, *condition)?;
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
                    .map(|clause| lower_case_clause(typed_hir, clause))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut lowered_body = Vec::new();
                for stmt in body {
                    lower_stmt(typed_hir, *stmt, &mut lowered_body)?;
                }
                lowered_arms.push((clauses, lowered_body));
            }
            let mut lowered_else = Vec::new();
            for stmt in else_body {
                lower_stmt(typed_hir, *stmt, &mut lowered_else)?;
            }
            out.push(BoundStmt::SelectCase {
                expr: lower_expr(typed_hir, *expr)?,
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
                lower_stmt(typed_hir, *stmt, &mut lowered_body)?;
            }
            out.push(BoundStmt::ForRange {
                var: symbol_name(typed_hir, *var)?,
                start: lower_expr(typed_hir, *start)?,
                end: lower_expr(typed_hir, *end)?,
                step: match step {
                    Some(step) => lower_expr(typed_hir, *step)?,
                    None => BoundExpr::IntConst(1),
                },
                body: lowered_body,
            });
        }
        HirStmtKind::Empty => {}
    }
    Ok(())
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
        HirExprKind::Name(symbol) => symbol_name(typed_hir, *symbol).map(BoundExpr::Var),
        HirExprKind::Unary { op, expr } => match op {
            HirUnaryOp::Negate => Ok(BoundExpr::UnaryOp {
                op: ArithOp::Neg,
                operand: Box::new(lower_expr(typed_hir, *expr)?),
            }),
            HirUnaryOp::Not => Ok(BoundExpr::LogicalNot {
                operand: Box::new(lower_expr(typed_hir, *expr)?),
            }),
        },
        HirExprKind::Binary { op, lhs, rhs } => lower_binary_expr(typed_hir, *op, *lhs, *rhs),
        HirExprKind::Call(call) => lower_call_expr(typed_hir, *call),
        other => Err(HirProductionLoweringError::Unsupported(format!(
            "expression {other:?}"
        ))),
    }
}

fn lower_binary_expr(
    typed_hir: &TypedHirModule,
    op: HirBinaryOp,
    lhs: HirExprId,
    rhs: HirExprId,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let lhs = lower_expr(typed_hir, lhs)?;
    let rhs = lower_expr(typed_hir, rhs)?;
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
    call: crate::frontend_hir::HirCallId,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let Some(call_data) = typed_hir.module.arenas.call(call) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing call".to_string(),
        ));
    };
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
    let target = lower_expr(typed_hir, call_data.target)?;
    let args = call_data
        .args
        .iter()
        .map(|arg| {
            lower_expr(typed_hir, *arg).map(|expr| BoundCallArg {
                name: None,
                expr,
                force_byval: false,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    match target {
        BoundExpr::Var(name) => Ok(BoundExpr::ProcCall { name, args }),
        other => Err(HirProductionLoweringError::Unsupported(format!(
            "call target {other:?}"
        ))),
    }
}

fn lower_case_clause(
    typed_hir: &TypedHirModule,
    clause: &HirCaseClause,
) -> Result<BoundCaseClause, HirProductionLoweringError> {
    match clause {
        HirCaseClause::Value(expr) => {
            lower_case_value(typed_hir, *expr).map(BoundCaseClause::Value)
        }
        HirCaseClause::Range { start, end } => Ok(BoundCaseClause::Range {
            start: lower_case_value(typed_hir, *start)?,
            end: lower_case_value(typed_hir, *end)?,
        }),
    }
}

fn lower_case_value(
    typed_hir: &TypedHirModule,
    expr: HirExprId,
) -> Result<i32, HirProductionLoweringError> {
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
        } => Ok(BoundCond::Not(Box::new(lower_condition(typed_hir, *expr)?))),
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
                    lhs: lower_expr(typed_hir, *lhs)?,
                    rhs: lower_expr(typed_hir, *rhs)?,
                })
            }
            HirBinaryOp::And => Ok(BoundCond::And(
                Box::new(lower_condition(typed_hir, *lhs)?),
                Box::new(lower_condition(typed_hir, *rhs)?),
            )),
            HirBinaryOp::Or => Ok(BoundCond::Or(
                Box::new(lower_condition(typed_hir, *lhs)?),
                Box::new(lower_condition(typed_hir, *rhs)?),
            )),
            _ => Ok(BoundCond::Truthy(lower_expr(typed_hir, expr)?)),
        },
        _ => Ok(BoundCond::Truthy(lower_expr(typed_hir, expr)?)),
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

fn declared_bound_type(typed_hir: &TypedHirModule, symbol: SymbolId) -> Option<BoundType> {
    typed_hir
        .hooks
        .declared_type(symbol)
        .map(|hook| bound_type_from_vba_type_id(hook.runtime_type))
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
}
