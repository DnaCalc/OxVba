use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{Bytecode, compile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Source,
    Host,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Procedural,
    Class,
    Document,
    Form,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Project,
    TypeLibrary,
    HostInjected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExportKind {
    Sub,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleAttributes {
    pub vb_name: String,
    pub vb_global_namespace: bool,
    pub vb_creatable: bool,
    pub vb_predeclared_id: bool,
    pub vb_exposed: bool,
    pub option_private_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleUnit {
    pub module_name: String,
    pub module_kind: ModuleKind,
    pub attributes: ModuleAttributes,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReference {
    pub referenced_project_name: String,
    pub reference_kind: ReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedProjectManifest {
    pub project_name: String,
    pub modules: Vec<ModuleUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifest {
    pub project_name: String,
    pub project_kind: ProjectKind,
    pub modules: Vec<ModuleUnit>,
    pub references: Vec<ProjectReference>,
    pub reference_projects: Vec<ReferencedProjectManifest>,
    pub conditional_constants: BTreeMap<String, i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProcedureExport {
    pub project_name: String,
    pub module_name: String,
    pub procedure_name: String,
    pub kind: ExportKind,
}

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub bytecode: Bytecode,
    pub rewritten_source: String,
    pub host_exports: Vec<HostProcedureExport>,
    pub reference_visible_exports: Vec<HostProcedureExport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectLoweringStrategy {
    ModuleAwareBindPlan,
    RewriteBridge,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProjectCompileError {
    #[error("PMR-E-PROJECT-NAME-INVALID: project name `{name}` is not a valid VBA identifier")]
    ProjectNameInvalid { name: String },
    #[error("PMR-E-PROJECT-NAME-DUPLICATE: duplicate project name `{name}`")]
    ProjectNameDuplicate { name: String },
    #[error("PMR-E-MODULE-NAME-INVALID: module name `{name}` is not a valid VBA identifier")]
    ModuleNameInvalid { name: String },
    #[error("PMR-E-MODULE-NAME-DUPLICATE: duplicate module name `{name}`")]
    ModuleNameDuplicate { name: String },
    #[error("PMR-E-MODULE-NAME-LENGTH: module name `{name}` exceeds 31 characters")]
    ModuleNameLength { name: String },
    #[error(
        "PMR-E-MODULE-HEADER-VB-NAME: module `{module_name}` has mismatched VB_Name `{vb_name}`"
    )]
    ModuleHeaderVbNameMismatch {
        module_name: String,
        vb_name: String,
    },
    #[error(
        "PMR-E-MODULE-HEADER-INVALID: module `{module_name}` has malformed header line `{line}`"
    )]
    ModuleHeaderInvalid { module_name: String, line: String },
    #[error(
        "PMR-E-MODULE-CLASS-ATTRIBUTE: source project class modules require VB_GlobalNamespace=False and VB_Creatable=False"
    )]
    SourceProjectClassAttributeConstraint,
    #[error(
        "PMR-E-OPTION-PRIVATE-MODULE-KIND: Option Private Module is only valid for procedural modules (`{module_name}`)"
    )]
    OptionPrivateModuleKind { module_name: String },
    #[error(
        "PMR-E-REFERENCE-NAME-INVALID: referenced project name `{name}` is not a valid VBA identifier"
    )]
    ReferenceNameInvalid { name: String },
    #[error("PMR-E-REFERENCE-DUPLICATE-TARGET: duplicate referenced project `{name}`")]
    ReferenceDuplicateTarget { name: String },
    #[error(
        "PMR-E-REFERENCE-PROJECT-NOT-DECLARED: reference project source `{name}` is present but not declared in references"
    )]
    ReferenceProjectNotDeclared { name: String },
    #[error("PMR-E-REFERENCE-PROJECT-DUPLICATE: duplicate reference project source `{name}`")]
    ReferenceProjectDuplicate { name: String },
    #[error(
        "PMR-E-REFERENCE-PROJECT-NOT-LOADED: referenced project `{name}` is declared but source was not provided"
    )]
    ReferenceProjectNotLoaded { name: String },
    #[error(
        "PMR-E-NAME-QUALIFICATION-REQUIRED: procedure name `{name}` is declared in multiple modules"
    )]
    NameQualificationRequired { name: String },
    #[error("PMR-E-NAME-RESOLUTION-NOT-FOUND: qualified call target `{name}` was not found")]
    NameResolutionNotFound { name: String },
    #[error("PMR-E-NAME-RESOLUTION-AMBIGUOUS: qualified call target `{name}` is ambiguous")]
    NameResolutionAmbiguous { name: String },
    #[error(
        "PMR-E-PROJECT-QUALIFICATION-INVALID: call target `{name}` uses unknown project qualifier"
    )]
    ProjectQualificationInvalid { name: String },
    #[error(
        "PMR-E-REFERENCE-CROSS-PROJECT-UNSUPPORTED: referenced-project call target `{name}` is not executable in current subset"
    )]
    CrossProjectReferenceUnsupported { name: String },
    #[error(
        "BIND-E-TYPELIB-QUALIFIER-UNRESOLVED: external type `{type_name}` uses unknown typelib qualifier `{qualifier}`"
    )]
    TypeLibraryQualifierUnresolved {
        type_name: String,
        qualifier: String,
    },
    #[error(
        "BIND-E-TYPELIB-CREATEOBJECT-UNSUPPORTED: `As New` external type `{type_name}` has no deterministic CreateObject selector mapping"
    )]
    TypeLibraryCreateObjectUnsupported { type_name: String },
    #[error(
        "BIND-E-TYPELIB-MEMBER-UNSUPPORTED: external member `{member_name}` is outside the current deterministic early-bind subset"
    )]
    TypeLibraryMemberUnsupported { member_name: String },
    #[error(
        "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED: external invoke target `{target}` exceeds current supported arity subset (0 or 1 args)"
    )]
    TypeLibraryInvokeArityUnsupported { target: String },
    #[error("PMR-E-BACKEND-COMPILE: {message}")]
    BackendCompile { message: String },
}

impl ProjectCompileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectNameInvalid { .. } => "PMR-E-PROJECT-NAME-INVALID",
            Self::ProjectNameDuplicate { .. } => "PMR-E-PROJECT-NAME-DUPLICATE",
            Self::ModuleNameInvalid { .. } => "PMR-E-MODULE-NAME-INVALID",
            Self::ModuleNameDuplicate { .. } => "PMR-E-MODULE-NAME-DUPLICATE",
            Self::ModuleNameLength { .. } => "PMR-E-MODULE-NAME-LENGTH",
            Self::ModuleHeaderVbNameMismatch { .. } => "PMR-E-MODULE-HEADER-VB-NAME",
            Self::ModuleHeaderInvalid { .. } => "PMR-E-MODULE-HEADER-INVALID",
            Self::SourceProjectClassAttributeConstraint => "PMR-E-MODULE-CLASS-ATTRIBUTE",
            Self::OptionPrivateModuleKind { .. } => "PMR-E-OPTION-PRIVATE-MODULE-KIND",
            Self::ReferenceNameInvalid { .. } => "PMR-E-REFERENCE-NAME-INVALID",
            Self::ReferenceDuplicateTarget { .. } => "PMR-E-REFERENCE-DUPLICATE-TARGET",
            Self::ReferenceProjectNotDeclared { .. } => "PMR-E-REFERENCE-PROJECT-NOT-DECLARED",
            Self::ReferenceProjectDuplicate { .. } => "PMR-E-REFERENCE-PROJECT-DUPLICATE",
            Self::ReferenceProjectNotLoaded { .. } => "PMR-E-REFERENCE-PROJECT-NOT-LOADED",
            Self::NameQualificationRequired { .. } => "PMR-E-NAME-QUALIFICATION-REQUIRED",
            Self::NameResolutionNotFound { .. } => "PMR-E-NAME-RESOLUTION-NOT-FOUND",
            Self::NameResolutionAmbiguous { .. } => "PMR-E-NAME-RESOLUTION-AMBIGUOUS",
            Self::ProjectQualificationInvalid { .. } => "PMR-E-PROJECT-QUALIFICATION-INVALID",
            Self::CrossProjectReferenceUnsupported { .. } => {
                "PMR-E-REFERENCE-CROSS-PROJECT-UNSUPPORTED"
            }
            Self::TypeLibraryQualifierUnresolved { .. } => "BIND-E-TYPELIB-QUALIFIER-UNRESOLVED",
            Self::TypeLibraryCreateObjectUnsupported { .. } => {
                "BIND-E-TYPELIB-CREATEOBJECT-UNSUPPORTED"
            }
            Self::TypeLibraryMemberUnsupported { .. } => "BIND-E-TYPELIB-MEMBER-UNSUPPORTED",
            Self::TypeLibraryInvokeArityUnsupported { .. } => {
                "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"
            }
            Self::BackendCompile { .. } => "PMR-E-BACKEND-COMPILE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcedureDecl {
    project_name: String,
    module_name: String,
    procedure_name: String,
    lowered_name: String,
    kind: ExportKind,
    is_public: bool,
    module_kind: ModuleKind,
    option_private_module: bool,
}

pub fn module_unit_from_source(
    module_name: impl Into<String>,
    module_kind: ModuleKind,
    source: impl Into<String>,
) -> Result<ModuleUnit, ProjectCompileError> {
    let module_name = module_name.into();
    let source = source.into();
    let mut attrs = ModuleAttributes {
        vb_name: module_name.clone(),
        ..ModuleAttributes::default()
    };
    for line in source.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("attribute ") {
            parse_attribute_line(trimmed, &mut attrs, &module_name)?;
        } else if lower == "option private module" {
            attrs.option_private_module = true;
        }
    }

    if !attrs.vb_name.eq_ignore_ascii_case(&module_name) {
        return Err(ProjectCompileError::ModuleHeaderVbNameMismatch {
            module_name,
            vb_name: attrs.vb_name,
        });
    }

    Ok(ModuleUnit {
        module_name,
        module_kind,
        attributes: attrs,
        source,
    })
}

pub fn compile_project(manifest: &ProjectManifest) -> Result<CompiledProject, ProjectCompileError> {
    compile_project_with_strategy(manifest, selected_project_lowering_strategy())
}

fn selected_project_lowering_strategy() -> ProjectLoweringStrategy {
    match std::env::var("OXVBA_PMR_LOWERING")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("rewrite-bridge") => ProjectLoweringStrategy::RewriteBridge,
        _ => ProjectLoweringStrategy::ModuleAwareBindPlan,
    }
}

fn compile_project_with_strategy(
    manifest: &ProjectManifest,
    strategy: ProjectLoweringStrategy,
) -> Result<CompiledProject, ProjectCompileError> {
    validate_manifest(manifest)?;
    let procedure_index = collect_project_procedures(manifest);
    let reference_order = build_reference_order_map(manifest);
    let active_project = normalize_identifier(&manifest.project_name);

    let rewritten_source = lower_project_source(
        strategy,
        manifest,
        &active_project,
        &procedure_index,
        &reference_order,
    )?;

    let bytecode = compile(&rewritten_source).map_err(|e| ProjectCompileError::BackendCompile {
        message: e.to_string(),
    })?;

    let host_exports = collect_host_exports(manifest, &procedure_index);
    let reference_visible_exports = collect_reference_visible_exports(manifest, &procedure_index);
    validate_compiled_project_contract(manifest, &host_exports, &reference_visible_exports)
        .map_err(|message| ProjectCompileError::BackendCompile {
            message: format!("PMR-E-INTERNAL-CONTRACT: {message}"),
        })?;
    Ok(CompiledProject {
        bytecode,
        rewritten_source,
        host_exports,
        reference_visible_exports,
    })
}

fn lower_project_source(
    strategy: ProjectLoweringStrategy,
    manifest: &ProjectManifest,
    active_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<String, ProjectCompileError> {
    let mut lowered_modules = Vec::new();
    for module in &manifest.modules {
        let lowered = lower_module_source(
            strategy,
            manifest,
            active_project,
            module,
            active_project,
            procedures,
            reference_order,
        )?;
        lowered_modules.push(lowered);
    }
    for referenced in ordered_reference_projects(manifest) {
        let project_name = normalize_identifier(&referenced.project_name);
        for module in &referenced.modules {
            let lowered = lower_module_source(
                strategy,
                manifest,
                active_project,
                module,
                &project_name,
                procedures,
                reference_order,
            )?;
            lowered_modules.push(lowered);
        }
    }
    Ok(lowered_modules.join("\n"))
}

#[allow(clippy::too_many_arguments)]
fn lower_module_source(
    strategy: ProjectLoweringStrategy,
    manifest: &ProjectManifest,
    active_project: &str,
    module: &ModuleUnit,
    current_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<String, ProjectCompileError> {
    match strategy {
        ProjectLoweringStrategy::ModuleAwareBindPlan => lower_module_source_module_aware(
            manifest,
            active_project,
            module,
            current_project,
            procedures,
            reference_order,
        ),
        ProjectLoweringStrategy::RewriteBridge => rewrite_module_source(
            manifest,
            active_project,
            module,
            current_project,
            procedures,
            reference_order,
        ),
    }
}

fn validate_manifest(manifest: &ProjectManifest) -> Result<(), ProjectCompileError> {
    if !is_valid_vba_identifier(&manifest.project_name) {
        return Err(ProjectCompileError::ProjectNameInvalid {
            name: manifest.project_name.clone(),
        });
    }
    let mut module_names = BTreeSet::new();
    for module in &manifest.modules {
        if !is_valid_vba_identifier(&module.module_name) {
            return Err(ProjectCompileError::ModuleNameInvalid {
                name: module.module_name.clone(),
            });
        }
        if module.module_name.chars().count() > 31 {
            return Err(ProjectCompileError::ModuleNameLength {
                name: module.module_name.clone(),
            });
        }
        if !module
            .attributes
            .vb_name
            .eq_ignore_ascii_case(&module.module_name)
        {
            return Err(ProjectCompileError::ModuleHeaderVbNameMismatch {
                module_name: module.module_name.clone(),
                vb_name: module.attributes.vb_name.clone(),
            });
        }
        if manifest.project_kind == ProjectKind::Source
            && module.module_kind == ModuleKind::Class
            && (module.attributes.vb_global_namespace || module.attributes.vb_creatable)
        {
            return Err(ProjectCompileError::SourceProjectClassAttributeConstraint);
        }
        if module.attributes.option_private_module && module.module_kind != ModuleKind::Procedural {
            return Err(ProjectCompileError::OptionPrivateModuleKind {
                module_name: module.module_name.clone(),
            });
        }
        let key = normalize_identifier(&module.module_name);
        if !module_names.insert(key) {
            return Err(ProjectCompileError::ModuleNameDuplicate {
                name: module.module_name.clone(),
            });
        }
    }

    let mut refs = BTreeSet::new();
    for reference in &manifest.references {
        if !is_valid_vba_identifier(&reference.referenced_project_name) {
            return Err(ProjectCompileError::ReferenceNameInvalid {
                name: reference.referenced_project_name.clone(),
            });
        }
        let key = normalize_identifier(&reference.referenced_project_name);
        if !refs.insert(key) {
            return Err(ProjectCompileError::ReferenceDuplicateTarget {
                name: reference.referenced_project_name.clone(),
            });
        }
    }

    let mut ref_projects = BTreeSet::new();
    for referenced in &manifest.reference_projects {
        if !is_valid_vba_identifier(&referenced.project_name) {
            return Err(ProjectCompileError::ReferenceNameInvalid {
                name: referenced.project_name.clone(),
            });
        }
        let key = normalize_identifier(&referenced.project_name);
        if !refs.contains(&key) {
            return Err(ProjectCompileError::ReferenceProjectNotDeclared {
                name: referenced.project_name.clone(),
            });
        }
        if !ref_projects.insert(key) {
            return Err(ProjectCompileError::ReferenceProjectDuplicate {
                name: referenced.project_name.clone(),
            });
        }
        validate_modules_for_project(
            &referenced.project_name,
            ProjectKind::Library,
            &referenced.modules,
        )?;
    }

    Ok(())
}

fn collect_project_procedures(manifest: &ProjectManifest) -> Vec<ProcedureDecl> {
    let mut procedures = Vec::new();
    let active_project = normalize_identifier(&manifest.project_name);
    for module in &manifest.modules {
        let module_name = normalize_identifier(&module.module_name);
        for line in module.source.lines() {
            if let Some((name, kind, is_public)) = parse_procedure_signature_line(line) {
                let lowered_name = lowered_proc_symbol(&active_project, &module_name, &name);
                procedures.push(ProcedureDecl {
                    project_name: active_project.clone(),
                    module_name: module_name.clone(),
                    procedure_name: name,
                    lowered_name,
                    kind,
                    is_public,
                    module_kind: module.module_kind,
                    option_private_module: module.attributes.option_private_module,
                });
            }
        }
    }
    for referenced in &manifest.reference_projects {
        let project_name = normalize_identifier(&referenced.project_name);
        for module in &referenced.modules {
            let module_name = normalize_identifier(&module.module_name);
            for line in module.source.lines() {
                if let Some((name, kind, is_public)) = parse_procedure_signature_line(line) {
                    let lowered_name = lowered_proc_symbol(&project_name, &module_name, &name);
                    procedures.push(ProcedureDecl {
                        project_name: project_name.clone(),
                        module_name: module_name.clone(),
                        procedure_name: name,
                        lowered_name,
                        kind,
                        is_public,
                        module_kind: module.module_kind,
                        option_private_module: module.attributes.option_private_module,
                    });
                }
            }
        }
    }
    procedures
}

fn validate_modules_for_project(
    project_name: &str,
    project_kind: ProjectKind,
    modules: &[ModuleUnit],
) -> Result<(), ProjectCompileError> {
    let mut module_names = BTreeSet::new();
    for module in modules {
        if !is_valid_vba_identifier(&module.module_name) {
            return Err(ProjectCompileError::ModuleNameInvalid {
                name: module.module_name.clone(),
            });
        }
        if module.module_name.chars().count() > 31 {
            return Err(ProjectCompileError::ModuleNameLength {
                name: module.module_name.clone(),
            });
        }
        if !module
            .attributes
            .vb_name
            .eq_ignore_ascii_case(&module.module_name)
        {
            return Err(ProjectCompileError::ModuleHeaderVbNameMismatch {
                module_name: module.module_name.clone(),
                vb_name: module.attributes.vb_name.clone(),
            });
        }
        if project_kind == ProjectKind::Source
            && module.module_kind == ModuleKind::Class
            && (module.attributes.vb_global_namespace || module.attributes.vb_creatable)
        {
            return Err(ProjectCompileError::SourceProjectClassAttributeConstraint);
        }
        if module.attributes.option_private_module && module.module_kind != ModuleKind::Procedural {
            return Err(ProjectCompileError::OptionPrivateModuleKind {
                module_name: format!("{project_name}.{}", module.module_name),
            });
        }
        let key = normalize_identifier(&module.module_name);
        if !module_names.insert(key) {
            return Err(ProjectCompileError::ModuleNameDuplicate {
                name: module.module_name.clone(),
            });
        }
    }
    Ok(())
}

fn build_reference_order_map(manifest: &ProjectManifest) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for (index, reference) in manifest.references.iter().enumerate() {
        out.insert(
            normalize_identifier(&reference.referenced_project_name),
            index,
        );
    }
    out
}

fn ordered_reference_projects(manifest: &ProjectManifest) -> Vec<&ReferencedProjectManifest> {
    let mut refs = manifest.reference_projects.iter().collect::<Vec<_>>();
    let order_map = build_reference_order_map(manifest);
    refs.sort_by_key(|referenced| {
        let key = normalize_identifier(&referenced.project_name);
        order_map.get(&key).copied().unwrap_or(usize::MAX)
    });
    refs
}

fn lowered_proc_symbol(project_name: &str, module_name: &str, procedure_name: &str) -> String {
    format!("pmr_{project_name}_{module_name}_{procedure_name}")
}

fn find_decl_by_signature<'a>(
    procedures: &'a [ProcedureDecl],
    project_name: &str,
    module_name: &str,
    procedure_name: &str,
) -> Option<&'a ProcedureDecl> {
    procedures.iter().find(|decl| {
        decl.project_name == project_name
            && decl.module_name == module_name
            && decl.procedure_name == procedure_name
    })
}

fn is_visible_from_active_project(
    decl: &ProcedureDecl,
    active_project: &str,
    current_project: &str,
    current_module: &str,
) -> bool {
    if decl.project_name == current_project && decl.module_name == current_module {
        return true;
    }
    if decl.project_name == active_project {
        return decl.is_public;
    }
    decl.is_public && !decl.option_private_module
}

fn unique_lowered_name_for_proc(
    procedures: &[ProcedureDecl],
    project_name: &str,
    procedure_name: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
) -> Result<Option<String>, ProjectCompileError> {
    let candidates = procedures
        .iter()
        .filter(|decl| {
            decl.project_name == project_name
                && decl.procedure_name == procedure_name
                && is_visible_from_active_project(
                    decl,
                    active_project,
                    current_project,
                    current_module,
                )
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates.len() > 1 {
        return Err(ProjectCompileError::NameQualificationRequired {
            name: procedure_name.to_string(),
        });
    }
    Ok(Some(candidates[0].lowered_name.clone()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InvocationBinding {
    start: usize,
    end: usize,
    raw_name: String,
    replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineBindPlan {
    drop_line: bool,
    lowered_line: String,
    bound_call_targets: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EarlyBoundBinding {
    qualified_type: String,
    create_selector: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalDimDecl {
    leading_ws: String,
    var_name: String,
    qualified_type: String,
    as_new: bool,
}

#[allow(clippy::too_many_arguments)]
fn lower_module_source_module_aware(
    manifest: &ProjectManifest,
    active_project: &str,
    module: &ModuleUnit,
    current_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<String, ProjectCompileError> {
    let current_module = normalize_identifier(&module.module_name);
    let mut out = Vec::new();
    let mut active_function_result: Option<(String, String)> = None;
    let mut early_bound = BTreeMap::<String, EarlyBoundBinding>::new();
    for line in module.source.lines() {
        let expanded = expand_early_bound_source_line(line, manifest, &mut early_bound)?;
        for expanded_line in expanded {
            let (plan, next_function_result) = build_line_bind_plan(
                manifest,
                active_project,
                module,
                current_project,
                &current_module,
                procedures,
                reference_order,
                &expanded_line,
                active_function_result.as_ref(),
            )?;
            active_function_result = next_function_result;
            let _ = &plan.bound_call_targets;
            if plan.drop_line {
                continue;
            }
            out.push(plan.lowered_line);
        }
    }
    Ok(out.join("\n"))
}

fn expand_early_bound_source_line(
    line: &str,
    manifest: &ProjectManifest,
    early_bound: &mut BTreeMap<String, EarlyBoundBinding>,
) -> Result<Vec<String>, ProjectCompileError> {
    if let Some(dim_decl) = parse_external_dim_declaration(line) {
        let (qualifier, _) = parse_qualified_type_reference(&dim_decl.qualified_type)
            .expect("external dim declaration must carry qualified type");
        let qualifier = qualifier.to_string();
        if !manifest.references.iter().any(|reference| {
            reference.reference_kind == ReferenceKind::TypeLibrary
                && normalize_identifier(&reference.referenced_project_name)
                    == normalize_identifier(&qualifier)
        }) {
            return Err(ProjectCompileError::TypeLibraryQualifierUnresolved {
                type_name: dim_decl.qualified_type,
                qualifier,
            });
        }
        let selector = known_create_object_selector(&dim_decl.qualified_type);
        early_bound.insert(
            normalize_identifier(&dim_decl.var_name),
            EarlyBoundBinding {
                qualified_type: dim_decl.qualified_type.clone(),
                create_selector: selector,
            },
        );
        let mut out = Vec::new();
        out.push(format!(
            "{}Dim {} As Object",
            dim_decl.leading_ws, dim_decl.var_name
        ));
        if dim_decl.as_new {
            let Some(selector) = selector else {
                return Err(ProjectCompileError::TypeLibraryCreateObjectUnsupported {
                    type_name: dim_decl.qualified_type,
                });
            };
            out.push(format!(
                "{}{} = CreateObject({selector})",
                dim_decl.leading_ws, dim_decl.var_name
            ));
        }
        return Ok(out);
    }

    let rewritten = rewrite_early_bound_member_dispatch(line, early_bound)?;
    Ok(vec![rewritten])
}

fn parse_external_dim_declaration(line: &str) -> Option<ExternalDimDecl> {
    let leading_ws_len = line.len().saturating_sub(line.trim_start().len());
    let leading_ws = line[..leading_ws_len].to_string();
    let trimmed = line.trim();
    if !trimmed.to_ascii_lowercase().starts_with("dim ") {
        return None;
    }
    let payload = trimmed[4..].trim();
    if payload.contains(',') {
        return None;
    }
    let (lhs, rhs) = split_keyword_ascii_ci(payload, " as ")?;
    let var_name = lhs.trim();
    if var_name.is_empty() || !is_valid_vba_identifier(var_name) {
        return None;
    }
    let mut rhs_trimmed = rhs.trim();
    let as_new = if rhs_trimmed.len() >= 4 && rhs_trimmed[..4].eq_ignore_ascii_case("new ") {
        rhs_trimmed = rhs_trimmed[4..].trim();
        true
    } else {
        false
    };
    let (_, normalized_type) = parse_qualified_type_reference(rhs_trimmed)?;
    Some(ExternalDimDecl {
        leading_ws,
        var_name: var_name.to_string(),
        qualified_type: normalized_type,
        as_new,
    })
}

fn parse_qualified_type_reference(type_text: &str) -> Option<(&str, String)> {
    let raw = type_text.trim();
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.split('.');
    let qualifier = parts.next()?.trim();
    if !is_valid_vba_identifier(qualifier) {
        return None;
    }
    let mut normalized = vec![qualifier.to_string()];
    let mut saw_tail = false;
    for part in parts {
        let token = part.trim();
        if !is_valid_vba_identifier(token) {
            return None;
        }
        normalized.push(token.to_string());
        saw_tail = true;
    }
    if !saw_tail {
        return None;
    }
    Some((qualifier, normalized.join(".")))
}

fn rewrite_early_bound_member_dispatch(
    line: &str,
    early_bound: &BTreeMap<String, EarlyBoundBinding>,
) -> Result<String, ProjectCompileError> {
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_rel) = line[cursor..].find('(') {
        let open = cursor + open_rel;
        let Some((name_start, name_end)) = invocation_name_span(line, open) else {
            cursor = open + 1;
            continue;
        };
        let Some(close) = find_matching_paren(line, open) else {
            cursor = open + 1;
            continue;
        };
        let raw_name = line[name_start..name_end].trim();
        let Some(dot_idx) = raw_name.find('.') else {
            cursor = close + 1;
            continue;
        };
        let var_name = raw_name[..dot_idx].trim();
        let member_name = raw_name[dot_idx + 1..].trim();
        if var_name.is_empty() || member_name.is_empty() {
            cursor = close + 1;
            continue;
        }
        let key = normalize_identifier(var_name);
        if !early_bound.contains_key(&key) {
            cursor = close + 1;
            continue;
        }
        let Some(member_token) = known_dispatch_member_token(member_name) else {
            return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                member_name: member_name.to_string(),
            });
        };
        let args_raw = line[open + 1..close].trim();
        let args = split_top_level_args(args_raw)?;
        if args.len() > 1 {
            return Err(ProjectCompileError::TypeLibraryInvokeArityUnsupported {
                target: raw_name.to_string(),
            });
        }
        let replacement = if args.is_empty() || args[0].trim().is_empty() {
            format!("DispatchInvoke({var_name}, {member_token})")
        } else {
            format!(
                "DispatchInvoke({var_name}, {member_token}, {})",
                args[0].trim()
            )
        };
        replacements.push((name_start, close + 1, replacement));
        cursor = close + 1;
    }
    if replacements.is_empty() {
        return Ok(line.to_string());
    }
    let mut out = String::with_capacity(line.len() + 32);
    let mut previous = 0usize;
    for (start, end, replacement) in replacements {
        if start < previous || end > line.len() || start >= end {
            continue;
        }
        out.push_str(&line[previous..start]);
        out.push_str(&replacement);
        previous = end;
    }
    out.push_str(&line[previous..]);
    Ok(out)
}

fn split_top_level_args(args: &str) -> Result<Vec<String>, ProjectCompileError> {
    if args.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let chars = args.as_bytes();
    let mut idx = 0usize;
    while idx < chars.len() {
        let ch = chars[idx] as char;
        if ch == '"' {
            in_string = !in_string;
            idx += 1;
            continue;
        }
        if in_string {
            idx += 1;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(args[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
        idx += 1;
    }
    if depth != 0 || in_string {
        return Err(ProjectCompileError::BackendCompile {
            message: "BIND-E-TYPELIB-ARG-PARSE: malformed argument list while rewriting early-bound member invocation".to_string(),
        });
    }
    out.push(args[start..].trim().to_string());
    Ok(out)
}

fn find_matching_paren(text: &str, open_idx: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if open_idx >= bytes.len() || bytes[open_idx] != b'(' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    for (idx, b) in bytes.iter().enumerate().skip(open_idx) {
        let ch = *b as char;
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

fn split_keyword_ascii_ci<'a>(text: &'a str, keyword: &'a str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let needle = keyword.to_ascii_lowercase();
    let idx = lower.find(&needle)?;
    Some((&text[..idx], &text[idx + keyword.len()..]))
}

fn known_create_object_selector(qualified_type: &str) -> Option<i32> {
    match normalize_identifier(qualified_type).as_str() {
        "oxvba.testdispatch" => Some(4),
        "scripting.dictionary" => Some(4),
        _ => None,
    }
}

fn known_dispatch_member_token(member_name: &str) -> Option<i32> {
    match normalize_identifier(member_name).as_str() {
        "count" => Some(1),
        "exists" => Some(2),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_line_bind_plan(
    manifest: &ProjectManifest,
    active_project: &str,
    module: &ModuleUnit,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
    line: &str,
    active_function_result: Option<&(String, String)>,
) -> Result<(LineBindPlan, Option<(String, String)>), ProjectCompileError> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("attribute ") || lower == "option private module" {
        return Ok((
            LineBindPlan {
                drop_line: true,
                lowered_line: String::new(),
                bound_call_targets: Vec::new(),
            },
            active_function_result.cloned(),
        ));
    }
    if module.module_kind == ModuleKind::Class && lower.starts_with("implements ") {
        return Ok((
            LineBindPlan {
                drop_line: true,
                lowered_line: String::new(),
                bound_call_targets: Vec::new(),
            },
            active_function_result.cloned(),
        ));
    }

    let normalized = normalize_visibility_prefixed_procedure_signature(line);
    if let Some((proc_name, _, _)) = parse_procedure_signature_line(&normalized)
        && let Some(decl) =
            find_decl_by_signature(procedures, current_project, current_module, &proc_name)
    {
        let rewritten = rewrite_signature_name(&normalized, &decl.lowered_name);
        let next_function_result = if decl.kind == ExportKind::Function {
            Some((proc_name, decl.lowered_name.clone()))
        } else {
            None
        };
        return Ok((
            LineBindPlan {
                drop_line: false,
                lowered_line: rewritten,
                bound_call_targets: Vec::new(),
            },
            next_function_result,
        ));
    }

    let invocation_bindings = bind_invocation_targets(
        &normalized,
        manifest,
        active_project,
        current_project,
        current_module,
        procedures,
        reference_order,
    )?;
    let mut lowered_line = apply_invocation_bindings(&normalized, &invocation_bindings);
    lowered_line = rewrite_call_statement_target_if_present(
        &lowered_line,
        manifest,
        active_project,
        current_project,
        current_module,
        procedures,
        reference_order,
    )?;
    if let Some((result_name, lowered_name)) = active_function_result {
        lowered_line = rewrite_bare_identifier(&lowered_line, result_name, lowered_name);
    }
    let next_function_result = if lower.starts_with("end function") {
        None
    } else {
        active_function_result.cloned()
    };
    let bound_call_targets = invocation_bindings
        .iter()
        .map(|binding| (binding.raw_name.clone(), binding.replacement.clone()))
        .collect();
    Ok((
        LineBindPlan {
            drop_line: false,
            lowered_line,
            bound_call_targets,
        },
        next_function_result,
    ))
}

fn apply_invocation_bindings(line: &str, bindings: &[InvocationBinding]) -> String {
    if bindings.is_empty() {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0usize;
    for binding in bindings {
        if binding.start < cursor || binding.end > line.len() || binding.start >= binding.end {
            continue;
        }
        out.push_str(&line[cursor..binding.start]);
        out.push_str(&binding.replacement);
        cursor = binding.end;
    }
    out.push_str(&line[cursor..]);
    out
}

#[allow(clippy::too_many_arguments)]
fn bind_invocation_targets(
    line: &str,
    manifest: &ProjectManifest,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<Vec<InvocationBinding>, ProjectCompileError> {
    let mut bindings = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_rel) = line[cursor..].find('(') {
        let open = cursor + open_rel;
        let Some((name_start, name_end)) = invocation_name_span(line, open) else {
            cursor = open + 1;
            continue;
        };
        let raw_name = line[name_start..name_end].trim();
        let replacement = resolve_invocation_name(
            raw_name,
            manifest,
            active_project,
            current_project,
            current_module,
            procedures,
            reference_order,
        )?;
        if let Some(replacement) = replacement {
            bindings.push(InvocationBinding {
                start: name_start,
                end: name_end,
                raw_name: raw_name.to_string(),
                replacement,
            });
        }
        cursor = open + 1;
    }
    bindings.sort_by_key(|binding| (binding.start, binding.end));
    Ok(bindings)
}

#[allow(clippy::too_many_arguments)]
fn rewrite_call_statement_target_if_present(
    line: &str,
    manifest: &ProjectManifest,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<String, ProjectCompileError> {
    let Some((start, end)) = call_statement_name_span(line) else {
        return Ok(line.to_string());
    };
    let raw_name = line[start..end].trim();
    let Some(replacement) = resolve_invocation_name(
        raw_name,
        manifest,
        active_project,
        current_project,
        current_module,
        procedures,
        reference_order,
    )?
    else {
        return Ok(line.to_string());
    };
    let mut out = String::with_capacity(line.len() + replacement.len());
    out.push_str(&line[..start]);
    out.push_str(&replacement);
    out.push_str(&line[end..]);
    Ok(out)
}

fn call_statement_name_span(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if !line[idx..].to_ascii_lowercase().starts_with("call ") {
        return None;
    }
    idx += 5;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    let start = idx;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            idx += 1;
        } else {
            break;
        }
    }
    if start == idx {
        None
    } else {
        Some((start, idx))
    }
}

fn rewrite_module_source(
    manifest: &ProjectManifest,
    active_project: &str,
    module: &ModuleUnit,
    current_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<String, ProjectCompileError> {
    let current_module = normalize_identifier(&module.module_name);
    let mut out = Vec::new();
    let mut active_function_result: Option<(String, String)> = None;
    let mut early_bound = BTreeMap::<String, EarlyBoundBinding>::new();
    for line in module.source.lines() {
        let expanded = expand_early_bound_source_line(line, manifest, &mut early_bound)?;
        for expanded_line in expanded {
            let trimmed = expanded_line.trim();
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("attribute ") || lower == "option private module" {
                continue;
            }
            if module.module_kind == ModuleKind::Class && lower.starts_with("implements ") {
                continue;
            }
            let normalized = normalize_visibility_prefixed_procedure_signature(&expanded_line);
            if let Some((proc_name, _, _)) = parse_procedure_signature_line(&normalized)
                && let Some(decl) =
                    find_decl_by_signature(procedures, current_project, &current_module, &proc_name)
            {
                let rewritten = rewrite_signature_name(&normalized, &decl.lowered_name);
                if decl.kind == ExportKind::Function {
                    active_function_result = Some((proc_name, decl.lowered_name.clone()));
                } else {
                    active_function_result = None;
                }
                out.push(rewritten);
                continue;
            }
            let mut rewritten = rewrite_invocation_targets(
                &normalized,
                manifest,
                active_project,
                current_project,
                &current_module,
                procedures,
                reference_order,
            )?;
            if let Some((result_name, lowered_name)) = &active_function_result {
                rewritten = rewrite_bare_identifier(&rewritten, result_name, lowered_name);
            }
            if lower.starts_with("end function") {
                active_function_result = None;
            }
            out.push(rewritten);
        }
    }
    Ok(out.join("\n"))
}

fn rewrite_bare_identifier(line: &str, identifier: &str, replacement: &str) -> String {
    if identifier.is_empty() {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if !(ch.is_ascii_alphabetic() || ch == '_') {
            out.push(ch);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c.is_ascii_alphanumeric() || c == '_' {
                i += 1;
            } else {
                break;
            }
        }
        if i < bytes.len() {
            let suffix = bytes[i] as char;
            if matches!(suffix, '%' | '&' | '^' | '!' | '#' | '$' | '@') {
                i += 1;
            }
        }
        let token = &line[start..i];
        let (base, _) = split_type_char(token);
        if base.eq_ignore_ascii_case(identifier) {
            out.push_str(replacement);
        } else {
            out.push_str(token);
        }
    }
    out
}

fn normalize_visibility_prefixed_procedure_signature(line: &str) -> String {
    let trimmed = line.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    let prefixes = [
        "public sub ",
        "private sub ",
        "public function ",
        "private function ",
        "public property get ",
        "private property get ",
        "public property let ",
        "private property let ",
        "public property set ",
        "private property set ",
    ];
    for prefix in prefixes {
        if lowered.starts_with(prefix) {
            let stripped = trimmed[prefix.find(' ').unwrap_or(0) + 1..].trim_start();
            return stripped.to_string();
        }
    }
    line.to_string()
}

fn rewrite_signature_name(line: &str, replacement_name: &str) -> String {
    let variants = [
        "sub ",
        "function ",
        "property get ",
        "property let ",
        "property set ",
    ];
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let lowered = trimmed.to_ascii_lowercase();
    for marker in variants {
        if lowered.starts_with(marker) {
            let head = &trimmed[..marker.len()];
            let tail = &trimmed[marker.len()..];
            let name_end = tail
                .find(|ch: char| ch.is_ascii_whitespace() || ch == '(')
                .unwrap_or(tail.len());
            let suffix = &tail[name_end..];
            return format!("{}{}{}{}", &line[..leading], head, replacement_name, suffix);
        }
    }
    line.to_string()
}

#[allow(clippy::too_many_arguments)]
fn rewrite_invocation_targets(
    line: &str,
    manifest: &ProjectManifest,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<String, ProjectCompileError> {
    let invocation_bindings = bind_invocation_targets(
        line,
        manifest,
        active_project,
        current_project,
        current_module,
        procedures,
        reference_order,
    )?;
    let rewritten = apply_invocation_bindings(line, &invocation_bindings);
    rewrite_call_statement_target_if_present(
        &rewritten,
        manifest,
        active_project,
        current_project,
        current_module,
        procedures,
        reference_order,
    )
}

fn invocation_name_span(text: &str, open_paren_idx: usize) -> Option<(usize, usize)> {
    if open_paren_idx == 0 || open_paren_idx > text.len() {
        return None;
    }
    let bytes = text.as_bytes();
    let mut idx = open_paren_idx;
    while idx > 0 && bytes[idx - 1].is_ascii_whitespace() {
        idx -= 1;
    }
    let end = idx;
    while idx > 0 {
        let b = bytes[idx - 1];
        let is_name = b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
        if !is_name {
            break;
        }
        idx -= 1;
    }
    if idx == end {
        return None;
    }
    Some((idx, end))
}

#[allow(clippy::too_many_arguments)]
fn resolve_invocation_name(
    name: &str,
    manifest: &ProjectManifest,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<Option<String>, ProjectCompileError> {
    let parts = name
        .split('.')
        .map(normalize_identifier)
        .collect::<Vec<_>>();
    if parts.len() == 1 {
        let proc = &parts[0];
        let local = procedures
            .iter()
            .find(|decl| {
                decl.project_name == current_project
                    && decl.module_name == current_module
                    && decl.procedure_name == *proc
            })
            .map(|decl| decl.lowered_name.clone());
        if local.is_some() {
            return Ok(local);
        }

        let active_candidates = procedures
            .iter()
            .filter(|decl| {
                decl.project_name == active_project
                    && decl.procedure_name == *proc
                    && decl.module_name != current_module
                    && decl.is_public
            })
            .collect::<Vec<_>>();
        if active_candidates.len() > 1 {
            return Err(ProjectCompileError::NameQualificationRequired { name: proc.clone() });
        }
        if let Some(decl) = active_candidates.first() {
            return Ok(Some(decl.lowered_name.clone()));
        }

        let mut ordered_refs = reference_order
            .iter()
            .map(|(project_name, order)| (project_name.clone(), *order))
            .collect::<Vec<_>>();
        ordered_refs.sort_by_key(|(_, order)| *order);
        for (project_name, _) in ordered_refs {
            let lowered = unique_lowered_name_for_proc(
                procedures,
                &project_name,
                proc,
                active_project,
                current_project,
                current_module,
            )?;
            if lowered.is_some() {
                return Ok(lowered);
            }
        }
        return Ok(None);
    }

    if parts.len() == 2 {
        let module_name = &parts[0];
        let proc_name = &parts[1];
        let decl = find_decl_by_signature(procedures, current_project, module_name, proc_name);
        return match decl {
            Some(decl) => Ok(Some(decl.lowered_name.clone())),
            None => Err(ProjectCompileError::NameResolutionNotFound {
                name: name.to_string(),
            }),
        };
    }

    if parts.len() != 3 {
        return Err(ProjectCompileError::NameResolutionNotFound {
            name: name.to_string(),
        });
    }

    let project_name = &parts[0];
    let module_name = &parts[1];
    let proc_name = &parts[2];
    if project_name != current_project {
        let is_reference = manifest.references.iter().any(|reference| {
            normalize_identifier(&reference.referenced_project_name) == *project_name
        });
        if !is_reference {
            return Err(ProjectCompileError::ProjectQualificationInvalid {
                name: name.to_string(),
            });
        }
        if !manifest
            .reference_projects
            .iter()
            .any(|project| normalize_identifier(&project.project_name) == *project_name)
        {
            return Err(ProjectCompileError::ReferenceProjectNotLoaded {
                name: project_name.clone(),
            });
        }
    }

    let Some(decl) = find_decl_by_signature(procedures, project_name, module_name, proc_name)
    else {
        return Err(ProjectCompileError::NameResolutionNotFound {
            name: name.to_string(),
        });
    };
    if !is_visible_from_active_project(decl, active_project, current_project, current_module) {
        return Err(ProjectCompileError::NameResolutionNotFound {
            name: name.to_string(),
        });
    }
    Ok(Some(decl.lowered_name.clone()))
}

fn collect_host_exports(
    manifest: &ProjectManifest,
    procedures: &[ProcedureDecl],
) -> Vec<HostProcedureExport> {
    let mut exports = Vec::new();
    let active_project = normalize_identifier(&manifest.project_name);
    for module in &manifest.modules {
        if module.module_kind != ModuleKind::Procedural {
            continue;
        }
        let module_name = normalize_identifier(&module.module_name);
        for procedure in procedures.iter().filter(|decl| {
            decl.project_name == active_project
                && decl.module_name == module_name
                && decl.is_public
                && decl.module_kind == ModuleKind::Procedural
        }) {
            exports.push(HostProcedureExport {
                project_name: active_project.clone(),
                module_name: module_name.clone(),
                procedure_name: procedure.procedure_name.clone(),
                kind: procedure.kind,
            });
        }
    }
    exports.sort_by(|lhs, rhs| {
        (
            lhs.module_name.as_str(),
            lhs.procedure_name.as_str(),
            lhs.kind,
        )
            .cmp(&(
                rhs.module_name.as_str(),
                rhs.procedure_name.as_str(),
                rhs.kind,
            ))
    });
    exports
}

fn collect_reference_visible_exports(
    manifest: &ProjectManifest,
    procedures: &[ProcedureDecl],
) -> Vec<HostProcedureExport> {
    let mut exports = Vec::new();
    let active_project = normalize_identifier(&manifest.project_name);
    for module in &manifest.modules {
        if module.module_kind != ModuleKind::Procedural || module.attributes.option_private_module {
            continue;
        }
        let module_name = normalize_identifier(&module.module_name);
        for procedure in procedures.iter().filter(|decl| {
            decl.project_name == active_project
                && decl.module_name == module_name
                && decl.is_public
                && decl.module_kind == ModuleKind::Procedural
        }) {
            exports.push(HostProcedureExport {
                project_name: active_project.clone(),
                module_name: module_name.clone(),
                procedure_name: procedure.procedure_name.clone(),
                kind: procedure.kind,
            });
        }
    }
    exports.sort_by(|lhs, rhs| {
        (
            lhs.module_name.as_str(),
            lhs.procedure_name.as_str(),
            lhs.kind,
        )
            .cmp(&(
                rhs.module_name.as_str(),
                rhs.procedure_name.as_str(),
                rhs.kind,
            ))
    });
    exports
}

fn validate_compiled_project_contract(
    manifest: &ProjectManifest,
    host_exports: &[HostProcedureExport],
    reference_visible_exports: &[HostProcedureExport],
) -> Result<(), String> {
    let active_project = normalize_identifier(&manifest.project_name);
    let module_kinds = manifest
        .modules
        .iter()
        .map(|module| {
            (
                normalize_identifier(&module.module_name),
                (module.module_kind, module.attributes.option_private_module),
            )
        })
        .collect::<BTreeMap<_, _>>();

    if !exports_are_sorted_unique(host_exports) {
        return Err("host export list is not strictly sorted and unique".to_string());
    }
    if !exports_are_sorted_unique(reference_visible_exports) {
        return Err("reference-visible export list is not strictly sorted and unique".to_string());
    }

    let host_set = host_exports.iter().map(export_key).collect::<BTreeSet<_>>();

    for export in host_exports {
        if export.project_name != active_project {
            return Err(format!(
                "host export `{}`.`{}` has non-active project `{}`",
                export.module_name, export.procedure_name, export.project_name
            ));
        }
        let Some((module_kind, _)) = module_kinds.get(&export.module_name).copied() else {
            return Err(format!(
                "host export `{}`.`{}` references unknown module",
                export.module_name, export.procedure_name
            ));
        };
        if module_kind != ModuleKind::Procedural {
            return Err(format!(
                "host export `{}`.`{}` is not procedural",
                export.module_name, export.procedure_name
            ));
        }
    }

    for export in reference_visible_exports {
        if !host_set.contains(&export_key(export)) {
            return Err(format!(
                "reference-visible export `{}`.`{}` is not present in host export surface",
                export.module_name, export.procedure_name
            ));
        }
        let Some((_, option_private)) = module_kinds.get(&export.module_name).copied() else {
            return Err(format!(
                "reference-visible export `{}`.`{}` references unknown module",
                export.module_name, export.procedure_name
            ));
        };
        if option_private {
            return Err(format!(
                "reference-visible export `{}`.`{}` leaks Option Private Module visibility",
                export.module_name, export.procedure_name
            ));
        }
    }

    Ok(())
}

fn export_key(export: &HostProcedureExport) -> (&str, &str, ExportKind) {
    (
        export.module_name.as_str(),
        export.procedure_name.as_str(),
        export.kind,
    )
}

fn exports_are_sorted_unique(exports: &[HostProcedureExport]) -> bool {
    exports
        .windows(2)
        .all(|pair| export_key(&pair[0]) < export_key(&pair[1]))
}
fn parse_attribute_line(
    line: &str,
    attrs: &mut ModuleAttributes,
    module_name: &str,
) -> Result<(), ProjectCompileError> {
    let Some(rest) = line
        .strip_prefix("Attribute ")
        .or_else(|| line.strip_prefix("attribute "))
    else {
        return Ok(());
    };
    let Some((lhs, rhs)) = rest.split_once('=') else {
        return Err(ProjectCompileError::ModuleHeaderInvalid {
            module_name: module_name.to_string(),
            line: line.to_string(),
        });
    };
    let key = lhs.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err(ProjectCompileError::ModuleHeaderInvalid {
            module_name: module_name.to_string(),
            line: line.to_string(),
        });
    }
    let value_text = rhs.trim().trim_matches('"');
    let value = value_text.to_ascii_lowercase();
    match key.as_str() {
        "vb_name" => attrs.vb_name = value,
        "vb_globalnamespace" => {
            attrs.vb_global_namespace = parse_bool_attribute(value_text).ok_or(
                ProjectCompileError::ModuleHeaderInvalid {
                    module_name: module_name.to_string(),
                    line: line.to_string(),
                },
            )?
        }
        "vb_creatable" => {
            attrs.vb_creatable = parse_bool_attribute(value_text).ok_or(
                ProjectCompileError::ModuleHeaderInvalid {
                    module_name: module_name.to_string(),
                    line: line.to_string(),
                },
            )?
        }
        "vb_predeclaredid" => {
            attrs.vb_predeclared_id = parse_bool_attribute(value_text).ok_or(
                ProjectCompileError::ModuleHeaderInvalid {
                    module_name: module_name.to_string(),
                    line: line.to_string(),
                },
            )?
        }
        "vb_exposed" => {
            attrs.vb_exposed = parse_bool_attribute(value_text).ok_or(
                ProjectCompileError::ModuleHeaderInvalid {
                    module_name: module_name.to_string(),
                    line: line.to_string(),
                },
            )?
        }
        _ => {}
    }
    Ok(())
}

fn parse_bool_attribute(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_procedure_signature_line(line: &str) -> Option<(String, ExportKind, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('\'') {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let (is_public, tail) = if lower.starts_with("public ") {
        (true, trimmed[7..].trim())
    } else if lower.starts_with("private ") {
        (false, trimmed[8..].trim())
    } else {
        (true, trimmed)
    };
    let lower_tail = tail.to_ascii_lowercase();
    let (kind, remainder) = if lower_tail.starts_with("sub ") {
        (ExportKind::Sub, tail[4..].trim())
    } else if lower_tail.starts_with("function ") {
        (ExportKind::Function, tail[9..].trim())
    } else {
        return None;
    };
    let token = remainder
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
        .next()
        .unwrap_or_default();
    let name = normalize_procedure_name(token)?;
    Some((name, kind, is_public))
}

fn normalize_procedure_name(token: &str) -> Option<String> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    let (token, _) = split_type_char(token);
    let mut chars = token.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return None;
    }
    Some(token.to_ascii_lowercase())
}

fn split_type_char(token: &str) -> (&str, Option<char>) {
    let Some(last) = token.chars().last() else {
        return (token, None);
    };
    if matches!(last, '%' | '&' | '^' | '!' | '#' | '$' | '@') {
        let cutoff = token.len() - last.len_utf8();
        (&token[..cutoff], Some(last))
    } else {
        (token, None)
    }
}

fn is_valid_vba_identifier(name: &str) -> bool {
    let name = name.trim();
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn normalize_identifier(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{
        ExportKind, ModuleKind, ProjectCompileError, ProjectKind, ProjectLoweringStrategy,
        ProjectManifest, ProjectReference, ReferenceKind, ReferencedProjectManifest,
        compile_project, compile_project_with_strategy, module_unit_from_source,
        validate_compiled_project_contract,
    };
    use std::collections::BTreeMap;

    fn base_manifest() -> ProjectManifest {
        let module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module parses");
        ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        }
    }

    #[test]
    fn module_unit_parses_header_attributes_and_option_private() {
        let unit = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nAttribute VB_PredeclaredId = True\nOption Private Module\nPublic Function Add(x, y)\nEnd Function",
        )
        .expect("module should parse");
        assert_eq!(unit.attributes.vb_name, "mathmodule");
        assert!(unit.attributes.vb_predeclared_id);
        assert!(unit.attributes.option_private_module);
    }

    #[test]
    fn module_unit_rejects_malformed_attribute_line() {
        let err = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_PredeclaredId = maybe\nPublic Sub Main()\nEnd Sub",
        )
        .expect_err("malformed attribute value should fail");
        assert_eq!(err.code(), "PMR-E-MODULE-HEADER-INVALID");
    }

    #[test]
    fn module_unit_tolerates_lowercase_attribute_keyword_and_option_private_spacing() {
        let unit = module_unit_from_source(
            "Module1",
            ModuleKind::Procedural,
            "attribute vb_name = \"Module1\"\n  Option Private Module  \nPublic Sub Main()\nEnd Sub",
        )
        .expect("attribute keyword casing and option-private spacing should be tolerated");
        assert_eq!(unit.attributes.vb_name, "module1");
        assert!(unit.attributes.option_private_module);
    }

    #[test]
    fn module_unit_rejects_non_boolean_known_header_attribute() {
        let err = module_unit_from_source(
            "Module1",
            ModuleKind::Procedural,
            "Attribute VB_Exposed = 1\nPublic Sub Main()\nEnd Sub",
        )
        .expect_err("known boolean attribute with non-boolean token should fail deterministically");
        assert_eq!(err.code(), "PMR-E-MODULE-HEADER-INVALID");
    }

    #[test]
    fn compile_project_rejects_duplicate_module_names() {
        let mut manifest = base_manifest();
        manifest.modules.push(
            module_unit_from_source(
                "mainmodule",
                ModuleKind::Procedural,
                "Attribute VB_Name = \"mainmodule\"\nPublic Sub Worker()\nEnd Sub",
            )
            .expect("module parses"),
        );
        let err = compile_project(&manifest).expect_err("duplicate modules should fail");
        assert_eq!(err.code(), "PMR-E-MODULE-NAME-DUPLICATE");
    }

    #[test]
    fn compile_project_rejects_option_private_for_non_procedural_module() {
        let mut manifest = base_manifest();
        let class_module = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nOption Private Module\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module parses");
        manifest.modules = vec![class_module];
        let err = compile_project(&manifest).expect_err("class Option Private Module should fail");
        assert_eq!(err.code(), "PMR-E-OPTION-PRIVATE-MODULE-KIND");
    }

    #[test]
    fn compile_project_rewrites_module_qualified_calls_for_unique_names() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall MathModule.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Sub Add(ByVal x, ByVal y)\nEnd Sub",
        )
        .expect("module parses");

        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("project compilation should succeed");
        assert!(
            compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("call pmr_projecta_mathmodule_add(1, 2)")
        );
    }

    #[test]
    fn compile_project_rejects_duplicate_reference_targets() {
        let mut manifest = base_manifest();
        manifest.references.push(ProjectReference {
            referenced_project_name: "CoreLib".to_string(),
            reference_kind: ReferenceKind::Project,
        });
        manifest.references.push(ProjectReference {
            referenced_project_name: "corelib".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        });
        let err = compile_project(&manifest).expect_err("duplicate references should fail");
        assert_eq!(err.code(), "PMR-E-REFERENCE-DUPLICATE-TARGET");
    }

    #[test]
    fn compile_project_exports_public_procedures_including_option_private_modules_for_host_calls() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub\nPublic Function Add(x, y)\nEnd Function",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "PrivateModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"PrivateModule\"\nOption Private Module\nPublic Function Hidden()\nEnd Function",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("project compile should succeed");
        assert_eq!(compiled.host_exports.len(), 3);
        assert!(
            compiled
                .host_exports
                .iter()
                .any(|entry| entry.module_name == "mainmodule"
                    && entry.procedure_name == "add"
                    && entry.kind == ExportKind::Function)
        );
        assert!(
            compiled
                .host_exports
                .iter()
                .any(|entry| entry.module_name == "privatemodule"
                    && entry.procedure_name == "hidden"
                    && entry.kind == ExportKind::Function)
        );
        assert!(
            compiled
                .reference_visible_exports
                .iter()
                .all(|entry| entry.module_name != "privatemodule"),
            "Option Private modules must not be reference-visible across project boundaries"
        );
    }

    #[test]
    fn compile_project_rejects_cross_project_call_when_reference_source_missing() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall OtherProject.Tools.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "Tools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Tools\"\nPublic Sub Add(x, y)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: vec![ProjectReference {
                referenced_project_name: "OtherProject".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err = compile_project(&manifest).expect_err("missing reference source should fail");
        assert_eq!(err.code(), "PMR-E-REFERENCE-PROJECT-NOT-LOADED");
    }

    #[test]
    fn compile_project_executes_cross_project_call_with_loaded_reference_source() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall OtherProject.Tools.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "Tools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Tools\"\nPublic Sub Add(x, y)\nEnd Sub",
        )
        .expect("module parses");
        let referenced_tools = module_unit_from_source(
            "Tools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Tools\"\nPublic Sub Add(ByVal x, ByVal y)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: vec![ProjectReference {
                referenced_project_name: "OtherProject".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "OtherProject".to_string(),
                modules: vec![referenced_tools],
            }],
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("cross-project call should lower");
        assert!(
            compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("call pmr_otherproject_tools_add(1, 2)")
        );
    }

    #[test]
    fn compile_project_rewrites_same_project_qualified_call() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall ProjectA.Math.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "Math",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Math\"\nPublic Sub Add(ByVal x, ByVal y)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("project compilation should succeed");
        assert!(
            compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("call pmr_projecta_math_add(1, 2)")
        );
    }

    #[test]
    fn compile_project_rewrites_function_result_assignments_to_lowered_symbol() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim x\nx = MathModule.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let math_module = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Function Add(ByVal a, ByVal b)\nAdd = a\nEnd Function",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, math_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("project compile should succeed");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("function pmr_projecta_mathmodule_add"));
        assert!(lowered.contains("pmr_projecta_mathmodule_add = a"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_unqualified_duplicate_procedure_name_subset() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "MathA",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathA\"\nPublic Sub Add(ByVal x, ByVal y)\nEnd Sub",
        )
        .expect("module parses");
        let module_c = module_unit_from_source(
            "MathB",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathB\"\nPublic Sub Add(ByVal x, ByVal y)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b, module_c],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err = compile_project(&manifest).expect_err("ambiguous duplicate is staged");
        assert_eq!(err.code(), "PMR-E-NAME-QUALIFICATION-REQUIRED");
    }

    #[test]
    fn compile_project_resolves_unqualified_call_using_reference_precedence() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Compute(1)\nEnd Sub",
        )
        .expect("module parses");
        let ref_first = module_unit_from_source(
            "FirstTools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"FirstTools\"\nPublic Sub Compute(ByVal x)\nEnd Sub",
        )
        .expect("module parses");
        let ref_second = module_unit_from_source(
            "SecondTools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"SecondTools\"\nPublic Sub Compute(ByVal x)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a],
            references: vec![
                ProjectReference {
                    referenced_project_name: "LibTwo".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
                ProjectReference {
                    referenced_project_name: "LibOne".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
            ],
            reference_projects: vec![
                ReferencedProjectManifest {
                    project_name: "LibOne".to_string(),
                    modules: vec![ref_first],
                },
                ReferencedProjectManifest {
                    project_name: "LibTwo".to_string(),
                    modules: vec![ref_second],
                },
            ],
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("precedence rewrite should compile");
        assert!(
            compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("call pmr_libtwo_secondtools_compute(1)")
        );
    }

    #[test]
    fn compile_project_referenced_option_private_module_is_not_visible() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Hidden(1)\nEnd Sub",
        )
        .expect("module parses");
        let ref_private = module_unit_from_source(
            "PrivateTools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"PrivateTools\"\nOption Private Module\nPublic Sub Hidden(ByVal x)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a],
            references: vec![ProjectReference {
                referenced_project_name: "LibOne".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "LibOne".to_string(),
                modules: vec![ref_private],
            }],
            conditional_constants: BTreeMap::new(),
        };
        let err =
            compile_project(&manifest).expect_err("unqualified hidden call should stay unresolved");
        assert_eq!(err.code(), "PMR-E-BACKEND-COMPILE");
        assert!(err.to_string().contains("unknown procedure: hidden"));
    }

    #[test]
    fn module_unit_tolerates_unknown_header_attributes() {
        let unit = module_unit_from_source(
            "Module1",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Module1\"\nAttribute VB_Description = \"sample\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("unknown attributes should be ignored");
        assert_eq!(unit.attributes.vb_name, "module1");
    }

    #[test]
    fn compile_project_tolerates_unknown_reference_module_attributes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall OtherProject.Tools.Ping\nEnd Sub",
        )
        .expect("module parses");
        let referenced_tools = module_unit_from_source(
            "Tools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Tools\"\nAttribute VB_Description = \"imported-module\"\nPublic Sub Ping()\nEnd Sub",
        )
        .expect("reference module with unknown attributes should parse");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OtherProject".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "OtherProject".to_string(),
                modules: vec![referenced_tools],
            }],
            conditional_constants: BTreeMap::new(),
        };
        let compiled =
            compile_project(&manifest).expect("compile should tolerate imported unknown attrs");
        assert!(
            compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("call pmr_otherproject_tools_ping")
        );
    }

    #[test]
    fn compile_project_class_module_allows_implements_subset_without_gate_diagnostic() {
        let class_interface = module_unit_from_source(
            "IThing",
            ModuleKind::Class,
            "Attribute VB_Name = \"IThing\"\nPublic Sub Ping()\nEnd Sub",
        )
        .expect("module parses");
        let class_impl = module_unit_from_source(
            "ThingImpl",
            ModuleKind::Class,
            "Attribute VB_Name = \"ThingImpl\"\nImplements IThing\nPrivate Sub IThing_Ping()\nEnd Sub",
        )
        .expect("module parses");
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, class_interface, class_impl],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("Implements subset should compile");
        assert!(
            !compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("implements "),
            "Implements directives should be consumed by project lowering path"
        );
    }

    #[test]
    fn compile_project_preserves_withevents_diagnostic_gate() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim WithEvents sink As Object\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err = compile_project(&manifest).expect_err("WithEvents should stay explicitly gated");
        assert_eq!(err.code(), "PMR-E-BACKEND-COMPILE");
        assert!(
            err.to_string()
                .contains("PMR-E-WITHEVENTS-MODULE-KIND-UNRESOLVED")
        );
    }

    #[test]
    fn compile_project_preserves_raiseevent_diagnostic_gate() {
        let class_module = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Fire()\nRaiseEvent Changed\nEnd Sub",
        )
        .expect("module parses");
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, class_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err = compile_project(&manifest).expect_err("RaiseEvent should stay explicitly gated");
        assert_eq!(err.code(), "PMR-E-BACKEND-COMPILE");
        assert!(
            err.to_string()
                .contains("PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED")
        );
    }

    #[test]
    fn compile_project_rewrites_early_bound_member_call_to_dispatchinvoke_subset() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nobj = CreateObject(4)\nx = obj.Count()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("early-bound rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dim obj as object"));
        assert!(lowered.contains("x = dispatchinvoke(obj, 1)"));
    }

    #[test]
    fn compile_project_rewrites_as_new_external_type_to_createobject_selector() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As New OxVba.TestDispatch\nDim x\nx = obj.Count()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("As New rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dim obj as object"));
        assert!(lowered.contains("obj = createobject(4)"));
        assert!(lowered.contains("x = dispatchinvoke(obj, 1)"));
    }

    #[test]
    fn compile_project_rejects_unresolved_external_typelib_qualifier() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As UnknownLib.Widget\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err = compile_project(&manifest).expect_err("unknown typelib qualifier should fail");
        assert_eq!(err.code(), "BIND-E-TYPELIB-QUALIFIER-UNRESOLVED");
    }

    #[test]
    fn compile_project_rejects_unsupported_external_member_token() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nobj = CreateObject(4)\nx = obj.UnknownMember()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err =
            compile_project(&manifest).expect_err("unsupported member should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-MEMBER-UNSUPPORTED");
    }

    #[test]
    fn compile_project_rejects_unsupported_external_member_arity() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nobj = CreateObject(4)\nx = obj.Exists(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err =
            compile_project(&manifest).expect_err("unsupported arity should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED");
    }

    #[test]
    fn project_compile_error_code_is_stable() {
        let err = ProjectCompileError::ProjectNameInvalid {
            name: "123".to_string(),
        };
        assert_eq!(err.code(), "PMR-E-PROJECT-NAME-INVALID");
        assert!(err.to_string().contains("PMR-E-PROJECT-NAME-INVALID"));
    }

    #[test]
    fn compiled_project_contract_rejects_unsorted_export_surface() {
        let manifest = base_manifest();
        let host_exports = vec![
            super::HostProcedureExport {
                project_name: "projecta".to_string(),
                module_name: "zmod".to_string(),
                procedure_name: "a".to_string(),
                kind: ExportKind::Sub,
            },
            super::HostProcedureExport {
                project_name: "projecta".to_string(),
                module_name: "amod".to_string(),
                procedure_name: "b".to_string(),
                kind: ExportKind::Sub,
            },
        ];
        let err = validate_compiled_project_contract(&manifest, &host_exports, &[])
            .expect_err("unsorted host exports should fail contract");
        assert!(err.contains("sorted and unique"));
    }

    #[test]
    fn compiled_project_contract_rejects_non_subset_reference_visible_exports() {
        let manifest = base_manifest();
        let host_exports = vec![super::HostProcedureExport {
            project_name: "projecta".to_string(),
            module_name: "mainmodule".to_string(),
            procedure_name: "main".to_string(),
            kind: ExportKind::Sub,
        }];
        let reference_visible_exports = vec![super::HostProcedureExport {
            project_name: "projecta".to_string(),
            module_name: "mainmodule".to_string(),
            procedure_name: "hidden".to_string(),
            kind: ExportKind::Function,
        }];
        let err = validate_compiled_project_contract(
            &manifest,
            &host_exports,
            &reference_visible_exports,
        )
        .expect_err("reference-visible exports must be a host-export subset");
        assert!(err.contains("not present in host export surface"));
    }

    #[test]
    fn compile_project_is_deterministic_for_identical_manifest() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall MathModule.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Function Add(ByVal x, ByVal y)\nAdd = x\nEnd Function",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let first = compile_project(&manifest).expect("first compile should succeed");
        let second = compile_project(&manifest).expect("second compile should succeed");
        assert_eq!(first.rewritten_source, second.rewritten_source);
        assert_eq!(first.host_exports, second.host_exports);
        assert_eq!(
            first.reference_visible_exports,
            second.reference_visible_exports
        );
        assert_eq!(first.bytecode.instructions, second.bytecode.instructions);
    }

    fn assert_strategy_parity(manifest: &ProjectManifest) {
        let module_aware =
            compile_project_with_strategy(manifest, ProjectLoweringStrategy::ModuleAwareBindPlan)
                .expect("module-aware path should compile");
        let rewrite_bridge =
            compile_project_with_strategy(manifest, ProjectLoweringStrategy::RewriteBridge)
                .expect("rewrite bridge path should compile");
        assert_eq!(
            module_aware.rewritten_source,
            rewrite_bridge.rewritten_source
        );
        assert_eq!(
            module_aware.bytecode.instructions,
            rewrite_bridge.bytecode.instructions
        );
        assert_eq!(module_aware.host_exports, rewrite_bridge.host_exports);
        assert_eq!(
            module_aware.reference_visible_exports,
            rewrite_bridge.reference_visible_exports
        );
    }

    #[test]
    fn compile_project_module_aware_matches_rewrite_bridge_for_shared_fixture() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall MathModule.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let module_b = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Function Add(ByVal x, ByVal y)\nAdd = x\nEnd Function",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a, module_b],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        assert_strategy_parity(&manifest);
    }

    #[test]
    fn compile_project_module_aware_matches_rewrite_bridge_for_cross_project_fixture() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall OtherProject.Tools.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let referenced_tools = module_unit_from_source(
            "Tools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"Tools\"\nPublic Sub Add(ByVal x, ByVal y)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a],
            references: vec![ProjectReference {
                referenced_project_name: "OtherProject".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "OtherProject".to_string(),
                modules: vec![referenced_tools],
            }],
            conditional_constants: BTreeMap::new(),
        };
        assert_strategy_parity(&manifest);
    }

    #[test]
    fn compile_project_module_aware_matches_rewrite_bridge_for_reference_precedence_fixture() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Compute(1)\nEnd Sub",
        )
        .expect("module parses");
        let ref_first = module_unit_from_source(
            "FirstTools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"FirstTools\"\nPublic Sub Compute(ByVal x)\nEnd Sub",
        )
        .expect("module parses");
        let ref_second = module_unit_from_source(
            "SecondTools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"SecondTools\"\nPublic Sub Compute(ByVal x)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a],
            references: vec![
                ProjectReference {
                    referenced_project_name: "LibTwo".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
                ProjectReference {
                    referenced_project_name: "LibOne".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
            ],
            reference_projects: vec![
                ReferencedProjectManifest {
                    project_name: "LibOne".to_string(),
                    modules: vec![ref_first],
                },
                ReferencedProjectManifest {
                    project_name: "LibTwo".to_string(),
                    modules: vec![ref_second],
                },
            ],
            conditional_constants: BTreeMap::new(),
        };
        assert_strategy_parity(&manifest);
    }

    #[test]
    fn compile_project_module_aware_matches_rewrite_bridge_for_function_result_fixture() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim x\nx = MathModule.Add(1, 2)\nEnd Sub",
        )
        .expect("module parses");
        let math_module = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Function Add(ByVal a, ByVal b)\nAdd = a\nEnd Function",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, math_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        assert_strategy_parity(&manifest);
    }

    #[test]
    fn compile_project_module_aware_rewrites_module_qualified_call_without_parentheses() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall MathModule.Ping\nEnd Sub",
        )
        .expect("module parses");
        let math_module = module_unit_from_source(
            "MathModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MathModule\"\nPublic Sub Ping()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, math_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled =
            compile_project_with_strategy(&manifest, ProjectLoweringStrategy::ModuleAwareBindPlan)
                .expect("module-aware path should rewrite bare Call target");
        assert!(
            compiled
                .rewritten_source
                .to_ascii_lowercase()
                .contains("call pmr_projecta_mathmodule_ping")
        );
    }

    #[test]
    fn compile_project_module_aware_and_rewrite_bridge_match_error_surface_for_hidden_reference() {
        let module_a = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Hidden(1)\nEnd Sub",
        )
        .expect("module parses");
        let ref_private = module_unit_from_source(
            "PrivateTools",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"PrivateTools\"\nOption Private Module\nPublic Sub Hidden(ByVal x)\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_a],
            references: vec![ProjectReference {
                referenced_project_name: "LibOne".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "LibOne".to_string(),
                modules: vec![ref_private],
            }],
            conditional_constants: BTreeMap::new(),
        };
        let module_aware =
            compile_project_with_strategy(&manifest, ProjectLoweringStrategy::ModuleAwareBindPlan)
                .expect_err("module-aware path should reject hidden reference");
        let rewrite_bridge =
            compile_project_with_strategy(&manifest, ProjectLoweringStrategy::RewriteBridge)
                .expect_err("rewrite path should reject hidden reference");
        assert_eq!(module_aware.code(), rewrite_bridge.code());
        assert_eq!(
            module_aware
                .to_string()
                .contains("unknown procedure: hidden"),
            rewrite_bridge
                .to_string()
                .contains("unknown procedure: hidden")
        );
    }

    #[test]
    fn compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As New OxVba.TestDispatch\nDim x\nx = obj.Count()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        assert_strategy_parity(&manifest);
    }
}
