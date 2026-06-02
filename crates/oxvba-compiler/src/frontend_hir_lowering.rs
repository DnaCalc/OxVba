use std::collections::{HashMap, HashSet, VecDeque};

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
    BoundCaseClause, BoundCond, BoundEnumDescriptor, BoundEnumMemberDescriptor, BoundExpr,
    BoundModule, BoundParam, BoundParamSourceMechanism, BoundProcedure, BoundStmt, BoundType,
    BoundUdtDescriptor, BoundUdtFieldDescriptor, CompareOp, LogicalBinOp, ProcKind,
    RuntimeArrayDimExpr, collect_declared_external_procedures, collect_option_base,
    collect_option_compare_mode, parse_proc_signature,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirNewExpressionBinding {
    pub type_name: String,
    pub object_handle: i32,
}

#[derive(Debug, Default)]
struct HirLoweringContext {
    new_expression_bindings: HashMap<String, VecDeque<i32>>,
    dynamic_array_names: HashSet<String>,
    fixed_array_bounds: HashMap<String, Vec<(i32, i32)>>,
}

impl HirLoweringContext {
    fn from_new_expression_bindings(bindings: &[HirNewExpressionBinding]) -> Self {
        let mut new_expression_bindings = HashMap::<String, VecDeque<i32>>::new();
        for binding in bindings {
            new_expression_bindings
                .entry(binding.type_name.to_ascii_lowercase())
                .or_default()
                .push_back(binding.object_handle);
        }
        Self {
            new_expression_bindings,
            dynamic_array_names: HashSet::new(),
            fixed_array_bounds: HashMap::new(),
        }
    }

    fn take_new_expression_handle(&mut self, type_name: &str) -> Option<i32> {
        self.new_expression_bindings
            .get_mut(&type_name.to_ascii_lowercase())
            .and_then(VecDeque::pop_front)
    }

    fn set_dynamic_array_names(&mut self, names: &HashSet<String>) {
        self.dynamic_array_names = names.clone();
    }

    fn set_fixed_array_bounds(&mut self, bounds: &HashMap<String, Vec<(i32, i32)>>) {
        self.fixed_array_bounds = bounds.clone();
    }

    fn is_dynamic_array(&self, name: &str) -> bool {
        self.dynamic_array_names
            .contains(&name.to_ascii_lowercase())
    }

    fn fixed_array_alias(&self, name: &str, indices: &[BoundCallArg]) -> Option<String> {
        let bounds = self.fixed_array_bounds.get(&name.to_ascii_lowercase())?;
        let indices = indices
            .iter()
            .map(|arg| match arg.expr {
                BoundExpr::IntConst(value) => Some(value),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let linear = linearize_static_array_index(bounds, &indices)?;
        Some(format!("{name}_{linear}"))
    }
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
    compile_source_with_runtime_metadata_via_hir_with_new_bindings(source, &[])
}

pub fn compile_source_with_runtime_metadata_via_hir_with_new_bindings(
    source: &str,
    new_expression_bindings: &[HirNewExpressionBinding],
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
    let bound = lower_typed_hir_to_bound_module_with_new_bindings(
        source,
        &typed_hir,
        new_expression_bindings,
    )?;
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
    if let Some(text) = first_unsupported_const_stmt(parsed.syntax()) {
        return Err(HirProductionLoweringError::Unsupported(format!(
            "const statement {text:?}"
        )));
    }
    Ok(())
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
    parse_const_statement_values(payload).is_some()
}

pub fn lower_typed_hir_to_bound_module(
    source: &str,
    typed_hir: &TypedHirModule,
) -> Result<BoundModule, HirProductionLoweringError> {
    lower_typed_hir_to_bound_module_with_new_bindings(source, typed_hir, &[])
}

pub fn lower_typed_hir_to_bound_module_with_new_bindings(
    source: &str,
    typed_hir: &TypedHirModule,
    new_expression_bindings: &[HirNewExpressionBinding],
) -> Result<BoundModule, HirProductionLoweringError> {
    let mut context = HirLoweringContext::from_new_expression_bindings(new_expression_bindings);
    let const_values = collect_const_values(source, typed_hir);
    let enum_descriptors = collect_hir_enum_descriptors(source);
    let lines = source.lines().map(str::to_string).collect::<Vec<_>>();
    let default_type_table = crate::resolve::collect_default_type_table(&lines);
    let (external_procedures, external_declarations, external_diagnostics) =
        collect_declared_external_procedures(&lines, &default_type_table);
    if !external_diagnostics.is_empty() {
        return Err(HirProductionLoweringError::Unsupported(format!(
            "external declaration diagnostics: {external_diagnostics:?}"
        )));
    }
    let option_base = collect_option_base(&lines);
    let compare_mode = collect_option_compare_mode(&lines);
    let mut procedures = external_procedures;
    let mut hir_procedure_count = 0usize;
    for decl in &typed_hir.module.declarations {
        if let Some(proc) = lower_procedure(
            source,
            typed_hir,
            &const_values,
            option_base,
            &default_type_table,
            *decl,
            &mut context,
        )? {
            hir_procedure_count += 1;
            procedures.push(proc);
        }
    }
    if hir_procedure_count == 0 {
        return Err(HirProductionLoweringError::Unsupported(
            "source contains no HIR procedures".to_string(),
        ));
    }
    Ok(BoundModule {
        source: source.to_string(),
        option_explicit: collect_option_explicit(&lines),
        is_class_module: false,
        compare_mode,
        default_type_table,
        resolution_diagnostics: Vec::new(),
        declarations: Vec::new(),
        declaration_types: HashMap::new(),
        array_descriptors: HashMap::new(),
        enum_descriptors,
        external_declarations,
        body: Vec::new(),
        procedures,
    })
}

fn collect_option_explicit(lines: &[String]) -> bool {
    lines
        .iter()
        .any(|line| line.trim().eq_ignore_ascii_case("Option Explicit"))
}

fn lower_procedure(
    source: &str,
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    option_base: i32,
    default_type_table: &[BoundType; 26],
    decl_id: HirDeclId,
    context: &mut HirLoweringContext,
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
    let mut fixed_array_bounds = HashMap::new();
    let mut bound_params = Vec::new();
    let udt_defs = collect_hir_udt_definitions(source);
    let mut udt_instances = HashMap::<String, String>::new();
    let parsed_signature = parsed_signature_for_hir_decl(source, decl, default_type_table);
    let parsed_params = parsed_signature
        .as_ref()
        .map(|(params, _return_type)| params.clone())
        .unwrap_or_default();

    for param in params {
        let param_name = symbol_name(typed_hir, *param)?;
        let parsed_param = parsed_params
            .iter()
            .find(|candidate| candidate.name.eq_ignore_ascii_case(&param_name));
        let ty = parsed_param
            .map(|candidate| candidate.ty)
            .or_else(|| declared_bound_type(typed_hir, *param))
            .unwrap_or(BoundType::Variant);
        let source_mechanism = parsed_param
            .map(|candidate| candidate.source_mechanism)
            .unwrap_or_else(|| parameter_source_mechanism(source, typed_hir, *param));
        let by_ref = parsed_param
            .map(|candidate| candidate.by_ref)
            .unwrap_or_else(|| {
                !matches!(source_mechanism, BoundParamSourceMechanism::ExplicitByVal)
            });
        declarations.push(param_name.clone());
        declaration_types.insert(param_name.clone(), ty);
        bound_params.push(BoundParam {
            name: param_name,
            source_mechanism,
            by_ref,
            param_array: parsed_param.is_some_and(|candidate| candidate.param_array),
            optional: parsed_param.is_some_and(|candidate| candidate.optional),
            default_value: parsed_param.and_then(|candidate| candidate.default_value),
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
        } else if let Some((bounds, element_type)) =
            fixed_array_declaration(source, symbol.id, typed_hir, option_base)
        {
            fixed_array_bounds.insert(local_name.to_ascii_lowercase(), bounds.clone());
            declaration_types.insert(local_name.clone(), BoundType::Array);
            array_descriptors.insert(
                local_name.clone(),
                BoundArrayDescriptor {
                    element_type,
                    rank: bounds.len(),
                    bounds: bounds.clone(),
                    dynamic: false,
                    option_base,
                },
            );
            let element_count = static_array_element_count(&bounds).ok_or_else(|| {
                HirProductionLoweringError::Unsupported(format!(
                    "fixed-array declaration has invalid bounds for {local_name}"
                ))
            })?;
            for index in 0..element_count {
                let alias = format!("{local_name}_{index}");
                if !declarations
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&alias))
                {
                    declarations.push(alias.clone());
                }
                declaration_types.insert(alias, element_type);
            }
        } else if let Some(udt_name) =
            declared_udt_type_name(source, symbol.id, typed_hir, &udt_defs)
        {
            declaration_types.insert(local_name.clone(), BoundType::Variant);
            udt_instances.insert(local_name.clone(), udt_name.clone());
            if let Some(fields) = udt_defs.get(&udt_name) {
                for field in fields {
                    let alias = format!("{local_name}_{}", field.name);
                    if !declarations
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&alias))
                    {
                        declarations.push(alias.clone());
                    }
                    declaration_types.insert(alias, field.bound_type);
                }
            }
        } else {
            declaration_types.insert(
                local_name,
                declared_bound_type_or_default(source, typed_hir, symbol.id, default_type_table)
                    .unwrap_or(BoundType::Variant),
            );
        }
    }

    if is_function {
        let return_type = parsed_signature
            .as_ref()
            .map(|(_params, return_type)| *return_type)
            .or_else(|| declared_bound_type(typed_hir, decl.symbol))
            .unwrap_or(BoundType::Variant);
        declarations.push(name.clone());
        declaration_types.insert(name.clone(), return_type);
    }

    let udt_field_aliases = build_hir_udt_field_aliases(&udt_defs, &udt_instances);
    let udt_instance_fields = build_hir_udt_instance_fields(&udt_defs, &udt_instances);
    context.set_dynamic_array_names(&dynamic_array_names);
    context.set_fixed_array_bounds(&fixed_array_bounds);
    let mut stmts = Vec::new();
    for stmt in body {
        lower_stmt(
            typed_hir,
            const_values,
            &udt_field_aliases,
            &udt_instance_fields,
            option_base,
            &dynamic_array_names,
            &mut fixed_array_bounds,
            &mut declarations,
            &mut declaration_types,
            &mut array_descriptors,
            *stmt,
            &mut stmts,
            context,
        )?;
    }
    for stmt in &stmts {
        if let BoundStmt::ReDimRuntime { name, bounds, .. } = stmt
            && let Some(descriptor) = array_descriptors.get_mut(name)
        {
            descriptor.rank = descriptor.rank.max(bounds.len());
        }
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
            parsed_signature
                .as_ref()
                .map(|(_params, return_type)| *return_type)
                .or_else(|| declared_bound_type(typed_hir, decl.symbol))
                .unwrap_or(BoundType::Variant)
        } else {
            BoundType::Variant
        },
        params: bound_params,
        module_scope_names: Vec::new(),
        declarations,
        declaration_types,
        array_descriptors,
        udt_descriptors: build_hir_udt_descriptors(&udt_defs, &udt_instances),
        duplicate_declarations: Vec::new(),
        body: stmts,
    }))
}

fn parsed_signature_for_hir_decl(
    source: &str,
    decl: &crate::frontend_hir::HirDecl,
    default_type_table: &[BoundType; 26],
) -> Option<(Vec<BoundParam>, BoundType)> {
    let kind = match decl.cst.syntax_kind.as_str() {
        "SubDecl" => ProcKind::Sub,
        "FunctionDecl" => ProcKind::Function,
        "PropertyDecl" => ProcKind::PropertyGet,
        _ => return None,
    };
    let start = decl.cst.span.start.min(source.len());
    let end = decl.cst.span.end.min(source.len());
    let text = source.get(start..end)?;
    let first_line = text.lines().next()?.trim();
    let (_name, params, return_type) = parse_proc_signature(first_line, kind, default_type_table)?;
    Some((params, return_type))
}

fn lower_stmt(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    udt_field_aliases: &HashMap<(String, String), String>,
    udt_instance_fields: &HashMap<String, Vec<String>>,
    option_base: i32,
    dynamic_array_names: &HashSet<String>,
    fixed_array_bounds: &mut HashMap<String, Vec<(i32, i32)>>,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
    array_descriptors: &mut HashMap<String, BoundArrayDescriptor>,
    stmt: HirStmtId,
    out: &mut Vec<BoundStmt>,
    context: &mut HirLoweringContext,
) -> Result<(), HirProductionLoweringError> {
    let Some(stmt_data) = typed_hir.module.arenas.stmt(stmt) else {
        return Ok(());
    };
    match &stmt_data.kind {
        HirStmtKind::Let { target, value } => {
            let target = match lower_assignment_target(typed_hir, udt_field_aliases, *target) {
                Ok(target) => target,
                Err(err) => {
                    match lower_expr(typed_hir, const_values, udt_field_aliases, *target, context)?
                    {
                        BoundExpr::Var(target) => {
                            let expr = lower_expr(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                *value,
                                context,
                            )?;
                            let intent = if stmt_data.cst.syntax_kind == "LetStmt" {
                                AssignmentIntent::Let
                            } else {
                                AssignmentIntent::Implicit
                            };
                            out.push(BoundStmt::Assign {
                                target,
                                expr,
                                intent,
                            });
                            return Ok(());
                        }
                        BoundExpr::Member {
                            receiver,
                            member,
                            args,
                        } => {
                            let expr = lower_expr(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                *value,
                                context,
                            )?;
                            let intent = if stmt_data.cst.syntax_kind == "LetStmt" {
                                AssignmentIntent::Let
                            } else {
                                AssignmentIntent::Implicit
                            };
                            out.push(BoundStmt::AssignMember {
                                receiver: *receiver,
                                member,
                                args,
                                expr,
                                intent,
                            });
                            return Ok(());
                        }
                        BoundExpr::ProcCall { name, args } => {
                            let expr = lower_expr(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                *value,
                                context,
                            )?;
                            let intent = if stmt_data.cst.syntax_kind == "LetStmt" {
                                AssignmentIntent::Let
                            } else {
                                AssignmentIntent::Implicit
                            };
                            out.push(BoundStmt::AssignDefaultMember {
                                receiver: name,
                                args,
                                expr,
                                intent,
                            });
                            return Ok(());
                        }
                        BoundExpr::IntrinsicCall { name, args } if name == "__oxvba_array_get" => {
                            if let Some((name, indices)) = runtime_array_target_from_args(args) {
                                let expr = lower_expr(
                                    typed_hir,
                                    const_values,
                                    udt_field_aliases,
                                    *value,
                                    context,
                                )?;
                                let intent = if stmt_data.cst.syntax_kind == "LetStmt" {
                                    AssignmentIntent::Let
                                } else {
                                    AssignmentIntent::Implicit
                                };
                                out.push(BoundStmt::AssignRuntimeArrayElement {
                                    name,
                                    indices,
                                    expr,
                                    intent,
                                });
                                return Ok(());
                            }
                        }
                        _ => {}
                    }
                    return Err(err);
                }
            };
            if let Some(source) = hir_name_expr(typed_hir, *value)?
                && let (Some(target_fields), Some(source_fields)) = (
                    udt_instance_fields.get(&target.to_ascii_lowercase()),
                    udt_instance_fields.get(&source.to_ascii_lowercase()),
                )
            {
                if target_fields == source_fields {
                    out.push(BoundStmt::UdtAssign {
                        target,
                        source,
                        fields: target_fields.clone(),
                    });
                    return Ok(());
                }
            }
            let expr = lower_expr(typed_hir, const_values, udt_field_aliases, *value, context)?;
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
            let target = match lower_assignment_target(typed_hir, udt_field_aliases, *target) {
                Ok(target) => target,
                Err(err) => {
                    match lower_expr(typed_hir, const_values, udt_field_aliases, *target, context)?
                    {
                        BoundExpr::Var(target) => {
                            let expr = lower_expr(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                *value,
                                context,
                            )?;
                            out.push(BoundStmt::Assign {
                                target,
                                expr,
                                intent: AssignmentIntent::Set,
                            });
                            return Ok(());
                        }
                        BoundExpr::Member {
                            receiver,
                            member,
                            args,
                        } => {
                            let expr = lower_expr(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                *value,
                                context,
                            )?;
                            out.push(BoundStmt::AssignMember {
                                receiver: *receiver,
                                member,
                                args,
                                expr,
                                intent: AssignmentIntent::Set,
                            });
                            return Ok(());
                        }
                        BoundExpr::ProcCall { name, args } => {
                            let expr = lower_expr(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                *value,
                                context,
                            )?;
                            out.push(BoundStmt::AssignDefaultMember {
                                receiver: name,
                                args,
                                expr,
                                intent: AssignmentIntent::Set,
                            });
                            return Ok(());
                        }
                        BoundExpr::IntrinsicCall { name, args } if name == "__oxvba_array_get" => {
                            if let Some((name, indices)) = runtime_array_target_from_args(args) {
                                let expr = lower_expr(
                                    typed_hir,
                                    const_values,
                                    udt_field_aliases,
                                    *value,
                                    context,
                                )?;
                                out.push(BoundStmt::AssignRuntimeArrayElement {
                                    name,
                                    indices,
                                    expr,
                                    intent: AssignmentIntent::Set,
                                });
                                return Ok(());
                            }
                        }
                        _ => {}
                    }
                    return Err(err);
                }
            };
            if let Some(source) = hir_name_expr(typed_hir, *value)?
                && let (Some(target_fields), Some(source_fields)) = (
                    udt_instance_fields.get(&target.to_ascii_lowercase()),
                    udt_instance_fields.get(&source.to_ascii_lowercase()),
                )
            {
                if target_fields == source_fields {
                    out.push(BoundStmt::UdtAssign {
                        target,
                        source,
                        fields: target_fields.clone(),
                    });
                    return Ok(());
                }
            }
            let expr = lower_expr(typed_hir, const_values, udt_field_aliases, *value, context)?;
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
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *child,
                    out,
                    context,
                )?;
            }
        }
        HirStmtKind::Expr(expr) => {
            match lower_expr(typed_hir, const_values, udt_field_aliases, *expr, context)? {
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
            }
        }
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
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *stmt,
                    &mut lowered_then,
                    context,
                )?;
            }
            let mut lowered_else = Vec::new();
            for stmt in else_body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *stmt,
                    &mut lowered_else,
                    context,
                )?;
            }
            out.push(BoundStmt::IfCond {
                cond: lower_condition(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *condition,
                    context,
                )?,
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
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *stmt,
                    &mut lowered_body,
                    context,
                )?;
            }
            let mut cond = lower_condition(
                typed_hir,
                const_values,
                udt_field_aliases,
                *condition,
                context,
            )?;
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
                    .map(|clause| {
                        lower_case_clause(
                            typed_hir,
                            const_values,
                            udt_field_aliases,
                            clause,
                            context,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut lowered_body = Vec::new();
                for stmt in body {
                    lower_stmt(
                        typed_hir,
                        const_values,
                        udt_field_aliases,
                        udt_instance_fields,
                        option_base,
                        dynamic_array_names,
                        fixed_array_bounds,
                        declarations,
                        declaration_types,
                        array_descriptors,
                        *stmt,
                        &mut lowered_body,
                        context,
                    )?;
                }
                lowered_arms.push((clauses, lowered_body));
            }
            let mut lowered_else = Vec::new();
            for stmt in else_body {
                lower_stmt(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *stmt,
                    &mut lowered_else,
                    context,
                )?;
            }
            out.push(BoundStmt::SelectCase {
                expr: lower_expr(typed_hir, const_values, udt_field_aliases, *expr, context)?,
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
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *stmt,
                    &mut lowered_body,
                    context,
                )?;
            }
            out.push(BoundStmt::ForRange {
                var: symbol_name(typed_hir, *var)?,
                start: lower_expr(typed_hir, const_values, udt_field_aliases, *start, context)?,
                end: lower_expr(typed_hir, const_values, udt_field_aliases, *end, context)?,
                step: match step {
                    Some(step) => {
                        lower_expr(typed_hir, const_values, udt_field_aliases, *step, context)?
                    }
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
                    udt_field_aliases,
                    udt_instance_fields,
                    option_base,
                    dynamic_array_names,
                    fixed_array_bounds,
                    declarations,
                    declaration_types,
                    array_descriptors,
                    *stmt,
                    &mut lowered_body,
                    context,
                )?;
            }
            out.push(BoundStmt::ForEach {
                var: symbol_name(typed_hir, *var)?,
                items: Vec::new(),
                iterable: Some(lower_expr(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *iterable,
                    context,
                )?),
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
            let folded_name = name.to_ascii_lowercase();
            if !dynamic_array_names.contains(&folded_name)
                && let Some(previous_bounds) = fixed_array_bounds.get(&folded_name).cloned()
            {
                let bounds = bounds
                    .iter()
                    .map(|bound| {
                        let lower = if let Some(lower) = bound.lower {
                            lower_static_redim_bound(
                                typed_hir,
                                const_values,
                                udt_field_aliases,
                                lower,
                                context,
                            )?
                        } else {
                            option_base
                        };
                        let upper = lower_static_redim_bound(
                            typed_hir,
                            const_values,
                            udt_field_aliases,
                            bound.upper,
                            context,
                        )?;
                        if upper < lower {
                            return Err(HirProductionLoweringError::Unsupported(format!(
                                "fixed-array ReDim upper bound is below lower bound for {name}"
                            )));
                        }
                        Ok((lower, upper))
                    })
                    .collect::<Result<Vec<_>, HirProductionLoweringError>>()?;
                let element_type = array_descriptors
                    .get(name)
                    .map(|descriptor| descriptor.element_type)
                    .or_else(|| declaration_types.get(&format!("{name}_0")).copied())
                    .unwrap_or(BoundType::Variant);
                ensure_fixed_array_aliases(
                    name,
                    &bounds,
                    element_type,
                    declarations,
                    declaration_types,
                )?;
                fixed_array_bounds.insert(folded_name, bounds.clone());
                context.set_fixed_array_bounds(fixed_array_bounds);
                array_descriptors.insert(
                    name.clone(),
                    BoundArrayDescriptor {
                        element_type,
                        rank: bounds.len(),
                        bounds: bounds.clone(),
                        dynamic: false,
                        option_base,
                    },
                );
                out.push(BoundStmt::ReDim {
                    name: name.clone(),
                    bounds,
                    previous_bounds: Some(previous_bounds),
                    preserve: *preserve,
                });
                return Ok(());
            }
            if !dynamic_array_names.contains(&folded_name) {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "ReDim production lowering requires a dynamic array declaration for {name}"
                )));
            }
            let bounds = bounds
                .iter()
                .map(|bound| {
                    let lower_bound = if let Some(lower) = bound.lower {
                        lower_static_redim_bound(
                            typed_hir,
                            const_values,
                            udt_field_aliases,
                            lower,
                            context,
                        )?
                    } else {
                        option_base
                    };
                    Ok(RuntimeArrayDimExpr {
                        lower_bound,
                        upper_bound: lower_expr(
                            typed_hir,
                            const_values,
                            udt_field_aliases,
                            bound.upper,
                            context,
                        )?,
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
                        expr: lower_expr(
                            typed_hir,
                            const_values,
                            udt_field_aliases,
                            *arg,
                            context,
                        )?,
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

fn fixed_array_declaration(
    source: &str,
    symbol: SymbolId,
    typed_hir: &TypedHirModule,
    option_base: i32,
) -> Option<(Vec<(i32, i32)>, BoundType)> {
    let span = typed_hir
        .module
        .symbols
        .symbol(symbol)
        .and_then(|symbol| symbol.provenance.span)?;
    let suffix = source.get(span.end..)?;
    let line_end = suffix.find('\n').unwrap_or(suffix.len());
    let segment = source.get(span.end..span.end + line_end)?.trim_start();
    if !segment.starts_with('(') || segment.starts_with("()") {
        return None;
    }
    let close = segment.find(')')?;
    let bounds = parse_static_array_bounds(&segment[1..close], option_base)?;
    let element_type = segment[close + 1..]
        .to_ascii_lowercase()
        .find(" as ")
        .and_then(|as_pos| {
            let ty_text = segment[close + 1 + as_pos + " as ".len()..]
                .trim_start()
                .split(|ch: char| ch == ',' || ch.is_whitespace())
                .next()?;
            bound_type_name(ty_text)
        })
        .or_else(|| declared_bound_type(typed_hir, symbol))
        .unwrap_or(BoundType::Variant);
    Some((bounds, element_type))
}

fn parse_static_array_bounds(raw: &str, option_base: i32) -> Option<Vec<(i32, i32)>> {
    let mut bounds = Vec::new();
    for dim in raw.split(',') {
        let trimmed = dim.trim();
        if trimmed.is_empty() {
            return None;
        }
        let (lower, upper) = if let Some((lhs, rhs)) = split_keyword_ci(trimmed, "to") {
            (lhs.trim().parse().ok()?, rhs.trim().parse().ok()?)
        } else {
            (option_base, trimmed.parse().ok()?)
        };
        if upper < lower {
            return None;
        }
        bounds.push((lower, upper));
    }
    (!bounds.is_empty()).then_some(bounds)
}

fn static_array_element_count(bounds: &[(i32, i32)]) -> Option<usize> {
    let mut total = 1usize;
    for (lower, upper) in bounds {
        if upper < lower {
            return None;
        }
        let width = (*upper as i64 - *lower as i64 + 1) as usize;
        total = total.checked_mul(width)?;
    }
    Some(total)
}

fn linearize_static_array_index(bounds: &[(i32, i32)], indices: &[i32]) -> Option<usize> {
    if bounds.len() != indices.len() {
        return None;
    }
    let mut offset = 0usize;
    let mut stride = 1usize;
    for dim in (0..bounds.len()).rev() {
        let (lower, upper) = bounds[dim];
        let idx = indices[dim];
        if idx < lower || idx > upper {
            return None;
        }
        offset = offset.checked_add(((idx - lower) as usize).checked_mul(stride)?)?;
        let width = (upper as i64 - lower as i64 + 1) as usize;
        stride = stride.checked_mul(width)?;
    }
    Some(offset)
}

fn ensure_fixed_array_aliases(
    name: &str,
    bounds: &[(i32, i32)],
    element_type: BoundType,
    declarations: &mut Vec<String>,
    declaration_types: &mut HashMap<String, BoundType>,
) -> Result<(), HirProductionLoweringError> {
    let element_count = static_array_element_count(bounds).ok_or_else(|| {
        HirProductionLoweringError::Unsupported(format!(
            "fixed-array ReDim has invalid bounds for {name}"
        ))
    })?;
    for index in 0..element_count {
        let alias = format!("{name}_{index}");
        if !declarations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&alias))
        {
            declarations.push(alias.clone());
        }
        declaration_types.insert(alias, element_type);
    }
    Ok(())
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

fn runtime_array_target_from_args(mut args: Vec<BoundExpr>) -> Option<(String, Vec<BoundExpr>)> {
    if args.len() < 2 {
        return None;
    }
    let BoundExpr::Var(name) = args.remove(0) else {
        return None;
    };
    Some((name, args))
}

fn lower_static_redim_bound(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    udt_field_aliases: &HashMap<(String, String), String>,
    expr: HirExprId,
    context: &mut HirLoweringContext,
) -> Result<i32, HirProductionLoweringError> {
    match lower_expr(typed_hir, const_values, udt_field_aliases, expr, context)? {
        BoundExpr::IntConst(value) => Ok(value),
        other => Err(HirProductionLoweringError::Unsupported(format!(
            "ReDim lower bound must be a static integer, got {other:?}"
        ))),
    }
}

fn lower_assignment_target(
    typed_hir: &TypedHirModule,
    udt_field_aliases: &HashMap<(String, String), String>,
    target: HirExprId,
) -> Result<String, HirProductionLoweringError> {
    let Some(expr) = typed_hir.module.arenas.expr(target) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing assignment target".to_string(),
        ));
    };
    match expr.kind {
        HirExprKind::Name(symbol) => symbol_name(typed_hir, symbol),
        HirExprKind::Member(member) => udt_member_alias(typed_hir, udt_field_aliases, member)
            .ok_or_else(|| {
                HirProductionLoweringError::Unsupported(format!(
                    "assignment target {:?}",
                    expr.kind
                ))
            }),
        _ => Err(HirProductionLoweringError::Unsupported(format!(
            "assignment target {:?}",
            expr.kind
        ))),
    }
}

fn lower_expr(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    udt_field_aliases: &HashMap<(String, String), String>,
    expr: HirExprId,
    context: &mut HirLoweringContext,
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
        HirExprKind::New { type_name } => {
            let Some(object_handle) = context.take_new_expression_handle(type_name) else {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "New expression `{type_name}` requires project-aware construction binding"
                )));
            };
            Ok(BoundExpr::StructuralIntrinsicCall {
                intrinsic: StructuralIntrinsic::ProjectInstance,
                args: vec![BoundExpr::IntConst(object_handle)],
            })
        }
        HirExprKind::Unary { op, expr } => match op {
            HirUnaryOp::Negate => Ok(BoundExpr::UnaryOp {
                op: ArithOp::Neg,
                operand: Box::new(lower_expr(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *expr,
                    context,
                )?),
            }),
            HirUnaryOp::Not => Ok(BoundExpr::LogicalNot {
                operand: Box::new(lower_expr(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *expr,
                    context,
                )?),
            }),
        },
        HirExprKind::Binary { op, lhs, rhs } => lower_binary_expr(
            typed_hir,
            const_values,
            udt_field_aliases,
            *op,
            *lhs,
            *rhs,
            context,
        ),
        HirExprKind::Call(call) => {
            lower_call_expr(typed_hir, const_values, udt_field_aliases, *call, context)
        }
        HirExprKind::Member(member) => {
            if let Some(alias) = udt_member_alias(typed_hir, udt_field_aliases, *member) {
                Ok(BoundExpr::Var(alias))
            } else {
                lower_member_expr(typed_hir, const_values, udt_field_aliases, *member, context)
            }
        }
        other => Err(HirProductionLoweringError::Unsupported(format!(
            "expression {other:?}"
        ))),
    }
}

fn lower_binary_expr(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    udt_field_aliases: &HashMap<(String, String), String>,
    op: HirBinaryOp,
    lhs: HirExprId,
    rhs: HirExprId,
    context: &mut HirLoweringContext,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let lhs = lower_expr(typed_hir, const_values, udt_field_aliases, lhs, context)?;
    let rhs = lower_expr(typed_hir, const_values, udt_field_aliases, rhs, context)?;
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
    udt_field_aliases: &HashMap<(String, String), String>,
    call: crate::frontend_hir::HirCallId,
    context: &mut HirLoweringContext,
) -> Result<BoundExpr, HirProductionLoweringError> {
    let Some(call_data) = typed_hir.module.arenas.call(call) else {
        return Err(HirProductionLoweringError::Unsupported(
            "missing call".to_string(),
        ));
    };
    let target = lower_expr(
        typed_hir,
        const_values,
        udt_field_aliases,
        call_data.target,
        context,
    )?;
    let args = call_data
        .args
        .iter()
        .map(|arg| {
            lower_expr(
                typed_hir,
                const_values,
                udt_field_aliases,
                arg.expr,
                context,
            )
            .map(|expr| BoundCallArg {
                name: arg.name.clone(),
                expr,
                force_byval: arg.force_byval,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    match target {
        BoundExpr::Var(name) => {
            if call_data.cst.syntax_kind == "IndexExpr"
                && let Some(alias) = context.fixed_array_alias(&name, &args)
            {
                return Ok(BoundExpr::Var(alias));
            }
            if call_data.cst.syntax_kind == "IndexExpr" && context.is_dynamic_array(&name) {
                let mut intrinsic_args = Vec::with_capacity(args.len() + 1);
                intrinsic_args.push(BoundExpr::Var(name));
                intrinsic_args.extend(args.into_iter().map(|arg| arg.expr));
                return Ok(BoundExpr::IntrinsicCall {
                    name: "__oxvba_array_get".to_string(),
                    args: intrinsic_args,
                });
            }
            if let Some(intrinsic) = StructuralIntrinsic::from_legacy_name(&name) {
                return Ok(BoundExpr::StructuralIntrinsicCall {
                    intrinsic,
                    args: args.into_iter().map(|arg| arg.expr).collect(),
                });
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
    udt_field_aliases: &HashMap<(String, String), String>,
    member: crate::frontend_hir::HirMemberId,
    context: &mut HirLoweringContext,
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
        receiver: Box::new(lower_expr(
            typed_hir,
            const_values,
            udt_field_aliases,
            receiver,
            context,
        )?),
        member: symbol_name(typed_hir, member_data.symbol)?,
        args: Vec::new(),
    })
}

fn lower_case_clause(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    udt_field_aliases: &HashMap<(String, String), String>,
    clause: &HirCaseClause,
    context: &mut HirLoweringContext,
) -> Result<BoundCaseClause, HirProductionLoweringError> {
    match clause {
        HirCaseClause::Value(expr) => {
            lower_case_value(typed_hir, const_values, udt_field_aliases, *expr, context)
                .map(BoundCaseClause::Value)
        }
        HirCaseClause::Range { start, end } => Ok(BoundCaseClause::Range {
            start: lower_case_value(typed_hir, const_values, udt_field_aliases, *start, context)?,
            end: lower_case_value(typed_hir, const_values, udt_field_aliases, *end, context)?,
        }),
        HirCaseClause::Is { op, value } => {
            let Some(op) = compare_op_from_hir(*op) else {
                return Err(HirProductionLoweringError::Unsupported(format!(
                    "Select Case Is operator {op:?}"
                )));
            };
            Ok(BoundCaseClause::Is {
                op,
                value: lower_case_value(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *value,
                    context,
                )?,
            })
        }
    }
}

fn lower_case_value(
    typed_hir: &TypedHirModule,
    const_values: &HashMap<SymbolId, BoundExpr>,
    udt_field_aliases: &HashMap<(String, String), String>,
    expr: HirExprId,
    context: &mut HirLoweringContext,
) -> Result<i32, HirProductionLoweringError> {
    if let BoundExpr::IntConst(value) =
        lower_expr(typed_hir, const_values, udt_field_aliases, expr, context)?
    {
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
    udt_field_aliases: &HashMap<(String, String), String>,
    expr: crate::frontend_hir::HirExprId,
    context: &mut HirLoweringContext,
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
            udt_field_aliases,
            *expr,
            context,
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
                    lhs: lower_expr(typed_hir, const_values, udt_field_aliases, *lhs, context)?,
                    rhs: lower_expr(typed_hir, const_values, udt_field_aliases, *rhs, context)?,
                })
            }
            HirBinaryOp::And => Ok(BoundCond::And(
                Box::new(lower_condition(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *lhs,
                    context,
                )?),
                Box::new(lower_condition(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *rhs,
                    context,
                )?),
            )),
            HirBinaryOp::Or => Ok(BoundCond::Or(
                Box::new(lower_condition(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *lhs,
                    context,
                )?),
                Box::new(lower_condition(
                    typed_hir,
                    const_values,
                    udt_field_aliases,
                    *rhs,
                    context,
                )?),
            )),
            _ => Ok(BoundCond::Truthy(lower_expr(
                typed_hir,
                const_values,
                udt_field_aliases,
                expr,
                context,
            )?)),
        },
        _ => Ok(BoundCond::Truthy(lower_expr(
            typed_hir,
            const_values,
            udt_field_aliases,
            expr,
            context,
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
    let enum_values = collect_hir_enum_constants(source);
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
            let value = const_literal_after_span(source, span).or_else(|| {
                let name = typed_hir
                    .module
                    .symbols
                    .name(symbol.name)
                    .map(|name| name.folded.as_str())?;
                enum_values.get(name).cloned()
            })?;
            Some((symbol.id, value))
        })
        .collect()
}

fn collect_hir_enum_constants(source: &str) -> HashMap<String, BoundExpr> {
    let mut constants = HashMap::new();
    for descriptor in collect_hir_enum_descriptors(source) {
        for member in descriptor.members {
            constants.insert(
                member.name.to_ascii_lowercase(),
                BoundExpr::IntConst(member.value),
            );
        }
    }
    constants
}

#[derive(Debug, Clone)]
struct HirUdtFieldDef {
    name: String,
    bound_type: BoundType,
}

type HirUdtDefMap = HashMap<String, Vec<HirUdtFieldDef>>;

fn collect_hir_udt_definitions(source: &str) -> HirUdtDefMap {
    let lines = source.lines().collect::<Vec<_>>();
    let mut defs = HashMap::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        let Some(type_name) = strip_keyword_prefix_ci(line, "type").and_then(normalize_hir_ident)
        else {
            index += 1;
            continue;
        };
        index += 1;
        let mut fields = Vec::new();
        while index < lines.len() {
            let line = lines[index].trim();
            if line.eq_ignore_ascii_case("end type") {
                break;
            }
            if let Some(field) = parse_hir_udt_field(line) {
                fields.push(field);
            }
            index += 1;
        }
        defs.insert(type_name, fields);
        index += 1;
    }
    defs
}

fn parse_hir_udt_field(line: &str) -> Option<HirUdtFieldDef> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('\'') {
        return None;
    }
    let (name_part, type_part) = split_keyword_ci(trimmed, "as").unwrap_or((trimmed, "Variant"));
    let name = normalize_hir_ident(name_part.split('(').next().unwrap_or(name_part).trim())?;
    let bound_type = parse_hir_bound_type(type_part.trim()).unwrap_or(BoundType::Variant);
    Some(HirUdtFieldDef { name, bound_type })
}

fn declared_udt_type_name(
    source: &str,
    symbol: SymbolId,
    typed_hir: &TypedHirModule,
    udt_defs: &HirUdtDefMap,
) -> Option<String> {
    let span = typed_hir.module.symbols.symbol(symbol)?.provenance.span?;
    let type_name = declared_type_name_after_span(source, span)?;
    udt_defs.contains_key(&type_name).then_some(type_name)
}

fn declared_type_name_after_span(
    source: &str,
    span: crate::frontend_symbols::FrontendSourceSpan,
) -> Option<String> {
    let suffix = source.get(span.end..)?;
    let line_end = span.end + suffix.find('\n').unwrap_or(suffix.len());
    let segment = source.get(span.end..line_end)?;
    let lower = segment.to_ascii_lowercase();
    let as_pos = lower.find(" as ")?;
    let after_as = span.end + as_pos + " as ".len();
    let ty_text = source
        .get(after_as..line_end)?
        .trim_start()
        .split(|ch: char| ch == ',' || ch == ')' || ch.is_whitespace())
        .next()?;
    normalize_hir_ident(ty_text)
}

fn build_hir_udt_descriptors(
    udt_defs: &HirUdtDefMap,
    instances: &HashMap<String, String>,
) -> Vec<BoundUdtDescriptor> {
    let mut grouped = HashMap::<String, Vec<String>>::new();
    for (instance, type_name) in instances {
        grouped
            .entry(type_name.clone())
            .or_default()
            .push(instance.clone());
    }
    let mut descriptors = grouped
        .into_iter()
        .filter_map(|(type_name, mut variable_names)| {
            variable_names.sort();
            variable_names.dedup();
            let fields = udt_defs.get(&type_name)?;
            Some(BoundUdtDescriptor {
                type_name,
                variable_names,
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| BoundUdtFieldDescriptor {
                        index,
                        name: field.name.clone(),
                        bound_type: field.bound_type,
                        nested_udt_name: None,
                        array_bounds: None,
                        fixed_string_len: None,
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| left.type_name.cmp(&right.type_name));
    descriptors
}

fn build_hir_udt_field_aliases(
    udt_defs: &HirUdtDefMap,
    instances: &HashMap<String, String>,
) -> HashMap<(String, String), String> {
    let mut aliases = HashMap::new();
    for (instance, type_name) in instances {
        let Some(fields) = udt_defs.get(type_name) else {
            continue;
        };
        for field in fields {
            aliases.insert(
                (
                    instance.to_ascii_lowercase(),
                    field.name.to_ascii_lowercase(),
                ),
                format!("{instance}_{}", field.name),
            );
        }
    }
    aliases
}

fn build_hir_udt_instance_fields(
    udt_defs: &HirUdtDefMap,
    instances: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for (instance, type_name) in instances {
        let Some(fields) = udt_defs.get(type_name) else {
            continue;
        };
        out.insert(
            instance.to_ascii_lowercase(),
            fields.iter().map(|field| field.name.clone()).collect(),
        );
    }
    out
}

fn udt_member_alias(
    typed_hir: &TypedHirModule,
    udt_field_aliases: &HashMap<(String, String), String>,
    member: crate::frontend_hir::HirMemberId,
) -> Option<String> {
    let member_data = typed_hir.module.arenas.member(member)?;
    let receiver = member_data.receiver?;
    let receiver_name = hir_name_expr(typed_hir, receiver).ok().flatten()?;
    let member_name = symbol_name(typed_hir, member_data.symbol).ok()?;
    udt_field_aliases
        .get(&(
            receiver_name.to_ascii_lowercase(),
            member_name.to_ascii_lowercase(),
        ))
        .cloned()
}

fn collect_hir_enum_descriptors(source: &str) -> Vec<BoundEnumDescriptor> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut descriptors = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        if let Some((type_name, is_public)) = parse_hir_enum_header(line) {
            index += 1;
            let mut members = Vec::new();
            let mut next_value = 0i32;
            while index < lines.len() {
                let line = lines[index].trim();
                if line.eq_ignore_ascii_case("end enum") {
                    break;
                }
                if let Some((name, explicit)) = parse_hir_enum_member(line) {
                    let value = explicit.unwrap_or(next_value);
                    members.push(BoundEnumMemberDescriptor {
                        name,
                        value,
                        ordinal: members.len(),
                        explicit_value: explicit.is_some(),
                    });
                    next_value = value.saturating_add(1);
                }
                index += 1;
            }
            descriptors.push(BoundEnumDescriptor {
                type_name,
                is_public,
                members,
            });
        }
        index += 1;
    }
    descriptors.sort_by(|left, right| {
        left.type_name
            .to_ascii_lowercase()
            .cmp(&right.type_name.to_ascii_lowercase())
    });
    descriptors
}

fn parse_hir_enum_header(line: &str) -> Option<(String, bool)> {
    strip_keyword_prefix_ci(line, "public enum")
        .and_then(|rest| normalize_hir_ident(rest).map(|name| (name, true)))
        .or_else(|| {
            strip_keyword_prefix_ci(line, "private enum")
                .and_then(|rest| normalize_hir_ident(rest).map(|name| (name, false)))
        })
        .or_else(|| {
            strip_keyword_prefix_ci(line, "enum")
                .and_then(|rest| normalize_hir_ident(rest).map(|name| (name, false)))
        })
}

fn parse_hir_enum_member(line: &str) -> Option<(String, Option<i32>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((lhs, rhs)) = trimmed.split_once('=') {
        let name = normalize_hir_ident(lhs.trim())?;
        let value = rhs.trim().parse::<i32>().ok()?;
        return Some((name, Some(value)));
    }
    normalize_hir_ident(trimmed).map(|name| (name, None))
}

fn strip_keyword_prefix_ci<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let trimmed = text.trim();
    if trimmed.len() < keyword.len() || !trimmed[..keyword.len()].eq_ignore_ascii_case(keyword) {
        return None;
    }
    let rest = &trimmed[keyword.len()..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim())
}

fn split_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let needle = format!(" {} ", keyword.to_ascii_lowercase());
    let pos = lower.find(&needle)?;
    Some((&text[..pos], &text[pos + needle.len()..]))
}

fn normalize_hir_ident(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
        let name = &trimmed[1..trimmed.len() - 1];
        return is_valid_hir_identifier(name).then(|| name.to_ascii_lowercase());
    }
    let name = trimmed.split_whitespace().next().unwrap_or_default();
    is_valid_hir_identifier(name).then(|| name.to_ascii_lowercase())
}

fn parse_hir_bound_type(text: &str) -> Option<BoundType> {
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

fn is_valid_hir_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
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
    let line = source.get(line_start..line_end)?;
    let const_env = const_values_before_span_on_line(line, span.start - line_start)?;
    let suffix = source.get(span.end..line_end)?;
    let suffix = first_const_declarator_tail(suffix);
    let (_, rhs) = suffix.split_once('=')?;
    parse_const_value(rhs.trim(), &const_env)
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

fn parse_const_statement_values(text: &str) -> Option<HashMap<String, BoundExpr>> {
    let mut values = HashMap::new();
    let declarators = split_const_declarators(text);
    if declarators.is_empty() {
        return None;
    }
    for declarator in declarators {
        let (name_part, rhs) = declarator.split_once('=')?;
        let name_part = split_keyword_ci(name_part.trim(), "as")
            .map(|(name, _)| name)
            .unwrap_or(name_part);
        let name = normalize_hir_ident(name_part.trim())?;
        let value = parse_const_value(rhs.trim(), &values)?;
        values.insert(name, value);
    }
    Some(values)
}

fn const_values_before_span_on_line(
    line: &str,
    span_start: usize,
) -> Option<HashMap<String, BoundExpr>> {
    let lower = line.to_ascii_lowercase();
    let const_pos = lower.find("const")?;
    let name_offset = span_start.checked_sub(const_pos + "const".len())?;
    let before_name =
        line[const_pos + "const".len()..const_pos + "const".len() + name_offset].trim();
    if before_name.is_empty() {
        return Some(HashMap::new());
    }
    parse_const_statement_values(before_name.trim_end_matches(','))
}

fn parse_const_value(text: &str, named_values: &HashMap<String, BoundExpr>) -> Option<BoundExpr> {
    let text = strip_balanced_outer_parens(text.trim());
    if let Some(value) = parse_const_literal(text) {
        return Some(value);
    }
    if let Some(name) = normalize_hir_ident(text)
        && let Some(value) = named_values.get(&name)
    {
        return Some(value.clone());
    }
    if let Some((lhs, op, rhs)) = split_const_binary_expr(text, &['+', '-', '&']) {
        let op = match op {
            '+' => ArithOp::Add,
            '-' => ArithOp::Sub,
            '&' => ArithOp::Concat,
            _ => return None,
        };
        return Some(BoundExpr::BinaryOp {
            op,
            lhs: Box::new(parse_const_value(lhs.trim(), named_values)?),
            rhs: Box::new(parse_const_value(rhs.trim(), named_values)?),
        });
    }
    if let Some((lhs, op, rhs)) = split_const_binary_expr(text, &['*', '/']) {
        let op = match op {
            '*' => ArithOp::Mul,
            '/' => ArithOp::Div,
            _ => return None,
        };
        return Some(BoundExpr::BinaryOp {
            op,
            lhs: Box::new(parse_const_value(lhs.trim(), named_values)?),
            rhs: Box::new(parse_const_value(rhs.trim(), named_values)?),
        });
    }
    if let Some(rest) = text.strip_prefix('-') {
        return Some(BoundExpr::UnaryOp {
            op: ArithOp::Neg,
            operand: Box::new(parse_const_value(rest.trim(), named_values)?),
        });
    }
    None
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

fn strip_balanced_outer_parens(mut text: &str) -> &str {
    loop {
        let trimmed = text.trim();
        if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
            return trimmed;
        }
        if !outer_parens_wrap_expression(trimmed) {
            return trimmed;
        }
        text = &trimmed[1..trimmed.len() - 1];
    }
}

fn outer_parens_wrap_expression(text: &str) -> bool {
    let mut depth = 0i32;
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
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                depth -= 1;
                if depth == 0 && idx != text.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0 && !in_string
}

fn split_const_binary_expr<'a>(
    text: &'a str,
    operators: &[char],
) -> Option<(&'a str, char, &'a str)> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut split = None;
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
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            _ if !in_string && depth == 0 && operators.contains(&ch) => {
                if ch == '-' && is_unary_const_minus(text, idx) {
                    continue;
                }
                split = Some((idx, ch));
            }
            _ => {}
        }
    }
    let (idx, op) = split?;
    let lhs = text[..idx].trim();
    let rhs = text[idx + op.len_utf8()..].trim();
    (!lhs.is_empty() && !rhs.is_empty()).then_some((lhs, op, rhs))
}

fn is_unary_const_minus(text: &str, idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let prior = text[..idx].trim_end().chars().next_back();
    prior.is_none_or(|ch| matches!(ch, '(' | '+' | '-' | '*' | '/' | '&'))
}

fn declared_bound_type(typed_hir: &TypedHirModule, symbol: SymbolId) -> Option<BoundType> {
    typed_hir
        .hooks
        .declared_type(symbol)
        .map(|hook| bound_type_from_vba_type_id(hook.runtime_type))
}

fn declared_bound_type_or_default(
    source: &str,
    typed_hir: &TypedHirModule,
    symbol: SymbolId,
    default_type_table: &[BoundType; 26],
) -> Option<BoundType> {
    let span = typed_hir.module.symbols.symbol(symbol)?.provenance.span?;
    if let Some(ty) = explicit_bound_type_after_span(source, span) {
        return Some(ty);
    }
    if let Some(ty) = type_char_bound_type_at_span(source, span) {
        return Some(ty);
    }
    let name = symbol_name(typed_hir, symbol).ok()?;
    Some(default_bound_type_for_name(&name, default_type_table))
}

fn explicit_bound_type_after_span(
    source: &str,
    span: crate::frontend_symbols::FrontendSourceSpan,
) -> Option<BoundType> {
    let suffix = source.get(span.end..)?;
    let line_end = span.end + suffix.find('\n').unwrap_or(suffix.len());
    let segment = source.get(span.end..line_end)?;
    let segment = first_hir_declarator_tail(segment);
    let lower = segment.to_ascii_lowercase();
    let as_pos = lower.find(" as ")?;
    let ty_text = segment
        .get(as_pos + " as ".len()..)?
        .trim_start()
        .split(|ch: char| ch == ',' || ch == ')' || ch.is_whitespace())
        .next()?;
    parse_hir_bound_type(ty_text)
}

fn first_hir_declarator_tail(text: &str) -> &str {
    let mut depth = 0i32;
    let mut in_string = false;
    for (idx, ch) in text.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string && depth > 0 => depth -= 1,
            ',' if !in_string && depth == 0 => return &text[..idx],
            _ => {}
        }
    }
    text
}

fn type_char_bound_type_at_span(
    source: &str,
    span: crate::frontend_symbols::FrontendSourceSpan,
) -> Option<BoundType> {
    let token = source.get(span.start..span.end)?.trim();
    token
        .chars()
        .next_back()
        .and_then(type_char_bound_type)
        .or_else(|| {
            source
                .get(span.end..)?
                .chars()
                .next()
                .and_then(type_char_bound_type)
        })
}

fn type_char_bound_type(ch: char) -> Option<BoundType> {
    match ch {
        '%' => Some(BoundType::Integer),
        '&' => Some(BoundType::Long),
        '^' => Some(BoundType::LongLong),
        '!' => Some(BoundType::Single),
        '#' => Some(BoundType::Double),
        '@' => Some(BoundType::Currency),
        '$' => Some(BoundType::String),
        _ => None,
    }
}

fn default_bound_type_for_name(name: &str, default_type_table: &[BoundType; 26]) -> BoundType {
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
        HirExprKind::New { .. } => Some(BoundType::Object),
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
    use crate::resolve::BoundCompareMode;
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
    fn hir_production_lowering_emits_multidimensional_runtime_redim_for_dynamic_array() {
        let source = "Sub Main()\nDim rows As Long\nDim cols As Long\nDim grid() As Long\nrows = 2\ncols = 3\nReDim grid(rows - 1, cols - 1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArrayResize {
                    upper_bounds,
                    lower_bounds,
                    element_type: crate::bytecode::RuntimeArrayElementType::Long,
                    ..
                } if upper_bounds.len() == 2 && lower_bounds == &vec![0, 0]
            )),
            "expected two-dimensional runtime ReDim bytecode: {:?}",
            bytecode.instructions
        );
        let proc = metadata.get("main").expect("main metadata");
        let shape = proc
            .array_shapes
            .iter()
            .find(|shape| shape.name == "grid")
            .expect("dynamic array shape");
        assert_eq!(shape.element_type, VbaTypeId::Long);
        assert_eq!(shape.rank, 2);
    }

    #[test]
    fn hir_production_lowering_emits_explicit_lower_bound_runtime_redim() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nlength = 3\nReDim buf(1 To length - 1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArrayResize {
                    lower_bounds,
                    element_type: crate::bytecode::RuntimeArrayElementType::Byte,
                    ..
                } if lower_bounds == &vec![1]
            )),
            "expected explicit lower-bound runtime ReDim bytecode: {:?}",
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
    fn hir_production_lowering_emits_dynamic_array_element_read() {
        let source =
            "Sub Main()\nDim buf() As Byte\nDim x As Long\nReDim buf(2)\nx = buf(1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArrayGet { indices, .. } if indices.len() == 1
            )),
            "expected dynamic array element read bytecode: {:?}",
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
    fn hir_production_lowering_emits_dynamic_array_element_write() {
        let source = "Sub Main()\nDim buf() As Byte\nReDim buf(2)\nbuf(1) = 7\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArraySet { indices, .. } if indices.len() == 1
            )),
            "expected dynamic array element write bytecode: {:?}",
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
    fn hir_production_lowering_emits_multidimensional_dynamic_array_element_access() {
        let source = "Sub Main()\nDim grid() As Long\nDim x As Long\nReDim grid(1, 1)\ngrid(1, 0) = 7\nx = grid(1, 0)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArraySet { indices, .. } if indices.len() == 2
            )),
            "expected multidimensional dynamic array write bytecode: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicArrayGet { indices, .. } if indices.len() == 2
            )),
            "expected multidimensional dynamic array read bytecode: {:?}",
            bytecode.instructions
        );
        let proc = metadata.get("main").expect("main metadata");
        let shape = proc
            .array_shapes
            .iter()
            .find(|shape| shape.name == "grid")
            .expect("dynamic array shape");
        assert_eq!(shape.element_type, VbaTypeId::Long);
        assert_eq!(shape.rank, 2);
    }

    #[test]
    fn hir_production_lowering_materializes_fixed_array_aliases() {
        let source =
            "Sub Main()\nDim a(1 To 2) As Integer\nDim x As Long\na(2) = 7\nx = a(2)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "expected fixed-array assignment value bytecode: {:?}",
            bytecode.instructions
        );
        let proc = metadata.get("main").expect("main metadata");
        assert!(
            proc.slots.iter().any(|slot| slot.name == "a_1"),
            "expected fixed-array alias slot: {:?}",
            proc.slots
        );
        let shape = proc
            .array_shapes
            .iter()
            .find(|shape| shape.name == "a")
            .expect("fixed array shape");
        assert_eq!(shape.element_type, VbaTypeId::Integer);
        assert_eq!(shape.rank, 1);
        assert_eq!(shape.bounds.len(), 1);
        assert_eq!(shape.bounds[0].lower_bound, 1);
        assert_eq!(shape.bounds[0].upper_bound, 2);
        assert_eq!(shape.storage, crate::emit::ArrayStorageKind::StaticFixed);
    }

    #[test]
    fn hir_production_lowering_materializes_multidimensional_fixed_array_aliases() {
        let source = "Sub Main()\nDim m(1 To 2, 1 To 2) As Integer\nDim x As Long\nm(2, 1) = 7\nx = m(2, 1)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "expected multidimensional fixed-array assignment value bytecode: {:?}",
            bytecode.instructions
        );
        let proc = metadata.get("main").expect("main metadata");
        assert!(
            proc.slots.iter().any(|slot| slot.name == "m_2"),
            "expected multidimensional fixed-array alias slot: {:?}",
            proc.slots
        );
        let shape = proc
            .array_shapes
            .iter()
            .find(|shape| shape.name == "m")
            .expect("fixed array shape");
        assert_eq!(shape.element_type, VbaTypeId::Integer);
        assert_eq!(shape.rank, 2);
        assert_eq!(shape.bounds.len(), 2);
        assert_eq!(shape.bounds[0].lower_bound, 1);
        assert_eq!(shape.bounds[0].upper_bound, 2);
        assert_eq!(shape.bounds[1].lower_bound, 1);
        assert_eq!(shape.bounds[1].upper_bound, 2);
        assert_eq!(shape.storage, crate::emit::ArrayStorageKind::StaticFixed);
    }

    #[test]
    fn hir_production_lowering_preserves_option_explicit_flag() {
        let source = "Option Explicit\nSub Main()\nDim x\nx = 1\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");

        assert!(bound.option_explicit);
    }

    #[test]
    fn hir_production_lowering_applies_def_type_to_untyped_dim() {
        let source = "DefLng A-Z\nSub Main()\nDim alpha\nalpha = 1\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        let main = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "main")
            .expect("main procedure");

        assert_eq!(
            main.declaration_types.get("alpha").copied(),
            Some(BoundType::Long)
        );
        assert_eq!(bound.default_type_table[0], BoundType::Long);
    }

    #[test]
    fn hir_production_lowering_type_char_overrides_def_type_for_dim() {
        let source = "DefObj A-Z\nSub Main()\nDim alpha%\nalpha = 1\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        let main = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "main")
            .expect("main procedure");

        assert_eq!(
            main.declaration_types.get("alpha").copied(),
            Some(BoundType::Integer)
        );
    }

    #[test]
    fn hir_production_lowering_explicit_as_variant_overrides_def_type_for_dim() {
        let source = "DefLng A-Z\nSub Main()\nDim alpha As Variant\nalpha = 1\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        let main = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "main")
            .expect("main procedure");

        assert_eq!(
            main.declaration_types.get("alpha").copied(),
            Some(BoundType::Variant)
        );
    }

    #[test]
    fn hir_production_lowering_later_declarator_as_type_does_not_bleed_to_prior_dim() {
        let source =
            "DefObj A-Z\nSub Main()\nDim alpha, beta As Long\nalpha = Nothing\nbeta = 1\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        let main = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "main")
            .expect("main procedure");

        assert_eq!(
            main.declaration_types.get("alpha").copied(),
            Some(BoundType::Object)
        );
        assert_eq!(
            main.declaration_types.get("beta").copied(),
            Some(BoundType::Long)
        );
    }

    #[test]
    fn hir_production_lowering_applies_def_type_and_type_char_precedence_for_params() {
        let source = "DefObj A-Z\nSub Use(alpha, beta%, gamma% As Long)\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Module1", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        let proc = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "use")
            .expect("use procedure");

        assert_eq!(proc.params[0].ty, BoundType::Object);
        assert_eq!(proc.params[1].ty, BoundType::Integer);
        assert_eq!(proc.params[2].ty, BoundType::Long);
    }

    #[test]
    fn hir_production_lowering_applies_def_type_and_type_char_precedence_for_function_return() {
        let typed_source = "DefObj A-Z\nFunction alpha%()\nalpha = 1\nEnd Function\n";
        let typed_hir = collect_type_hooks_from_source("Module1", typed_source)
            .expect("typed HIR should collect");
        let typed_bound = lower_typed_hir_to_bound_module(typed_source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        let typed_proc = typed_bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "alpha")
            .expect("alpha function");
        assert_eq!(typed_proc.return_type, BoundType::Integer);
        assert_eq!(
            typed_proc.declaration_types.get("alpha").copied(),
            Some(BoundType::Integer)
        );

        let default_source = "DefObj A-Z\nFunction alpha()\nalpha = Nothing\nEnd Function\n";
        let default_hir = collect_type_hooks_from_source("Module1", default_source)
            .expect("typed HIR should collect");
        let default_bound = lower_typed_hir_to_bound_module(default_source, &default_hir)
            .expect("HIR production lowering should produce bound module");
        let default_proc = default_bound
            .procedures
            .iter()
            .find(|procedure| procedure.name == "alpha")
            .expect("alpha function");
        assert_eq!(default_proc.return_type, BoundType::Object);
        assert_eq!(
            default_proc.declaration_types.get("alpha").copied(),
            Some(BoundType::Object)
        );
    }

    #[test]
    fn hir_production_lowering_preserves_option_compare_text_mode() {
        let source = "Option Compare Text\nSub Main()\nDim x\nx = \"a\" = \"A\"\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::CmpEqSlots {
                    mode: crate::bytecode::StringCompareMode::Text,
                    ..
                }
            )),
            "expected text comparison bytecode: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_preserves_option_compare_database_mode() {
        let source = "Option Compare Database\nSub Main()\nDim x\nx = \"a\" = \"A\"\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should collect");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("HIR production lowering should produce bound module");
        assert_eq!(bound.compare_mode, BoundCompareMode::Database);

        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::CmpEqSlots {
                    mode: crate::bytecode::StringCompareMode::Binary,
                    ..
                }
            )),
            "database compare currently emits binary runtime comparison: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_rematerializes_fixed_array_redim_aliases() {
        let source = "Sub Main()\nDim a(1)\nDim x As Long\na(0) = 7\nReDim Preserve a(3)\nx = a(3)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "expected fixed-array pre-Redim assignment bytecode: {:?}",
            bytecode.instructions
        );
        let proc = metadata.get("main").expect("main metadata");
        assert!(
            proc.slots.iter().any(|slot| slot.name == "a_3"),
            "expected rematerialized fixed-array alias slot: {:?}",
            proc.slots
        );
        let shape = proc
            .array_shapes
            .iter()
            .find(|shape| shape.name == "a")
            .expect("fixed array shape");
        assert_eq!(shape.rank, 1);
        assert_eq!(shape.bounds.len(), 1);
        assert_eq!(shape.bounds[0].lower_bound, 0);
        assert_eq!(shape.bounds[0].upper_bound, 3);
        assert_eq!(shape.storage, crate::emit::ArrayStorageKind::StaticFixed);
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
    fn hir_production_lowering_accepts_single_source_implements_directive() {
        let source = "Implements IFoo\nSub Main()\nEnd Sub\n";
        let (_bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

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
    fn hir_production_lowering_preserves_named_call_arguments() {
        let source = "Sub Use(ByVal target, ByVal value)\nEnd Sub\nSub Main()\nCall Use(value := 9, target := 3)\nEnd Sub\n";
        let (_bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        let main = metadata.get("main").expect("main metadata");
        let call_site = main
            .call_sites
            .iter()
            .find(|call_site| call_site.target_name.eq_ignore_ascii_case("use"))
            .unwrap_or_else(|| panic!("expected Use call-site metadata: {main:#?}"));
        assert_eq!(call_site.arguments.len(), 2);
        assert!(call_site.arguments.iter().any(|argument| {
            argument.source_index == Some(0)
                && argument.source_name.as_deref() == Some("value")
                && argument.parameter_name.as_deref() == Some("value")
                && argument.source_kind == crate::emit::ArgumentSourceKindDescriptor::Named
        }));
        assert!(call_site.arguments.iter().any(|argument| {
            argument.source_index == Some(1)
                && argument.source_name.as_deref() == Some("target")
                && argument.parameter_name.as_deref() == Some("target")
                && argument.source_kind == crate::emit::ArgumentSourceKindDescriptor::Named
        }));
    }

    #[test]
    fn hir_production_lowering_preserves_optional_parameter_defaults() {
        let source = "Sub Use(Optional ByVal n = 7)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let (_bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(use_metadata.signature.parameters.len(), 1);
        assert!(use_metadata.signature.parameters[0].optional);
        assert_eq!(use_metadata.signature.parameters[0].default_value, Some(7));
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
    fn hir_production_lowering_accepts_with_member_assignment_target() {
        let source = "Sub Main()\nDim obj\nWith obj\n.Value = 1\nEnd With\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        args,
                        call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertyLet),
                        ..
                    } if args.len() == 1
                )
            }),
            "expected With member assignment to emit property-let dispatch: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_member_assignment_targets() {
        let source =
            "Sub Main()\nDim obj\nDim other\nobj.Value = 1\nSet obj.Ref = other\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        args,
                        call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertyLet),
                        ..
                    } if args.len() == 1
                )
            }),
            "expected member Let assignment to emit property-let dispatch: {:?}",
            bytecode.instructions
        );
        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        args,
                        call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertySet),
                        ..
                    } if args.len() == 1
                )
            }),
            "expected member Set assignment to emit property-set dispatch: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_bang_member_assignment_target() {
        let source = "Sub Main()\nDim obj\nobj!Value = 1\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        args,
                        call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertyLet),
                        ..
                    } if args.len() == 1
                )
            }),
            "expected bang member assignment to emit property-let dispatch: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_bang_member_access() {
        let source = "Sub Main()\nDim obj\nDim x\nx = obj!Value\nEnd Sub\n";
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
            "expected bang member read to emit dispatch invoke: {:?}",
            bytecode.instructions
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_accepts_late_bound_default_member_call() {
        let source = "Sub Main()\nDim obj\nDim x\nx = obj(42)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        args,
                        early_bound: false,
                        ..
                    } if args.len() == 1
                )
            }),
            "expected default-member dispatch invoke: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| {
                call_site.target_kind
                    == crate::emit::CallTargetKindDescriptor::LateBoundDefaultMember
                    && call_site.default_member_policy
                        == crate::emit::DefaultMemberPolicyDescriptor::DefaultMemberFallback
                    && call_site.arguments.len() == 1
            }),
            "expected late-bound default-member call-site metadata: {main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_late_bound_default_member_let_assignment() {
        let source = "Sub Main()\nDim obj\nobj(index := 2) = 9\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        member,
                        args,
                        early_bound: false,
                        call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertyLet),
                        ..
                    } if args.len() == 2
                        && args.first().and_then(|arg| arg.name.as_deref()) == Some("index")
                        && bytecode.instructions.iter().any(|candidate| matches!(
                            candidate,
                            crate::bytecode::Instruction::LoadConstI32 {
                                slot,
                                value: 0,
                            } if slot == member
                        ))
                )
            }),
            "expected indexed default-member property-let dispatch against member id 0: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| {
                call_site.target_kind
                    == crate::emit::CallTargetKindDescriptor::LateBoundDefaultMember
                    && call_site.invocation_syntax
                        == crate::emit::CallInvocationSyntaxDescriptor::SyntheticPropertyAssignment
                    && call_site.default_member_policy
                        == crate::emit::DefaultMemberPolicyDescriptor::DefaultMemberFallback
                    && call_site.arguments.len() == 2
                    && call_site
                        .arguments
                        .first()
                        .and_then(|arg| arg.source_name.as_deref())
                        == Some("index")
                    && call_site
                        .arguments
                        .get(1)
                        .and_then(|arg| arg.parameter_name.as_deref())
                        == Some("value")
            }),
            "expected default-member assignment call-site metadata: {main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_late_bound_default_member_set_assignment() {
        let source = "Sub Main()\nDim obj\nDim other\nSet obj(2) = other\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");

        assert!(
            bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::IntrinsicDispatchInvokeHost {
                        member,
                        args,
                        early_bound: false,
                        call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertySet),
                        ..
                    } if args.len() == 2
                        && bytecode.instructions.iter().any(|candidate| matches!(
                            candidate,
                            crate::bytecode::Instruction::LoadConstI32 {
                                slot,
                                value: 0,
                            } if slot == member
                        ))
                )
            }),
            "expected indexed default-member property-set dispatch against member id 0: {:?}",
            bytecode.instructions
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| {
                call_site.target_kind
                    == crate::emit::CallTargetKindDescriptor::LateBoundDefaultMember
                    && call_site.invocation_syntax
                        == crate::emit::CallInvocationSyntaxDescriptor::SyntheticPropertyAssignment
                    && call_site.default_member_policy
                        == crate::emit::DefaultMemberPolicyDescriptor::DefaultMemberFallback
                    && call_site.arguments.len() == 2
                    && call_site
                        .arguments
                        .get(1)
                        .and_then(|arg| arg.parameter_name.as_deref())
                        == Some("value")
            }),
            "expected default-member Set assignment call-site metadata: {main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_rejects_new_expression_until_project_binding_is_available() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = New Widget\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("New expression needs project-aware construction binding");

        match err {
            HirProductionLoweringError::Unsupported(reason) => {
                assert!(
                    reason.contains(
                        "New expression `widget` requires project-aware construction binding"
                    ),
                    "unexpected New residual reason: {reason}"
                );
            }
            other => panic!("New expression must remain fallback-eligible, got {other:?}"),
        }
    }

    #[test]
    fn hir_lowering_binds_new_expression_to_project_instance_intrinsic() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = New Widget\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should build");
        let bound = lower_typed_hir_to_bound_module_with_new_bindings(
            source,
            &typed_hir,
            &[HirNewExpressionBinding {
                type_name: "Widget".to_string(),
                object_handle: 7,
            }],
        )
        .expect("bound New expression should lower");
        let main = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name.eq_ignore_ascii_case("main"))
            .expect("main procedure");

        assert!(main.body.iter().any(|stmt| {
            matches!(
                stmt,
                BoundStmt::Assign {
                    target,
                    expr:
                        BoundExpr::StructuralIntrinsicCall {
                            intrinsic: StructuralIntrinsic::ProjectInstance,
                            args,
                        },
                    intent: AssignmentIntent::Set,
                } if target == "obj" && matches!(args.as_slice(), [BoundExpr::IntConst(7)])
            )
        }));
    }

    #[test]
    fn hir_compile_binds_new_expression_to_project_instance_bytecode() {
        let source = "Sub Main()\nDim obj As Object\nDim same As Boolean\nSet obj = New Widget\nsame = obj Is Nothing\nEnd Sub\n";
        let (bytecode, metadata) = compile_source_with_runtime_metadata_via_hir_with_new_bindings(
            source,
            &[HirNewExpressionBinding {
                type_name: "Widget".to_string(),
                object_handle: 7,
            }],
        )
        .expect("bound New expression should compile");

        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "{bytecode:#?}"
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LoadProjectObjectRef { .. })),
            "{bytecode:#?}"
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_lowering_lowers_structural_intrinsic_call_targets() {
        let source = "Sub Main()\nDim discard\nDim instance\ninstance = 1\ndiscard = __oxvba_withevents_set(instance, 313695537, 1)\nEnd Sub\n";
        let typed_hir =
            collect_type_hooks_from_source("Main", source).expect("typed HIR should build");
        let bound = lower_typed_hir_to_bound_module(source, &typed_hir)
            .expect("structural intrinsic call should lower");
        let main = bound
            .procedures
            .iter()
            .find(|procedure| procedure.name.eq_ignore_ascii_case("main"))
            .expect("main procedure");

        assert!(main.body.iter().any(|stmt| {
            matches!(
                stmt,
                BoundStmt::Assign {
                    target,
                    expr:
                        BoundExpr::StructuralIntrinsicCall {
                            intrinsic: StructuralIntrinsic::WithEventsSet,
                            args,
                        },
                    ..
                } if target == "discard" && args.len() == 3
            )
        }));
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
    fn hir_production_lowering_accepts_enum_member_constants() {
        let source = "Public Enum Mode\n' ignored enum comment\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 4, .. }
            )),
            "{bytecode:#?}"
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            !main
                .slots
                .iter()
                .any(|slot| slot.name.eq_ignore_ascii_case("safe")),
            "{main:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_declared_external_call() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert_eq!(bytecode.external_call_descriptors.len(), 1);
        let descriptor = &bytecode.external_call_descriptors[0];
        assert_eq!(descriptor.declared_name, "hostping");
        assert_eq!(descriptor.library, "host");
        assert_eq!(descriptor.alias, "ping");
        assert_eq!(descriptor.param_count, 1);
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicInvokeSymbolHost { args, .. } if args.len() == 1
            )),
            "{bytecode:#?}"
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
    }

    #[test]
    fn hir_production_lowering_rejects_declare_without_ptrsafe_for_fallback() {
        let source = "Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub\n";
        let err = compile_source_with_runtime_metadata_via_hir(source)
            .expect_err("unsupported Declare shapes remain fallback-eligible");

        assert!(
            matches!(err, HirProductionLoweringError::Unsupported(_)),
            "Declare diagnostics must remain fallback-eligible, got {err:?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_udt_layout_descriptors() {
        let source =
            "Type Point\nX As Long\nY As String\nEnd Type\nSub Main()\nDim p As Point\nEnd Sub\n";
        let (_bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        let main = metadata.get("main").expect("main metadata");
        let point = main
            .udt_types
            .iter()
            .find(|descriptor| descriptor.type_name.eq_ignore_ascii_case("point"))
            .expect("point UDT descriptor");
        assert!(point.instances.iter().any(|instance| instance.name == "p"));
        assert!(point.fields.iter().any(|field| field.name == "x"));
        assert!(point.fields.iter().any(|field| field.name == "y"));
        assert!(main.slots.iter().any(|slot| slot.name == "p_x"));
        assert!(main.slots.iter().any(|slot| slot.name == "p_y"));
    }

    #[test]
    fn hir_production_lowering_accepts_udt_field_read_write_aliases() {
        let source = "Type Point\nX As Long\nEnd Type\nSub Main()\nDim p As Point\nDim y As Long\np.X = 1\ny = p.X + 2\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        let main = metadata.get("main").expect("main metadata");
        assert!(main.slots.iter().any(|slot| slot.name == "p_x"));
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 1, .. }
            )),
            "{bytecode:#?}"
        );
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::AddConstI32 { value: 2, .. } | Instruction::AddSlots { .. }
            )),
            "{bytecode:#?}"
        );
    }

    #[test]
    fn hir_production_lowering_accepts_expression_const_statement() {
        let source = "Const CBase = 1 + 2, COffset = -1 + 2, CTotal = CBase + COffset\nSub Main()\nDim x\nDim y\nDim z\nx = CBase\ny = COffset\nz = CTotal\nEnd Sub\n";
        let (bytecode, metadata) =
            compile_source_with_runtime_metadata_via_hir(source).expect("HIR production lowering");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::AddSlots { .. })),
            "{bytecode:#?}"
        );
        let main = metadata.get("main").expect("main metadata");
        assert!(
            !main
                .slots
                .iter()
                .any(|slot| slot.name.eq_ignore_ascii_case("cbase")
                    || slot.name.eq_ignore_ascii_case("coffset")
                    || slot.name.eq_ignore_ascii_case("ctotal")),
            "{main:#?}"
        );
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
