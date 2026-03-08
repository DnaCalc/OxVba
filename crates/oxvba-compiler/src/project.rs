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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectEventDispatchBinding {
    pub source_project_name: String,
    pub source_module_name: String,
    pub event_name: String,
    pub handler_symbol: String,
}

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub bytecode: Bytecode,
    pub rewritten_source: String,
    pub host_exports: Vec<HostProcedureExport>,
    pub reference_visible_exports: Vec<HostProcedureExport>,
    pub event_dispatch_bindings: Vec<ProjectEventDispatchBinding>,
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
        "PMR-E-WITHEVENTS-MODULE-KIND: `WithEvents` declaration is only valid in class/document/form modules (`{module_name}`)"
    )]
    WithEventsModuleKind { module_name: String },
    #[error(
        "PMR-E-IMPLEMENTS-MODULE-KIND: `Implements` directive is only valid in class modules (`{module_name}`)"
    )]
    ImplementsModuleKind { module_name: String },
    #[error(
        "PMR-E-IMPLEMENTS-INTERFACE-NOT-FOUND: class `{module_name}` implements unknown interface `{interface_name}`"
    )]
    ImplementsInterfaceNotFound {
        module_name: String,
        interface_name: String,
    },
    #[error(
        "PMR-E-IMPLEMENTS-MEMBER-MISSING: class `{module_name}` is missing `{interface_name}_{member_name}` for Implements coverage"
    )]
    ImplementsMemberMissing {
        module_name: String,
        interface_name: String,
        member_name: String,
    },
    #[error(
        "PMR-E-RAISEEVENT-MODULE-KIND: `RaiseEvent` is only valid in class modules (`{module_name}`)"
    )]
    RaiseEventModuleKind { module_name: String },
    #[error(
        "PMR-E-RAISEEVENT-UNDECLARED: class module `{module_name}` raises undeclared event `{event_name}`"
    )]
    RaiseEventUndeclared {
        module_name: String,
        event_name: String,
    },
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
            Self::WithEventsModuleKind { .. } => "PMR-E-WITHEVENTS-MODULE-KIND",
            Self::ImplementsModuleKind { .. } => "PMR-E-IMPLEMENTS-MODULE-KIND",
            Self::ImplementsInterfaceNotFound { .. } => "PMR-E-IMPLEMENTS-INTERFACE-NOT-FOUND",
            Self::ImplementsMemberMissing { .. } => "PMR-E-IMPLEMENTS-MEMBER-MISSING",
            Self::RaiseEventModuleKind { .. } => "PMR-E-RAISEEVENT-MODULE-KIND",
            Self::RaiseEventUndeclared { .. } => "PMR-E-RAISEEVENT-UNDECLARED",
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
    param_count: usize,
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
    validate_event_semantics(manifest, &procedure_index, &reference_order)?;
    let event_dispatch_plan =
        collect_event_dispatch_plan(manifest, &procedure_index, &reference_order);

    let rewritten_source = lower_project_source(
        strategy,
        manifest,
        &active_project,
        &procedure_index,
        &reference_order,
        &event_dispatch_plan,
    )?;

    let bytecode = compile(&rewritten_source).map_err(|e| ProjectCompileError::BackendCompile {
        message: e.to_string(),
    })?;

    let host_exports = collect_host_exports(manifest, &procedure_index);
    let reference_visible_exports = collect_reference_visible_exports(manifest, &procedure_index);
    let event_dispatch_bindings = flatten_event_dispatch_plan(&event_dispatch_plan);
    validate_compiled_project_contract(manifest, &host_exports, &reference_visible_exports)
        .map_err(|message| ProjectCompileError::BackendCompile {
            message: format!("PMR-E-INTERNAL-CONTRACT: {message}"),
        })?;
    Ok(CompiledProject {
        bytecode,
        rewritten_source,
        host_exports,
        reference_visible_exports,
        event_dispatch_bindings,
    })
}

fn lower_project_source(
    strategy: ProjectLoweringStrategy,
    manifest: &ProjectManifest,
    active_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
    event_dispatch_plan: &EventDispatchPlan,
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
            event_dispatch_plan,
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
                event_dispatch_plan,
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
    event_dispatch_plan: &EventDispatchPlan,
) -> Result<String, ProjectCompileError> {
    match strategy {
        ProjectLoweringStrategy::ModuleAwareBindPlan => lower_module_source_module_aware(
            manifest,
            active_project,
            module,
            current_project,
            procedures,
            reference_order,
            event_dispatch_plan,
        ),
        ProjectLoweringStrategy::RewriteBridge => rewrite_module_source(
            manifest,
            active_project,
            module,
            current_project,
            procedures,
            reference_order,
            event_dispatch_plan,
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

fn validate_event_semantics(
    manifest: &ProjectManifest,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> Result<(), ProjectCompileError> {
    let mut class_public_members = BTreeMap::<(String, String), BTreeSet<String>>::new();
    let mut class_declared_events = BTreeMap::<(String, String), BTreeSet<String>>::new();

    for (project_name, module) in iter_all_modules(manifest, reference_order) {
        let project_key = normalize_identifier(project_name);
        let module_key = normalize_identifier(&module.module_name);
        let events = collect_declared_events(module);
        if !events.is_empty() {
            class_declared_events.insert((project_key.clone(), module_key.clone()), events);
        }
    }

    for decl in procedures {
        if decl.module_kind == ModuleKind::Class && decl.is_public {
            class_public_members
                .entry((decl.project_name.clone(), decl.module_name.clone()))
                .or_default()
                .insert(decl.procedure_name.clone());
        }
    }

    for (project_name, module) in iter_all_modules(manifest, reference_order) {
        let project_key = normalize_identifier(project_name);
        let module_key = normalize_identifier(&module.module_name);
        let module_name = format!("{project_name}.{}", module.module_name);
        for line in module.source.lines() {
            if parse_withevents_declaration(line).is_some()
                && !matches!(
                    module.module_kind,
                    ModuleKind::Class | ModuleKind::Document | ModuleKind::Form
                )
            {
                return Err(ProjectCompileError::WithEventsModuleKind {
                    module_name: module_name.clone(),
                });
            }

            if let Some(interface_name) = parse_implements_directive(line) {
                if module.module_kind != ModuleKind::Class {
                    return Err(ProjectCompileError::ImplementsModuleKind {
                        module_name: module_name.clone(),
                    });
                }

                let Some((iface_project, iface_module)) = resolve_interface_module(
                    manifest,
                    &project_key,
                    &interface_name,
                    reference_order,
                ) else {
                    return Err(ProjectCompileError::ImplementsInterfaceNotFound {
                        module_name: module_name.clone(),
                        interface_name,
                    });
                };

                let required_members = class_public_members
                    .get(&(iface_project.clone(), iface_module.clone()))
                    .cloned()
                    .unwrap_or_default();
                if required_members.is_empty() {
                    continue;
                }
                let implemented_members = procedures
                    .iter()
                    .filter(|decl| {
                        decl.project_name == project_key
                            && decl.module_name == module_key
                            && decl.module_kind == ModuleKind::Class
                    })
                    .map(|decl| decl.procedure_name.clone())
                    .collect::<BTreeSet<_>>();
                let iface_prefix = normalize_identifier(&interface_name);
                for member in required_members {
                    let expected = format!("{iface_prefix}_{member}");
                    if !implemented_members.contains(&expected) {
                        return Err(ProjectCompileError::ImplementsMemberMissing {
                            module_name: module_name.clone(),
                            interface_name: iface_prefix.clone(),
                            member_name: member,
                        });
                    }
                }
            }

            if let Some(event_name) = parse_raiseevent_name(line) {
                if module.module_kind != ModuleKind::Class {
                    return Err(ProjectCompileError::RaiseEventModuleKind {
                        module_name: module_name.clone(),
                    });
                }
                let key = (project_key.clone(), module_key.clone());
                let declared = class_declared_events.get(&key).cloned().unwrap_or_default();
                if !declared.contains(&event_name) {
                    return Err(ProjectCompileError::RaiseEventUndeclared {
                        module_name: module_name.clone(),
                        event_name,
                    });
                }
            }
        }
    }

    Ok(())
}

fn iter_all_modules<'a>(
    manifest: &'a ProjectManifest,
    reference_order: &'a BTreeMap<String, usize>,
) -> Vec<(&'a str, &'a ModuleUnit)> {
    let mut out = Vec::new();
    out.extend(
        manifest
            .modules
            .iter()
            .map(|module| (manifest.project_name.as_str(), module)),
    );

    let mut referenced = manifest.reference_projects.iter().collect::<Vec<_>>();
    referenced.sort_by_key(|entry| {
        reference_order
            .get(&normalize_identifier(&entry.project_name))
            .copied()
            .unwrap_or(usize::MAX)
    });
    for entry in referenced {
        out.extend(
            entry
                .modules
                .iter()
                .map(|module| (entry.project_name.as_str(), module)),
        );
    }
    out
}

fn resolve_interface_module(
    manifest: &ProjectManifest,
    current_project: &str,
    interface_name: &str,
    reference_order: &BTreeMap<String, usize>,
) -> Option<(String, String)> {
    let iface = normalize_identifier(interface_name);
    for module in &manifest.modules {
        if module.module_kind == ModuleKind::Class
            && normalize_identifier(&module.module_name) == iface
            && normalize_identifier(&manifest.project_name) == current_project
        {
            return Some((
                normalize_identifier(&manifest.project_name),
                normalize_identifier(&module.module_name),
            ));
        }
    }

    let mut referenced = manifest.reference_projects.iter().collect::<Vec<_>>();
    referenced.sort_by_key(|entry| {
        reference_order
            .get(&normalize_identifier(&entry.project_name))
            .copied()
            .unwrap_or(usize::MAX)
    });
    for entry in referenced {
        for module in &entry.modules {
            if module.module_kind == ModuleKind::Class
                && normalize_identifier(&module.module_name) == iface
            {
                return Some((
                    normalize_identifier(&entry.project_name),
                    normalize_identifier(&module.module_name),
                ));
            }
        }
    }

    None
}

fn collect_declared_events(module: &ModuleUnit) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in module.source.lines() {
        if let Some(name) = parse_event_declaration(line) {
            out.insert(name);
        }
    }
    out
}

fn parse_implements_directive(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = if lower.starts_with("implements ") {
        trimmed[11..].trim()
    } else {
        return None;
    };
    normalize_procedure_name(rest)
}

fn parse_event_declaration(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let payload = if lower.starts_with("event ") {
        trimmed[6..].trim()
    } else if lower.starts_with("public event ") {
        trimmed[13..].trim()
    } else if lower.starts_with("private event ") {
        trimmed[14..].trim()
    } else {
        return None;
    };
    let token = payload
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
        .next()
        .unwrap_or_default();
    normalize_procedure_name(token)
}

fn parse_withevents_declaration(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let payload = if lower.starts_with("dim withevents ") {
        trimmed[15..].trim()
    } else if lower.starts_with("public withevents ") {
        trimmed[18..].trim()
    } else if lower.starts_with("private withevents ") {
        trimmed[19..].trim()
    } else {
        return None;
    };
    let token = payload
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',' || ch == '(')
        .next()
        .unwrap_or_default();
    normalize_procedure_name(token)
}

fn parse_raiseevent_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("raiseevent ") {
        return None;
    }
    let payload = trimmed[10..].trim();
    let token = payload
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
        .next()
        .unwrap_or_default();
    normalize_procedure_name(token)
}

fn collect_project_procedures(manifest: &ProjectManifest) -> Vec<ProcedureDecl> {
    let mut procedures = Vec::new();
    let active_project = normalize_identifier(&manifest.project_name);
    for module in &manifest.modules {
        let module_name = normalize_identifier(&module.module_name);
        for line in module.source.lines() {
            if let Some((name, kind, is_public)) = parse_procedure_signature_line(line) {
                let param_count = procedure_signature_param_count(line).unwrap_or(0);
                let lowered_name = lowered_proc_symbol(&active_project, &module_name, &name);
                procedures.push(ProcedureDecl {
                    project_name: active_project.clone(),
                    module_name: module_name.clone(),
                    procedure_name: name,
                    lowered_name,
                    kind,
                    is_public,
                    param_count,
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
                    let param_count = procedure_signature_param_count(line).unwrap_or(0);
                    let lowered_name = lowered_proc_symbol(&project_name, &module_name, &name);
                    procedures.push(ProcedureDecl {
                        project_name: project_name.clone(),
                        module_name: module_name.clone(),
                        procedure_name: name,
                        lowered_name,
                        kind,
                        is_public,
                        param_count,
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

type EventDispatchKey = (String, String, String);
type EventDispatchPlan = BTreeMap<EventDispatchKey, Vec<EventDispatchRoute>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EventDispatchRoute {
    handler_symbol: String,
    sink_project_name: String,
    sink_module_name: String,
    withevents_var: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EarlyBoundBinding {
    qualified_type: String,
    create_selector: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalClassBinding {
    project_name: String,
    module_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalDimDecl {
    leading_ws: String,
    var_name: String,
    qualified_type: String,
    as_new: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalClassDimDecl {
    leading_ws: String,
    var_name: String,
    type_name: String,
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
    event_dispatch_plan: &EventDispatchPlan,
) -> Result<String, ProjectCompileError> {
    let current_module = normalize_identifier(&module.module_name);
    let mut out = Vec::new();
    let mut active_function_result: Option<(String, String)> = None;
    let mut early_bound = BTreeMap::<String, EarlyBoundBinding>::new();
    let mut internal_class_bindings = BTreeMap::<String, InternalClassBinding>::new();
    let mut withevents_bindings = BTreeSet::<String>::new();
    let mut next_internal_instance_id = 1i32;
    for line in module.source.lines() {
        let expanded = expand_bound_source_line(
            line,
            manifest,
            current_project,
            reference_order,
            &mut early_bound,
            &mut internal_class_bindings,
            &mut withevents_bindings,
            &mut next_internal_instance_id,
        )?;
        for expanded_line in expanded {
            let expanded_line = rewrite_internal_class_set_assignment(
                &expanded_line,
                current_project,
                &current_module,
                &internal_class_bindings,
                &withevents_bindings,
            );
            let expanded_line = rewrite_internal_class_member_dispatch(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let (plan, next_function_result) = build_line_bind_plan(
                manifest,
                active_project,
                module,
                current_project,
                &current_module,
                procedures,
                reference_order,
                event_dispatch_plan,
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
    out.extend(emit_event_guard_wrappers_for_module(
        current_project,
        &current_module,
        event_dispatch_plan,
        procedures,
        &withevents_bindings,
    ));
    Ok(out.join("\n"))
}

fn expand_bound_source_line(
    line: &str,
    manifest: &ProjectManifest,
    current_project: &str,
    reference_order: &BTreeMap<String, usize>,
    early_bound: &mut BTreeMap<String, EarlyBoundBinding>,
    internal_class_bindings: &mut BTreeMap<String, InternalClassBinding>,
    withevents_bindings: &mut BTreeSet<String>,
    next_internal_instance_id: &mut i32,
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

    if let Some(dim_decl) = parse_internal_class_dim_declaration(line)
        && let Some((target_project, target_module)) = resolve_interface_module(
            manifest,
            current_project,
            &dim_decl.type_name,
            reference_order,
        )
    {
        internal_class_bindings.insert(
            normalize_identifier(&dim_decl.var_name),
            InternalClassBinding {
                project_name: target_project,
                module_name: target_module,
            },
        );
        let mut out = vec![format!("{}Dim {}", dim_decl.leading_ws, dim_decl.var_name)];
        if dim_decl.as_new {
            out.push(format!(
                "{}{} = {}",
                dim_decl.leading_ws, dim_decl.var_name, *next_internal_instance_id
            ));
            *next_internal_instance_id = next_internal_instance_id.saturating_add(1);
        }
        return Ok(out);
    }

    if let Some((withevents_var, source_type)) = parse_withevents_declaration_binding(line)
        && let Some((target_project, target_module)) =
            resolve_event_source_module(manifest, current_project, &source_type, reference_order)
    {
        let leading_ws_len = line.len().saturating_sub(line.trim_start().len());
        let leading_ws = &line[..leading_ws_len];
        internal_class_bindings.insert(
            normalize_identifier(&withevents_var),
            InternalClassBinding {
                project_name: target_project,
                module_name: target_module,
            },
        );
        withevents_bindings.insert(normalize_identifier(&withevents_var));
        return Ok(vec![format!("{leading_ws}Public {withevents_var}")]);
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

fn parse_internal_class_dim_declaration(line: &str) -> Option<InternalClassDimDecl> {
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
    let type_token = rhs_trimmed
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
        .next()
        .unwrap_or_default()
        .trim();
    if type_token.is_empty() {
        return None;
    }
    let type_name = type_token
        .split('.')
        .next_back()
        .map(normalize_identifier)?;
    if type_name.is_empty() {
        return None;
    }
    Some(InternalClassDimDecl {
        leading_ws,
        var_name: var_name.to_string(),
        type_name,
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

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_member_dispatch(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
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
        let receiver = normalize_identifier(raw_name[..dot_idx].trim());
        let member = normalize_identifier(raw_name[dot_idx + 1..].trim());
        if receiver.is_empty() || member.is_empty() {
            cursor = close + 1;
            continue;
        }
        let Some((target, instance_arg)) = resolve_internal_class_member_target(
            &receiver,
            &member,
            raw_name,
            active_project,
            current_project,
            current_module,
            procedures,
            internal_class_bindings,
        )?
        else {
            cursor = close + 1;
            continue;
        };
        let args_raw = line[open + 1..close].trim();
        let args = split_top_level_args(args_raw)?;
        let mut rewritten_args = vec![instance_arg];
        rewritten_args.extend(args.into_iter().filter(|arg| !arg.trim().is_empty()));
        let replacement = format!("{target}({})", rewritten_args.join(", "));
        replacements.push((name_start, close + 1, replacement));
        cursor = close + 1;
    }
    let rewritten = if replacements.is_empty() {
        line.to_string()
    } else {
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
        out
    };

    rewrite_internal_class_call_statement_without_parens(
        &rewritten,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_internal_class_member_target(
    receiver: &str,
    member: &str,
    raw_name: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<Option<(String, String)>, ProjectCompileError> {
    let Some((target_project, target_module, instance_arg)) = internal_class_bindings
        .get(receiver)
        .map(|binding| {
            (
                binding.project_name.clone(),
                binding.module_name.clone(),
                receiver.to_string(),
            )
        })
        .or_else(|| {
            procedures
                .iter()
                .any(|decl| {
                    decl.project_name == current_project
                        && decl.module_name == receiver
                        && decl.module_kind == ModuleKind::Class
                })
                .then(|| {
                    (
                        current_project.to_string(),
                        receiver.to_string(),
                        "0".to_string(),
                    )
                })
        })
    else {
        return Ok(None);
    };

    let mut candidates = procedures
        .iter()
        .filter(|decl| {
            decl.project_name == target_project
                && decl.module_name == target_module
                && decl.procedure_name == member
                && is_visible_from_active_project(
                    decl,
                    active_project,
                    current_project,
                    current_module,
                )
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(ProjectCompileError::NameResolutionNotFound {
            name: raw_name.to_string(),
        });
    }
    candidates.sort_by_key(|decl| decl.lowered_name.clone());
    Ok(Some((candidates[0].lowered_name.clone(), instance_arg)))
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_call_statement_without_parens(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("call ") {
        return Ok(line.to_string());
    }
    let payload = trimmed[5..].trim_start();
    if payload.is_empty() || payload.contains('(') {
        return Ok(line.to_string());
    }
    let callee_end = payload.find(char::is_whitespace).unwrap_or(payload.len());
    let callee = payload[..callee_end].trim();
    let args_tail = payload[callee_end..].trim();
    let Some(dot_idx) = callee.find('.') else {
        return Ok(line.to_string());
    };
    let receiver = normalize_identifier(callee[..dot_idx].trim());
    let member = normalize_identifier(callee[dot_idx + 1..].trim());
    if receiver.is_empty() || member.is_empty() {
        return Ok(line.to_string());
    }
    let Some((target, instance_arg)) = resolve_internal_class_member_target(
        &receiver,
        &member,
        callee,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
    )?
    else {
        return Ok(line.to_string());
    };
    let joined_args = if args_tail.is_empty() {
        instance_arg
    } else {
        format!("{instance_arg}, {args_tail}")
    };
    Ok(format!(
        "{}Call {}({})",
        &line[..leading],
        target,
        joined_args
    ))
}

fn class_module_self_targets(
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
) -> BTreeSet<String> {
    procedures
        .iter()
        .filter(|decl| {
            decl.project_name == current_project
                && decl.module_name == current_module
                && decl.module_kind == ModuleKind::Class
        })
        .map(|decl| decl.lowered_name.clone())
        .collect()
}

fn rewrite_internal_class_self_call_statement_without_parens(
    line: &str,
    self_targets: &BTreeSet<String>,
) -> String {
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("call ") {
        return line.to_string();
    }
    let payload = trimmed[5..].trim_start();
    if payload.is_empty() || payload.contains('(') {
        return line.to_string();
    }
    let callee_end = payload.find(char::is_whitespace).unwrap_or(payload.len());
    let callee = payload[..callee_end].trim();
    if callee.contains('.') {
        return line.to_string();
    }
    let callee_normalized = normalize_identifier(callee);
    if !self_targets.contains(&callee_normalized) {
        return line.to_string();
    }
    let args_tail = payload[callee_end..].trim();
    let joined_args = if args_tail.is_empty() {
        "__oxvba_this_instance".to_string()
    } else {
        format!("__oxvba_this_instance, {args_tail}")
    };
    format!("{}Call {}({})", &line[..leading], callee, joined_args)
}

fn rewrite_internal_class_self_dispatch(
    line: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
) -> Result<String, ProjectCompileError> {
    let self_targets = class_module_self_targets(current_project, current_module, procedures);
    if self_targets.is_empty() {
        return Ok(line.to_string());
    }

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_rel) = line[cursor..].find('(') {
        let open = cursor + open_rel;
        let Some((name_start, name_end)) = invocation_name_span(line, open) else {
            cursor = open + 1;
            continue;
        };
        let raw_name = line[name_start..name_end].trim();
        if raw_name.contains('.') {
            cursor = open + 1;
            continue;
        }
        let normalized = normalize_identifier(raw_name);
        if !self_targets.contains(&normalized) {
            cursor = open + 1;
            continue;
        }
        let Some(close) = find_matching_paren(line, open) else {
            cursor = open + 1;
            continue;
        };
        let args_raw = line[open + 1..close].trim();
        let args = split_top_level_args(args_raw)?;
        let mut rewritten_args = vec!["__oxvba_this_instance".to_string()];
        rewritten_args.extend(args.into_iter().filter(|arg| !arg.trim().is_empty()));
        let replacement = format!("{raw_name}({})", rewritten_args.join(", "));
        replacements.push((name_start, close + 1, replacement));
        cursor = close + 1;
    }

    let rewritten = if replacements.is_empty() {
        line.to_string()
    } else {
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
        out
    };

    Ok(rewrite_internal_class_self_call_statement_without_parens(
        &rewritten,
        &self_targets,
    ))
}

fn rewrite_internal_class_set_assignment(
    line: &str,
    current_project: &str,
    current_module: &str,
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
    withevents_bindings: &BTreeSet<String>,
) -> String {
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("set ") {
        return line.to_string();
    }
    let payload = trimmed[4..].trim_start();
    let Some(eq_idx) = payload.find('=') else {
        return line.to_string();
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return line.to_string();
    }
    if !internal_class_bindings.contains_key(&normalize_identifier(lhs)) {
        return line.to_string();
    }
    let normalized_lhs = normalize_identifier(lhs);
    if withevents_bindings.contains(&normalized_lhs) {
        let binding_token = withevents_binding_token(current_project, current_module, lhs);
        return format!(
            "{}{} = __oxvba_withevents_set({}, {})",
            &line[..leading],
            lhs,
            binding_token,
            rhs
        );
    }
    format!("{}{} = {}", &line[..leading], lhs, rhs)
}

fn withevents_binding_token(project: &str, module: &str, var_name: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    let key = format!(
        "{}|{}|{}",
        normalize_identifier(project),
        normalize_identifier(module),
        normalize_identifier(var_name)
    );
    for byte in key.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    let token = (hash & 0x7fff_ffff) as i32;
    if token == 0 { 1 } else { token }
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

fn collect_event_dispatch_plan(
    manifest: &ProjectManifest,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
) -> EventDispatchPlan {
    let mut declared_events = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for (project_name, module) in iter_all_modules(manifest, reference_order) {
        if module.module_kind != ModuleKind::Class {
            continue;
        }
        let events = collect_declared_events(module);
        if events.is_empty() {
            continue;
        }
        declared_events.insert(
            (
                normalize_identifier(project_name),
                normalize_identifier(&module.module_name),
            ),
            events,
        );
    }

    let mut plan = EventDispatchPlan::new();
    for (project_name, module) in iter_all_modules(manifest, reference_order) {
        let project_key = normalize_identifier(project_name);
        let module_key = normalize_identifier(&module.module_name);
        let module_handlers = procedures
            .iter()
            .filter(|decl| decl.project_name == project_key && decl.module_name == module_key)
            .collect::<Vec<_>>();
        if module_handlers.is_empty() {
            continue;
        }
        for line in module.source.lines() {
            let Some((withevents_var, source_type)) = parse_withevents_declaration_binding(line)
            else {
                continue;
            };
            let Some((source_project, source_module)) =
                resolve_event_source_module(manifest, &project_key, &source_type, reference_order)
            else {
                continue;
            };
            let Some(available_events) =
                declared_events.get(&(source_project.clone(), source_module.clone()))
            else {
                continue;
            };
            let prefix = format!("{withevents_var}_");
            for handler in &module_handlers {
                let Some(event_name) = handler.procedure_name.strip_prefix(&prefix) else {
                    continue;
                };
                if event_name.is_empty() || !available_events.contains(event_name) {
                    continue;
                }
                let key = (
                    source_project.clone(),
                    source_module.clone(),
                    event_name.to_string(),
                );
                plan.entry(key).or_default().push(EventDispatchRoute {
                    handler_symbol: handler.lowered_name.clone(),
                    sink_project_name: project_key.clone(),
                    sink_module_name: module_key.clone(),
                    withevents_var: withevents_var.clone(),
                });
            }
        }
    }

    for routes in plan.values_mut() {
        routes.sort();
        routes.dedup();
    }

    plan
}

fn flatten_event_dispatch_plan(plan: &EventDispatchPlan) -> Vec<ProjectEventDispatchBinding> {
    let mut out = Vec::new();
    for ((project_name, module_name, event_name), routes) in plan {
        for route in routes {
            out.push(ProjectEventDispatchBinding {
                source_project_name: project_name.clone(),
                source_module_name: module_name.clone(),
                event_name: event_name.clone(),
                handler_symbol: route.handler_symbol.clone(),
            });
        }
    }
    out
}

fn resolve_event_source_module(
    manifest: &ProjectManifest,
    current_project: &str,
    source_type: &str,
    reference_order: &BTreeMap<String, usize>,
) -> Option<(String, String)> {
    resolve_interface_module(manifest, current_project, source_type, reference_order)
}

fn parse_withevents_declaration_binding(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let payload = if lower.starts_with("dim withevents ") {
        trimmed[15..].trim()
    } else if lower.starts_with("public withevents ") {
        trimmed[18..].trim()
    } else if lower.starts_with("private withevents ") {
        trimmed[19..].trim()
    } else {
        return None;
    };

    let (lhs, rhs) = split_keyword_ascii_ci(payload, " as ")?;
    let var_token = lhs
        .trim()
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',' || ch == '(')
        .next()
        .unwrap_or_default();
    let var_name = normalize_procedure_name(var_token)?;

    let mut rhs_trimmed = rhs.trim();
    if rhs_trimmed.len() >= 4 && rhs_trimmed[..4].eq_ignore_ascii_case("new ") {
        rhs_trimmed = rhs_trimmed[4..].trim();
    }
    let type_token = rhs_trimmed
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
        .next()
        .unwrap_or_default();
    let mut type_name = type_token
        .split('.')
        .next_back()
        .map(normalize_identifier)?;
    if type_name.is_empty() {
        return None;
    }
    type_name = normalize_identifier(&type_name);
    Some((var_name, type_name))
}

#[allow(clippy::too_many_arguments)]
fn rewrite_raiseevent_to_handler_calls(
    line: &str,
    manifest: &ProjectManifest,
    module: &ModuleUnit,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
    event_dispatch_plan: &EventDispatchPlan,
    active_function_result: Option<&(String, String)>,
) -> Result<Option<String>, ProjectCompileError> {
    if module.module_kind != ModuleKind::Class {
        return Ok(None);
    }
    let Some((event_name, args_payload)) = parse_raiseevent_invocation(line) else {
        return Ok(None);
    };
    let dispatch_key = (
        normalize_identifier(current_project),
        current_module.to_string(),
        event_name,
    );
    let routes = event_dispatch_plan
        .get(&dispatch_key)
        .cloned()
        .unwrap_or_default();
    if routes.is_empty() {
        return Ok(Some(String::new()));
    }

    let leading_ws_len = line.len().saturating_sub(line.trim_start().len());
    let leading_ws = &line[..leading_ws_len];
    let mut lowered_lines = Vec::new();
    let parsed_args = if let Some(args) = args_payload.as_ref() {
        split_top_level_args(args)?
    } else {
        Vec::new()
    };
    let event_arg_count = parsed_args
        .iter()
        .filter(|arg| !arg.trim().is_empty())
        .count();
    if event_arg_count > 1 {
        return Err(ProjectCompileError::BackendCompile {
            message:
                "BIND-E-EVENT-ARITY-UNSUPPORTED: class event dispatch currently supports up to one event argument"
                    .to_string(),
        });
    }

    for route in routes {
        let wrapper = event_guard_wrapper_symbol(
            current_project,
            current_module,
            &dispatch_key.2,
            &route,
            event_arg_count,
        );
        let mut call_line = if event_arg_count == 0 {
            format!("{leading_ws}Call {wrapper}(__oxvba_this_instance)")
        } else {
            format!(
                "{leading_ws}Call {wrapper}(__oxvba_this_instance, {})",
                parsed_args[0].trim()
            )
        };
        call_line = rewrite_invocation_targets(
            &call_line,
            manifest,
            active_project,
            current_project,
            current_module,
            procedures,
            reference_order,
        )?;
        if let Some((result_name, lowered_name)) = active_function_result {
            call_line = rewrite_bare_identifier(&call_line, result_name, lowered_name);
        }
        lowered_lines.push(call_line);
    }

    Ok(Some(lowered_lines.join("\n")))
}

fn parse_raiseevent_invocation(line: &str) -> Option<(String, Option<String>)> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("raiseevent ") {
        return None;
    }
    let payload = trimmed[10..].trim();
    if payload.is_empty() {
        return None;
    }
    let mut split_idx = payload.len();
    for (idx, ch) in payload.char_indices() {
        if ch.is_ascii_whitespace() || ch == '(' {
            split_idx = idx;
            break;
        }
    }
    let event_token = payload[..split_idx].trim();
    let event_name = normalize_procedure_name(event_token)?;

    let remainder = payload[split_idx..].trim();
    if remainder.is_empty() {
        return Some((event_name, None));
    }
    if remainder.starts_with('(') {
        let close = find_matching_paren(remainder, 0)?;
        let args = remainder[1..close].trim().to_string();
        return Some((event_name, Some(args)));
    }
    Some((event_name, Some(remainder.to_string())))
}

fn event_guard_wrapper_symbol(
    source_project: &str,
    source_module: &str,
    event_name: &str,
    route: &EventDispatchRoute,
    event_arg_count: usize,
) -> String {
    format!(
        "pmr_evtguard_{source_project}_{source_module}_{event_name}_{}_{}_a{}",
        route.sink_module_name, route.withevents_var, event_arg_count
    )
}

fn emit_event_guard_wrappers_for_module(
    current_project: &str,
    current_module: &str,
    event_dispatch_plan: &EventDispatchPlan,
    procedures: &[ProcedureDecl],
    withevents_bindings: &BTreeSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut emitted = BTreeSet::<String>::new();
    for ((source_project, source_module, event_name), routes) in event_dispatch_plan {
        for route in routes {
            if route.sink_project_name != current_project
                || route.sink_module_name != current_module
            {
                continue;
            }
            let handler_param_count = procedures
                .iter()
                .find(|decl| decl.lowered_name == route.handler_symbol)
                .map(|decl| decl.param_count)
                .unwrap_or(0);
            for event_arg_count in [0usize, 1usize] {
                let wrapper = event_guard_wrapper_symbol(
                    source_project,
                    source_module,
                    event_name,
                    route,
                    event_arg_count,
                );
                if !emitted.insert(wrapper.clone()) {
                    continue;
                }
                let normalized_var = normalize_identifier(&route.withevents_var);
                if !withevents_bindings.contains(&normalized_var) {
                    continue;
                }
                let binding_token = withevents_binding_token(
                    current_project,
                    current_module,
                    &route.withevents_var,
                );
                let guard_expr =
                    format!("__oxvba_withevents_get({binding_token}) = __oxvba_source_instance");
                let call_args = if handler_param_count == 0 {
                    "__oxvba_source_instance".to_string()
                } else if event_arg_count == 0 {
                    "__oxvba_source_instance, 0".to_string()
                } else {
                    "__oxvba_source_instance, __oxvba_arg0".to_string()
                };
                let wrapper_body = if event_arg_count == 0 {
                    format!(
                        "Sub {wrapper}(Optional ByVal __oxvba_source_instance = 0)\nIf {guard_expr} Then\nCall {}({call_args})\nEnd If\nEnd Sub",
                        route.handler_symbol,
                    )
                } else {
                    format!(
                        "Sub {wrapper}(Optional ByVal __oxvba_source_instance = 0, Optional ByVal __oxvba_arg0 = 0)\nIf {guard_expr} Then\nCall {}({call_args})\nEnd If\nEnd Sub",
                        route.handler_symbol,
                    )
                };
                out.push(wrapper_body);
            }
        }
    }
    out
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
    event_dispatch_plan: &EventDispatchPlan,
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
        let mut rewritten = rewrite_signature_name(&normalized, &decl.lowered_name);
        if module.module_kind == ModuleKind::Class {
            rewritten = inject_hidden_instance_param(&rewritten);
            rewritten = strip_signature_param_types(&rewritten);
        }
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

    if let Some(dispatch_line) = rewrite_raiseevent_to_handler_calls(
        &normalized,
        manifest,
        module,
        active_project,
        current_project,
        current_module,
        procedures,
        reference_order,
        event_dispatch_plan,
        active_function_result,
    )? {
        return Ok((
            LineBindPlan {
                drop_line: dispatch_line.is_empty(),
                lowered_line: dispatch_line,
                bound_call_targets: Vec::new(),
            },
            active_function_result.cloned(),
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
    if module.module_kind == ModuleKind::Class {
        lowered_line = rewrite_internal_class_self_dispatch(
            &lowered_line,
            current_project,
            current_module,
            procedures,
        )?;
    }
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
    event_dispatch_plan: &EventDispatchPlan,
) -> Result<String, ProjectCompileError> {
    let current_module = normalize_identifier(&module.module_name);
    let mut out = Vec::new();
    let mut active_function_result: Option<(String, String)> = None;
    let mut early_bound = BTreeMap::<String, EarlyBoundBinding>::new();
    let mut internal_class_bindings = BTreeMap::<String, InternalClassBinding>::new();
    let mut withevents_bindings = BTreeSet::<String>::new();
    let mut next_internal_instance_id = 1i32;
    for line in module.source.lines() {
        let expanded = expand_bound_source_line(
            line,
            manifest,
            current_project,
            reference_order,
            &mut early_bound,
            &mut internal_class_bindings,
            &mut withevents_bindings,
            &mut next_internal_instance_id,
        )?;
        for expanded_line in expanded {
            let expanded_line = rewrite_internal_class_set_assignment(
                &expanded_line,
                current_project,
                &current_module,
                &internal_class_bindings,
                &withevents_bindings,
            );
            let trimmed = expanded_line.trim();
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("attribute ") || lower == "option private module" {
                continue;
            }
            if module.module_kind == ModuleKind::Class && lower.starts_with("implements ") {
                continue;
            }
            let expanded_line = rewrite_internal_class_member_dispatch(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let normalized = normalize_visibility_prefixed_procedure_signature(&expanded_line);
            if let Some((proc_name, _, _)) = parse_procedure_signature_line(&normalized)
                && let Some(decl) =
                    find_decl_by_signature(procedures, current_project, &current_module, &proc_name)
            {
                let mut rewritten = rewrite_signature_name(&normalized, &decl.lowered_name);
                if module.module_kind == ModuleKind::Class {
                    rewritten = inject_hidden_instance_param(&rewritten);
                    rewritten = strip_signature_param_types(&rewritten);
                }
                if decl.kind == ExportKind::Function {
                    active_function_result = Some((proc_name, decl.lowered_name.clone()));
                } else {
                    active_function_result = None;
                }
                out.push(rewritten);
                continue;
            }
            if let Some(dispatch_line) = rewrite_raiseevent_to_handler_calls(
                &normalized,
                manifest,
                module,
                active_project,
                current_project,
                &current_module,
                procedures,
                reference_order,
                event_dispatch_plan,
                active_function_result.as_ref(),
            )? {
                if !dispatch_line.is_empty() {
                    out.push(dispatch_line);
                }
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
            if module.module_kind == ModuleKind::Class {
                rewritten = rewrite_internal_class_self_dispatch(
                    &rewritten,
                    current_project,
                    &current_module,
                    procedures,
                )?;
            }
            if let Some((result_name, lowered_name)) = &active_function_result {
                rewritten = rewrite_bare_identifier(&rewritten, result_name, lowered_name);
            }
            if lower.starts_with("end function") {
                active_function_result = None;
            }
            out.push(rewritten);
        }
    }
    out.extend(emit_event_guard_wrappers_for_module(
        current_project,
        &current_module,
        event_dispatch_plan,
        procedures,
        &withevents_bindings,
    ));
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

fn inject_hidden_instance_param(line: &str) -> String {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let lowered = trimmed.to_ascii_lowercase();
    let markers = [
        "sub ",
        "function ",
        "property get ",
        "property let ",
        "property set ",
    ];
    if !markers.iter().any(|marker| lowered.starts_with(marker)) {
        return line.to_string();
    }
    if lowered.starts_with("end ") {
        return line.to_string();
    }
    let hidden = "ByVal __oxvba_this_instance";
    if let Some(open_idx) = trimmed.find('(')
        && let Some(close_idx) = find_matching_paren(trimmed, open_idx)
    {
        let existing = trimmed[open_idx + 1..close_idx].trim();
        let joined = if existing.is_empty() {
            hidden.to_string()
        } else {
            format!("{hidden}, {existing}")
        };
        let mut out = String::new();
        out.push_str(&line[..leading]);
        out.push_str(&trimmed[..open_idx + 1]);
        out.push_str(&joined);
        out.push_str(&trimmed[close_idx..]);
        return out;
    }

    // Signature without explicit parens.
    format!("{}{}({hidden})", &line[..leading], trimmed)
}

fn strip_signature_param_types(line: &str) -> String {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let Some(open_idx) = trimmed.find('(') else {
        return line.to_string();
    };
    let Some(close_idx) = find_matching_paren(trimmed, open_idx) else {
        return line.to_string();
    };
    let payload = trimmed[open_idx + 1..close_idx].trim();
    let normalized = if payload.is_empty() {
        String::new()
    } else {
        payload
            .split(',')
            .map(|param| {
                let raw = param.trim();
                if raw.is_empty() {
                    return String::new();
                }
                if let Some((lhs, _)) = split_keyword_ascii_ci(raw, " as ") {
                    lhs.trim().to_string()
                } else {
                    raw.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut out = String::new();
    out.push_str(&line[..leading]);
    out.push_str(&trimmed[..open_idx + 1]);
    out.push_str(&normalized);
    out.push_str(&trimmed[close_idx..]);
    out
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

fn procedure_signature_param_count(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    let open = trimmed.find('(')?;
    let close = find_matching_paren(trimmed, open)?;
    let payload = trimmed[open + 1..close].trim();
    if payload.is_empty() {
        return Some(0);
    }
    let mut depth = 0i32;
    let mut count = 1usize;
    let mut in_string = false;
    for ch in payload.chars() {
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    Some(count)
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
        ExportKind, ModuleKind, ProjectCompileError, ProjectEventDispatchBinding, ProjectKind,
        ProjectLoweringStrategy, ProjectManifest, ProjectReference, ReferenceKind,
        ReferencedProjectManifest, compile_project, compile_project_with_strategy,
        module_unit_from_source, validate_compiled_project_contract,
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
    fn compile_project_rejects_withevents_in_procedural_module() {
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
        let err =
            compile_project(&manifest).expect_err("WithEvents should reject in procedural module");
        assert_eq!(err.code(), "PMR-E-WITHEVENTS-MODULE-KIND");
    }

    #[test]
    fn compile_project_allows_withevents_in_class_module() {
        let class_module = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Setup()\nDim WithEvents sink As Object\nEnd Sub",
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
        compile_project(&manifest).expect("WithEvents in class module should compile");
    }

    #[test]
    fn compile_project_rejects_raiseevent_undeclared_event() {
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
        let err = compile_project(&manifest).expect_err("RaiseEvent must target declared event");
        assert_eq!(err.code(), "PMR-E-RAISEEVENT-UNDECLARED");
    }

    #[test]
    fn compile_project_rejects_raiseevent_in_non_class_module() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nRaiseEvent Changed\nEnd Sub",
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
        let err =
            compile_project(&manifest).expect_err("RaiseEvent should reject in standard module");
        assert_eq!(err.code(), "PMR-E-RAISEEVENT-MODULE-KIND");
    }

    #[test]
    fn compile_project_allows_raiseevent_for_declared_event() {
        let class_module = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Event Changed()\nPublic Sub Fire()\nRaiseEvent Changed\nEnd Sub",
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
        compile_project(&manifest).expect("RaiseEvent should compile when event is declared");
    }

    #[test]
    fn compile_project_rewrites_raiseevent_to_known_withevents_handlers() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Emitter.Fire\nEnd Sub",
        )
        .expect("module parses");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\nPublic Sub Fire()\nRaiseEvent Changed\nEnd Sub",
        )
        .expect("module parses");
        let sink = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub em_changed()\nCall MainModule.Main\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("RaiseEvent rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains(
                "call pmr_evtguard_projecta_emitter_changed_sinka_em_a0(__oxvba_this_instance)"
            ),
            "RaiseEvent should lower to guard-dispatched handler call"
        );
        assert!(
            lowered.contains("call pmr_projecta_sinka_em_changed(__oxvba_source_instance)"),
            "guard wrapper should invoke concrete WithEvents handler"
        );
        assert_eq!(
            compiled.event_dispatch_bindings,
            vec![ProjectEventDispatchBinding {
                source_project_name: "projecta".to_string(),
                source_module_name: "emitter".to_string(),
                event_name: "changed".to_string(),
                handler_symbol: "pmr_projecta_sinka_em_changed".to_string(),
            }]
        );
    }

    #[test]
    fn compile_project_rewrites_withevents_set_and_guard_through_runtime_binding_intrinsics() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim e As New Emitter\nDim s As New Sink\nCall s.Attach(e)\nCall e.Fire(1)\nEnd Sub",
        )
        .expect("module parses");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Tick(ByVal n As Integer)\nPublic Sub Fire(ByVal n As Integer)\nRaiseEvent Tick(n)\nEnd Sub",
        )
        .expect("module parses");
        let sink = module_unit_from_source(
            "Sink",
            ModuleKind::Class,
            "Attribute VB_Name = \"Sink\"\nPrivate WithEvents src As Emitter\nPublic Sub Attach(ByVal e As Emitter)\nSet src = e\nEnd Sub\nPrivate Sub src_tick(ByVal n As Integer)\nCall MainModule.Main\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("__oxvba_withevents_set("),
            "WithEvents Set assignment should route through runtime binding setter"
        );
        assert!(
            lowered.contains("__oxvba_withevents_get("),
            "event guard should route through runtime binding getter"
        );
    }

    #[test]
    fn compile_project_event_dispatch_bindings_are_sorted_and_stable() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nCall Emitter.Fire\nEnd Sub",
        )
        .expect("module parses");
        let emitter = module_unit_from_source(
            "Emitter",
            ModuleKind::Class,
            "Attribute VB_Name = \"Emitter\"\nPublic Event Changed()\nPublic Sub Fire()\nRaiseEvent Changed\nEnd Sub",
        )
        .expect("module parses");
        let sink_b = module_unit_from_source(
            "SinkB",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkB\"\nPrivate WithEvents em As Emitter\nPublic Sub em_changed()\nEnd Sub",
        )
        .expect("module parses");
        let sink_a = module_unit_from_source(
            "SinkA",
            ModuleKind::Class,
            "Attribute VB_Name = \"SinkA\"\nPrivate WithEvents em As Emitter\nPublic Sub em_changed()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, emitter, sink_b, sink_a],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("event binding extraction should compile");
        let handlers = compiled
            .event_dispatch_bindings
            .iter()
            .map(|binding| binding.handler_symbol.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            handlers,
            vec![
                "pmr_projecta_sinka_em_changed".to_string(),
                "pmr_projecta_sinkb_em_changed".to_string()
            ]
        );
    }

    #[test]
    fn compile_project_rejects_implements_unknown_interface() {
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
            modules: vec![main_module, class_impl],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let err = compile_project(&manifest).expect_err("unknown interface should fail");
        assert_eq!(err.code(), "PMR-E-IMPLEMENTS-INTERFACE-NOT-FOUND");
    }

    #[test]
    fn compile_project_rejects_implements_in_non_class_module() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nImplements IThing\nPublic Sub Main()\nEnd Sub",
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
        let err =
            compile_project(&manifest).expect_err("Implements should reject outside class modules");
        assert_eq!(err.code(), "PMR-E-IMPLEMENTS-MODULE-KIND");
    }

    #[test]
    fn compile_project_rejects_implements_missing_member_coverage() {
        let class_interface = module_unit_from_source(
            "IThing",
            ModuleKind::Class,
            "Attribute VB_Name = \"IThing\"\nPublic Sub Ping()\nEnd Sub",
        )
        .expect("module parses");
        let class_impl = module_unit_from_source(
            "ThingImpl",
            ModuleKind::Class,
            "Attribute VB_Name = \"ThingImpl\"\nImplements IThing\nPrivate Sub NotTheInterfaceMethod()\nEnd Sub",
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
        let err =
            compile_project(&manifest).expect_err("missing interface implementation should fail");
        assert_eq!(err.code(), "PMR-E-IMPLEMENTS-MEMBER-MISSING");
    }

    #[test]
    fn compile_project_allows_implements_interface_from_referenced_project() {
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
        let ref_interface = module_unit_from_source(
            "IThing",
            ModuleKind::Class,
            "Attribute VB_Name = \"IThing\"\nPublic Sub Ping()\nEnd Sub",
        )
        .expect("reference interface parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, class_impl],
            references: vec![ProjectReference {
                referenced_project_name: "LibOne".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "LibOne".to_string(),
                modules: vec![ref_interface],
            }],
            conditional_constants: BTreeMap::new(),
        };
        compile_project(&manifest).expect("Implements should resolve reference project interface");
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
