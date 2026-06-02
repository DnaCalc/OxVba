//! oxvba-compiler: semantic analysis and bytecode emission scaffolding.

pub mod bundle;
pub mod bytecode;
pub mod descriptor_identity;
pub mod emit;
pub mod frontend_assignment_semantics;
pub mod frontend_class_semantics;
pub mod frontend_diagnostics;
pub mod frontend_diff;
pub mod frontend_event_semantics;
pub mod frontend_external_references;
pub mod frontend_grammar_matrix_route_audit;
pub mod frontend_hir;
pub mod frontend_hir_lowering;
pub mod frontend_language_service;
pub mod frontend_legacy_route_audit;
pub mod frontend_lowering_contract;
pub mod frontend_member_dispatch;
pub mod frontend_operator_normalization;
pub mod frontend_project_symbols;
pub mod frontend_query;
pub mod frontend_retirement_inventory;
pub mod frontend_route_policy;
pub mod frontend_semantic_model;
pub mod frontend_structural_intrinsics;
pub mod frontend_symbols;
pub mod frontend_type_hooks;
pub mod lsp_support;
pub mod optimize;
pub mod project;
pub mod resolve;
pub(crate) mod syntax_bridge;
pub mod typecheck;

use thiserror::Error;

pub use bundle::{
    BundleCallableDescriptor, BundleComClassDescriptor, BundleComEventDescriptor,
    BundleComInterfaceDescriptor, BundleComMemberDescriptor, BundleComParamDescriptor,
    BundleCompileContext, BundleConditionalConstantFact, BundleDefaultTypeFamilyFact,
    BundleDescriptorInventoryError, BundleHostCapabilityRequirement, BundleModuleSourceMap,
    BundleNativeLibraryFact, BundlePackageDiagnostic, BundlePackageGapClassification,
    BundleProcedureAnnotation, BundleProcedureParameterDescriptor, BundleProcedureSignature,
    BundleProjectContext, BundleProjectModuleFact, BundleProjectReferenceFact,
    BundleReferencedProjectFact, BundleSourceLineMapping, BundleVbaTypeDescriptor,
    ComClassExportEntry, DescriptorInventory, OxBundle,
};
pub use bytecode::{Bytecode, DeclareParamType, Instruction};
pub use descriptor_identity::{
    DescriptorFamily, DescriptorIdentity, VbaTypeRegistryEntry, canonical_descriptor_id,
    descriptor_digest_debug, descriptor_digest_from_fields, descriptor_identity_debug,
    vba_type_registry,
};
pub use emit::{
    ArgumentBindingDescriptor, ArgumentBindingKindDescriptor, ArgumentExpressionKindDescriptor,
    ArgumentSourceKindDescriptor, ArgumentWritebackDescriptor, ArrayBoundDescriptor,
    ArrayShapeDescriptor, ArrayStorageKind, CallDiagnosticKindDescriptor,
    CallDiagnosticOwnerDescriptor, CallDiagnosticPolicyDescriptor, CallInvocationSyntaxDescriptor,
    CallReturnDescriptor, CallSiteDescriptor, CallTargetKindDescriptor, CarrierLayoutDescriptor,
    CarrierLayoutKind, CoercionDescriptor, CoercionKindDescriptor, CoercionStaticStatusDescriptor,
    DefaultMemberPolicyDescriptor, EvaluationOrderDescriptor, ExpressionClassificationDescriptor,
    ExpressionSemanticsDescriptor, ExpressionSourceContextDescriptor,
    ImplicitCurrentObjectDescriptor, MemberDispatchKindDescriptor, NameBindingDescriptor,
    NameBindingKindDescriptor, NameBindingPrecedenceDescriptor, ObjectActivationDescriptor,
    ObjectDefaultMemberDescriptor, ObjectDescriptorSupport, ObjectEventBindingDescriptor,
    ObjectInstanceDescriptor, ObjectMemberBindingDescriptor, ObjectMemberKindDescriptor,
    ObjectTypeDescriptor, ObjectTypeDescriptorKind, OperatorCompareModeDescriptor,
    OperatorFamilyDescriptor, OperatorSemanticsDescriptor, OptionalDefaultValue,
    OptionalMissingStatePolicy, OptionalParameterDescriptor, ParamArrayBindingDescriptor,
    ParamArrayDescriptor, ParameterDescriptor, ParameterPassingMode, ParameterRole,
    ProcedureKindDescriptor, ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind,
    ProcedureRuntimeSlotMetadata, ProcedureSignatureDescriptor, ResolvedParameterMechanism,
    RuntimeCarrierKind, RuntimeFailurePolicyDescriptor, SlotInitialState, SlotRole,
    SlotTypeDescriptor, SourceParameterMechanism, UdtCleanupDescriptor, UdtCopySemanticsDescriptor,
    UdtFieldAliasDescriptor, UdtFieldDescriptor, UdtInstanceDescriptor, UdtStorageKind,
    UdtTypeDescriptor, ValueStateDescriptor, ValueStateKind, ValueStateSource,
    VbaOperatorDescriptor, VbaTypeId, bound_type_to_declare_param_type,
};
pub use project::{
    CallableCapability, CallingShape, CompiledProject, CompilerLineMapping,
    CompilerModuleSourceMap, CompilerSourceLineKind, CompilerSourceMap, ExportKind,
    HostProcedureExport, InvocationLane, ModuleAttributes, ModuleDescriptor, ModuleKind,
    ModuleUnit, ModuleVisibility, PassingMode, ProcedureAnnotation, ProcedureDescriptor,
    ProcedureKind, ProcedureParameterDescriptor, ProcedureSignature, ProcedureVisibility,
    ProjectComWithEventsRoute, ProjectCompileError, ProjectCompileRoute, ProjectDynamicMemberKind,
    ProjectDynamicMemberRoute, ProjectDynamicObjectRoute, ProjectDynamicParamRoute,
    ProjectEventDispatchBinding, ProjectIdentity, ProjectKind, ProjectManifest, ProjectReference,
    ProjectReflection, ReferenceKind, ReferencedProjectManifest, RuntimeProcedureRoute, SourceSpan,
    UnsupportedReason, VbaType, VbaTypeDescriptor, compile_project, module_unit_from_source,
    reflect_project,
};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("empty source")]
    EmptySource,
    #[error("resolve error: {0}")]
    ResolveError(String),
    #[error("type error: {0}")]
    TypeError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompileOptions {
    pub frontend_v2: bool,
}

#[derive(Debug, Clone)]
pub struct FrontendV2Analysis {
    pub semantic_model: frontend_semantic_model::SemanticModel,
    pub typed_hir: frontend_type_hooks::TypedHirModule,
    pub diagnostics: Vec<frontend_diagnostics::FrontendDiagnostic>,
}

pub fn analyze_frontend_v2_source(source: &str) -> Result<FrontendV2Analysis, CompileError> {
    let diagnostics = frontend_diagnostics::FrontendDiagnosticMapper::from_source_parse(source);
    if !diagnostics.diagnostics().is_empty() {
        let messages = diagnostics
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                format!(
                    "{} at {}..{}: {}",
                    diagnostic.code, diagnostic.span.start, diagnostic.span.end, diagnostic.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(CompileError::ResolveError(format!(
            "frontend_v2 diagnostics: {messages}"
        )));
    }

    let typed_hir =
        frontend_type_hooks::collect_type_hooks_from_source("Main", source).map_err(|err| {
            CompileError::ResolveError(format!("frontend_v2 binder/HIR error: {err}"))
        })?;
    let mut semantic_model =
        frontend_semantic_model::SemanticModel::from_bound_hir_module(typed_hir.module.clone());
    for diagnostic in diagnostics.semantic_diagnostics() {
        semantic_model.push_diagnostic(diagnostic);
    }
    Ok(FrontendV2Analysis {
        semantic_model,
        typed_hir,
        diagnostics: diagnostics.diagnostics().to_vec(),
    })
}

pub fn compile(source: &str) -> Result<Bytecode, CompileError> {
    compile_with_runtime_metadata(source).map(|(bytecode, _)| bytecode)
}

pub fn compile_with_options(
    source: &str,
    options: CompileOptions,
) -> Result<Bytecode, CompileError> {
    if options.frontend_v2 {
        let _analysis = analyze_frontend_v2_source(source)?;
        return match frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source) {
            Ok((bytecode, _)) => Ok(bytecode),
            Err(frontend_hir_lowering::HirProductionLoweringError::Unsupported(reason)) => Err(
                CompileError::ResolveError(format!("frontend_v2 HIR unsupported: {reason}")),
            ),
            Err(frontend_hir_lowering::HirProductionLoweringError::Compile(err)) => Err(err),
        };
    }
    match frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source) {
        Ok((bytecode, _)) => Ok(bytecode),
        Err(frontend_hir_lowering::HirProductionLoweringError::Unsupported(_)) => {
            compile_with_runtime_metadata_legacy(source).map(|(bytecode, _)| bytecode)
        }
        Err(frontend_hir_lowering::HirProductionLoweringError::Compile(err)) => Err(err),
    }
}

pub fn compile_with_runtime_metadata(
    source: &str,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    compile_with_runtime_metadata_object_locals(source, &std::collections::BTreeMap::new())
}

pub(crate) fn compile_with_runtime_metadata_legacy(
    source: &str,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    compile_with_runtime_metadata_legacy_object_locals_class(
        source,
        &std::collections::BTreeMap::new(),
        false,
    )
}

/// Compile a single source snippet into a complete, strict executable semantic
/// package held in memory.
///
/// Wraps the snippet in a one-module project and runs the full project compile,
/// so the resulting `OxBundle` carries the manifest, project context, descriptor
/// inventory, and export inventory required by the strict package support gate —
/// unlike the lightweight `compile_with_runtime_metadata` + `OxBundle::new` path,
/// which produces an incomplete (non-strict) package. This is the in-memory
/// counterpart of a serialized bundle: same strict completeness, no serialization.
pub fn compile_source_to_bundle(source: &str) -> Result<OxBundle, ProjectCompileError> {
    let manifest = ProjectManifest {
        project_name: "InMemory".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module_unit_from_source(
            "Main",
            ModuleKind::Procedural,
            source,
        )?],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };
    let compiled = compile_project(&manifest)?;
    Ok(OxBundle::from_compiled_project(
        &compiled,
        &manifest.project_name,
    ))
}

pub(crate) fn compile_with_runtime_metadata_object_locals(
    source: &str,
    forced_object_locals_by_proc: &std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    compile_with_runtime_metadata_object_locals_class(source, forced_object_locals_by_proc, false)
}

pub(crate) fn compile_with_runtime_metadata_object_locals_class(
    source: &str,
    forced_object_locals_by_proc: &std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
    has_class_modules: bool,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    if source.trim().is_empty() {
        return Err(CompileError::EmptySource);
    }

    if forced_object_locals_by_proc.is_empty() && !has_class_modules {
        let hir_source = resolve::apply_conditional_compilation_to_source(source);
        if !source_is_eligible_for_lightweight_hir_default(&hir_source) {
            return compile_with_runtime_metadata_legacy_object_locals_class(
                source,
                forced_object_locals_by_proc,
                has_class_modules,
            );
        }
        match frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(&hir_source) {
            Ok(compiled) => return Ok(compiled),
            Err(frontend_hir_lowering::HirProductionLoweringError::Unsupported(_)) => {}
            Err(frontend_hir_lowering::HirProductionLoweringError::Compile(err)) => {
                return Err(err);
            }
        }
    }

    compile_with_runtime_metadata_legacy_object_locals_class(
        source,
        forced_object_locals_by_proc,
        has_class_modules,
    )
}

pub(crate) fn compile_with_runtime_metadata_legacy_object_locals_class(
    source: &str,
    forced_object_locals_by_proc: &std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
    has_class_modules: bool,
) -> Result<
    (
        Bytecode,
        std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
    ),
    CompileError,
> {
    validate_frontend_assignment_diagnostics(source)?;
    let mut bound = resolve::resolve_symbols(source);
    bound.is_class_module = has_class_modules;
    if !bound.resolution_diagnostics.is_empty() {
        return Err(CompileError::ResolveError(
            bound.resolution_diagnostics.join("; "),
        ));
    }
    let checked = typecheck::check_types(bound).map_err(CompileError::TypeError)?;
    let optimized = if std::env::var("OXVBA_DISABLE_OPT").ok().as_deref() == Some("1") {
        checked
    } else {
        optimize::optimize_module(checked)
    };
    let mut optimized = optimized;
    for proc in &mut optimized.procedures {
        let Some(vars) = forced_object_locals_by_proc.get(&proc.name) else {
            continue;
        };
        for var in vars {
            proc.declaration_types
                .insert(var.clone(), resolve::BoundType::Object);
        }
    }
    let (bytecode, metadata) = emit::emit_bytecode_with_runtime_metadata(&optimized);
    validate_frontend_property_accessor_metadata(source, &metadata)?;
    validate_frontend_assignment_coercion_metadata(source, &metadata)?;
    validate_frontend_lowering_contract_metadata(source, &metadata)?;
    Ok((bytecode, metadata))
}

fn source_is_eligible_for_lightweight_hir_default(source: &str) -> bool {
    if source.lines().map(str::trim_start).any(|line| {
        line.to_ascii_lowercase().starts_with("def") && !is_supported_def_type_line(line)
    }) {
        return false;
    }

    let parsed = oxvba_syntax::parse(source);
    if !parsed.errors().is_empty() {
        return false;
    }
    if source_has_unsupported_option_stmt(parsed.syntax()) {
        return false;
    }
    if source_has_hir_parameter_signature_mismatch(source) {
        return false;
    }
    if source_has_unsupported_property_declaration(source) {
        return false;
    }
    true
}

fn source_has_unsupported_property_declaration(source: &str) -> bool {
    if !source
        .lines()
        .any(|line| contains_ascii_word(line, "property"))
    {
        return false;
    }

    let lines = source.lines().map(str::to_string).collect::<Vec<_>>();
    let default_type_table = resolve::collect_default_type_table(&lines);
    let module_constants = resolve::collect_module_constants(&lines);
    for line in source.lines() {
        if !contains_ascii_word(line, "property")
            || line.trim().eq_ignore_ascii_case("End Property")
        {
            continue;
        }
        let Some(kind) = proc_kind_for_signature_line(line) else {
            return true;
        };
        if !matches!(
            kind,
            resolve::ProcKind::PropertyGet
                | resolve::ProcKind::PropertyLet
                | resolve::ProcKind::PropertySet
        ) {
            continue;
        }
        let Some((name, params, _)) = resolve::parse_proc_signature_with_module_constants(
            line,
            kind,
            &default_type_table,
            &module_constants,
        ) else {
            return true;
        };
        let supported = match kind {
            resolve::ProcKind::PropertyGet => {
                params.is_empty()
                    || source_uses_indexed_property_get_with_arguments(
                        source,
                        source_property_name(&name),
                    )
            }
            resolve::ProcKind::PropertyLet => {
                params.len() == 1
                    || (params.len() > 1
                        && source_uses_indexed_property_write_with_arguments(
                            source,
                            source_property_name(&name),
                        ))
            }
            resolve::ProcKind::PropertySet => {
                params.len() == 1
                    || (params.len() > 1
                        && source_uses_indexed_property_write_with_arguments(
                            source,
                            source_property_name(&name),
                        ))
            }
            _ => true,
        };
        if !supported {
            return true;
        }
    }
    false
}

fn source_property_name(name: &str) -> &str {
    name.strip_prefix("property_get_")
        .or_else(|| name.strip_prefix("property_let_"))
        .or_else(|| name.strip_prefix("property_set_"))
        .unwrap_or(name)
}

fn source_uses_indexed_property_get_with_arguments(source: &str, property_name: &str) -> bool {
    let mut saw_indexed_use = false;
    let mut in_matching_get_body = false;
    let property_name = property_name.to_ascii_lowercase();

    for line in source.lines() {
        if line.trim().eq_ignore_ascii_case("End Property") {
            in_matching_get_body = false;
            continue;
        }
        if let Some(kind) = proc_kind_for_signature_line(line)
            && matches!(kind, resolve::ProcKind::PropertyGet)
        {
            let lower = line.to_ascii_lowercase();
            if lower
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .any(|word| word.eq_ignore_ascii_case(&property_name))
            {
                in_matching_get_body = true;
            }
            continue;
        }
        if in_matching_get_body {
            continue;
        }
        if contains_ascii_word(line, "property") {
            continue;
        }
        match property_get_line_use_state(line, &property_name) {
            PropertyGetUseState::NoUse => {}
            PropertyGetUseState::Indexed => saw_indexed_use = true,
            PropertyGetUseState::Bare => return false,
        }
    }

    saw_indexed_use
}

fn source_uses_indexed_property_write_with_arguments(source: &str, property_name: &str) -> bool {
    let mut in_property_body = false;
    let property_name = property_name.to_ascii_lowercase();

    for line in source.lines() {
        if line.trim().eq_ignore_ascii_case("End Property") {
            in_property_body = false;
            continue;
        }
        if proc_kind_for_signature_line(line).is_some_and(|kind| {
            matches!(
                kind,
                resolve::ProcKind::PropertyGet
                    | resolve::ProcKind::PropertyLet
                    | resolve::ProcKind::PropertySet
            )
        }) {
            in_property_body = true;
            continue;
        }
        if in_property_body {
            continue;
        }
        if line_has_indexed_property_write(line, &property_name) {
            return true;
        }
    }

    false
}

fn line_has_indexed_property_write(line: &str, property_name: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let mut start = 0usize;
    while let Some(relative) = lower[start..].find(property_name) {
        let idx = start + relative;
        let end = idx + property_name.len();
        let before_is_ident = lower[..idx]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let after_is_ident = lower[end..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if !before_is_ident && !after_is_ident {
            let after_name = lower[end..].trim_start();
            if let Some(rest) = after_name.strip_prefix('(')
                && let Some(close) = rest.find(')')
                && rest[close + 1..].trim_start().starts_with('=')
            {
                return true;
            }
        }
        start = end;
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyGetUseState {
    NoUse,
    Indexed,
    Bare,
}

fn property_get_line_use_state(line: &str, property_name: &str) -> PropertyGetUseState {
    let lower = line.to_ascii_lowercase();
    let mut start = 0usize;
    let mut saw_indexed = false;
    while let Some(relative) = lower[start..].find(property_name) {
        let idx = start + relative;
        let end = idx + property_name.len();
        let before_is_ident = lower[..idx]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let after_is_ident = lower[end..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if !before_is_ident && !after_is_ident {
            let after = lower[end..].trim_start();
            if after.starts_with('(') {
                saw_indexed = true;
            } else {
                return PropertyGetUseState::Bare;
            }
        }
        start = end;
    }
    if saw_indexed {
        PropertyGetUseState::Indexed
    } else {
        PropertyGetUseState::NoUse
    }
}

fn source_has_hir_parameter_signature_mismatch(source: &str) -> bool {
    let Ok(typed_hir) = frontend_type_hooks::collect_type_hooks_from_source("Main", source) else {
        return true;
    };
    let lines = source.lines().map(str::to_string).collect::<Vec<_>>();
    let default_type_table = resolve::collect_default_type_table(&lines);
    let module_constants = resolve::collect_module_constants(&lines);
    for decl_id in &typed_hir.module.declarations {
        let Some(decl) = typed_hir.module.arenas.decl(*decl_id) else {
            continue;
        };
        let frontend_hir::HirDeclKind::Procedure { params, .. } = &decl.kind else {
            continue;
        };
        let signature_line = source_line_for_span(source, decl.cst.span.start);
        let Some(kind) = proc_kind_for_signature_line(signature_line) else {
            return true;
        };
        let Some((_, parsed_params, _)) = resolve::parse_proc_signature_with_module_constants(
            signature_line,
            kind,
            &default_type_table,
            &module_constants,
        ) else {
            return true;
        };
        if parsed_params.len() != params.len() {
            return true;
        }
        for (param, parsed_param) in params.iter().zip(parsed_params.iter()) {
            let Some(symbol) = typed_hir.module.symbols.symbol(*param) else {
                return true;
            };
            let Some(name) = typed_hir.module.symbols.name(symbol.name) else {
                return true;
            };
            if !name.folded.eq_ignore_ascii_case(&parsed_param.name) {
                return true;
            }
        }
    }
    false
}

fn source_line_for_span(source: &str, start: usize) -> &str {
    let prefix_start = source[..start]
        .rfind(['\n', '\r'])
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let suffix = &source[start..];
    let suffix_end = suffix.find(['\n', '\r']).unwrap_or(suffix.len());
    &source[prefix_start..start + suffix_end]
}

fn proc_kind_for_signature_line(line: &str) -> Option<resolve::ProcKind> {
    if contains_ascii_word(line, "sub") {
        Some(resolve::ProcKind::Sub)
    } else if contains_ascii_word(line, "function") {
        Some(resolve::ProcKind::Function)
    } else if contains_ascii_word(line, "property") && contains_ascii_word(line, "get") {
        Some(resolve::ProcKind::PropertyGet)
    } else if contains_ascii_word(line, "property") && contains_ascii_word(line, "let") {
        Some(resolve::ProcKind::PropertyLet)
    } else if contains_ascii_word(line, "property") && contains_ascii_word(line, "set") {
        Some(resolve::ProcKind::PropertySet)
    } else {
        None
    }
}

fn contains_ascii_word(text: &str, needle: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| word.eq_ignore_ascii_case(needle))
}

fn is_supported_def_type_line(line: &str) -> bool {
    let normalized = line.to_ascii_lowercase();
    let parts: Vec<_> = normalized.split_whitespace().collect();
    if parts.len() < 2 {
        return false;
    }
    matches!(
        parts.as_slice(),
        [
            "defbool"
                | "defbyte"
                | "defint"
                | "deflng"
                | "deflnglng"
                | "deflngptr"
                | "defsng"
                | "defdbl"
                | "defdec"
                | "defcur"
                | "defdate"
                | "defstr"
                | "defobj"
                | "defvar",
            ..
        ]
    )
}

fn source_has_unsupported_option_stmt(node: oxvba_syntax::SyntaxNode<'_>) -> bool {
    if node.kind() == oxvba_syntax::SyntaxKind::OptionStmt {
        let normalized = node.text().to_ascii_lowercase();
        let parts: Vec<_> = normalized.split_whitespace().collect();
        return !matches!(
            parts.as_slice(),
            ["option", "base", "0" | "1"]
                | ["option", "compare", "binary" | "text" | "database"]
                | ["option", "explicit"]
                | ["option", "private", "module"]
        );
    }
    node.child_nodes()
        .into_iter()
        .any(source_has_unsupported_option_stmt)
}

fn validate_frontend_lowering_contract_metadata(
    source: &str,
    metadata: &std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Result<(), CompileError> {
    let Ok(typed_hir) = frontend_type_hooks::collect_type_hooks_from_source("Main", source) else {
        return Ok(());
    };
    let contracts =
        frontend_lowering_contract::collect_lowering_contracts_from_typed_hir(&typed_hir);
    for contract in contracts {
        if contract.uses_legacy_intrinsic_names() {
            return Err(CompileError::ResolveError(
                "frontend_v2 lowering contract used legacy intrinsic names".to_string(),
            ));
        }
        if contract.assumes_flat_slots() {
            return Err(CompileError::ResolveError(
                "frontend_v2 lowering contract assumed flat slots".to_string(),
            ));
        }
        let Some(decl) = typed_hir.module.arenas.decl(contract.entry_decl) else {
            continue;
        };
        let Some(proc_name) =
            frontend_lowering_contract::symbol_folded_name(&typed_hir, decl.symbol)
        else {
            continue;
        };
        let Some(procedure) = metadata.get(proc_name) else {
            return Err(CompileError::ResolveError(format!(
                "frontend_v2 lowering metadata missing procedure {proc_name}"
            )));
        };
        for slot in &contract.frame_overlay.locals {
            let frontend_lowering_contract::HirFrameSlotSource::Symbol(symbol) = slot.source else {
                continue;
            };
            let Some(name) = frontend_lowering_contract::symbol_folded_name(&typed_hir, symbol)
            else {
                continue;
            };
            if !procedure.slots.iter().any(|runtime_slot| {
                runtime_slot.name.eq_ignore_ascii_case(name)
                    && matches!(
                        runtime_slot.kind,
                        ProcedureRuntimeSlotKind::Local
                            | ProcedureRuntimeSlotKind::Parameter
                            | ProcedureRuntimeSlotKind::ReturnValue
                    )
            }) {
                return Err(CompileError::ResolveError(format!(
                    "frontend_v2 lowering metadata missing frame slot {proc_name}.{name}"
                )));
            }
        }
        if !contract.returns.is_empty() && procedure.return_slot.is_none() {
            return Err(CompileError::ResolveError(format!(
                "frontend_v2 lowering metadata missing return slot for {proc_name}"
            )));
        }
        for temp in &contract.frame_overlay.temporaries {
            let frontend_lowering_contract::HirFrameSlotSource::Coercion(coercion) = &temp.source
            else {
                continue;
            };
            if !procedure.coercions.iter().any(|metadata_coercion| {
                metadata_coercion.kind == coercion.kind
                    && metadata_coercion.source_declared_type == coercion.source_type
                    && metadata_coercion.target_declared_type == coercion.target_type
            }) {
                return Err(CompileError::ResolveError(format!(
                    "frontend_v2 lowering metadata missing coercion overlay for {proc_name}"
                )));
            }
        }
    }
    Ok(())
}

fn validate_frontend_property_accessor_metadata(
    source: &str,
    metadata: &std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Result<(), CompileError> {
    let Ok(typed_hir) = frontend_type_hooks::collect_type_hooks_from_source("Main", source) else {
        return Ok(());
    };
    for accessor in
        frontend_assignment_semantics::collect_property_accessors_from_typed_hir(&typed_hir)
    {
        let Some(symbol) = typed_hir.module.symbols.symbol(accessor.property) else {
            continue;
        };
        let Some(symbol_name) = typed_hir.module.symbols.name(symbol.name) else {
            continue;
        };
        let key = symbol_name.folded.as_str();
        let Some(procedure) = metadata.get(key) else {
            return Err(CompileError::ResolveError(format!(
                "frontend_v2 property metadata missing for {key}"
            )));
        };
        let signature = procedure.procedure_signature_descriptor();
        let expected_kind = match accessor.kind {
            frontend_hir::HirPropertyKind::Get => ProcedureKindDescriptor::PropertyGet,
            frontend_hir::HirPropertyKind::Let => ProcedureKindDescriptor::PropertyLet,
            frontend_hir::HirPropertyKind::Set => ProcedureKindDescriptor::PropertySet,
        };
        if signature.kind != expected_kind {
            return Err(CompileError::ResolveError(format!(
                "frontend_v2 property metadata kind mismatch for {key}: expected {expected_kind:?}, got {:?}",
                signature.kind
            )));
        }
        let expected_group = key
            .strip_prefix("property_get_")
            .or_else(|| key.strip_prefix("property_let_"))
            .or_else(|| key.strip_prefix("property_set_"));
        if signature.property_group.as_deref() != expected_group {
            return Err(CompileError::ResolveError(format!(
                "frontend_v2 property metadata group mismatch for {key}: expected {expected_group:?}, got {:?}",
                signature.property_group
            )));
        }
    }
    Ok(())
}

fn validate_frontend_assignment_coercion_metadata(
    source: &str,
    metadata: &std::collections::BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Result<(), CompileError> {
    let Ok(typed_hir) = frontend_type_hooks::collect_type_hooks_from_source("Main", source) else {
        return Ok(());
    };
    let semantics =
        frontend_assignment_semantics::collect_assignment_semantics_from_typed_hir(&typed_hir);
    for semantic in semantics {
        if semantic.value_type == semantic.target_type {
            continue;
        }
        if !metadata.values().any(|procedure| {
            procedure.coercions.iter().any(|coercion| {
                coercion.kind == semantic.coercion
                    && coercion.source_declared_type == semantic.value_type
                    && coercion.target_declared_type == semantic.target_type
            })
        }) {
            return Err(CompileError::ResolveError(format!(
                "frontend_v2 assignment coercion metadata missing for {:?} {:?}->{:?}",
                semantic.coercion, semantic.value_type, semantic.target_type
            )));
        }
    }
    Ok(())
}

fn validate_frontend_assignment_diagnostics(source: &str) -> Result<(), CompileError> {
    let Ok(typed_hir) = frontend_type_hooks::collect_type_hooks_from_source("Main", source) else {
        return Ok(());
    };
    let semantics =
        frontend_assignment_semantics::collect_assignment_semantics_from_typed_hir(&typed_hir);
    for semantic in semantics {
        let Some(diagnostic) = semantic.diagnostic.as_ref() else {
            continue;
        };
        let target_name = frontend_assignment_target_name(&typed_hir, semantic.target)
            .unwrap_or_else(|| "target".to_string());
        let message = match diagnostic.code.as_str() {
            "BIND-E-SET-REQUIRES-OBJECT" => format!(
                "type mismatch in assignment: Set requires Object or Variant target, got {} variable {}",
                frontend_assignment_bound_type_name(semantic.target_type),
                target_name
            ),
            "BIND-E-SET-REQUIRES-OBJECT-VALUE" => {
                format!(
                    "type mismatch in assignment: Set requires object value for variable {target_name}"
                )
            }
            "BIND-E-LET-OBJECT-TARGET" => {
                if !source_has_explicit_let_assignment(source, &target_name) {
                    continue;
                }
                format!(
                    "type mismatch in assignment: Let cannot assign to Object variable {target_name}"
                )
            }
            _ => diagnostic.message.clone(),
        };
        return Err(CompileError::TypeError(message));
    }
    Ok(())
}

fn source_has_explicit_let_assignment(source: &str, target_name: &str) -> bool {
    let target = target_name.trim().to_ascii_lowercase();
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .get(..4)
            .filter(|prefix| prefix.eq_ignore_ascii_case("let "))
            .map(|_| trimmed[4..].trim_start())
        else {
            return false;
        };
        let lhs = rest.split_once('=').map(|(lhs, _)| lhs.trim());
        lhs.is_some_and(|lhs| lhs.eq_ignore_ascii_case(&target))
    })
}

fn frontend_assignment_target_name(
    typed_hir: &frontend_type_hooks::TypedHirModule,
    target: frontend_hir::HirExprId,
) -> Option<String> {
    let expr = typed_hir.module.arenas.expr(target)?;
    let frontend_hir::HirExprKind::Name(symbol_id) = expr.kind else {
        return None;
    };
    let symbol = typed_hir.module.symbols.symbol(symbol_id)?;
    let name = typed_hir.module.symbols.name(symbol.name)?;
    Some(name.folded.clone())
}

fn frontend_assignment_bound_type_name(ty: VbaTypeId) -> &'static str {
    match ty {
        VbaTypeId::Boolean => "Boolean",
        VbaTypeId::Byte => "Byte",
        VbaTypeId::Integer => "Integer",
        VbaTypeId::Long => "Long",
        VbaTypeId::LongLong => "LongLong",
        VbaTypeId::LongPtr => "LongPtr",
        VbaTypeId::Single => "Single",
        VbaTypeId::Double => "Double",
        VbaTypeId::Currency => "Currency",
        VbaTypeId::Date => "Date",
        VbaTypeId::String => "String",
        VbaTypeId::Variant => "Variant",
        VbaTypeId::Object => "Object",
        VbaTypeId::Array => "Array",
        VbaTypeId::InteropAny => "Any",
        VbaTypeId::Unknown => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArgumentBindingKindDescriptor, ArgumentSourceKindDescriptor, ArrayStorageKind, Bytecode,
        CallDiagnosticKindDescriptor, CallDiagnosticOwnerDescriptor,
        CallInvocationSyntaxDescriptor, CallTargetKindDescriptor, CoercionKindDescriptor,
        DefaultMemberPolicyDescriptor, EvaluationOrderDescriptor,
        ExpressionClassificationDescriptor, Instruction, NameBindingKindDescriptor,
        ObjectMemberKindDescriptor, OperatorCompareModeDescriptor, OperatorFamilyDescriptor,
        OptionalDefaultValue, OptionalMissingStatePolicy, OptionalParameterDescriptor,
        ParameterPassingMode, ParameterRole, ProcedureKindDescriptor, ProcedureRuntimeSlotKind,
        ResolvedParameterMechanism, RuntimeCarrierKind, SlotInitialState, SlotRole,
        SourceParameterMechanism, UdtCopySemanticsDescriptor, ValueStateKind,
        VbaOperatorDescriptor, VbaTypeId, compile, compile_with_runtime_metadata,
    };
    use crate::bytecode::{
        RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
    };
    use crate::{resolve, typecheck};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum DispatchInvokeMemberSelector {
        I32(i32),
        String(String),
    }

    fn dispatch_invoke_member_before(
        instructions: &[Instruction],
        invoke_index: usize,
        member_slot: usize,
    ) -> Option<DispatchInvokeMemberSelector> {
        for instruction in instructions[..invoke_index].iter().rev() {
            match instruction {
                Instruction::LoadConstI32 { slot, value } if *slot == member_slot => {
                    return Some(DispatchInvokeMemberSelector::I32(*value));
                }
                Instruction::LoadConstString { slot, value } if *slot == member_slot => {
                    return Some(DispatchInvokeMemberSelector::String(value.clone()));
                }
                _ => {}
            }
        }
        None
    }

    fn dispatch_invoke_members(out: &Bytecode) -> Vec<DispatchInvokeMemberSelector> {
        out.instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { member, .. } => {
                    dispatch_invoke_member_before(&out.instructions, index, *member)
                }
                _ => None,
            })
            .collect()
    }

    fn assert_has_dispatch_member(out: &Bytecode, expected_token: i32, expected_name: &str) {
        let members = dispatch_invoke_members(out);
        assert!(
            members.iter().any(|member| {
                matches!(member, DispatchInvokeMemberSelector::I32(value) if *value == expected_token)
                    || matches!(member, DispatchInvokeMemberSelector::String(value) if value.eq_ignore_ascii_case(expected_name))
            }),
            "expected dispatch selector token {expected_token} or name {expected_name:?}, got: {members:?}"
        );
    }

    fn dispatchinvoke_member_literal_from_source(source: &str) -> Option<&str> {
        for line in source.lines() {
            if !line.contains("DispatchInvoke(") {
                continue;
            }
            let mut literals = Vec::new();
            let mut start = None;
            for (index, ch) in line.char_indices() {
                match (ch, start) {
                    ('"', None) => start = Some(index + 1),
                    ('"', Some(begin)) => {
                        literals.push(&line[begin..index]);
                        start = None;
                    }
                    _ => {}
                }
            }
            if literals.len() >= 2 {
                return Some(literals[1]);
            }
        }
        None
    }

    fn assert_dispatchinvoke_source_member(out: &Bytecode, source: &str, expected_token: i32) {
        let expected_name = dispatchinvoke_member_literal_from_source(source)
            .expect("DispatchInvoke source should contain an explicit member literal");
        assert_has_dispatch_member(out, expected_token, expected_name);
    }

    fn optional_default_param_debug(source: &str) -> String {
        let typed = match super::frontend_type_hooks::collect_type_hooks_from_source("Main", source)
        {
            Ok(typed) => typed,
            Err(err) => return format!("typed-HIR error: {err:?}"),
        };
        let lines = source.lines().map(str::to_string).collect::<Vec<_>>();
        let default_type_table = resolve::collect_default_type_table(&lines);
        let module_constants = resolve::collect_module_constants(&lines);
        let mut rows = Vec::new();
        for decl_id in &typed.module.declarations {
            let Some(decl) = typed.module.arenas.decl(*decl_id) else {
                continue;
            };
            let super::frontend_hir::HirDeclKind::Procedure { params, .. } = &decl.kind else {
                continue;
            };
            let signature_line = super::source_line_for_span(source, decl.cst.span.start);
            let parsed = super::proc_kind_for_signature_line(signature_line).and_then(|kind| {
                resolve::parse_proc_signature_with_module_constants(
                    signature_line,
                    kind,
                    &default_type_table,
                    &module_constants,
                )
            });
            let hir_names = params
                .iter()
                .filter_map(|param| {
                    typed
                        .module
                        .symbols
                        .symbol(*param)
                        .and_then(|symbol| typed.module.symbols.name(symbol.name))
                        .map(|name| name.folded.clone())
                })
                .collect::<Vec<_>>();
            let parsed_names = parsed
                .map(|(_, params, _)| params.into_iter().map(|param| param.name).collect())
                .unwrap_or_else(Vec::new);
            rows.push(format!(
                "line={signature_line:?}; hir={hir_names:?}; parsed={parsed_names:?}"
            ));
        }
        rows.join("; ")
    }

    #[test]
    fn compile_simple_module() {
        let out = compile("Sub Main()\nEnd Sub").expect("compile should succeed");
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_options_default_uses_frontend_v2_for_completed_constructs() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let frontend =
            super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
                .expect("frontend_v2 compile should succeed");
        let defaulted = super::compile_with_options(source, super::CompileOptions::default())
            .expect("default compile");
        assert_eq!(
            format!("{:?}", frontend.instructions),
            format!("{:?}", defaulted.instructions),
            "default compile options should route completed constructs through frontend_v2"
        );
    }

    #[test]
    fn compile_options_default_uses_frontend_v2_for_source_backed_frontend_seed_rows() {
        for fixture in super::frontend_diff::frontend_rework_seed_corpus() {
            let Some(source) = fixture.source.as_deref() else {
                continue;
            };

            let frontend =
                super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
                    .unwrap_or_else(|err| {
                        panic!(
                            "frontend_v2 seed row `{}` should compile: {err}",
                            fixture.name
                        )
                    });
            let defaulted = super::compile_with_options(source, super::CompileOptions::default())
                .unwrap_or_else(|err| {
                    panic!("default seed row `{}` should compile: {err}", fixture.name)
                });

            assert_eq!(
                format!("{:?}", frontend.instructions),
                format!("{:?}", defaulted.instructions),
                "default compile should use frontend_v2 for source-backed seed row `{}`",
                fixture.name
            );
        }
    }

    #[test]
    fn compile_with_runtime_metadata_uses_hir_for_completed_constructs() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("HIR production lowering should support fixture");
        let defaulted = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should succeed");
        assert_eq!(
            format!("{:?}", hir.0.instructions),
            format!("{:?}", defaulted.0.instructions),
            "default runtime metadata compile should use HIR production for completed constructs"
        );
        assert_eq!(
            hir.1, defaulted.1,
            "runtime metadata should come from the HIR production route for completed constructs"
        );
    }

    #[test]
    fn compile_options_default_uses_explicit_legacy_helper_for_unsupported_residuals() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = New Widget\nEnd Sub\n";
        let strict =
            super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
                .expect_err("strict frontend_v2 should reject unsupported residual");
        assert!(
            strict.to_string().contains("frontend_v2 HIR unsupported"),
            "unexpected strict error: {strict}"
        );

        let legacy = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy helper should own the residual result");
        let defaulted = super::compile_with_options(source, super::CompileOptions::default())
            .expect_err("default compile should fall back to explicit legacy helper");
        assert_eq!(
            legacy.to_string(),
            defaulted.to_string(),
            "unsupported residual fallback error should match the explicit legacy helper"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_option_base_hir_route() {
        let source =
            "Option Base 1\nSub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "Option Base should not disqualify otherwise-completed HIR constructs"
        );
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(
                "OPTION   BASE   1\nSub Main()\nEnd Sub\n"
            ),
            "Option Base routing should not depend on exact whitespace or casing"
        );
        super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route Option Base source through HIR");
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_binary_option_compare_hir_route() {
        let source =
            "Option Compare Binary\nSub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "Option Compare Binary should not disqualify otherwise-completed HIR constructs"
        );
        super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route Option Compare Binary source through HIR",
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_text_option_compare_hir_route() {
        let source = "Option Compare Text\nSub Main()\n    Dim x\n    x = \"a\" = \"A\"\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "Option Compare Text should not disqualify otherwise-completed HIR constructs"
        );
        super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route Option Compare Text source through HIR",
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_database_option_compare_hir_route() {
        let source = "Option Compare Database\nSub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "Option Compare Database should not disqualify otherwise-completed HIR constructs"
        );
        super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route Option Compare Database source through HIR",
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_def_type_hir_route() {
        let source =
            "DefLng A-Z\nSub Main()\n    Dim alpha\n    alpha = 1: alpha = alpha + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "known DefType directives should not disqualify otherwise-completed HIR constructs"
        );
        super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route known DefType source through HIR",
        );
    }

    #[test]
    fn lightweight_hir_default_rejects_unknown_def_type_statement() {
        for source in [
            "DefFoo A-Z\nSub Main()\nEnd Sub\n",
            "DefLng\nSub Main()\nEnd Sub\n",
        ] {
            assert!(
                !super::source_is_eligible_for_lightweight_hir_default(source),
                "unknown or malformed DefType directive should remain outside default HIR route: {source}"
            );
        }
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_option_explicit_hir_route() {
        let source =
            "Option Explicit\nSub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "Option Explicit should not disqualify otherwise-completed HIR constructs"
        );
        super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route Option Explicit through HIR");
    }

    #[test]
    fn compile_with_runtime_metadata_default_allows_option_private_module_hir_route() {
        let source =
            "Option Private Module\nSub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "Option Private Module should not disqualify otherwise-completed single-source HIR constructs"
        );
        super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route single-source Option Private Module through HIR",
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_conditional_compilation_through_hir() {
        let source = "#Const ENABLE = True\nSub Main()\nDim x As Long\n#If ENABLE Then\nx = 7: x = x + 1\n#Else\nx = 1\n#End If\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should reject the active inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route filtered #If source through HIR",
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "expected active #If branch bytecode: {bytecode:#?}"
        );
        assert!(
            !bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 1, .. }
            )),
            "inactive #Else branch should not be emitted: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_conditional_elseif_through_hir() {
        let source = "#Const A = False\n#Const B = True\nSub Main()\nDim x As Long\n#If A Then\nx = 1\n#ElseIf B Then\nx = 9: x = x + 1\n#Else\nx = 3\n#End If\nEnd Sub\n";
        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route filtered #ElseIf source through HIR",
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 9, .. }
            )),
            "expected active #ElseIf branch bytecode: {bytecode:#?}"
        );
        assert!(
            !bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 3, .. }
            )),
            "inactive #Else branch should not be emitted: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_conditional_arithmetic_through_hir() {
        let source = "#Const LIMIT = 2 * 3 + 1\n#Const CHECK = 8 Mod 5 \\ 2\n#Const NEG = -2147483648\nSub Main()\nDim x As Long\n#If LIMIT Mod 4 = 3 And LIMIT \\ 2 = 3 And CHECK = 0 And NEG < 0 Then\nx = 11: x = x + 1\n#Else\nx = 1\n#End If\nEnd Sub\n";
        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route arithmetic #If source through HIR",
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 11, .. }
            )),
            "expected active arithmetic #If branch bytecode: {bytecode:#?}"
        );
        assert!(
            !bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 1, .. }
            )),
            "inactive arithmetic #Else branch should not be emitted: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_module_attribute_through_hir() {
        let source = "Attribute VB_Name = \"Module1\"\nSub Main()\nDim x As Long\nx = 7: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should reject the active inline sequence after an attribute");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route attribute-bearing source through HIR",
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "expected post-attribute procedure bytecode: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_typed_const_through_hir() {
        let source =
            "Const CBase As Long = 7\nSub Main()\nDim x As Long\nx = CBase: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should reject the active inline sequence after typed const");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route typed const source through HIR");
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 7, .. }
            )),
            "expected typed const value in bytecode: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_typed_const_expression_through_hir() {
        let source = "Const CBase As Long = 2 ^ 3 \\ 2 Mod 3, CTotal As Long = CBase + 4\nSub Main()\nDim x As Long\nx = CTotal: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source).expect_err(
            "legacy path should reject the active inline sequence after typed const expressions",
        );
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support typed const expressions");
        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route typed const expressions through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for typed const expressions"
        );
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::PowSlots { .. })),
            "expected typed expression const bytecode: {bytecode:#?}"
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::IntDivSlots { .. })),
            "expected typed integer-division const bytecode: {bytecode:#?}"
        );
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::ModSlots { .. })),
            "expected typed Mod const bytecode: {bytecode:#?}"
        );
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 4, .. }
            )),
            "expected same-statement typed const reference bytecode: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_cross_statement_typed_const_expression_through_hir()
     {
        let source = "Const CBase As Long = 2 ^ 3 \\ 2 Mod 3\nConst CTotal As Long = CBase + 4\nSub Main()\nDim x As Long\nx = CTotal: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source).expect_err(
            "legacy path should reject the active inline sequence after cross-statement typed const expressions",
        );
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        let hir = super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(
            source,
        )
        .expect(
            "direct HIR production lowering should support cross-statement typed const expressions",
        );
        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route cross-statement typed const expressions through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for cross-statement typed const expressions"
        );
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::LoadConstI32 { value: 4, .. }
            )),
            "expected cross-statement typed const reference bytecode: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_rejects_overflowing_typed_long_const() {
        let source = "Const CTotal As Long = 2 ^ 31\nSub Main()\nEnd Sub\n";
        let err = super::compile_with_runtime_metadata(source)
            .expect_err("default route should diagnose overflowing Long const");
        assert!(
            matches!(
                err,
                super::CompileError::ResolveError(ref message)
                    if message.contains("constant ctotal value 2147483648 overflows Long")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_rejects_overflowing_typed_byte_const() {
        let source = "Const CTotal As Byte = 255 + 1\nSub Main()\nEnd Sub\n";
        let err = super::compile_with_runtime_metadata(source)
            .expect_err("default route should diagnose overflowing Byte const");
        assert!(
            matches!(
                err,
                super::CompileError::ResolveError(ref message)
                    if message.contains("constant ctotal value 256 overflows Byte")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_optional_param_through_hir() {
        let source =
            "Sub Use(Optional ByVal n As Long = 7)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "optional parameters with explicit simple defaults should be eligible for default HIR"
        );

        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route optional parameter source through HIR",
        );
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(use_metadata.signature.parameters.len(), 1);
        assert!(use_metadata.signature.parameters[0].optional);
        assert_eq!(use_metadata.signature.parameters[0].default_value, Some(7));
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_optional_integer_expression_defaults_through_hir()
     {
        let source = "Sub Use(Optional ByVal n As Long = 2 ^ 3 \\ 2 Mod 3 + &H10 + 5)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "integer constant-expression defaults should stay eligible for default HIR"
        );

        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect(
                    "direct HIR production lowering should support integer default expressions",
                );
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route integer default expression through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for integer default expressions"
        );
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(use_metadata.signature.parameters[0].default_value, Some(22));
        assert!(matches!(
            use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitI32(22),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            }
        ));
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_optional_module_constant_defaults_through_hir()
    {
        let source = "Const CBase = &H10 + 1\nSub Use(Optional ByVal n As Long = CBase + 2)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "integer module-constant defaults should stay eligible for default HIR"
        );

        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support module-constant defaults");
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route module-constant default through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for module-constant defaults"
        );
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(use_metadata.signature.parameters[0].default_value, Some(19));
        assert!(matches!(
            use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitI32(19),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            }
        ));
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_optional_enum_constant_defaults_through_hir() {
        let source = "Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Use(Optional ByVal n As Long = Safe)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "integer enum defaults should stay eligible for default HIR"
        );

        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support enum defaults");
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route enum default through HIR");
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for enum defaults"
        );
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(use_metadata.signature.parameters[0].default_value, Some(4));
        assert!(matches!(
            use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitI32(4),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            }
        ));
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_optional_string_bool_defaults_through_hir() {
        let source = "Sub Use(Optional ByVal text As String = \"ready\", Optional ByVal flag As Boolean = True)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let parsed = oxvba_syntax::parse(source);
        assert!(
            parsed.errors().is_empty(),
            "syntax parser should accept string/Boolean optional defaults: {:?}",
            parsed.errors()
        );
        assert!(
            !super::source_has_hir_parameter_signature_mismatch(source),
            "typed HIR parameter symbols should match parsed string/Boolean default parameters: {}",
            optional_default_param_debug(source)
        );
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "string and Boolean optional defaults should stay eligible for default HIR"
        );

        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support string/Boolean defaults");
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route string/Boolean defaults through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for string/Boolean defaults"
        );
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(use_metadata.signature.parameters[0].default_value, None);
        assert!(matches!(
            &use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitString(value),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            } if value == "ready"
        ));
        assert_eq!(use_metadata.signature.parameters[1].default_value, None);
        assert!(matches!(
            use_metadata.signature.parameters[1].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitBool(true),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            }
        ));
        let main = metadata.get("main").expect("Main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| {
                call_site.arguments.iter().any(|arg| {
                    arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitString("ready".to_string()))
                }) && call_site.arguments.iter().any(|arg| {
                    arg.optional_default == Some(OptionalDefaultValue::ExplicitBool(true))
                })
            }),
            "expected omitted optional string/Boolean argument metadata: {main:#?}"
        );
    }

    #[test]
    fn optional_string_bool_module_constant_defaults_route_through_hir() {
        let source = "Const CText = \"ready\"\nConst CFlag = True\nSub Use(Optional ByVal text As String = CText, Optional ByVal flag As Boolean = CFlag)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("string/Boolean module-constant defaults should route through HIR");
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert!(matches!(
            &use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitString(value),
                ..
            } if value == "ready"
        ));
        assert!(matches!(
            use_metadata.signature.parameters[1].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitBool(true),
                ..
            }
        ));
    }

    #[test]
    fn optional_string_concat_defaults_route_through_hir() {
        let source = "Const Prefix = \"re\"\nSub Use(Optional ByVal text As String = Prefix & \"ady\")\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("string concat defaults should route through HIR");
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert!(matches!(
            &use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitString(value),
                ..
            } if value == "ready"
        ));
        let main = metadata.get("main").expect("Main metadata");
        assert!(main.call_sites.iter().any(|call_site| {
            call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("text")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitString("ready".to_string()))
            })
        }));
    }

    #[test]
    fn optional_boolean_expression_defaults_route_through_hir() {
        let source = "Const Prefix = \"re\"\nConst Enabled = True\nSub Use(Optional ByVal flag As Boolean = Enabled = Not False And 2 > 1 And Prefix & \"ady\" = \"ready\")\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("Boolean expression defaults should route through HIR");
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert!(matches!(
            use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitBool(true),
                ..
            }
        ));
        let main = metadata.get("main").expect("Main metadata");
        assert!(main.call_sites.iter().any(|call_site| {
            call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("flag")
                    && arg.optional_default == Some(OptionalDefaultValue::ExplicitBool(true))
            })
        }));
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_param_array_through_hir() {
        let source = "Sub Use(ParamArray items() As Variant)\nEnd Sub\nSub Main()\nCall Use(1, 2)\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "ParamArray declarations should be eligible for default HIR once packed calls lower correctly"
        );

        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route ParamArray source through HIR");
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert_eq!(
            use_metadata.signature.parameters[0].role,
            ParameterRole::ParamArray
        );
        let main = metadata.get("main").expect("Main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| {
                call_site
                    .arguments
                    .iter()
                    .any(|arg| arg.binding_kind == ArgumentBindingKindDescriptor::ParamArrayPack)
            }),
            "expected default HIR route to preserve ParamArray pack call-site metadata: {main:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_optional_without_default_through_hir() {
        let source =
            "Sub Use(Optional ByVal n As Variant)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "optional parameters without explicit defaults should be eligible once missing-state descriptors lower through HIR"
        );

        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route optional missing-state source through HIR",
        );
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert!(matches!(
            use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::VariantMissingError448,
                missing_state: OptionalMissingStatePolicy::PreserveMissingArgumentState,
            }
        ));
        let main = metadata.get("main").expect("Main metadata");
        assert!(
            main.call_sites.iter().any(|call_site| {
                call_site.arguments.iter().any(|arg| {
                    arg.optional_default == Some(OptionalDefaultValue::VariantMissingError448)
                })
            }),
            "expected omitted optional Variant argument metadata: {main:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_routes_typed_optional_declared_defaults_through_hir() {
        let source = "Sub Use(Optional ByVal text As String, Optional ByVal flag As Boolean, Optional ByVal n As Long)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let (_bytecode, metadata) =
            super::compile_with_runtime_metadata(source).expect("compile should route through HIR");
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert!(matches!(
            &use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitString(value),
                ..
            } if value.is_empty()
        ));
        assert!(matches!(
            use_metadata.signature.parameters[1].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitBool(false),
                ..
            }
        ));
        assert!(matches!(
            use_metadata.signature.parameters[2].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitI32(0),
                ..
            }
        ));
        let main = metadata.get("main").expect("Main metadata");
        assert!(main.call_sites.iter().any(|call_site| {
            call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("text")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitString(String::new()))
            }) && call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("flag")
                    && arg.optional_default == Some(OptionalDefaultValue::ExplicitBool(false))
            }) && call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("n")
                    && arg.optional_default == Some(OptionalDefaultValue::ExplicitI32(0))
            })
        }));
    }

    #[test]
    fn optional_date_currency_defaults_route_through_hir() {
        let source = "Const CAmount = 1.25@\nConst CStamp = 2.0\nSub Use(Optional ByVal amount As Currency = CAmount * 2@ - 1.0@, Optional ByVal stamp As Date = (CStamp + 3.0) / 2.0, Optional ByVal literalStamp As Date = #2026-02-28#, Optional ByVal blankAmount As Currency, Optional ByVal blankStamp As Date)\nEnd Sub\nSub Main()\nCall Use()\nEnd Sub\n";
        let (_bytecode, metadata) =
            super::compile_with_runtime_metadata(source).expect("compile should route through HIR");
        let use_metadata = metadata.get("use").expect("Use metadata");
        assert!(matches!(
            use_metadata.signature.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitCurrencyScaledI64(15_000),
                ..
            }
        ));
        assert!(matches!(
            use_metadata.signature.parameters[1].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitDateSerialF64(bits),
                ..
            } if f64::from_bits(bits) == 2.5
        ));
        assert!(matches!(
            use_metadata.signature.parameters[2].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitDateSerialF64(bits),
                ..
            } if f64::from_bits(bits) == 46_081.0
        ));
        assert!(matches!(
            use_metadata.signature.parameters[3].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitCurrencyScaledI64(0),
                ..
            }
        ));
        assert!(matches!(
            use_metadata.signature.parameters[4].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitDateSerialF64(bits),
                ..
            } if f64::from_bits(bits) == 0.0
        ));
        let main = metadata.get("main").expect("Main metadata");
        assert!(main.call_sites.iter().any(|call_site| {
            call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("amount")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitCurrencyScaledI64(15_000))
            }) && call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("stamp")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitDateSerialF64(
                            2.5f64.to_bits(),
                        ))
            }) && call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("literalstamp")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitDateSerialF64(
                            46_081.0f64.to_bits(),
                        ))
            }) && call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("blankamount")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitCurrencyScaledI64(0))
            }) && call_site.arguments.iter().any(|arg| {
                arg.parameter_name.as_deref() == Some("blankstamp")
                    && arg.optional_default
                        == Some(OptionalDefaultValue::ExplicitDateSerialF64(
                            0.0f64.to_bits(),
                        ))
            })
        }));
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_simple_property_declarations_through_hir() {
        let source = "Sub Main()\nDim x\nx = Value\nValue = x\nEnd Sub\nProperty Get Value() As Long\nValue = 9\nEnd Property\nProperty Let Value(ByRef target)\ntarget = target + 1\nEnd Property\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "simple non-indexed property declarations should be eligible for default HIR"
        );

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route simple property source through HIR",
        );
        assert!(metadata.contains_key("property_get_value"), "{metadata:#?}");
        assert!(metadata.contains_key("property_let_value"), "{metadata:#?}");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CallProc { .. })),
            "expected property get/let procedure calls: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_indexed_property_get_through_hir() {
        let source = "Sub Main()\nDim x\nx = Value(1)\nEnd Sub\nProperty Get Value(ByVal index As Long) As Long\nValue = index\nEnd Property\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "indexed Property Get declarations should be eligible once read-side calls lower through HIR"
        );
        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support indexed Property Get");

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route indexed Property Get source through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for indexed Property Get"
        );
        assert!(metadata.contains_key("property_get_value"), "{metadata:#?}");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CallProc { .. })),
            "expected indexed property get to lower as a procedure call: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_indexed_property_let_through_hir() {
        let source = "Sub Main()\nValue(1) = 7\nEnd Sub\nProperty Let Value(ByVal index As Long, ByVal newValue As Long)\nEnd Property\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "same-module indexed Property Let writeback should be eligible once indices and value intent lower together"
        );
        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support indexed Property Let");

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route indexed Property Let source through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for indexed Property Let"
        );
        assert!(metadata.contains_key("property_let_value"), "{metadata:#?}");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CallProc { .. })),
            "expected indexed property let to lower as a procedure call: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_named_indexed_property_let_through_hir() {
        let source = "Sub Main()\nValue(index := 1) = 7\nEnd Sub\nProperty Let Value(ByVal index As Long, ByVal newValue As Long)\nEnd Property\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "same-module named indexed Property Let writeback should stay eligible for default HIR"
        );
        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support named indexed Property Let");

        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route named indexed Property Let through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for named indexed Property Let"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_indexed_property_set_through_hir() {
        let source = "Sub Main()\nSet Value(1) = Nothing\nEnd Sub\nProperty Set Value(ByVal index As Long, ByVal newValue As Object)\nEnd Property\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "same-module indexed Property Set writeback should be eligible once object value intent lowers through HIR"
        );
        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support indexed Property Set");

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route indexed Property Set source through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for indexed Property Set"
        );
        assert!(metadata.contains_key("property_set_value"), "{metadata:#?}");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CallProc { .. })),
            "expected indexed property set to lower as a procedure call: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_named_indexed_property_set_through_hir() {
        let source = "Sub Main()\nSet Value(index := 1) = Nothing\nEnd Sub\nProperty Set Value(ByVal index As Long, ByVal newValue As Object)\nEnd Property\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "same-module named indexed Property Set writeback should stay eligible for default HIR"
        );
        let hir =
            super::frontend_hir_lowering::compile_source_with_runtime_metadata_via_hir(source)
                .expect("direct HIR production lowering should support named indexed Property Set");

        let (_bytecode, metadata) = super::compile_with_runtime_metadata(source).expect(
            "default runtime metadata compile should route named indexed Property Set through HIR",
        );
        assert_eq!(
            hir.1, metadata,
            "default route metadata should come from HIR production for named indexed Property Set"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_simple_createobject_through_hir() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Scripting.Dictionary\")\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "simple CreateObject source should be eligible for default HIR"
        );
        let (bytecode, _metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route simple CreateObject source");
        assert!(
            bytecode.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::IntrinsicCreateObjectHost { .. }
            )),
            "expected CreateObject host intrinsic through default route: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_dispatchinvoke_with_named_args_through_hir() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetIndexedValue\", value := 11, lhs := 7)\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "DispatchInvoke with named args should be eligible once HIR preserves dispatch argument names"
        );
        let (bytecode, _metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route named DispatchInvoke source");
        let args = bytecode
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { args, .. } => Some(args),
                _ => None,
            });
        assert!(
            matches!(
                args,
                Some(args)
                    if args.first().and_then(|arg| arg.name.as_deref()) == Some("value")
                        && args.get(1).and_then(|arg| arg.name.as_deref()) == Some("lhs")
            ),
            "expected named DispatchInvoke args to survive default HIR route: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_earlyinvoke_with_named_args_through_hir() {
        let source = "Sub Main()\nDim x\nx = __oxvbaearlyinvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetIndexedValue\", value := 11, lhs := 7)\nEnd Sub\n";
        assert!(
            super::source_is_eligible_for_lightweight_hir_default(source),
            "__oxvbaearlyinvoke with named args should be eligible once HIR preserves dispatch argument names"
        );
        let (bytecode, _metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route named early invoke source");
        let dispatch = bytecode
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::IntrinsicDispatchInvokeHost {
                    args, early_bound, ..
                } => Some((args, *early_bound)),
                _ => None,
            });
        assert!(
            matches!(
                dispatch,
                Some((args, true))
                    if args.first().and_then(|arg| arg.name.as_deref()) == Some("value")
                        && args.get(1).and_then(|arg| arg.name.as_deref()) == Some("lhs")
            ),
            "expected named early-dispatch args to survive default HIR route: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_options_frontend_v2_is_opt_in_hir_route() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1 + 2\nEnd Sub\n";
        let out = super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
            .expect("frontend_v2 HIR compile should succeed");
        assert!(!out.instructions.is_empty());
    }

    #[test]
    fn frontend_v2_analysis_exposes_shared_binder_hir_semantic_facts() {
        let source =
            "Sub Main(ByVal seed As Long)\n    Dim count As Long\n    count = seed\nEnd Sub\n";
        let analysis = super::analyze_frontend_v2_source(source).expect("frontend analysis");
        assert!(analysis.diagnostics.is_empty(), "{analysis:#?}");
        assert!(
            analysis
                .typed_hir
                .hooks
                .declared_type(
                    analysis
                        .typed_hir
                        .module
                        .symbols
                        .symbols()
                        .iter()
                        .find(|symbol| {
                            symbol.namespace == crate::frontend_symbols::SymbolNamespace::Local
                                && analysis
                                    .typed_hir
                                    .module
                                    .symbols
                                    .name(symbol.name)
                                    .is_some_and(|name| name.folded == "count")
                        })
                        .expect("count symbol")
                        .id
                )
                .is_some(),
            "expected declared type hook for count"
        );
        let seed_start = source.rfind("seed").expect("seed use");
        assert!(
            analysis
                .semantic_model
                .symbol_for_span(crate::frontend_symbols::FrontendSourceSpan {
                    start: seed_start,
                    end: seed_start + "seed".len()
                })
                .is_some(),
            "expected SemanticModel symbol query for parameter use"
        );
    }

    #[test]
    fn compile_options_default_enables_completed_hir_construct() {
        let source = "Sub Main()\n    Dim x As Long\n    x = 1: x = x + 1\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not accept inline sequence");
        assert!(
            legacy_err.to_string().contains("unsupported statement"),
            "unexpected legacy error: {legacy_err}"
        );

        let defaulted = super::compile_with_options(source, super::CompileOptions::default())
            .expect("default route should compile completed inline construct");
        assert!(!defaulted.instructions.is_empty());

        let out = super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
            .expect("frontend_v2 HIR should compile completed inline construct");
        assert!(!out.instructions.is_empty());
    }

    #[test]
    fn compile_options_frontend_v2_compiles_bare_object_is_identity() {
        let source = "Sub Main()\n    Dim obj As Object\n    Dim same As Boolean\n    same = obj Is Nothing\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not parse bare object Is");
        assert!(
            legacy_err.to_string().contains("unsupported statement")
                || legacy_err.to_string().contains("cannot parse expression"),
            "unexpected legacy error: {legacy_err}"
        );

        let out = super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
            .expect("frontend_v2 should compile object identity");
        assert!(
            out.instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CmpObjectIsSlots { .. })),
            "expected object identity bytecode: {:?}",
            out.instructions
        );
    }

    #[test]
    fn compile_with_runtime_metadata_default_routes_bare_object_is_identity_through_hir() {
        let source = "Sub Main()\nDim obj As Object\nDim same As Boolean\nsame = obj Is Nothing: same = Not same\nEnd Sub\n";
        let legacy_err = super::compile_with_runtime_metadata_legacy(source)
            .expect_err("legacy path should not parse bare object Is with inline continuation");
        assert!(
            legacy_err.to_string().contains("unsupported statement")
                || legacy_err.to_string().contains("cannot parse expression"),
            "unexpected legacy error: {legacy_err}"
        );

        let (bytecode, metadata) = super::compile_with_runtime_metadata(source)
            .expect("default runtime metadata compile should route object identity through HIR");
        assert!(metadata.contains_key("main"), "{metadata:#?}");
        assert!(
            bytecode
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::CmpObjectIsSlots { .. })),
            "expected object identity bytecode: {bytecode:#?}"
        );
    }

    #[test]
    fn compile_options_frontend_v2_compiles_object_identity_conformance_fixtures() {
        for source in [
            include_str!("../../../conformance/tests/object_identity_is_nothing.bas"),
            include_str!("../../../conformance/tests/object_identity_is_same_and_different.bas"),
        ] {
            let out =
                super::compile_with_options(source, super::CompileOptions { frontend_v2: true })
                    .expect("frontend_v2 should compile object identity fixture");
            assert!(
                out.instructions
                    .iter()
                    .any(|instruction| matches!(instruction, Instruction::CmpObjectIsSlots { .. })),
                "expected object identity bytecode: {:?}",
                out.instructions
            );
        }
    }

    #[test]
    fn compile_options_frontend_v2_rejects_syntax_before_legacy_lowering() {
        let err = super::compile_with_options(
            "Sub Main()\n    x = (1 + 2\nEnd Sub\n",
            super::CompileOptions { frontend_v2: true },
        )
        .expect_err("frontend_v2 HIR route should reject syntax parse errors first");
        assert!(
            err.to_string().contains("frontend_v2 diagnostics"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn reject_empty_input() {
        assert!(compile(" \n ").is_err());
    }

    #[test]
    fn resolve_and_typecheck_dynamic_byte_array_function_return_call() {
        let source = "Private Function MakeBuf() As Byte()\nDim buf() As Byte\nReDim buf(2)\nbuf(0) = 90\nbuf(1) = 91\nbuf(2) = 92\nMakeBuf = buf\nEnd Function\n\nSub Main()\nDim result() As Byte\nresult = MakeBuf()\nEnd Sub";
        let bound = resolve::resolve_symbols(source);
        let procedure_names = bound
            .procedures
            .iter()
            .map(|procedure| procedure.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            procedure_names,
            vec!["makebuf".to_string(), "main".to_string()]
        );
        typecheck::check_types(bound)
            .expect("same-module dynamic byte-array function call should typecheck");
    }

    #[test]
    fn compile_dynamic_byte_array_function_return_emits_all_index_reads() {
        let source = "Private Function MakeBuf() As Byte()\nDim buf() As Byte\nReDim buf(2)\nbuf(0) = 90\nbuf(1) = 91\nbuf(2) = 92\nMakeBuf = buf\nEnd Function\n\nSub Main()\nDim result() As Byte\nDim x0 As Long\nDim x1 As Long\nDim x2 As Long\nresult = MakeBuf()\nx0 = result(0)\nx1 = result(1)\nx2 = result(2)\nEnd Sub";
        let (bytecode, metadata) = compile_with_runtime_metadata(source)
            .expect("dynamic byte-array return assignment should compile");
        let get_count = bytecode
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::IntrinsicArrayGet { .. }))
            .count();
        assert_eq!(
            get_count, 3,
            "expected one array read per indexed assignment"
        );
        let main = metadata.get("main").expect("main metadata should exist");
        let main_slot_names = main
            .slots
            .iter()
            .map(|slot| slot.name.clone())
            .collect::<Vec<_>>();
        assert!(
            main_slot_names.contains(&"result".to_string())
                && main_slot_names.contains(&"x0".to_string())
                && main_slot_names.contains(&"x1".to_string())
                && main_slot_names.contains(&"x2".to_string()),
            "main slot metadata should preserve all locals, got {main_slot_names:?}"
        );
    }

    #[test]
    fn compile_dim_assign_and_add() {
        let source = "Sub Main()\nDim x\nx = 10\nx = x + 5\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_dim_assign_and_subtract() {
        let source = "Sub Main()\nDim x\nx = 10\nx = x - 3\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::SubConstI32 { slot: 0, value: 3 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_line_continuation_expression() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + _\n2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::AddConstI32 { slot: 0, value: 2 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn compile_with_runtime_metadata_projects_named_slot_kinds() {
        let source = "Function AddOne(ByVal value As Long) As Long\n\
                      Dim localValue As Long\n\
                      localValue = value + 1\n\
                      AddOne = localValue\n\
                      End Function";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let add_one = metadata
            .get("addone")
            .expect("function metadata should be present");
        assert!(add_one.slots.iter().any(|slot| {
            slot.name.eq_ignore_ascii_case("value")
                && slot.kind == ProcedureRuntimeSlotKind::Parameter
        }));
        assert!(add_one.slots.iter().any(|slot| {
            slot.name.eq_ignore_ascii_case("localvalue")
                && slot.kind == ProcedureRuntimeSlotKind::Local
        }));
        assert!(add_one.slots.iter().any(|slot| {
            slot.name.eq_ignore_ascii_case("addone")
                && slot.kind == ProcedureRuntimeSlotKind::ReturnValue
        }));
    }

    #[test]
    fn procedure_runtime_metadata_projects_first_slot_type_descriptor_view() {
        let source = "Function Combine(ByVal amount As Double, suffix As String) As Variant\n\
                      Dim localValue As Long\n\
                      Dim values(0 To 1) As Long\n\
                      localValue = 7\n\
                      values(0) = localValue\n\
                      Combine = CStr(amount + localValue) & suffix\n\
                      End Function";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let combine = metadata
            .get("combine")
            .expect("function metadata should be present");
        let descriptors = combine.slot_type_descriptors();

        let amount = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("amount"))
            })
            .expect("amount parameter descriptor should be present");
        assert_eq!(amount.role, SlotRole::Parameter);
        assert_eq!(amount.declared_type, VbaTypeId::Double);
        assert_eq!(amount.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(amount.carrier, RuntimeCarrierKind::F64);

        let suffix = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("suffix"))
            })
            .expect("suffix parameter descriptor should be present");
        assert_eq!(suffix.role, SlotRole::Parameter);
        assert_eq!(suffix.declared_type, VbaTypeId::String);
        assert_eq!(suffix.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(suffix.carrier, RuntimeCarrierKind::BStr);

        let return_value = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("Combine"))
            })
            .expect("function return descriptor should be present");
        assert_eq!(return_value.role, SlotRole::ReturnValue);
        assert_eq!(return_value.declared_type, VbaTypeId::Variant);
        assert_eq!(return_value.initial_state, SlotInitialState::Empty);
        assert_eq!(return_value.carrier, RuntimeCarrierKind::Variant);

        let local = descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("localValue"))
            })
            .expect("local descriptor should be present");
        assert_eq!(local.role, SlotRole::Local);
        assert_eq!(local.declared_type, VbaTypeId::Long);
        assert_eq!(local.initial_state, SlotInitialState::ScalarZero);
        assert_eq!(local.carrier, RuntimeCarrierKind::I32);

        let generated = descriptors
            .iter()
            .find(|descriptor| {
                descriptor.role == SlotRole::CompilerGenerated
                    && descriptor
                        .name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case("values_0"))
            })
            .expect("fixed-array element descriptor should be compiler-generated");
        assert_eq!(generated.declared_type, VbaTypeId::Long);
        assert_eq!(generated.initial_state, SlotInitialState::ScalarZero);
        assert_eq!(generated.carrier, RuntimeCarrierKind::I32);

        let temporary = descriptors
            .iter()
            .find(|descriptor| descriptor.role == SlotRole::Temporary)
            .expect("expression temporary descriptor should be present");
        assert_eq!(
            temporary.declared_type,
            VbaTypeId::Unknown,
            "temporary declared types are intentionally unknown until expression typing is preserved"
        );
        assert_eq!(temporary.initial_state, SlotInitialState::CompilerDefined);
        assert_eq!(temporary.carrier, RuntimeCarrierKind::Unknown);
    }

    #[test]
    fn procedure_runtime_metadata_carries_typed_carriers_and_value_states() {
        let source = "Type Point\n\
                      X As Long\n\
                      Label As String\n\
                      End Type\n\
                      Sub Main()\n\
                      End Sub\n\
                      Sub Probe(Optional ByVal marker As Variant, Optional ByVal amount As Long = 7)\n\
                      Dim l As Long\n\
                      Dim d As Double\n\
                      Dim b As Boolean\n\
                      Dim s As String\n\
                      Dim v As Variant\n\
                      Dim o As Object\n\
                      Dim p As Point\n\
                      Dim dec As Decimal\n\
                      Debug.Print Empty\n\
                      Debug.Print Null\n\
                      Debug.Print CVErr(7)\n\
                      Debug.Print vbNullString\n\
                      dec = CDec(1)\n\
                      Debug.Print dec\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let probe = metadata.get("probe").unwrap_or_else(|| {
            panic!(
                "Probe metadata keys={:?}",
                metadata.keys().cloned().collect::<Vec<_>>()
            )
        });

        for carrier in [
            RuntimeCarrierKind::I32,
            RuntimeCarrierKind::F64,
            RuntimeCarrierKind::Boolean,
            RuntimeCarrierKind::BStr,
            RuntimeCarrierKind::Variant,
            RuntimeCarrierKind::ObjectRef,
            RuntimeCarrierKind::Decimal96VariantSubtype,
        ] {
            assert!(
                probe
                    .carrier_layouts
                    .iter()
                    .any(|layout| layout.carrier == carrier),
                "missing carrier layout {carrier:?}"
            );
        }
        assert!(
            probe
                .carrier_layouts
                .iter()
                .any(|layout| matches!(layout.carrier, RuntimeCarrierKind::UdtFields { .. })),
            "UDT aggregate carrier layout should be present"
        );
        assert!(
            probe.carrier_layouts.iter().any(|layout| layout.carrier
                == RuntimeCarrierKind::Decimal96VariantSubtype
                && layout
                    .notes
                    .iter()
                    .any(|note| note == "declared-decimal-extension=variant-subtype")),
            "declared Decimal should be classified as the Decimal96 Variant subtype extension"
        );

        for state in [
            ValueStateKind::Empty,
            ValueStateKind::Null,
            ValueStateKind::Error,
            ValueStateKind::MissingArgument,
            ValueStateKind::OmittedDefault,
            ValueStateKind::Nothing,
            ValueStateKind::VbNullString,
            ValueStateKind::DecimalVariantSubtype,
        ] {
            assert!(
                probe
                    .value_states
                    .iter()
                    .any(|descriptor| descriptor.state == state),
                "missing value-state descriptor {state:?}"
            );
        }
    }

    #[test]
    fn procedure_runtime_metadata_carries_array_udt_enum_aggregate_facts() {
        let source = "Option Base 1\n\
                      Enum Mode\n\
                      Fast = 3\n\
                      Safe\n\
                      End Enum\n\
                      Type Inner\n\
                      X As Long\n\
                      End Type\n\
                      Type Record\n\
                      Name As String * 5\n\
                      Scores(1 To 2) As Long\n\
                      Inner As Inner\n\
                      End Type\n\
                      Sub Main()\n\
                      Dim fixed(3) As Long\n\
                      Dim explicit(0 To 2) As Long\n\
                      Dim matrix(1 To 2, 3 To 4) As Long\n\
                      Dim dyn() As Long\n\
                      Dim r As Record\n\
                      Dim modeValue As Long\n\
                      modeValue = Safe\n\
                      ReDim dyn(2 To 4)\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");

        let fixed = main
            .array_shapes
            .iter()
            .find(|descriptor| descriptor.name.eq_ignore_ascii_case("fixed"))
            .expect("fixed array shape should be present");
        assert_eq!(fixed.storage, ArrayStorageKind::StaticFixed);
        assert_eq!(fixed.option_base, 1);
        assert_eq!(fixed.bounds[0].lower_bound, 1);
        assert_eq!(fixed.bounds[0].upper_bound, 3);

        let matrix = main
            .array_shapes
            .iter()
            .find(|descriptor| descriptor.name.eq_ignore_ascii_case("matrix"))
            .expect("multi-rank array shape should be present");
        assert_eq!(matrix.rank, 2);
        assert_eq!(matrix.bounds[0].lower_bound, 1);
        assert_eq!(matrix.bounds[0].upper_bound, 2);
        assert_eq!(matrix.bounds[1].lower_bound, 3);
        assert_eq!(matrix.bounds[1].upper_bound, 4);

        let dyn_array = main
            .array_shapes
            .iter()
            .find(|descriptor| descriptor.name.eq_ignore_ascii_case("dyn"))
            .expect("dynamic array shape should be present");
        assert_eq!(dyn_array.storage, ArrayStorageKind::Dynamic);
        assert!(dyn_array.bounds.is_empty());

        let record = main
            .udt_types
            .iter()
            .find(|descriptor| descriptor.type_name.eq_ignore_ascii_case("Record"))
            .expect("Record UDT descriptor should be present");
        assert_eq!(
            record.copy_semantics,
            UdtCopySemanticsDescriptor::FieldWiseCopy
        );
        assert!(record.cleanup.owns_bstr);
        let name_field = record
            .fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case("Name"))
            .expect("fixed string field should be present");
        assert_eq!(name_field.fixed_string_len, Some(5));
        let scores_field = record
            .fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case("Scores"))
            .expect("fixed array field should be present");
        assert_eq!(scores_field.array_bounds[0].lower_bound, 1);
        assert_eq!(scores_field.array_bounds[0].upper_bound, 2);
        let inner_field = record
            .fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case("Inner"))
            .expect("nested UDT field should be present");
        assert_eq!(inner_field.nested_udt_name.as_deref(), Some("inner"));

        assert!(main.name_bindings.iter().any(|binding| {
            binding.binding_id == "NAME-BINDING-ENUM-TYPE"
                && binding.target.as_deref() == Some("enum:mode")
        }));
        assert!(main.name_bindings.iter().any(|binding| {
            binding.binding_id == "NAME-BINDING-ENUM-MEMBER"
                && binding.target.as_deref() == Some("enum:mode:safe=4")
        }));
    }

    #[test]
    fn procedure_runtime_metadata_projects_first_signature_descriptor_view() {
        let source = "Sub Main()\n\
                      End Sub\n\
                      Sub Capture(target As Long, ByRef alias As Long, Optional ByVal value As Long = 7)\n\
                      End Sub\n\
                      Sub Maybe(Optional ByVal arg As Variant)\n\
                      End Sub\n\
                      Sub Pack(ByRef target As Variant, ParamArray items() As Variant)\n\
                      End Sub\n\
                      Function Echo(ByVal text As String) As String\n\
                      Echo = text\n\
                      End Function\n\
                      Property Get Value() As Long\n\
                      Value = 1\n\
                      End Property\n\
                      Property Let Value(ByRef newValue As Long)\n\
                      End Property";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");

        let capture = metadata
            .get("capture")
            .expect("Capture metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(capture.kind, ProcedureKindDescriptor::Sub);
        assert_eq!(capture.return_type, None);
        assert_eq!(capture.parameters.len(), 3);
        assert_eq!(capture.parameters[0].name, "target");
        assert_eq!(capture.parameters[0].role, ParameterRole::Positional);
        assert_eq!(
            capture.parameters[0].passing_mode,
            ParameterPassingMode::ByRef
        );
        assert_eq!(
            capture.parameters[0].source_mechanism,
            SourceParameterMechanism::Omitted
        );
        assert_eq!(
            capture.parameters[0].resolved_mechanism,
            ResolvedParameterMechanism::ByRef
        );
        assert_eq!(capture.parameters[0].declared_type, VbaTypeId::Long);
        assert_eq!(capture.parameters[1].name, "alias");
        assert_eq!(
            capture.parameters[1].source_mechanism,
            SourceParameterMechanism::ExplicitByRef
        );
        assert_eq!(
            capture.parameters[1].passing_mode,
            ParameterPassingMode::ByRef
        );
        assert_eq!(capture.parameters[2].name, "value");
        assert_eq!(capture.parameters[2].role, ParameterRole::Optional);
        assert_eq!(
            capture.parameters[2].source_mechanism,
            SourceParameterMechanism::ExplicitByVal
        );
        assert_eq!(
            capture.parameters[2].passing_mode,
            ParameterPassingMode::ByVal
        );
        assert_eq!(capture.parameters[2].default_value, Some(7));
        assert_eq!(
            capture.parameters[2].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::ExplicitI32(7),
                missing_state: OptionalMissingStatePolicy::AssignDefaultLocal,
            }
        );

        let maybe = metadata
            .get("maybe")
            .expect("Maybe metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(
            maybe.parameters[0].optional_descriptor,
            OptionalParameterDescriptor::Optional {
                default_value: OptionalDefaultValue::VariantMissingError448,
                missing_state: OptionalMissingStatePolicy::PreserveMissingArgumentState,
            }
        );

        let pack = metadata
            .get("pack")
            .expect("Pack metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(pack.parameters[1].name, "items");
        assert_eq!(pack.parameters[1].role, ParameterRole::ParamArray);
        assert_eq!(pack.parameters[1].passing_mode, ParameterPassingMode::ByVal);
        assert_eq!(pack.parameters[1].declared_type, VbaTypeId::Array);
        assert_eq!(
            pack.parameters[1]
                .param_array_descriptor
                .as_ref()
                .map(|descriptor| (descriptor.element_type, descriptor.array_lower_bound)),
            Some((VbaTypeId::Variant, 0))
        );

        let echo = metadata
            .get("echo")
            .expect("Echo metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(echo.kind, ProcedureKindDescriptor::Function);
        assert_eq!(echo.return_type, Some(VbaTypeId::String));
        assert_eq!(echo.return_slot, metadata["echo"].return_slot);
        assert_eq!(echo.parameters[0].passing_mode, ParameterPassingMode::ByVal);
        assert_eq!(echo.parameters[0].declared_type, VbaTypeId::String);

        let property_get = metadata
            .get("property_get_value")
            .expect("Property Get metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(property_get.kind, ProcedureKindDescriptor::PropertyGet);
        assert_eq!(property_get.return_type, Some(VbaTypeId::Long));
        assert_eq!(property_get.property_group.as_deref(), Some("value"));

        let property_let = metadata
            .get("property_let_value")
            .expect("Property Let metadata should be present")
            .procedure_signature_descriptor();
        assert_eq!(property_let.kind, ProcedureKindDescriptor::PropertyLet);
        assert_eq!(property_let.return_type, None);
        assert_eq!(property_let.property_group.as_deref(), Some("value"));
        assert_eq!(
            property_let.parameters[0].role,
            ParameterRole::PropertyValue
        );
        assert_eq!(
            property_let.parameters[0].passing_mode,
            ParameterPassingMode::ByRef,
            "legacy passing_mode still reports parsed source mechanism"
        );
        assert_eq!(
            property_let.parameters[0].source_mechanism,
            SourceParameterMechanism::ExplicitByRef
        );
        assert_eq!(
            property_let.parameters[0].resolved_mechanism,
            ResolvedParameterMechanism::PropertyValueByVal
        );
    }

    #[test]
    fn procedure_runtime_metadata_projects_first_call_site_descriptor_view() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      Dim y As Long\n\
                      Dim observed As Long\n\
                      x = 1\n\
                      Call Touch(x)\n\
                      Call Fill(target := y)\n\
                      observed = Echo(x)\n\
                      Call Capture(y, 5, 7)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub\n\
                      Sub Fill(ByRef target As Long, Optional ByVal value As Long = 7)\n\
                      target = value\n\
                      End Sub\n\
                      Function Echo(ByVal value As Long) As Long\n\
                      Echo = value\n\
                      End Function\n\
                      Sub Capture(ByRef target As Long, ParamArray items() As Variant)\n\
                      target = UBound(items)\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");
        assert_eq!(main.call_sites.len(), 4);

        let touch = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Touch"))
            .expect("Touch call descriptor should be present");
        assert!(touch.call_site_id.starts_with("callsite:main@pc:"));
        assert_eq!(touch.target_kind, CallTargetKindDescriptor::Procedure);
        assert!(touch.target_entry_pc.is_some());
        assert_eq!(
            touch.default_member_policy,
            DefaultMemberPolicyDescriptor::NotApplicable
        );
        assert_eq!(
            touch.arguments[0].binding_kind,
            ArgumentBindingKindDescriptor::ByRefAlias
        );
        assert!(
            touch.arguments[0]
                .writeback
                .as_ref()
                .is_some_and(|w| w.required)
        );

        let fill = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Fill"))
            .expect("Fill call descriptor should be present");
        assert_eq!(fill.arguments[0].source_name.as_deref(), Some("target"));
        assert_eq!(
            fill.arguments[0].source_kind,
            ArgumentSourceKindDescriptor::Named
        );
        assert_eq!(
            fill.arguments[1].source_kind,
            ArgumentSourceKindDescriptor::Omitted
        );
        assert_eq!(
            fill.arguments[1].binding_kind,
            ArgumentBindingKindDescriptor::OptionalDefault
        );
        assert_eq!(
            fill.arguments[1].optional_default,
            Some(OptionalDefaultValue::ExplicitI32(7))
        );

        let echo = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Echo"))
            .expect("Echo call descriptor should be present");
        assert_eq!(echo.target_kind, CallTargetKindDescriptor::Function);
        assert!(
            echo.return_value
                .as_ref()
                .is_some_and(|return_value| return_value.copyout_required)
        );
        assert_eq!(
            echo.arguments[0].binding_kind,
            ArgumentBindingKindDescriptor::ByValCopy
        );

        let capture = main
            .call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Capture"))
            .expect("Capture call descriptor should be present");
        assert_eq!(
            capture.arguments[1].source_kind,
            ArgumentSourceKindDescriptor::ParamArrayPack
        );
        assert_eq!(
            capture.arguments[1].binding_kind,
            ArgumentBindingKindDescriptor::ParamArrayPack
        );
        assert_eq!(
            capture.arguments[1]
                .param_array
                .as_ref()
                .map(|param_array| param_array.element_count),
            Some(2)
        );
    }

    #[test]
    fn call_site_descriptors_preserve_invocation_syntax_and_diagnostic_policy() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      Dim y As Long\n\
                      Touch x\n\
                      Call Touch(y)\n\
                      Touch (x)\n\
                      y = Echo(x)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub\n\
                      Function Echo(ByVal value As Long) As Long\n\
                      Echo = value\n\
                      End Function";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::StatementNoCall
                && call.arguments[0].binding_kind == ArgumentBindingKindDescriptor::ByRefAlias
                && call.argument_evaluation_order == vec![0]),
            "statement calls without Call should preserve source order and alias policy"
        );
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::StatementCallKeyword
                && call.arguments[0].binding_kind == ArgumentBindingKindDescriptor::ByRefAlias),
            "Call keyword invocation should be package-visible"
        );
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::StatementNoCall
                && call.arguments[0].force_byval
                && call.arguments[0].binding_kind
                    == ArgumentBindingKindDescriptor::ByRefExpressionTemp
                && call
                    .diagnostic_policies
                    .iter()
                    .any(|policy| policy.diagnostic
                        == CallDiagnosticKindDescriptor::ByRefExpressionNoWriteback
                        && policy.owner == CallDiagnosticOwnerDescriptor::OracleNeeded)),
            "parenthesized statement-level argument should record no-writeback temp policy"
        );
        assert!(
            main.call_sites.iter().any(|call| call.invocation_syntax
                == CallInvocationSyntaxDescriptor::ExpressionCall
                && call.target_name.eq_ignore_ascii_case("Echo")
                && call
                    .return_value
                    .as_ref()
                    .is_some_and(|return_value| return_value.copyout_required)),
            "assignment from function should record expression-call syntax and return copyout"
        );
        assert!(
            main.call_sites.iter().any(|call| call
                .diagnostic_policies
                .iter()
                .any(|policy| policy.diagnostic
                    == CallDiagnosticKindDescriptor::MissingRequiredArgument
                    && policy.owner == CallDiagnosticOwnerDescriptor::CompilerCurrent)),
            "current invalid-call diagnostics should remain compiler-owned package policy"
        );
    }

    #[test]
    fn procedure_runtime_metadata_carries_expression_operator_and_coercion_descriptors() {
        let source = "Option Compare Text\n\
                      Sub Main()\n\
                      Dim x As Long\n\
                      Dim y As Double\n\
                      Dim s As String\n\
                      x = 1\n\
                      y = x\n\
                      y = y + x\n\
                      Dim v As Variant\n\
                      v = Null + 1\n\
                      v = Empty + 1\n\
                      v = CVErr(7) + 1\n\
                      s = \"a\" & CStr(x)\n\
                      If Null = 0 Or Empty = \"\" Then\n\
                      s = s\n\
                      End If\n\
                      If x > 0 And y > 0 Then\n\
                      s = s & vbNullString\n\
                      End If\n\
                      If x Then\n\
                      s = s\n\
                      End If\n\
                      Call TakeDouble(x)\n\
                      End Sub\n\
                      Sub TakeDouble(ByVal value As Double)\n\
                      End Sub";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");

        assert!(main.expression_semantics.iter().any(|descriptor| {
            descriptor.classification == ExpressionClassificationDescriptor::Variable
        }));
        assert!(main.expression_semantics.iter().any(|descriptor| {
            descriptor.classification == ExpressionClassificationDescriptor::OperatorResult
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-CONCAT-AMPERSAND"
                && descriptor.operator == VbaOperatorDescriptor::Concatenate
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-BOOL-NOT-AND-OR"
                && descriptor.evaluation_order == EvaluationOrderDescriptor::LeftToRightEager
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-CMP-NULL"
                && descriptor
                    .result_value_states
                    .contains(&ValueStateKind::Null)
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-CMP-EMPTY-STRING"
                && descriptor
                    .result_value_states
                    .contains(&ValueStateKind::Empty)
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-ADD-I32-COMPAT"
                && descriptor
                    .result_value_states
                    .contains(&ValueStateKind::Error)
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.operator_id == "OP-IIF-EAGER-DEFERRED"
                && descriptor.evaluation_order == EvaluationOrderDescriptor::UnsupportedDeferred
        }));
        assert!(main.operator_semantics.iter().any(|descriptor| {
            descriptor.family == OperatorFamilyDescriptor::Comparison
                && descriptor.compare_mode == Some(OperatorCompareModeDescriptor::Text)
        }));
        assert!(main.coercions.iter().any(|descriptor| {
            descriptor.coercion_id == "COERCE-LET-NUMERIC-WIDEN"
                && descriptor.kind == CoercionKindDescriptor::Let
        }));
        assert!(main.coercions.iter().any(|descriptor| {
            descriptor.coercion_id == "COERCE-CALL-BYVAL-DECLARED-TARGET"
                && descriptor.kind == CoercionKindDescriptor::CallLet
                && descriptor.source_declared_type == VbaTypeId::Long
                && descriptor.target_declared_type == VbaTypeId::Double
        }));
        assert!(
            main.coercions
                .iter()
                .any(|descriptor| { descriptor.coercion_id == "COERCE-VM-TRUTHINESS" })
        );
    }

    #[test]
    fn procedure_runtime_metadata_carries_name_and_property_member_descriptors() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      x = 1\n\
                      Value = x\n\
                      End Sub\n\
                      Property Get Value() As Long\n\
                      Value = 1\n\
                      End Property\n\
                      Property Let Value(ByRef newValue As Long)\n\
                      End Property";
        let (_bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let all_metadata = metadata.values().collect::<Vec<_>>();
        let main = metadata
            .get("main")
            .expect("Main metadata should be present");

        assert!(main.name_bindings.iter().any(|descriptor| {
            descriptor.binding_id == "NAME-BINDING-PROCEDURE-POLICY"
                && descriptor.binding_kind == NameBindingKindDescriptor::Policy
                && descriptor
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic == "with-context=deferred")
        }));
        assert!(all_metadata.iter().any(|metadata| {
            metadata.name_bindings.iter().any(|descriptor| {
                descriptor.binding_id == "NAME-BINDING-PROPERTY-ACCESSOR"
                    && descriptor.name == "value"
            })
        }));
        assert!(all_metadata.iter().any(|metadata| {
            metadata.object_member_bindings.iter().any(|descriptor| {
                descriptor.binding_id == "BIND-PROPERTY-LET-VALUE"
                    && descriptor.member_kind == ObjectMemberKindDescriptor::PropertyLet
                    && descriptor.argument_binding_policy == "value-param-is-runtime-ByVal"
            })
        }));
        assert!(
            main.coercions
                .iter()
                .any(|descriptor| { descriptor.coercion_id == "COERCE-PROPERTY-VALUE-BYVAL" })
        );
    }

    #[test]
    fn compile_with_block_member_assignments() {
        let source = "Sub Main()\nDim x\nWith x\n.Value = 1\n.Value = .Value + 2\nx = .Value\nEnd With\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 1, .. }))
        );
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicDispatchInvokeHost {
                args,
                call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertyLet),
                ..
            } if args.len() == 1
        )));
    }

    #[test]
    fn compile_with_block_direct_member_target_assignments() {
        let source = "Sub Main()\nDim x\nWith x.inner\n.Value = 4\n.Value = .Value + 3\nx = .Value\nEnd With\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicDispatchInvokeHost {
                args,
                call_kind_hint: Some(crate::bytecode::ProjectMemberCallKind::PropertyLet),
                ..
            } if args.len() == 1
        )));
    }

    #[test]
    fn compile_conditional_compilation_if_else_branch() {
        let source = "#Const ENABLE = True\nSub Main()\nDim x\n#If ENABLE Then\nx = 7\n#Else\nx = 1\n#End If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::ClearErr,
                Instruction::LoadConstI32 { slot: 0, value: 7 },
                Instruction::ClearErr,
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn undeclared_variable_with_option_explicit_is_rejected() {
        let source = "Option Explicit\nSub Main()\nx = 1\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("undeclared variable"));
    }

    #[test]
    fn duplicate_dim_declaration_is_rejected() {
        let source = "Sub Main()\nDim x\nDim x\nx = 1\nEnd Sub";
        let err = compile(source).expect_err("duplicate declarations should fail");
        assert!(err.to_string().contains("duplicate declaration"));
    }

    #[test]
    fn duplicate_label_declaration_is_rejected() {
        let source = "Sub Main()\nDim x\nGoSub mark\nIf Err.Number = -1 Then\nmark:\nx = 1\nReturn\nmark:\nx = 2\nReturn\nEnd If\nEnd Sub";
        let err = compile(source).expect_err("duplicate labels should fail");
        assert!(err.to_string().contains("duplicate label declaration"));
    }

    #[test]
    fn declaration_same_name_as_other_procedure_is_allowed() {
        // VBA allows a local variable to have the same name as another procedure.
        let source = "Sub Main()\nDim helper\nhelper = 1\nEnd Sub\nSub Helper()\nEnd Sub";
        compile(source).expect("variable sharing name with another procedure should compile");
    }

    #[test]
    fn defobj_applies_to_implicit_declarations() {
        let source = "DefObj A-Z\nSub Main()\na = 1\nEnd Sub";
        let err = compile(source).expect_err("DefObj should type implicit a as Object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn type_char_overrides_def_type_for_dim() {
        let source = "DefObj A-Z\nSub Main()\nDim a%\na = 1\nEnd Sub";
        compile(source).expect("type character should override DefObj for Dim");
    }

    #[test]
    fn explicit_as_overrides_type_char_for_dim() {
        let source = "Sub Main()\nDim a% As Object\na = 1\nEnd Sub";
        let err = compile(source).expect_err("explicit As should override type character");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn def_type_applies_to_untyped_params() {
        let source = "DefObj A-Z\nSub Main()\nCall Use(1)\nEnd Sub\nSub Use(ByVal alpha)\nEnd Sub";
        let err = compile(source).expect_err("DefObj should type alpha as Object");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn type_char_overrides_def_type_for_params() {
        let source = "DefObj A-Z\nSub Main()\nCall Use(1)\nEnd Sub\nSub Use(ByVal alpha%)\nEnd Sub";
        compile(source).expect("type character should override DefObj for parameters");
    }

    #[test]
    fn explicit_as_overrides_type_char_for_params() {
        let source = "Sub Main()\nCall Use(1)\nEnd Sub\nSub Use(ByVal alpha% As Object)\nEnd Sub";
        let err = compile(source).expect_err("explicit As should override parameter type char");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn function_return_typechar_overrides_def_type() {
        let source = "DefObj A-Z\nFunction alpha%()\nalpha = 1\nEnd Function\nSub Main()\nEnd Sub";
        compile(source).expect("function return type character should override DefObj");
    }

    #[test]
    fn function_return_explicit_as_overrides_typechar() {
        let source = "Function alpha%() As Object\nalpha = 1\nEnd Function\nSub Main()\nEnd Sub";
        let err = compile(source).expect_err("explicit As should control function return type");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn byref_typed_exact_match_is_required() {
        let source = "Sub Main()\nDim x As Integer\nx = 1\nCall Touch(x)\nEnd Sub\nSub Touch(ByRef target As Long)\ntarget = target + 1\nEnd Sub";
        let err = compile(source).expect_err("typed ByRef mismatch should be rejected");
        assert!(err.to_string().contains("ByRef parameter"));
    }

    #[test]
    fn byref_typed_exact_match_accepts_same_type() {
        let source = "Sub Main()\nDim x As Long\nx = 1\nCall Touch(x)\nEnd Sub\nSub Touch(ByRef target As Long)\ntarget = target + 1\nEnd Sub";
        compile(source).expect("typed ByRef with exact type should compile");
    }

    #[test]
    fn late_bound_object_default_member_call_is_executable_subset() {
        let source = "Sub Main()\nDim obj As Object\nCall obj(1)\nEnd Sub";
        let out = compile(source).expect("late-bound target should compile in executable subset");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn late_bound_named_argument_call_preserves_dispatch_lowering() {
        let source = "Sub Main()\nDim obj As Object\nCall obj(x:=1)\nEnd Sub";
        let out = compile(source).expect(
            "late-bound named-arg target should lower into the default-member dispatch path",
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { args, .. } if args.len() == 1 && args[0].name.as_deref() == Some("x")))
        );
    }

    #[test]
    fn mixed_call_coercion_variant_to_long_is_allowed() {
        let source = "Sub Main()\nDim v As Variant\nv = 5\nCall Use(v)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub";
        compile(source).expect("mixed-mode variant argument should coerce to long parameter");
    }

    #[test]
    fn coercion_assignment_variant_to_long_is_allowed() {
        let source = "Sub Main()\nDim v As Variant\nDim x As Long\nx = v\nEnd Sub";
        compile(source).expect("variant should be assignable to typed target in current matrix");
    }

    #[test]
    fn coercion_assignment_object_to_long_is_rejected() {
        let source = "Sub Main()\nDim o As Object\nDim x As Long\nx = o\nEnd Sub";
        let err = compile(source).expect_err("object should not coerce to long");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn coercion_argument_object_to_long_is_rejected() {
        let source =
            "Sub Main()\nDim o As Object\nCall Use(o)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub";
        let err = compile(source).expect_err("object argument should not coerce to long");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn conversion_intrinsic_cint_to_object_assignment_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = CInt(5)\nEnd Sub";
        let err = compile(source).expect_err("typed conversion result should not assign to object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn set_keyword_accepts_variant_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim v As Variant\nSet v = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect("Set should allow object-producing call result into Variant target");
    }

    #[test]
    fn set_keyword_accepts_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect("Set should allow object-producing call result into Object target");
    }

    #[test]
    fn set_keyword_rejects_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nSet v = 7\nEnd Sub";
        let err = compile(source).expect_err("Set should reject scalar source for Variant target");
        assert!(
            err.to_string()
                .contains("Set requires object value for variable v")
        );
    }

    #[test]
    fn set_keyword_rejects_object_target_for_scalar_source() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = 7\nEnd Sub";
        let err = compile(source).expect_err("Set should reject scalar source for Object target");
        assert!(
            err.to_string()
                .contains("Set requires object value for variable obj")
        );
    }

    #[test]
    fn set_keyword_rejects_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nSet n = 7\nEnd Sub";
        let err = compile(source).expect_err("Set should reject scalar source for scalar target");
        assert!(
            err.to_string()
                .contains("Set requires Object or Variant target, got Long variable n")
        );
    }

    #[test]
    fn set_keyword_rejects_scalar_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim n As Long\nSet n = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("Set should reject object-producing call result for scalar target");
        assert!(
            err.to_string()
                .contains("Set requires Object or Variant target, got Long variable n")
        );
    }

    #[test]
    fn let_keyword_rejects_object_target_for_object_call_result() {
        let source = "Sub Main()\nDim obj As Object\nLet obj = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("Let should reject object-producing call result on Object target");
        assert!(
            err.to_string()
                .contains("Let cannot assign to Object variable obj")
        );
    }

    #[test]
    fn let_keyword_accepts_variant_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim v As Variant\nLet v = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect("Let should allow object-producing call result into Variant target");
    }

    #[test]
    fn let_keyword_rejects_object_target_for_scalar_source() {
        let source = "Sub Main()\nDim obj As Object\nLet obj = 7\nEnd Sub";
        let err = compile(source).expect_err("Let should reject scalar source on Object target");
        assert!(
            err.to_string()
                .contains("Let cannot assign to Object variable obj")
        );
    }

    #[test]
    fn let_keyword_accepts_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nLet v = 7\nEnd Sub";
        compile(source).expect("Let should allow scalar source on Variant target");
    }

    #[test]
    fn let_keyword_accepts_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nLet n = 7\nEnd Sub";
        compile(source).expect("Let should allow scalar source on scalar target");
    }

    #[test]
    fn let_keyword_rejects_scalar_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim n As Long\nLet n = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("Let should reject object-producing call result for scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn implicit_assignment_accepts_variant_target_for_object_call_result() {
        let source =
            "Sub Main()\nDim v As Variant\nv = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        compile(source).expect(
            "implicit assignment should allow object-producing call result into Variant target",
        );
    }

    #[test]
    fn implicit_assignment_rejects_object_target_for_object_call_result_without_set() {
        let source =
            "Sub Main()\nDim obj As Object\nobj = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source)
            .expect_err("implicit assignment should require Set for object call result");
        assert!(
            err.to_string()
                .contains("Set required for Object variable obj")
        );
    }

    #[test]
    fn implicit_assignment_rejects_scalar_target_for_object_call_result() {
        let source = "Sub Main()\nDim n As Long\nn = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let err = compile(source).expect_err(
            "implicit assignment should reject object-producing call result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn implicit_assignment_rejects_object_target_for_scalar_source() {
        let source = "Sub Main()\nDim obj As Object\nobj = 7\nEnd Sub";
        let err = compile(source)
            .expect_err("implicit assignment should reject scalar source on Object target");
        assert!(
            err.to_string()
                .contains("cannot assign Long to Object variable obj")
        );
    }

    #[test]
    fn implicit_assignment_accepts_variant_target_for_scalar_source() {
        let source = "Sub Main()\nDim v As Variant\nv = 7\nEnd Sub";
        compile(source).expect("implicit assignment should allow scalar source on Variant target");
    }

    #[test]
    fn implicit_assignment_accepts_scalar_target_for_scalar_source() {
        let source = "Sub Main()\nDim n As Long\nn = 7\nEnd Sub";
        compile(source).expect("implicit assignment should allow scalar source on scalar target");
    }

    #[test]
    fn object_source_assignment_accepts_set_targets_and_variant_let_implicit() {
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet v = src\nEnd Sub",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet v = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Object\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nv = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            compile(source).unwrap_or_else(|err| panic!("{label} should compile, got {err}"));
        }
    }

    #[test]
    fn object_source_assignment_rejects_object_and_scalar_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Object\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\ndst = src\nEnd Sub",
                "Set required for Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet n = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Object\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nn = src\nEnd Sub",
                "cannot assign Object to Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match compile(source) {
                Ok(_) => panic!("{label} should reject"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }

    #[test]
    fn variant_source_scalar_payload_assignment_accepts_runtime_checked_lanes() {
        let cases = [
            (
                "Set Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nLet v = src\nEnd Sub",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nLet n = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nv = src\nEnd Sub",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\ndst = src\nEnd Sub",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nn = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            compile(source).unwrap_or_else(|err| panic!("{label} should compile, got {err}"));
        }
    }

    #[test]
    fn variant_source_scalar_payload_assignment_rejects_compile_time_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nsrc = 7\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match compile(source) {
                Ok(_) => panic!("{label} should reject"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }

    #[test]
    fn variant_source_object_payload_assignment_accepts_runtime_checked_lanes() {
        let cases = [
            (
                "Set Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet v = src\nEnd Sub",
            ),
            (
                "Set Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet dst = src\nEnd Sub",
            ),
            (
                "Let Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet v = src\nEnd Sub",
            ),
            (
                "implicit Variant target",
                "Sub Main()\nDim src As Variant\nDim v As Variant\nSet src = CreateObject(\"OxVba.TestDispatch\")\nv = src\nEnd Sub",
            ),
            (
                "implicit Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\ndst = src\nEnd Sub",
            ),
            (
                "Let scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet n = src\nEnd Sub",
            ),
            (
                "implicit scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nn = src\nEnd Sub",
            ),
        ];

        for (label, source) in cases {
            compile(source).unwrap_or_else(|err| panic!("{label} should compile, got {err}"));
        }
    }

    #[test]
    fn variant_source_object_payload_assignment_rejects_compile_time_mismatch_lanes() {
        let cases = [
            (
                "Let Object target",
                "Sub Main()\nDim src As Variant\nDim dst As Object\nSet src = CreateObject(\"OxVba.TestDispatch\")\nLet dst = src\nEnd Sub",
                "Let cannot assign to Object variable dst",
            ),
            (
                "Set scalar target",
                "Sub Main()\nDim src As Variant\nDim n As Long\nSet src = CreateObject(\"OxVba.TestDispatch\")\nSet n = src\nEnd Sub",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, source, expected) in cases {
            let err = match compile(source) {
                Ok(_) => panic!("{label} should reject"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains(expected), "{label}: {message}");
        }
    }

    #[test]
    fn variant_source_set_assignment_emits_runtime_validation() {
        let out = compile(
            "Sub Main()\nDim src As Variant\nDim v As Variant\nsrc = 7\nSet v = src\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(out.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::ValidateRuntimeAssignment {
                    intent: RuntimeAssignmentIntent::Set,
                    target_kind: RuntimeAssignmentTargetKind::Variant,
                    target_name,
                    ..
                } if target_name == "v"
            )
        }));
    }

    #[test]
    fn variant_source_implicit_object_assignment_emits_runtime_validation() {
        let out = compile(
            "Sub Main()\nDim src As Variant\nDim dst As Object\nsrc = 7\ndst = src\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(out.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::ValidateRuntimeAssignment {
                    intent: RuntimeAssignmentIntent::Implicit,
                    target_kind: RuntimeAssignmentTargetKind::Object,
                    target_name,
                    ..
                } if target_name == "dst"
            )
        }));
    }

    #[test]
    fn conversion_intrinsic_cint_to_long_assignment_is_allowed() {
        let source = "Sub Main()\nDim x As Long\nx = CInt(5)\nEnd Sub";
        compile(source).expect("typed conversion result should assign to widening numeric target");
    }

    #[test]
    fn conversion_intrinsic_str_to_object_assignment_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = Str(5)\nEnd Sub";
        let err = compile(source).expect_err("Str result should not assign to object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn instrrev_result_to_object_assignment_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = InStrRev(123231, 23)\nEnd Sub";
        let err = compile(source).expect_err("InStrRev result should be typed as Long");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn mid_statement_object_target_is_rejected() {
        let source = "Sub Main()\nDim o As Object\nMid(o, 2, 2) = 99\nEnd Sub";
        let err = compile(source).expect_err("Mid statement should reject object targets");
        assert!(
            err.to_string()
                .contains("type mismatch in Mid assignment target")
        );
    }

    #[test]
    fn vbnullstring_assigns_to_string() {
        let source = "Sub Main()\nDim s As String\ns = vbNullString\nEnd Sub";
        compile(source).expect("vbNullString should be treated as string-typed intrinsic constant");
    }

    #[test]
    fn vbnullstring_assignment_to_object_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = vbNullString\nEnd Sub";
        let err = compile(source).expect_err("vbNullString should not assign to object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn vbnullstring_assignment_to_long_is_rejected() {
        let source = "Sub Main()\nDim x As Long\nx = vbNullString\nEnd Sub";
        let err = compile(source).expect_err("vbNullString should not assign to numeric target");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn vbnullstring_argument_to_long_param_is_rejected() {
        let source =
            "Sub Main()\nCall Use(vbNullString)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub";
        let err = compile(source).expect_err("vbNullString should not pass to numeric parameter");
        assert!(err.to_string().contains("argument type mismatch"));
    }

    #[test]
    fn compile_cverr_emits_retained_error_instruction() {
        let source = "Sub Main()\nDim x\nx = CVErr(7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCVErr { .. }))
        );
        assert!(
            !out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { .. }))
        );
    }

    #[test]
    fn compile_financial_intrinsics_emit_algorithmic_ops() {
        let source = "Sub Main()\nDim c\nDim d\nDim e\nDim f\nDim g\nc = NPV(1, 10, 20, 30)\nd = IRR(50)\ne = MIRR(70, 1, 2)\nf = Rate(10, 2, 99)\ng = NPer(1, 2, 88, 3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicNpvI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicIrrI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMirrI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicRateI32 { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicNPerI32 { .. }))
        );
    }

    #[test]
    fn arithmetic_object_plus_const_is_rejected() {
        let source = "Sub Main()\nDim o As Object\no = o + 1\nEnd Sub";
        let err = compile(source).expect_err("object arithmetic should be rejected");
        assert!(
            err.to_string()
                .contains("type mismatch in arithmetic expression")
        );
    }

    #[test]
    fn comparison_object_long_is_rejected() {
        let source = "Sub Main()\nDim o As Object\nIf o = 1 Then\nEnd If\nEnd Sub";
        let err = compile(source).expect_err("object vs long comparison should be rejected");
        assert!(err.to_string().contains("type mismatch in comparison"));
    }

    #[test]
    fn comparison_variant_long_is_allowed() {
        let source = "Sub Main()\nDim v As Variant\nIf v = 1 Then\nv = 2\nEnd If\nEnd Sub";
        compile(source).expect("variant comparison with long should compile");
    }

    #[test]
    fn compile_mul_expression() {
        let source = "Sub Main()\nDim x\nx = x * 2\nEnd Sub";
        let out = compile(source).expect("multiply expression should compile");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::MulSlots { .. }))
        );
    }

    #[test]
    fn undeclared_variable_without_option_explicit_is_accepted() {
        let source = "Sub Main()\nx = 1\nx = x + 1\nEnd Sub";
        let out = compile(source).expect("implicit declaration should compile");
        assert_eq!(out.slot_count, 1);
    }

    #[test]
    fn compile_if_statement_emits_branch_instructions() {
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = x + 2\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn compile_if_statement_with_relational_and_boolean_ops() {
        let source =
            "Sub Main()\nDim x\nx = 1\nIf Not x <> 1 Or x < 2 Then\nx = x + 1\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpNeSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLtSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolNot { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolOr { .. }))
        );
    }

    #[test]
    fn compile_single_line_if_statement_emits_branch_instructions() {
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then x = x + 2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn compile_for_statement_emits_loop_instructions() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLeSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_if_else_if_else_emits_chain_jumps() {
        let source = "Sub Main()\nDim x\nx = 2\nIf x = 1 Then\nx = 10\nElseIf x = 2 Then\nx = 20\nElse\nx = 30\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let jump_if_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::JumpIfZero { .. }))
            .count();
        let jump_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Jump { .. }))
            .count();
        assert!(jump_if_count >= 2);
        assert!(jump_count >= 2);
    }

    #[test]
    fn compile_do_while_emits_loop_jumps() {
        let source = "Sub Main()\nDim x\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_for_step_emits_addslots() {
        let source =
            "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 5 To 1 Step -2\nx = x + 1\nNext i\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
    }

    #[test]
    fn compile_for_step_zero_is_diagnostic_error() {
        let source = "Sub Main()\nDim i\nFor i = 1 To 5 Step 0\ni = i + 1\nNext i\nEnd Sub";
        let err = compile(source).expect_err("compile should fail for zero step");
        assert!(matches!(
            err,
            super::CompileError::TypeError(msg) if msg.contains("for loop step cannot be zero")
        ));
    }

    #[test]
    fn compile_select_case_is_range_emits_clause_dispatch() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase Is < 0\nx = 1\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLtSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolAnd { .. }))
        );
    }

    #[test]
    fn compile_goto_label_binds_target() {
        let source = "Sub Main()\nDim x\nx = 1\nGoTo done\nx = 99\ndone:\nx = x + 1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_resume_label_emits_resume_label_instruction() {
        let source =
            "Sub Main()\nOn Error GoTo handler\nError 5\nhandler:\nResume done\ndone:\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ResumeLabel { .. }))
        );
    }

    #[test]
    fn compile_err_clear_emits_clear_err_instruction() {
        let source = "Sub Main()\nOn Error Resume Next\nError 7\nErr.Clear\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ClearErr))
        );
    }

    #[test]
    fn compile_select_case_emits_case_dispatch() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase 1\nx = 10\nCase 2, 3\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolOr { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn compile_named_sub_call_emits_callproc() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall Foo\nEnd Sub\nSub Foo()\nDim y\ny = 2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Return))
        );
    }

    #[test]
    fn compile_with_runtime_metadata_reports_entry_points_for_named_procedures() {
        let source = "Sub Main()\nDim x\nx = 1\nCall Foo(x)\nEnd Sub\nSub Foo(ByVal n)\nDim y\ny = n\nEnd Sub";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let main = metadata
            .get("main")
            .expect("entry procedure metadata should exist");
        let foo = metadata
            .get("foo")
            .expect("named procedure metadata should exist");
        assert!(main.entry_pc < bytecode.instructions.len());
        assert!(foo.entry_pc < bytecode.instructions.len());
        assert_eq!(foo.param_slots.len(), 1);
        assert_eq!(main.procedure_name, "main");
        assert_eq!(main.source_line_start, 1);
        assert_eq!(main.source_line_end, 5);
        assert_eq!(main.statement_line_numbers, vec![2, 3, 4]);
        assert_eq!(main.statement_entry_pcs.len(), 2);
        assert_eq!(main.statement_entry_pcs[0], main.entry_pc + 1);
        assert_eq!(foo.procedure_name, "foo");
        assert_eq!(foo.source_line_start, 6);
        assert_eq!(foo.source_line_end, 9);
        assert_eq!(foo.statement_line_numbers, vec![7, 8]);
        assert_eq!(foo.statement_entry_pcs.len(), 1);
        assert_eq!(foo.statement_entry_pcs[0], foo.entry_pc + 1);
    }

    #[test]
    fn compile_optional_param_call_accepts_omitted_arg() {
        let source = "Sub Main()\nDim x\nCall Fill(x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_optional_param_call_rejects_missing_required_arg() {
        let source = "Sub Main()\nCall Fill\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("missing required argument"));
    }

    #[test]
    fn compile_named_args_call_is_supported() {
        let source = "Sub Main()\nDim x\nCall Fill(target := x, value := 9)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_rejects_positional_argument_after_named_argument() {
        let source = "Sub Main()\nDim x\nCall Fill(value := 9, x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(
            err.to_string()
                .contains("positional argument cannot follow named argument")
        );
    }

    #[test]
    fn compile_paramarray_call_accepts_trailing_positional_args() {
        let source = "Sub Main()\nDim x\nCall Capture(x, 5, 7, 9)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicUBoundArray { .. }))
        );
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::IntrinsicArrayLiteral { values, .. } if values.len() == 3)
        ));
    }

    #[test]
    fn compile_paramarray_named_fixed_arg_with_positional_pack_compiles() {
        let source = "Sub Main()\nDim x\nCall Capture(target := x, 5, 7)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::IntrinsicArrayLiteral { values, .. } if values.len() == 2)
        ));
    }

    #[test]
    fn compile_paramarray_named_param_still_rejects_named_pack_target() {
        let source = "Sub Main()\nDim x\nCall Capture(target := x, items := 5)\nEnd Sub\nSub Capture(ByRef target, ParamArray items() As Variant)\ntarget = UBound(items)\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("ParamArray"));
    }

    #[test]
    fn compile_gosub_and_return_subset() {
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nx = x + 1\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let call_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::CallProc { .. }))
            .count();
        let return_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Return))
            .count();
        assert!(call_count >= 1);
        assert!(return_count >= 1);
    }

    #[test]
    fn compile_gosub_rejects_missing_label() {
        let source = "Sub Main()\nGoSub nope\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("gosub target label not found"));
    }

    #[test]
    fn compile_redim_preserve_subset() {
        let source =
            "Sub Main()\nDim a(1)\nDim x\na(0) = 7\nReDim Preserve a(3)\nx = a(0)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
    }

    #[test]
    fn compile_redim_shrink_bounds_violation_is_rejected() {
        let source = "Sub Main()\nDim a(3)\nReDim a(1)\na(2) = 9\nEnd Sub";
        let err = compile(source).expect_err("compile should fail");
        let message = err.to_string();
        assert!(
            message.contains("a(2)") || message.contains("a_2") || message.contains("unsupported"),
            "expected bounds rejection diagnostic, got: {message}"
        );
    }

    #[test]
    fn compile_runtime_redim_expression_bounds_on_dynamic_array_emits_resize_instruction() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nlength = 3\nReDim buf(length - 1)\nEnd Sub";
        assert!(
            compile(source)
                .expect("compile should succeed")
                .instructions
                .iter()
                .any(|i| matches!(
                    i,
                    Instruction::IntrinsicArrayResize {
                        lower_bounds,
                        element_type: crate::bytecode::RuntimeArrayElementType::Byte,
                        ..
                    } if lower_bounds == &vec![0]
                )),
            "expected runtime array resize instruction for dynamic ReDim"
        );
    }

    #[test]
    fn compile_runtime_redim_preserve_expression_bounds_emits_runtime_preserve_resize() {
        let source = "Sub Main()\nDim length As Long\nDim buf() As Byte\nlength = 3\nReDim Preserve buf(length - 1)\nEnd Sub";
        assert!(
            compile(source)
                .expect("compile should succeed")
                .instructions
                .iter()
                .any(|i| matches!(
                    i,
                    Instruction::IntrinsicArrayResizePreserve {
                        lower_bounds,
                        element_type: crate::bytecode::RuntimeArrayElementType::Byte,
                        ..
                    } if lower_bounds == &vec![0]
                )),
            "expected runtime preserve array resize instruction for dynamic ReDim Preserve"
        );
    }

    #[test]
    fn compile_option_base_one_array_indexing_subset() {
        let source =
            "Option Base 1\nSub Main()\nDim a(3)\nDim x\na(1) = 4\na(3) = 9\nx = a(3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 9, .. }))
        );
    }

    #[test]
    fn compile_multidim_array_reference_subset() {
        let source = "Sub Main()\nDim m(1 To 2, 1 To 3)\nDim x\nm(2, 3) = 17\nx = m(2, 3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 17, .. }))
        );
    }

    #[test]
    fn compile_redim_preserve_non_last_dimension_change_is_rejected() {
        let source = "Sub Main()\nDim m(1 To 2, 1 To 2)\nReDim Preserve m(1 To 3, 1 To 2)\nEnd Sub";
        let err = compile(source).expect_err("compile should fail");
        assert!(
            err.to_string()
                .contains("redim preserve only supports resizing")
        );
    }

    #[test]
    fn compile_module_const_usage_is_supported() {
        let source = "Const BASE = 5\nSub Main()\nDim x\nx = BASE + 2\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 5, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { value: 2, .. }))
        );
    }

    #[test]
    fn compile_enum_member_usage_is_supported() {
        let source =
            "Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddConstI32 { value: 1, .. }))
                || out
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
    }

    #[test]
    fn compile_udt_declaration_block_is_accepted() {
        let source =
            "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim x\nx = 9\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 9, .. }))
        );
    }

    #[test]
    fn compile_intrinsic_conversion_subset_is_accepted() {
        let source = "Sub Main()\nDim x\nx = CLng(CInt(7))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
    }

    #[test]
    fn compile_len_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = Len(1234)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicLenDigits { .. }))
        );
    }

    #[test]
    fn compile_mid_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = Mid(12345, 2, 2)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMidDigits { .. }))
        );
    }

    #[test]
    fn compile_mid_statement_emits_mutation_instruction() {
        let source = "Sub Main()\nDim x\nx = 12345\nMid(x, 2, 2) = 99\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMidStmtDigits { .. }))
        );
    }

    #[test]
    fn compile_replace_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = Replace(12345, 23, 67)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicReplaceDigits { .. }))
        );
    }

    #[test]
    fn compile_strcomp_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = StrComp(12, 123)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicStrCompDigits { .. }))
        );
    }

    #[test]
    fn compile_instrrev_intrinsic_emits_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = InStrRev(123231, 23)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInStrRevDigits { .. }))
        );
    }

    #[test]
    fn compile_option_compare_text_emits_text_compare_mode_intrinsics() {
        let source = "Option Compare Text\nSub Main()\nDim x\nx = StrComp(12, 12)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicStrCompDigits {
                mode: StringCompareMode::Text,
                ..
            }
        )));
    }

    #[test]
    fn compile_like_condition_emits_like_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nDim y\ny = 12\nIf y Like 12 Then\nx = 1\nEnd If\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicLikeDigits { .. }))
        );
    }

    #[test]
    fn compile_like_value_expr_emits_like_intrinsic_instruction() {
        let source = "Sub Main()\nDim x\nx = \"ABC\" Like \"abc\"\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicLikeDigits { .. }))
        );
    }

    #[test]
    fn compile_dateserial_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = DateSerial(2026, 2, 28)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDateSerialDigits { .. }))
        );
    }

    #[test]
    fn compile_math_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = Abs(-7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicAbsI32 { .. }))
        );
    }

    #[test]
    fn compile_array_introspection_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = UBound(Array(1, 2, 3))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicUBoundArray { .. }))
        );
    }

    #[test]
    fn compile_array_span_expr_preserves_lbound_ubound_and_binary_shape() {
        let source = concat!(
            "Private Sub MeasureBounds(ByRef value() As Byte, ByRef spanValue As Long)\n",
            "    spanValue = UBound(value) - LBound(value) + 1\n",
            "End Sub\n",
            "Sub Main()\n",
            "    Dim buf(2) As Byte\n",
            "    Dim spanValue As Long\n",
            "    MeasureBounds buf, spanValue\n",
            "End Sub",
        );
        let out = compile(source).expect("compile should succeed");
        assert_eq!(
            out.instructions
                .iter()
                .filter(|i| matches!(i, Instruction::IntrinsicLBoundArray { .. }))
                .count(),
            1
        );
        assert_eq!(
            out.instructions
                .iter()
                .filter(|i| matches!(i, Instruction::IntrinsicUBoundArray { .. }))
                .count(),
            1
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SubSlots { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::AddSlots { .. }))
        );
    }

    #[test]
    fn compile_host_sensitive_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = Environ(77)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicEnvironHost { .. }))
        );
    }

    #[test]
    fn compile_time_locale_intrinsics_emit_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = Date()\nb = Time()\nc = Now()\nd = Timer()\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDateNowHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicTimeNowHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicNowHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicTimerHost { .. }))
        );
    }

    #[test]
    fn compile_freefile_intrinsic_emits_host_instruction() {
        let source = "Sub Main()\nDim a\nDim b\na = FreeFile()\nb = FreeFile(1)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicFreeFileHost {
                range_selector: None,
                ..
            }
        )));
        assert!(out.instructions.iter().any(|i| matches!(
            i,
            Instruction::IntrinsicFreeFileHost {
                range_selector: Some(_),
                ..
            }
        )));
    }

    #[test]
    fn compile_file_position_intrinsics_emit_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = EOF(1)\nb = LOF(1)\nc = Seek(1)\nd = Loc(1)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileEofHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileLofHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileSeekHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileLocHost { .. }))
        );
    }

    #[test]
    fn compile_file_write_statement_emits_host_instruction() {
        let source =
            "Sub Main()\nOpen \"x\" For Output As #1\nWrite #1, \"hello,world\"\nClose #1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileWriteHost { .. }))
        );
    }

    #[test]
    fn compile_file_kill_statement_emits_host_instruction() {
        let source = "Sub Main()\nDim path As String\npath = \"x\"\nKill path\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicFileKillHost { .. }))
        );
    }

    #[test]
    fn compile_multi_field_file_write_statement_emits_multiple_host_instructions() {
        let source = "Sub Main()\nOpen \"x\" For Output As #1\nWrite #1, 42, True, \"hello,world\"\nClose #1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::IntrinsicFileWriteHost { .. }))
            .count();
        assert_eq!(count, 3, "expected one host write per Write# field");
    }

    #[test]
    fn compile_console_io_statements_emit_console_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nPrint \"hello\"\nInput a\nLine Input b\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicConsolePrintHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicConsoleInputHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicConsoleLineInputHost { .. }))
        );
    }

    #[test]
    fn compile_beep_statement_emits_host_instruction() {
        let source = "Sub Main()\nBeep\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicBeepHost { .. }))
        );
    }

    #[test]
    fn compile_direct_top_level_mainline_with_option_private_module_succeeds() {
        let source = "Option Private Module\nvalueOut = 41\nCall Bump(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub";
        compile(source)
            .expect("direct top-level mainline with Option Private Module should compile");
    }

    #[test]
    fn compile_direct_top_level_mainline_with_defobj_preserves_module_scope_defaults() {
        let source = "DefObj A-Z\na = 1\n";
        let err =
            compile(source).expect_err("DefObj should still type implicit top-level `a` as Object");
        assert!(err.to_string().contains("type mismatch in assignment"));
    }

    #[test]
    fn compile_direct_top_level_mainline_with_module_const_directive_succeeds() {
        let source =
            "#Const ENABLE = True\n#If ENABLE Then\nvalueOut = 41\n#Else\nvalueOut = 0\n#End If\n";
        compile(source).expect("top-level mainline with module #Const should compile");
    }

    #[test]
    fn compile_direct_top_level_mainline_with_mixed_module_scope_declarations_succeeds() {
        let source = concat!(
            "Option Explicit\n",
            "Option Private Module\n",
            "Rem module comment\n",
            "#Const ENABLE = True\n",
            "DefLng A-Z\n",
            "Public valueOut As Long\n",
            "Public sharedCount As Long\n",
            "Private counter As Long\n",
            "Global totalCount As Long\n",
            "Static stickyCount As Long\n",
            "Private Type CounterState\n",
            "    Value As Long\n",
            "End Type\n",
            "Public Enum CounterMode\n",
            "    CounterModeDefault = 1\n",
            "End Enum\n",
            "counter = 41\n",
            "valueOut = counter\n",
            "Call Bump(valueOut)\n",
            "Public Sub Bump(ByRef value)\n",
            "    value = value + 1\n",
            "End Sub\n",
        );
        compile(source).expect("mixed module declarations plus top-level mainline should compile");
    }

    #[test]
    fn compile_debug_print_emits_diagnostics_host_instruction() {
        let source = "Sub Main()\nDebug.Print \"hello\"\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDebugPrintHost { .. }))
        );
    }

    #[test]
    fn compile_debug_print_multiple_exprs_emits_diagnostics_host_instruction() {
        let source = "Sub Main()\nDebug.Print \"hello\", Err.LastDllError\nEnd Sub";
        let out = compile(source).expect("multi-expr Debug.Print compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDebugPrintHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ConcatSlots { .. })),
            "expected concatenation for multi-expr Debug.Print in {:?}",
            out.instructions
        );
    }

    #[test]
    fn compile_exit_function_is_accepted() {
        let source = "Function F()\nF = 1\nExit Function\nF = 2\nEnd Function\nSub Main()\nDim y\ny = F()\nEnd Sub";
        compile(source).expect("Exit Function compile should succeed");
    }

    #[test]
    fn compile_boolean_literal_emits_bool_const_instruction() {
        let source = "Sub Main()\nDim x\nx = True\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstBool { value: true, .. }))
        );
    }

    #[test]
    fn compile_ui_event_intrinsics_emit_host_instructions() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\na = MsgBox(7, 3)\nb = InputBox(9, 4)\nc = DoEvents()\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicMsgBoxHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInputBoxHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDoEventsHost { .. }))
        );
    }

    #[test]
    fn compile_collection_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = CollectionCount(CollectionAdd(0, 9))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCollectionAdd { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCollectionCount { .. }))
        );
    }

    #[test]
    fn compile_dispatch_intrinsic_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), 2, 3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCreateObjectHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn compile_dispatch_intrinsic_with_array_argument_emits_instruction() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), 6, Array(1, 2, 3))\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::IntrinsicArrayLiteral { values, .. } if values.len() == 3)
        ));
    }

    #[test]
    fn compile_createobject_with_progid_literal_preserves_string_literal() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Scripting.Dictionary\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for ProgID literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicCreateObjectHost { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstString { value, .. } if value == "Scripting.Dictionary"))
        );
    }

    #[test]
    fn compile_createobject_with_oxvba_test_dispatch_literal_preserves_string_literal() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for controlled test ProgID");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstString { value, .. } if value == "OxVba.TestDispatch"))
        );
    }

    #[test]
    fn compile_dispatchinvoke_with_member_literal_maps_to_known_member_token() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"Scripting.Dictionary\"), \"Count\", 0)\nEnd Sub";
        let out = compile(source).expect("compile should succeed for known member literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 1);
    }

    #[test]
    fn compile_dispatchinvoke_with_firechanged_literal_maps_to_member_token_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"FireChanged\", 7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed for controlled event member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 3);
    }

    #[test]
    fn compile_dispatchinvoke_with_property_set_literals_maps_to_member_tokens() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetValue\", 7)\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetValueRef\", 7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed for controlled setter members");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_has_dispatch_member(&out, 7, "SetValue");
        assert_has_dispatch_member(&out, 8, "SetValueRef");
    }

    #[test]
    fn compile_dispatchinvoke_with_long_result_literal_maps_to_member_token_thirty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLong\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 35);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_long_result_literal_maps_to_member_token_thirty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedLong\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 36);
    }

    #[test]
    fn compile_dispatchinvoke_with_byte_result_literal_maps_to_member_token_thirty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByte\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for byte result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 37);
    }

    #[test]
    fn compile_dispatchinvoke_with_signed_byte_result_literal_maps_to_member_token_thirty_nine() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSignedByte\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for signed-byte result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 39);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_int_result_literal_maps_to_member_token_forty_one() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformInt\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for platform-int result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 41);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_uint_result_literal_maps_to_member_token_forty_two() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformUInt\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for platform-uint result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 42);
    }
    #[test]
    fn compile_dispatchinvoke_with_hyper_result_literal_maps_to_member_token_forty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnHyper\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 45);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_hyper_result_literal_maps_to_member_token_forty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedHyper\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 46);
    }
    #[test]
    fn compile_dispatchinvoke_with_double_result_literal_maps_to_member_token_forty_nine() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDouble\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for double result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 49);
    }

    #[test]
    fn compile_dispatchinvoke_with_single_result_literal_maps_to_member_token_fifty_one() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSingle\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for single result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 51);
    }

    #[test]
    fn compile_dispatchinvoke_with_date_result_literal_maps_to_member_token_fifty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDate\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for date result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 53);
    }

    #[test]
    fn compile_dispatchinvoke_with_currency_result_literal_maps_to_member_token_fifty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnCurrency\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for currency result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 55);
    }

    #[test]
    fn compile_dispatchinvoke_with_decimal_result_literal_maps_to_member_token_fifty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDecimal\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for decimal result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 57);
    }

    #[test]
    fn compile_dispatchinvoke_with_bool_result_literal_maps_to_member_token_sixty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnBool\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for bool result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 63);
    }

    #[test]
    fn compile_dispatchinvoke_with_string_result_literal_maps_to_member_token_sixty_four() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnString\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for string result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 64);
    }

    #[test]
    fn compile_dispatchinvoke_with_missing_member_name_result_literal_maps_to_member_token_seventy_six()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnMissingMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for missing-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 76);
    }

    #[test]
    fn compile_dispatchinvoke_with_ping_member_name_result_literal_maps_to_member_token_seventy_seven()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPingMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for ping-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 77);
    }

    #[test]
    fn compile_dispatchinvoke_with_lookup_member_name_result_literal_maps_to_member_token_seventy_eight()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLookupMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for lookup-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 78);
    }

    #[test]
    fn compile_dispatchinvoke_with_sum_pair_member_name_result_literal_maps_to_member_token_seventy_nine()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSumPairMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for sum-pair-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 79);
    }

    #[test]
    fn compile_dispatchinvoke_with_lookup_pair_member_name_result_literal_maps_to_member_token_eighty()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLookupPairMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for lookup-pair-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 80);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_value_member_name_result_literal_maps_to_member_token_eighty_one()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetValueMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for set-value-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 81);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_value_ref_member_name_result_literal_maps_to_member_token_eighty_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetValueRefMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for set-value-ref-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 82);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_indexed_value_member_name_result_literal_maps_to_member_token_eighty_three()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetIndexedValueMemberName\")\nEnd Sub";
        let out = compile(source).expect(
            "compile should succeed for set-indexed-value-member-name result fixture member",
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 83);
    }

    #[test]
    fn compile_dispatchinvoke_with_set_indexed_value_ref_member_name_result_literal_maps_to_member_token_eighty_four()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSetIndexedValueRefMemberName\")\nEnd Sub";
        let out = compile(source).expect(
            "compile should succeed for set-indexed-value-ref-member-name result fixture member",
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 84);
    }

    #[test]
    fn compile_dispatchinvoke_with_value_member_name_result_literal_maps_to_member_token_eighty_five()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnValueMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for value-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 85);
    }

    #[test]
    fn compile_dispatchinvoke_with_default_member_name_result_literal_maps_to_member_token_eighty_six()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDefaultMemberName\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for default-member-name result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 86);
    }

    #[test]
    fn compile_dispatchinvoke_with_empty_result_literal_maps_to_member_token_sixty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnEmpty\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for empty result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 65);
    }

    #[test]
    fn compile_dispatchinvoke_with_null_result_literal_maps_to_member_token_sixty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnNull\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for null result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 66);
    }

    #[test]
    fn compile_dispatchinvoke_with_error_result_literal_maps_to_member_token_sixty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnError\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for error result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 67);
    }

    #[test]
    fn compile_dispatchinvoke_with_byref_long_result_literal_maps_to_member_token_sixty_eight() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByRefLong\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for byref-long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 68);
    }

    #[test]
    fn compile_dispatchinvoke_with_byref_long_array_result_literal_maps_to_member_token_sixty_nine()
    {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByRefLongArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for byref-long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 69);
    }

    #[test]
    fn compile_dispatchinvoke_with_typed_array_result_literal_maps_to_member_token_twenty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSmallIntArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for typed SAFEARRAY fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 20);
    }

    #[test]
    fn compile_dispatchinvoke_with_object_result_literal_maps_to_member_token_twenty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfDispatch\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for object-result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 23);
    }

    #[test]
    fn compile_dispatchinvoke_with_variant_classifier_literal_maps_to_member_token_twenty_five() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ClassifyVariantArg\", 7)\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for variant classifier fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 25);
    }

    #[test]
    fn compile_dispatchinvoke_with_unknown_member_literal_preserves_string_selector() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestEventServer\"), \"IsSelf\", CreateObject(\"OxVba.TestEventServer\"))\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for external string member selector");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert!(
            out.instructions.iter().any(
                |i| matches!(i, Instruction::LoadConstString { value, .. } if value == "IsSelf")
            )
        );
    }

    #[test]
    fn compile_dispatchinvoke_with_external_colliding_member_literal_preserves_string_selector() {
        let source = "Sub Main()\nDim obj\nDim x\nobj = CreateObject(\"OxVba.TestEventServer\")\nx = DispatchInvoke(obj, \"SumPair\", 3, 14)\nEnd Sub";
        let out = compile(source)
            .expect("compile should preserve external colliding string member selector");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert!(out.instructions.iter().any(
            |i| matches!(i, Instruction::LoadConstString { value, .. } if value == "SumPair")
        ));
        assert!(
            !out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 12, .. })),
            "external colliding member literal must not lower to deterministic TestDispatch token"
        );
    }

    #[test]
    fn compile_dispatchinvoke_with_variant_array_classifier_literal_maps_to_member_token_twenty_six()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ClassifyVariantArrayFirstElementArg\", Array(1))\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for variant-array classifier fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 26);
    }

    #[test]
    fn compile_dispatchinvoke_with_dispatch_array_result_literal_maps_to_member_token_twenty_seven()
    {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfDispatchArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for dispatch-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 27);
    }

    #[test]
    fn compile_dispatchinvoke_with_typed_dispatch_array_result_literal_maps_to_member_token_twenty_eight()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfTypedDispatchArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for typed dispatch-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 28);
    }

    #[test]
    fn compile_dispatchinvoke_with_typed_unknown_array_result_literal_maps_to_member_token_twenty_nine()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSelfTypedUnknownArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for typed unknown-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 29);
    }

    #[test]
    fn compile_dispatchinvoke_with_plain_unknown_result_literal_maps_to_member_token_thirty_one() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlainUnknown\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for plain-unknown result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 31);
    }
    #[test]
    fn compile_dispatchinvoke_with_plain_unknown_array_result_literal_maps_to_member_token_thirty_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlainUnknownArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for plain-unknown-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 32);
    }
    #[test]
    fn compile_dispatchinvoke_with_long_array_result_literal_maps_to_member_token_thirty_three() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnLongArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 33);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_long_array_result_literal_maps_to_member_token_thirty_four()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedLongArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 34);
    }

    #[test]
    fn compile_dispatchinvoke_with_byte_array_result_literal_maps_to_member_token_thirty_eight() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnByteArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for byte-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 38);
    }

    #[test]
    fn compile_dispatchinvoke_with_signed_byte_array_result_literal_maps_to_member_token_forty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSignedByteArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for signed-byte-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 40);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_int_array_result_literal_maps_to_member_token_forty_three()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformIntArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for platform-int-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 43);
    }

    #[test]
    fn compile_dispatchinvoke_with_platform_uint_array_result_literal_maps_to_member_token_forty_four()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlatformUIntArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for platform-uint-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 44);
    }
    #[test]
    fn compile_dispatchinvoke_with_hyper_array_result_literal_maps_to_member_token_forty_seven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnHyperArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 47);
    }

    #[test]
    fn compile_dispatchinvoke_with_unsigned_hyper_array_result_literal_maps_to_member_token_forty_eight()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnUnsignedHyperArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for unsigned-hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 48);
    }
    #[test]
    fn compile_dispatchinvoke_with_double_array_result_literal_maps_to_member_token_fifty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDoubleArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for double-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 50);
    }

    #[test]
    fn compile_dispatchinvoke_with_single_array_result_literal_maps_to_member_token_fifty_two() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSingleArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for single-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 52);
    }

    #[test]
    fn compile_dispatchinvoke_with_date_array_result_literal_maps_to_member_token_fifty_four() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDateArray\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for date-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 54);
    }

    #[test]
    fn compile_dispatchinvoke_with_currency_array_result_literal_maps_to_member_token_fifty_six() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnCurrencyArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for currency-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 56);
    }

    #[test]
    fn compile_dispatchinvoke_with_decimal_array_result_literal_maps_to_member_token_fifty_eight() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnDecimalArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for decimal-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 58);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_long_result_literal_maps_to_member_token_fifty_nine()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedLong\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-long result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 59);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_long_array_result_literal_maps_to_member_token_sixty()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedLongArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-long-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 60);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_platform_uint_result_literal_maps_to_member_token_sixty_one()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWidePlatformUInt\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide platform-uint result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 61);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_platform_uint_array_result_literal_maps_to_member_token_sixty_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWidePlatformUIntArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide platform-uint-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 62);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_hyper_result_literal_maps_to_member_token_seventy() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideHyper\")\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for wide hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 70);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_hyper_array_result_literal_maps_to_member_token_seventy_one()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideHyperArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 71);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_hyper_result_literal_maps_to_member_token_seventy_two()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedHyper\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-hyper result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 72);
    }

    #[test]
    fn compile_dispatchinvoke_with_wide_unsigned_hyper_array_result_literal_maps_to_member_token_seventy_three()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnWideUnsignedHyperArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for wide unsigned-hyper-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 73);
    }

    #[test]
    fn compile_dispatchinvoke_with_smallint_matrix_result_literal_maps_to_member_token_thirty() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnSmallIntMatrix\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for smallint-matrix result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 30);
    }

    #[test]
    fn compile_dispatchinvoke_with_variant_matrix_result_literal_maps_to_member_token_seventy_four()
    {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnVariantMatrix\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for variant-matrix result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 74);
    }

    #[test]
    fn compile_dispatchinvoke_with_plain_unknown_variant_array_result_literal_maps_to_member_token_seventy_five()
     {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"ReturnPlainUnknownVariantArray\")\nEnd Sub";
        let out = compile(source)
            .expect("compile should succeed for plain-unknown-variant-array result fixture member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 75);
    }

    #[test]
    fn compile_dispatchinvoke_with_source_interface_event_literal_maps_to_member_token_eleven() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"FireChangedSourceInterface\", 7)\nEnd Sub";
        let out =
            compile(source).expect("compile should succeed for source-interface trigger member");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 11);
    }

    #[test]
    fn compile_dispatchinvoke_with_quit_literal_maps_to_member_token_ten() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"Quit\")\nEnd Sub";
        let out = compile(source).expect("compile should succeed for quit member literal");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
        assert_dispatchinvoke_source_member(&out, source, 10);
    }

    #[test]
    fn compile_dispatchinvoke_accepts_two_arg_property_get_form() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"Scripting.Dictionary\"), \"Count\")\nEnd Sub";
        let out = compile(source).expect("two-arg DispatchInvoke should compile");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn compile_dispatchinvoke_accepts_multi_arg_form() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SumPair\", 3, 14)\nEnd Sub";
        let out = compile(source).expect("multi-arg DispatchInvoke should compile");
        let invoke = out
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { args, .. } => Some(args.clone()),
                _ => None,
            })
            .expect("dispatch invoke instruction should be present");
        assert_eq!(invoke.len(), 2);
        assert!(invoke.iter().all(|arg| arg.name.is_none()));
    }

    #[test]
    fn compile_dispatchinvoke_accepts_named_args_in_assignment_form() {
        let source = "Sub Main()\nDim x\nx = DispatchInvoke(CreateObject(\"OxVba.TestDispatch\"), \"SetIndexedValue\", value := 11, lhs := 7)\nEnd Sub";
        let out = compile(source).expect("named DispatchInvoke assignment form should compile");
        let invoke = out
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::IntrinsicDispatchInvokeHost { args, .. } => Some(args.clone()),
                _ => None,
            })
            .expect("dispatch invoke instruction should be present");
        assert_eq!(invoke.len(), 2);
        assert_eq!(invoke[0].name.as_deref(), Some("value"));
        assert_eq!(invoke[1].name.as_deref(), Some("lhs"));
    }

    #[test]
    fn compile_createobject_with_unknown_progid_literal_is_allowed() {
        let source = "Sub Main()\nDim x\nx = CreateObject(\"Unknown.Component\")\nEnd Sub";
        let out = compile(source).expect("unknown ProgID literal should remain a runtime concern");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstString { value, .. } if value == "Unknown.Component"))
        );
    }

    #[test]
    fn compile_err_raise_statement_is_supported() {
        let source = "Sub Main()\nOn Error Resume Next\nErr.Raise 7\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::RaiseError { code: 7 }))
        );
    }

    #[test]
    fn compile_property_let_assignment_routes_to_call() {
        let source = "Sub Main()\nDim x\nx = 1\nValue = x\nEnd Sub\nProperty Let Value(ByRef target)\ntarget = target + 2\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_property_set_assignment_routes_to_call() {
        let source = "Sub Main()\nDim x\nx = 2\nSet Obj = x\nEnd Sub\nProperty Set Obj(ByRef target)\ntarget = target + 5\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_property_get_procedure_is_accepted() {
        let source =
            "Sub Main()\nDim x\nx = 4\nEnd Sub\nProperty Get Value()\nDim y\ny = 1\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 4, .. }))
        );
    }

    #[test]
    fn compile_on_error_resume_next_emits_error_state_ops() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetOnErrorResumeNext))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::RaiseError { code: 5 }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrNumber { .. }))
        );
    }

    #[test]
    fn compile_err_member_aliases_are_accepted_under_option_explicit() {
        let source = "Option Explicit\nSub Main()\nDim a\nDim b\nDim c\na = Err.Description\nb = Err.Source\nc = Err.HelpContext\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrDescription { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrSource { .. }))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 0, .. }))
        );
    }

    #[test]
    fn compile_on_error_goto_zero_and_resume_next_emits_ops() {
        let source =
            "Sub Main()\nOn Error Resume Next\nOn Error GoTo 0\nResume Next\nError 3\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetOnErrorGoto0))
        );
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::ResumeNext))
        );
    }

    #[test]
    fn compile_procedure_boundaries_insert_clearerr_guards() {
        let source = "Sub Main()\nCall Worker\nEnd Sub\nSub Worker()\nDim x\nx = 1\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let clear_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::ClearErr))
            .count();
        assert!(
            clear_count >= 4,
            "expected at least main/worker entry+exit clear guards, found {clear_count}"
        );
    }

    #[test]
    fn compile_for_each_subset_is_accepted() {
        let source = "Sub Main()\nDim x\nDim v\nFor Each v In Array(1, 2, 3)\nx = v\nNext\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 3, .. }))
        );
    }

    #[test]
    fn compile_line_number_statement_form_is_supported() {
        let source = "Sub Main()\nDim x\nGoTo 200\n100 x = 1\n200 x = 5\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
    }

    #[test]
    fn compile_property_get_read_routes_to_assign_from_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = Value\nEnd Sub\nProperty Get Value()\nValue = 9\nEnd Property";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
    }

    #[test]
    fn compile_udt_field_access_subset_is_accepted() {
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim p As Point\nDim x\np.X = 7\np.Y = p.X\nx = p.Y\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadConstI32 { value: 7, .. }))
        );
    }

    #[test]
    fn compile_udt_whole_assignment_emits_field_copy_slots() {
        let source = "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim a As Point\nDim b As Point\na.X = 7\na.Y = 9\nb = a\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        let copy_count = out
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::CopySlot { .. }))
            .count();
        assert!(
            copy_count >= 2,
            "expected field copy lowering for UDT assignment"
        );
    }

    #[test]
    fn compile_late_bound_assignment_emits_dispatchinvoke_subset() {
        let source = "Sub Main()\nDim obj As Object\nDim x\nx = obj(7)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicDispatchInvokeHost { .. }))
        );
    }

    #[test]
    fn compile_declare_function_stub_binding_subset_is_accepted() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInvokeSymbolHost { .. }))
        );
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert_eq!(descriptor.declared_name.to_ascii_lowercase(), "hostping");
        assert_eq!(descriptor.library, "host");
        assert_eq!(descriptor.alias, "ping");
        assert_eq!(descriptor.marshal_lane, "m0-deterministic");
        assert_eq!(descriptor.calling_convention, "platform-default");
        assert_eq!(descriptor.selection_policy, "case-insensitive-canonical");
    }

    #[test]
    fn compile_private_declare_function_stub_binding_subset_is_accepted() {
        let source = "Private Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("private declare compile should succeed");
        assert!(
            out.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicInvokeSymbolHost { .. }))
        );
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert_eq!(descriptor.declared_name.to_ascii_lowercase(), "hostping");
        assert_eq!(descriptor.library, "host");
        assert_eq!(descriptor.alias, "ping");
    }

    #[test]
    fn compile_declare_descriptor_table_is_stable_for_identical_source() {
        let source = "Declare PtrSafe Function HostPing Lib \"HOST\" Alias \"PiNg\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out1 = compile(source).expect("first compile should succeed");
        let out2 = compile(source).expect("second compile should succeed");
        assert_eq!(
            out1.external_call_descriptors,
            out2.external_call_descriptors
        );
    }

    #[test]
    fn compile_declare_without_alias_preserves_source_symbol_case() {
        let source = "Declare PtrSafe Function MultiByteToWideChar Lib \"kernel32\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub";
        let out = compile(source).expect("declare without alias should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert_eq!(
            descriptor.declared_name.to_ascii_lowercase(),
            "multibytetowidechar"
        );
        assert_eq!(descriptor.alias, "MultiByteToWideChar");
    }

    #[test]
    fn compile_same_module_statement_call_without_parentheses_and_without_args_succeeds() {
        let source = "Public Sub Main()\nTestVersion\nEnd Sub\nPublic Sub TestVersion()\nEnd Sub";
        compile(source).expect("same-module no-paren no-arg call should compile");
    }

    #[test]
    fn compile_declare_without_ptrsafe_is_rejected() {
        let source = "Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("non-ptrsafe declare should be rejected");
        assert!(err.to_string().contains("PtrSafe keyword is required"));
    }

    #[test]
    fn compile_declare_with_invalid_ordinal_alias_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"#12a\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source).expect_err("invalid ordinal alias should be rejected");
        assert!(err.to_string().contains("ordinal alias"));
    }

    #[test]
    fn compile_declare_with_ordinal_alias_uses_ordinal_selection_policy() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"#0007\" (ByVal x As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("ordinal alias subset declaration should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        let descriptor = &out.external_call_descriptors[0];
        assert!(descriptor.ordinal_alias);
        assert_eq!(descriptor.alias, "#7");
        assert_eq!(descriptor.selection_policy, "ordinal-literal-canonical");
    }

    #[test]
    fn compile_declare_with_multiple_arguments_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long, ByVal y As Long) As Long\nSub Main()\nDim z\nz = HostPing(3, 4)\nEnd Sub";
        let out = compile(source).expect("multi-arg declare should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        assert_eq!(out.external_call_descriptors[0].param_count, 2);
    }

    #[test]
    fn compile_declare_with_non_long_parameter_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As String) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("non-Long declare param should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
    }

    #[test]
    fn compile_declare_with_variant_parameter_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Variant) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("Variant declare param should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
    }

    #[test]
    fn compile_declare_with_array_parameter_is_rejected() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x() As Long) As Long\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let err = compile(source)
            .expect_err("array declare param should be rejected in dynamic-link subset");
        let rendered = err.to_string().to_ascii_lowercase();
        assert!(
            rendered.contains("declare")
                || rendered.contains("external")
                || rendered.contains("array"),
            "unexpected declare-array rejection: {rendered}"
        );
    }

    #[test]
    fn compile_declare_with_non_long_return_succeeds() {
        let source = "Declare PtrSafe Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As String\nSub Main()\nDim y\ny = HostPing(3)\nEnd Sub";
        let out = compile(source).expect("non-Long declare return should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn compile_native_declare_uses_native_ffi_lane_on_native_targets() {
        let source = "Declare PtrSafe Function LstrlenW Lib \"kernel32\" Alias \"lstrlenW\" (ByVal lpString As LongPtr) As Long\nSub Main()\nDim y\ny = LstrlenW(0)\nEnd Sub";
        let out = compile(source).expect("native declare should compile");
        assert_eq!(out.external_call_descriptors.len(), 1);
        assert_eq!(
            out.external_call_descriptors[0].marshal_lane,
            "m1-native-ffi"
        );
    }

    #[test]
    fn compile_withevents_declaration_in_single_module_subset_succeeds() {
        let source = "Sub Main()\nDim WithEvents app As Object\nEnd Sub";
        compile(source).expect("WithEvents declaration should compile in deterministic subset");
    }

    #[test]
    fn compile_implements_directive_in_single_module_subset_succeeds() {
        let source = "Implements IFoo\nSub Main()\nEnd Sub";
        compile(source).expect("Implements directive should compile in deterministic subset");
    }

    #[test]
    fn compile_raiseevent_statement_in_single_module_subset_succeeds() {
        let source = "Sub Main()\nRaiseEvent Tick\nEnd Sub";
        compile(source).expect("RaiseEvent statement should compile in deterministic subset");
    }

    #[test]
    fn compile_withevents_runtime_binding_intrinsics_emit_deterministically() {
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_set(0, 7, 42)\nIf __oxvba_withevents_get(0, 7) = 42 Then\nx = 1\nEnd If\nEnd Sub";
        let out = compile(source).expect("WithEvents binding intrinsics should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsSet { .. })),
            "expected IntrinsicWithEventsSet emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsGet { .. })),
            "expected IntrinsicWithEventsGet emission"
        );
    }

    #[test]
    fn compile_withevents_clear_owner_intrinsic_emits_deterministically() {
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_clear_owner(11)\nEnd Sub";
        let out = compile(source).expect("WithEvents clear-owner intrinsic should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsClearOwner { .. })),
            "expected IntrinsicWithEventsClearOwner emission"
        );
    }

    #[test]
    fn compile_withevents_owner_iteration_intrinsics_emit_deterministically() {
        let source = "Sub Main()\nDim x\nx = __oxvba_withevents_first_owner(1, 7)\nx = __oxvba_withevents_next_owner()\nEnd Sub";
        let out = compile(source).expect("WithEvents owner iteration intrinsics should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsFirstOwner { .. })),
            "expected IntrinsicWithEventsFirstOwner emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicWithEventsNextOwner { .. })),
            "expected IntrinsicWithEventsNextOwner emission"
        );
    }

    #[test]
    fn compile_com_event_subscription_intrinsics_emit_deterministically() {
        let source = "Sub Main()\nDim x\nDim y\nDim z\nDim w\nx = __oxvba_com_subscribe_event(20001, 1)\ny = __oxvba_com_callback_subscription(60001)\nz = __oxvba_com_callback_arg(60001, 0)\nw = __oxvba_com_release_callback(60001)\ny = __oxvba_com_unsubscribe_event(x)\nEnd Sub";
        let out = compile(source).expect("COM event subscription intrinsics should compile");
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicComSubscribeEventHost { .. })),
            "expected IntrinsicComSubscribeEventHost emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicComUnsubscribeEventHost { .. })),
            "expected IntrinsicComUnsubscribeEventHost emission"
        );
        assert!(
            out.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::IntrinsicComEventCallbackSubscriptionHost { .. }
            )),
            "expected IntrinsicComEventCallbackSubscriptionHost emission"
        );
        assert!(
            out.instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::IntrinsicComEventCallbackArgHost { .. })),
            "expected IntrinsicComEventCallbackArgHost emission"
        );
        assert!(
            out.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::IntrinsicComReleaseEventCallbackHost { .. }
            )),
            "expected IntrinsicComReleaseEventCallbackHost emission"
        );
    }
}
