use std::collections::{BTreeMap, BTreeSet};

use oxvba_com::{
    ComMemberSpec, TypeLibMemberInvokeKind, TypeLibMemberLookupResult, TypeLibMetadataBlob,
    build_typelib_metadata, create_object_selector_from_typelib_metadata,
    known_typelib_identity_for_prog_id_name,
    resolve_default_member_token_and_spec_from_typelib_metadata,
    resolve_member_token_and_spec_from_typelib_metadata_name,
};
use oxvba_runtime::ObjectHandle;
use thiserror::Error;

use crate::{Bytecode, ProcedureRuntimeMetadata, compile_with_runtime_metadata_object_locals};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDynamicMemberKind {
    Method,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcedureDeclKind {
    Sub,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

impl ProcedureDeclKind {
    fn export_kind(self) -> Option<ExportKind> {
        match self {
            Self::Sub => Some(ExportKind::Sub),
            Self::Function => Some(ExportKind::Function),
            Self::PropertyGet | Self::PropertyLet | Self::PropertySet => None,
        }
    }

    fn dynamic_member_kind(self) -> ProjectDynamicMemberKind {
        match self {
            Self::Sub => ProjectDynamicMemberKind::Method,
            Self::Function => ProjectDynamicMemberKind::Function,
            Self::PropertyGet => ProjectDynamicMemberKind::PropertyGet,
            Self::PropertyLet => ProjectDynamicMemberKind::PropertyLet,
            Self::PropertySet => ProjectDynamicMemberKind::PropertySet,
        }
    }

    fn has_return_value(self) -> bool {
        matches!(self, Self::Function | Self::PropertyGet)
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDynamicMemberRoute {
    pub member_name: String,
    pub lowered_name: String,
    pub known_dispatch_token: Option<i32>,
    pub is_default_member: bool,
    pub kind: ProjectDynamicMemberKind,
    pub visible_param_count: usize,
    pub entry_pc: usize,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDynamicObjectRoute {
    pub object_handle: ObjectHandle,
    pub project_name: String,
    pub module_name: String,
    pub members: Vec<ProjectDynamicMemberRoute>,
}

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub bytecode: Bytecode,
    pub procedure_runtime_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    pub rewritten_source: String,
    pub host_exports: Vec<HostProcedureExport>,
    pub reference_visible_exports: Vec<HostProcedureExport>,
    pub event_dispatch_bindings: Vec<ProjectEventDispatchBinding>,
    pub project_dynamic_objects: Vec<ProjectDynamicObjectRoute>,
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
        "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS: non-authoritative default-member target `{name}` is ambiguous"
    )]
    DefaultMemberResolutionAmbiguous { name: String },
    #[error(
        "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING: non-authoritative default-member target `{name}` has no visible candidate of the required kind"
    )]
    DefaultMemberResolutionMissing { name: String },
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
        "BIND-E-TYPELIB-MEMBER-NOT-FOUND: external invoke target `{target}` was not found in typelib metadata"
    )]
    TypeLibraryMemberNotFound { target: String },
    #[error(
        "BIND-E-TYPELIB-MEMBER-AMBIGUOUS: external invoke target `{target}` resolved ambiguously in typelib metadata"
    )]
    TypeLibraryMemberAmbiguous { target: String },
    #[error(
        "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED: external invoke target `{target}` expects {expected} args from typelib metadata, got {actual}"
    )]
    TypeLibraryInvokeArityUnsupported {
        target: String,
        expected: usize,
        actual: usize,
    },
    #[error(
        "BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED: external invoke target `{target}` uses unsupported imported member shape `{shape}` in the current deterministic early-bind subset"
    )]
    TypeLibraryMemberShapeUnsupported { target: String, shape: String },
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
            Self::DefaultMemberResolutionAmbiguous { .. } => {
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS"
            }
            Self::DefaultMemberResolutionMissing { .. } => {
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING"
            }
            Self::ProjectQualificationInvalid { .. } => "PMR-E-PROJECT-QUALIFICATION-INVALID",
            Self::CrossProjectReferenceUnsupported { .. } => {
                "PMR-E-REFERENCE-CROSS-PROJECT-UNSUPPORTED"
            }
            Self::TypeLibraryQualifierUnresolved { .. } => "BIND-E-TYPELIB-QUALIFIER-UNRESOLVED",
            Self::TypeLibraryCreateObjectUnsupported { .. } => {
                "BIND-E-TYPELIB-CREATEOBJECT-UNSUPPORTED"
            }
            Self::TypeLibraryMemberUnsupported { .. } => "BIND-E-TYPELIB-MEMBER-UNSUPPORTED",
            Self::TypeLibraryMemberNotFound { .. } => "BIND-E-TYPELIB-MEMBER-NOT-FOUND",
            Self::TypeLibraryMemberAmbiguous { .. } => "BIND-E-TYPELIB-MEMBER-AMBIGUOUS",
            Self::TypeLibraryInvokeArityUnsupported { .. } => {
                "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"
            }
            Self::TypeLibraryMemberShapeUnsupported { .. } => {
                "BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED"
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
    kind: ProcedureDeclKind,
    is_public: bool,
    is_default_member: bool,
    param_count: usize,
    module_kind: ModuleKind,
    option_private_module: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MemberAttributes {
    vb_user_mem_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectDynamicInstanceBindingDraft {
    object_handle: ObjectHandle,
    project_name: String,
    module_name: String,
}

type ForcedObjectLocalsByProc = BTreeMap<String, BTreeSet<String>>;
type LoweredProjectSource = (
    String,
    Vec<ProjectDynamicInstanceBindingDraft>,
    ForcedObjectLocalsByProc,
);

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

    let (rewritten_source, dynamic_instance_bindings, forced_object_locals_by_proc) =
        lower_project_source(
            strategy,
            manifest,
            &active_project,
            &procedure_index,
            &reference_order,
            &event_dispatch_plan,
        )?;

    let (bytecode, procedure_runtime_metadata) = compile_with_runtime_metadata_object_locals(
        &rewritten_source,
        &forced_object_locals_by_proc,
    )
    .map_err(|e| ProjectCompileError::BackendCompile {
        message: e.to_string(),
    })?;

    let host_exports = collect_host_exports(manifest, &procedure_index);
    let reference_visible_exports = collect_reference_visible_exports(manifest, &procedure_index);
    let event_dispatch_bindings = flatten_event_dispatch_plan(&event_dispatch_plan);
    let project_dynamic_objects = build_project_dynamic_object_routes(
        &dynamic_instance_bindings,
        &procedure_index,
        &procedure_runtime_metadata,
    );
    validate_compiled_project_contract(manifest, &host_exports, &reference_visible_exports)
        .map_err(|message| ProjectCompileError::BackendCompile {
            message: format!("PMR-E-INTERNAL-CONTRACT: {message}"),
        })?;
    Ok(CompiledProject {
        bytecode,
        procedure_runtime_metadata,
        rewritten_source,
        host_exports,
        reference_visible_exports,
        event_dispatch_bindings,
        project_dynamic_objects,
    })
}

fn lower_project_source(
    strategy: ProjectLoweringStrategy,
    manifest: &ProjectManifest,
    active_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
    event_dispatch_plan: &EventDispatchPlan,
) -> Result<LoweredProjectSource, ProjectCompileError> {
    let mut lowered_modules = Vec::new();
    let mut next_internal_instance_id = 1i32;
    let mut dynamic_instance_bindings = Vec::new();
    let mut forced_object_locals_by_proc = BTreeMap::<String, BTreeSet<String>>::new();
    for module in &manifest.modules {
        let (lowered, object_locals) = lower_module_source(
            strategy,
            manifest,
            active_project,
            module,
            active_project,
            procedures,
            reference_order,
            event_dispatch_plan,
            &mut next_internal_instance_id,
            &mut dynamic_instance_bindings,
        )?;
        lowered_modules.push(lowered);
        merge_forced_object_locals(&mut forced_object_locals_by_proc, object_locals);
    }
    for referenced in ordered_reference_projects(manifest) {
        let project_name = normalize_identifier(&referenced.project_name);
        for module in &referenced.modules {
            let (lowered, object_locals) = lower_module_source(
                strategy,
                manifest,
                active_project,
                module,
                &project_name,
                procedures,
                reference_order,
                event_dispatch_plan,
                &mut next_internal_instance_id,
                &mut dynamic_instance_bindings,
            )?;
            lowered_modules.push(lowered);
            merge_forced_object_locals(&mut forced_object_locals_by_proc, object_locals);
        }
    }
    Ok((
        lowered_modules.join("\n"),
        dynamic_instance_bindings,
        forced_object_locals_by_proc,
    ))
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
    next_internal_instance_id: &mut i32,
    dynamic_instance_bindings: &mut Vec<ProjectDynamicInstanceBindingDraft>,
) -> Result<(String, ForcedObjectLocalsByProc), ProjectCompileError> {
    match strategy {
        ProjectLoweringStrategy::ModuleAwareBindPlan => lower_module_source_module_aware(
            manifest,
            active_project,
            module,
            current_project,
            procedures,
            reference_order,
            event_dispatch_plan,
            next_internal_instance_id,
            dynamic_instance_bindings,
        ),
        ProjectLoweringStrategy::RewriteBridge => rewrite_module_source(
            manifest,
            active_project,
            module,
            current_project,
            procedures,
            reference_order,
            event_dispatch_plan,
            next_internal_instance_id,
            dynamic_instance_bindings,
        ),
    }
}

fn merge_forced_object_locals(
    target: &mut ForcedObjectLocalsByProc,
    source: ForcedObjectLocalsByProc,
) {
    for (proc_name, vars) in source {
        target.entry(proc_name).or_default().extend(vars);
    }
}

fn record_internal_class_object_local(
    forced_object_locals_by_proc: &mut ForcedObjectLocalsByProc,
    active_procedure_name: &Option<String>,
    line: &str,
    manifest: &ProjectManifest,
    current_project: &str,
    reference_order: &BTreeMap<String, usize>,
) {
    let Some(proc_name) = active_procedure_name else {
        return;
    };
    let Some(dim_decl) = parse_internal_class_dim_declaration(line) else {
        return;
    };
    if resolve_interface_module(
        manifest,
        current_project,
        &dim_decl.type_name,
        reference_order,
    )
    .is_none()
    {
        return;
    }
    forced_object_locals_by_proc
        .entry(proc_name.clone())
        .or_default()
        .insert(normalize_identifier(&dim_decl.var_name));
}

fn lowered_procedure_binding_for_line(
    line: &str,
    procedures: &[ProcedureDecl],
    current_project: &str,
    current_module: &str,
) -> Option<String> {
    let normalized = normalize_visibility_prefixed_procedure_signature(line);
    let (proc_name, kind, _) = parse_procedure_signature_line(&normalized)?;
    let decl = find_decl_by_signature(
        procedures,
        current_project,
        current_module,
        &proc_name,
        kind,
    )?;
    Some(decl.lowered_name.clone())
}

fn clear_active_procedure_name_if_end(active_procedure_name: &mut Option<String>, line: &str) {
    let lower = line.trim().to_ascii_lowercase();
    if lower == "end sub" || lower == "end function" || lower == "end property" {
        *active_procedure_name = None;
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
        let member_attributes = collect_member_attributes(&module.source);
        for line in module.source.lines() {
            if let Some((name, kind, is_public)) = parse_procedure_signature_line(line) {
                let param_count = procedure_signature_param_count(line).unwrap_or(0);
                let lowered_name = lowered_proc_symbol(&active_project, &module_name, &name, kind);
                procedures.push(ProcedureDecl {
                    project_name: active_project.clone(),
                    module_name: module_name.clone(),
                    procedure_name: name.clone(),
                    lowered_name,
                    kind,
                    is_public,
                    is_default_member: member_attributes
                        .get(&name)
                        .is_some_and(|attrs| attrs.vb_user_mem_id == Some(0)),
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
            let member_attributes = collect_member_attributes(&module.source);
            for line in module.source.lines() {
                if let Some((name, kind, is_public)) = parse_procedure_signature_line(line) {
                    let param_count = procedure_signature_param_count(line).unwrap_or(0);
                    let lowered_name =
                        lowered_proc_symbol(&project_name, &module_name, &name, kind);
                    procedures.push(ProcedureDecl {
                        project_name: project_name.clone(),
                        module_name: module_name.clone(),
                        procedure_name: name.clone(),
                        lowered_name,
                        kind,
                        is_public,
                        is_default_member: member_attributes
                            .get(&name)
                            .is_some_and(|attrs| attrs.vb_user_mem_id == Some(0)),
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

fn lowered_proc_symbol(
    project_name: &str,
    module_name: &str,
    procedure_name: &str,
    kind: ProcedureDeclKind,
) -> String {
    let base = lowered_proc_signature_symbol(project_name, module_name, procedure_name);
    match kind {
        ProcedureDeclKind::Sub | ProcedureDeclKind::Function => base,
        ProcedureDeclKind::PropertyGet => format!("property_get_{base}"),
        ProcedureDeclKind::PropertyLet => format!("property_let_{base}"),
        ProcedureDeclKind::PropertySet => format!("property_set_{base}"),
    }
}

fn lowered_proc_signature_symbol(
    project_name: &str,
    module_name: &str,
    procedure_name: &str,
) -> String {
    format!("pmr_{project_name}_{module_name}_{procedure_name}")
}

fn lowered_proc_signature_name(decl: &ProcedureDecl) -> &str {
    match decl.kind {
        ProcedureDeclKind::Sub | ProcedureDeclKind::Function => decl.lowered_name.as_str(),
        ProcedureDeclKind::PropertyGet => decl
            .lowered_name
            .strip_prefix("property_get_")
            .unwrap_or(decl.lowered_name.as_str()),
        ProcedureDeclKind::PropertyLet => decl
            .lowered_name
            .strip_prefix("property_let_")
            .unwrap_or(decl.lowered_name.as_str()),
        ProcedureDeclKind::PropertySet => decl
            .lowered_name
            .strip_prefix("property_set_")
            .unwrap_or(decl.lowered_name.as_str()),
    }
}

fn find_decl_by_signature<'a>(
    procedures: &'a [ProcedureDecl],
    project_name: &str,
    module_name: &str,
    procedure_name: &str,
    kind: ProcedureDeclKind,
) -> Option<&'a ProcedureDecl> {
    procedures.iter().find(|decl| {
        decl.project_name == project_name
            && decl.module_name == module_name
            && decl.procedure_name == procedure_name
            && decl.kind == kind
    })
}

fn find_decl_by_name<'a>(
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
    typelib_metadata: Option<TypeLibMetadataBlob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalClassBinding {
    project_name: String,
    module_name: String,
    generated_instance_id: Option<i32>,
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
    next_internal_instance_id: &mut i32,
    dynamic_instance_bindings: &mut Vec<ProjectDynamicInstanceBindingDraft>,
) -> Result<(String, BTreeMap<String, BTreeSet<String>>), ProjectCompileError> {
    let current_module = normalize_identifier(&module.module_name);
    let mut out = Vec::new();
    let mut active_function_result: Option<(String, String)> = None;
    let mut active_procedure_name: Option<String> = None;
    let mut early_bound = BTreeMap::<String, EarlyBoundBinding>::new();
    let mut internal_class_bindings = BTreeMap::<String, InternalClassBinding>::new();
    let mut forced_object_locals_by_proc = ForcedObjectLocalsByProc::new();
    let mut withevents_bindings = BTreeSet::<String>::new();
    let class_state_bindings =
        collect_class_state_bindings(module, current_project, &current_module);
    let source_lines = module_source_lines_with_class_terminate_cleanup(module);
    for line in &source_lines {
        record_internal_class_object_local(
            &mut forced_object_locals_by_proc,
            &active_procedure_name,
            line,
            manifest,
            current_project,
            reference_order,
        );
        let expanded = expand_bound_source_line(
            line,
            manifest,
            current_project,
            reference_order,
            procedures,
            &mut early_bound,
            &mut internal_class_bindings,
            &mut withevents_bindings,
            next_internal_instance_id,
            dynamic_instance_bindings,
        )?;
        for expanded_line in expanded {
            let expanded_line = rewrite_internal_class_set_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
                &withevents_bindings,
            )?;
            let expanded_line = rewrite_internal_class_property_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let expanded_line = rewrite_internal_class_default_member_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let state_assigned =
                rewrite_internal_class_state_assignment(&expanded_line, &class_state_bindings);
            let expanded_line = if state_assigned != expanded_line {
                state_assigned
            } else {
                rewrite_internal_class_state_reads(&state_assigned, &class_state_bindings)
            };
            let expanded_line = rewrite_internal_class_default_member_read_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let expanded_line = rewrite_internal_class_property_reads(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let expanded_line = rewrite_internal_class_member_dispatch(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let next_active_procedure_name = lowered_procedure_binding_for_line(
                &expanded_line,
                procedures,
                current_project,
                &current_module,
            );
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
            out.push(plan.lowered_line.clone());
            if let Some(proc_name) = next_active_procedure_name {
                active_procedure_name = Some(proc_name);
            } else {
                clear_active_procedure_name_if_end(&mut active_procedure_name, &plan.lowered_line);
            }
        }
    }
    out.extend(emit_event_guard_wrappers_for_module(
        current_project,
        &current_module,
        event_dispatch_plan,
        procedures,
        &withevents_bindings,
    ));
    Ok((out.join("\n"), forced_object_locals_by_proc))
}

fn module_source_lines_with_class_terminate_cleanup(module: &ModuleUnit) -> Vec<String> {
    if module.module_kind != ModuleKind::Class {
        return module.source.lines().map(|line| line.to_string()).collect();
    }

    let mut out = Vec::new();
    let mut in_class_terminate = false;
    for line in module.source.lines() {
        let normalized = normalize_visibility_prefixed_procedure_signature(line);
        if let Some((proc_name, _, _)) = parse_procedure_signature_line(&normalized) {
            in_class_terminate = proc_name.eq_ignore_ascii_case("class_terminate");
        }

        let lower = line.trim().to_ascii_lowercase();
        if in_class_terminate && lower == "end sub" {
            let leading_ws_len = line.len().saturating_sub(line.trim_start().len());
            let leading_ws = &line[..leading_ws_len];
            let cleanup_line = format!(
                "{leading_ws}__oxvba_this_instance = __oxvba_withevents_clear_owner(__oxvba_this_instance)"
            );
            let already_injected = out
                .last()
                .map(|prior: &String| prior.trim().eq_ignore_ascii_case(cleanup_line.trim()))
                .unwrap_or(false);
            if !already_injected {
                out.push(cleanup_line);
            }
            in_class_terminate = false;
        }
        out.push(line.to_string());
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn expand_bound_source_line(
    line: &str,
    manifest: &ProjectManifest,
    current_project: &str,
    reference_order: &BTreeMap<String, usize>,
    procedures: &[ProcedureDecl],
    early_bound: &mut BTreeMap<String, EarlyBoundBinding>,
    internal_class_bindings: &mut BTreeMap<String, InternalClassBinding>,
    withevents_bindings: &mut BTreeSet<String>,
    next_internal_instance_id: &mut i32,
    dynamic_instance_bindings: &mut Vec<ProjectDynamicInstanceBindingDraft>,
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
        let typelib_metadata = known_typelib_identity_for_prog_id_name(&dim_decl.qualified_type)
            .map(|identity| build_typelib_metadata(&identity));
        let selector = typelib_metadata
            .as_ref()
            .and_then(create_object_selector_from_typelib_metadata);
        early_bound.insert(
            normalize_identifier(&dim_decl.var_name),
            EarlyBoundBinding {
                qualified_type: dim_decl.qualified_type.clone(),
                create_selector: selector,
                typelib_metadata,
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
                "{}Set {} = CreateObject({selector})",
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
                project_name: target_project.clone(),
                module_name: target_module.clone(),
                generated_instance_id: dim_decl.as_new.then_some(*next_internal_instance_id),
            },
        );
        let mut out = vec![format!("{}Dim {}", dim_decl.leading_ws, dim_decl.var_name)];
        if dim_decl.as_new {
            let object_handle = ObjectHandle::new(*next_internal_instance_id);
            dynamic_instance_bindings.push(ProjectDynamicInstanceBindingDraft {
                object_handle,
                project_name: target_project.clone(),
                module_name: target_module.clone(),
            });
            out.push(format!(
                "{}{} = {}",
                dim_decl.leading_ws, dim_decl.var_name, *next_internal_instance_id
            ));
            if let Some(class_initialize) = find_decl_by_signature(
                procedures,
                &target_project,
                &target_module,
                "class_initialize",
                ProcedureDeclKind::Sub,
            ) {
                out.push(format!(
                    "{}Call {}({})",
                    dim_decl.leading_ws,
                    class_initialize.lowered_name,
                    object_handle.raw()
                ));
            }
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
                generated_instance_id: None,
            },
        );
        withevents_bindings.insert(normalize_identifier(&withevents_var));
        return Ok(vec![format!("{leading_ws}Public {withevents_var}")]);
    }

    let rewritten = rewrite_early_bound_property_assignment(line, early_bound)?;
    let rewritten = rewrite_early_bound_property_read_assignment(&rewritten, early_bound)?;
    let rewritten = rewrite_early_bound_member_dispatch(&rewritten, early_bound)?;
    let rewritten = rewrite_early_bound_call_statement_without_parens(&rewritten, early_bound)?;
    let rewritten =
        rewrite_early_bound_statement_invoke_without_parentheses(&rewritten, early_bound)?;
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
        let (var_name, target_name, member_token, member_spec) =
            if let Some(dot_idx) = raw_name.find('.') {
                let var_name = raw_name[..dot_idx].trim();
                let member_name = raw_name[dot_idx + 1..].trim();
                if var_name.is_empty() || member_name.is_empty() {
                    cursor = close + 1;
                    continue;
                }
                let key = normalize_identifier(var_name);
                let Some(binding) = early_bound.get(&key) else {
                    cursor = close + 1;
                    continue;
                };
                let target_name = format!("{}.{}", binding.qualified_type, member_name);
                let (member_token, member_spec) =
                    match resolve_early_bound_binding_member_token_and_spec(binding, member_name) {
                        KnownTypeLibMemberResolution::Resolved(member_token, member_spec) => {
                            (member_token, member_spec)
                        }
                        KnownTypeLibMemberResolution::Unsupported => {
                            return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                                member_name: member_name.to_string(),
                            });
                        }
                        KnownTypeLibMemberResolution::Missing => {
                            return Err(ProjectCompileError::TypeLibraryMemberNotFound {
                                target: target_name,
                            });
                        }
                        KnownTypeLibMemberResolution::Ambiguous => {
                            return Err(ProjectCompileError::TypeLibraryMemberAmbiguous {
                                target: target_name,
                            });
                        }
                    };
                (var_name.to_string(), target_name, member_token, member_spec)
            } else {
                let var_name = raw_name.trim();
                if var_name.is_empty() {
                    cursor = close + 1;
                    continue;
                }
                let key = normalize_identifier(var_name);
                let Some(binding) = early_bound.get(&key) else {
                    cursor = close + 1;
                    continue;
                };
                let default_target = format!("{}.<default>", binding.qualified_type);
                let (member_token, member_spec) =
                    match resolve_early_bound_binding_default_member_token_and_spec(binding) {
                        KnownTypeLibMemberResolution::Resolved(member_token, member_spec) => {
                            (member_token, member_spec)
                        }
                        KnownTypeLibMemberResolution::Unsupported => {
                            return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                                member_name: default_target,
                            });
                        }
                        KnownTypeLibMemberResolution::Missing => {
                            return Err(ProjectCompileError::TypeLibraryMemberNotFound {
                                target: default_target,
                            });
                        }
                        KnownTypeLibMemberResolution::Ambiguous => {
                            return Err(ProjectCompileError::TypeLibraryMemberAmbiguous {
                                target: default_target,
                            });
                        }
                    };
                (
                    var_name.to_string(),
                    format!("{}.{}", binding.qualified_type, member_spec.name),
                    member_token,
                    member_spec,
                )
            };
        if !matches!(
            member_spec.invoke_kind,
            TypeLibMemberInvokeKind::PropertyGet | TypeLibMemberInvokeKind::Method
        ) {
            return Err(ProjectCompileError::TypeLibraryMemberShapeUnsupported {
                target: target_name.clone(),
                shape: render_typelib_invoke_kind(member_spec.invoke_kind).to_string(),
            });
        }
        let args_raw = line[open + 1..close].trim();
        let args = split_top_level_args(args_raw)?;
        let actual_arity = args.iter().filter(|arg| !arg.trim().is_empty()).count();
        let expected_arity = member_spec.parameter_names.len();
        if actual_arity != expected_arity {
            return Err(ProjectCompileError::TypeLibraryInvokeArityUnsupported {
                target: target_name,
                expected: expected_arity,
                actual: actual_arity,
            });
        }
        let replacement = if args.is_empty() || args.iter().all(|arg| arg.trim().is_empty()) {
            format!("DispatchInvoke({var_name}, {member_token})")
        } else {
            let rendered_args = args
                .iter()
                .map(|arg| arg.trim())
                .filter(|arg| !arg.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            format!("DispatchInvoke({var_name}, {member_token}, {rendered_args})")
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

fn resolve_early_bound_invoke_target(
    raw_name: &str,
    early_bound: &BTreeMap<String, EarlyBoundBinding>,
) -> Result<Option<(String, String, i32, ComMemberSpec)>, ProjectCompileError> {
    if let Some(dot_idx) = raw_name.find('.') {
        let var_name = raw_name[..dot_idx].trim();
        let member_name = raw_name[dot_idx + 1..].trim();
        if var_name.is_empty() || member_name.is_empty() {
            return Ok(None);
        }
        let key = normalize_identifier(var_name);
        let Some(binding) = early_bound.get(&key) else {
            return Ok(None);
        };
        let target_name = format!("{}.{}", binding.qualified_type, member_name);
        let (member_token, member_spec) =
            match resolve_early_bound_binding_member_token_and_spec(binding, member_name) {
                KnownTypeLibMemberResolution::Resolved(member_token, member_spec) => {
                    (member_token, member_spec)
                }
                KnownTypeLibMemberResolution::Unsupported => {
                    return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                        member_name: member_name.to_string(),
                    });
                }
                KnownTypeLibMemberResolution::Missing => {
                    return Err(ProjectCompileError::TypeLibraryMemberNotFound {
                        target: target_name,
                    });
                }
                KnownTypeLibMemberResolution::Ambiguous => {
                    return Err(ProjectCompileError::TypeLibraryMemberAmbiguous {
                        target: target_name,
                    });
                }
            };
        Ok(Some((
            var_name.to_string(),
            format!("{}.{}", binding.qualified_type, member_spec.name),
            member_token,
            member_spec,
        )))
    } else {
        let var_name = raw_name.trim();
        if var_name.is_empty() {
            return Ok(None);
        }
        let key = normalize_identifier(var_name);
        let Some(binding) = early_bound.get(&key) else {
            return Ok(None);
        };
        let default_target = format!("{}.<default>", binding.qualified_type);
        let (member_token, member_spec) =
            match resolve_early_bound_binding_default_member_token_and_spec(binding) {
                KnownTypeLibMemberResolution::Resolved(member_token, member_spec) => {
                    (member_token, member_spec)
                }
                KnownTypeLibMemberResolution::Unsupported => {
                    return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                        member_name: default_target,
                    });
                }
                KnownTypeLibMemberResolution::Missing => {
                    return Err(ProjectCompileError::TypeLibraryMemberNotFound {
                        target: default_target,
                    });
                }
                KnownTypeLibMemberResolution::Ambiguous => {
                    return Err(ProjectCompileError::TypeLibraryMemberAmbiguous {
                        target: default_target,
                    });
                }
            };
        Ok(Some((
            var_name.to_string(),
            format!("{}.{}", binding.qualified_type, member_spec.name),
            member_token,
            member_spec,
        )))
    }
}

fn validate_early_bound_invoke_shape(
    target_name: &str,
    member_spec: ComMemberSpec,
    actual_arity: usize,
) -> Result<(), ProjectCompileError> {
    if !matches!(
        member_spec.invoke_kind,
        TypeLibMemberInvokeKind::PropertyGet | TypeLibMemberInvokeKind::Method
    ) {
        return Err(ProjectCompileError::TypeLibraryMemberShapeUnsupported {
            target: target_name.to_string(),
            shape: render_typelib_invoke_kind(member_spec.invoke_kind).to_string(),
        });
    }
    let expected_arity = member_spec.parameter_names.len();
    if actual_arity != expected_arity {
        return Err(ProjectCompileError::TypeLibraryInvokeArityUnsupported {
            target: target_name.to_string(),
            expected: expected_arity,
            actual: actual_arity,
        });
    }
    Ok(())
}

fn render_dispatch_invoke(var_name: &str, member_token: i32, args: &[String]) -> String {
    if args.is_empty() {
        format!("DispatchInvoke({var_name}, {member_token})")
    } else {
        let rendered_args = args
            .iter()
            .map(|arg| arg.trim())
            .filter(|arg| !arg.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        format!("DispatchInvoke({var_name}, {member_token}, {rendered_args})")
    }
}

fn rewrite_early_bound_call_statement_without_parens(
    line: &str,
    early_bound: &BTreeMap<String, EarlyBoundBinding>,
) -> Result<String, ProjectCompileError> {
    if early_bound.is_empty() {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("call ") {
        return Ok(line.to_string());
    }
    let payload = trimmed[5..].trim_start();
    if payload.is_empty()
        || payload.contains('(')
        || find_top_level_assignment_eq(payload).is_some()
    {
        return Ok(line.to_string());
    }
    let callee_end = payload.find(char::is_whitespace).unwrap_or(payload.len());
    let callee = payload[..callee_end].trim();
    if callee.is_empty() {
        return Ok(line.to_string());
    }
    let args_tail = payload[callee_end..].trim();
    let Some((var_name, target_name, member_token, member_spec)) =
        resolve_early_bound_invoke_target(callee, early_bound)?
    else {
        return Ok(line.to_string());
    };
    let args = if args_tail.is_empty() {
        Vec::new()
    } else {
        split_top_level_args(args_tail)?
            .into_iter()
            .map(|arg| arg.trim().to_string())
            .filter(|arg| !arg.is_empty())
            .collect::<Vec<_>>()
    };
    validate_early_bound_invoke_shape(&target_name, member_spec, args.len())?;
    Ok(format!(
        "{}Call {}",
        &line[..leading],
        render_dispatch_invoke(&var_name, member_token, &args)
    ))
}

fn rewrite_early_bound_statement_invoke_without_parentheses(
    line: &str,
    early_bound: &BTreeMap<String, EarlyBoundBinding>,
) -> Result<String, ProjectCompileError> {
    if early_bound.is_empty() {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.is_empty()
        || lower.starts_with("call ")
        || trimmed.contains('(')
        || find_top_level_assignment_eq(trimmed).is_some()
    {
        return Ok(line.to_string());
    }
    let callee_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    if callee_end == trimmed.len() {
        return Ok(line.to_string());
    }
    let callee = trimmed[..callee_end].trim();
    let args_tail = trimmed[callee_end..].trim();
    if callee.is_empty() || args_tail.is_empty() {
        return Ok(line.to_string());
    }
    let Some((var_name, target_name, member_token, member_spec)) =
        resolve_early_bound_invoke_target(callee, early_bound)?
    else {
        return Ok(line.to_string());
    };
    let args = split_top_level_args(args_tail)?
        .into_iter()
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    validate_early_bound_invoke_shape(&target_name, member_spec, args.len())?;
    Ok(format!(
        "{}{}",
        &line[..leading],
        render_dispatch_invoke(&var_name, member_token, &args)
    ))
}

fn rewrite_early_bound_property_assignment(
    line: &str,
    early_bound: &BTreeMap<String, EarlyBoundBinding>,
) -> Result<String, ProjectCompileError> {
    if early_bound.is_empty() || class_state_line_is_non_executable(line) {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lowered = trimmed.to_ascii_lowercase();
    let explicit_set = lowered.starts_with("set ");
    let explicit_let = lowered.starts_with("let ");
    let payload = if explicit_set || explicit_let {
        trimmed[4..].trim_start()
    } else {
        trimmed
    };
    let Some(eq_idx) = find_top_level_assignment_eq(payload) else {
        return Ok(line.to_string());
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return Ok(line.to_string());
    }
    let Some(dot_idx) = lhs.find('.') else {
        return Ok(line.to_string());
    };
    let var_name = lhs[..dot_idx].trim();
    let Some(binding) = early_bound.get(&normalize_identifier(var_name)) else {
        return Ok(line.to_string());
    };
    let member_expr = lhs[dot_idx + 1..].trim();
    let (member_name, mut args) = if let Some(open_idx) = member_expr.find('(') {
        let Some(close_idx) = find_matching_paren(member_expr, open_idx) else {
            return Ok(line.to_string());
        };
        if close_idx != member_expr.len().saturating_sub(1) {
            return Ok(line.to_string());
        }
        let member_name = member_expr[..open_idx].trim();
        if member_name.is_empty() {
            return Ok(line.to_string());
        }
        let args = split_top_level_args(member_expr[open_idx + 1..close_idx].trim())?
            .into_iter()
            .filter(|arg| !arg.trim().is_empty())
            .collect::<Vec<_>>();
        (member_name.to_string(), args)
    } else {
        if member_expr.is_empty() {
            return Ok(line.to_string());
        }
        (member_expr.to_string(), Vec::new())
    };
    let target_name = format!("{}.{}", binding.qualified_type, member_name);
    let (member_token, member_spec) =
        match resolve_early_bound_binding_member_token_and_spec(binding, &member_name) {
            KnownTypeLibMemberResolution::Resolved(member_token, member_spec) => {
                (member_token, member_spec)
            }
            KnownTypeLibMemberResolution::Unsupported => {
                return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                    member_name: member_name.to_string(),
                });
            }
            KnownTypeLibMemberResolution::Missing => {
                return Err(ProjectCompileError::TypeLibraryMemberNotFound {
                    target: target_name,
                });
            }
            KnownTypeLibMemberResolution::Ambiguous => {
                return Err(ProjectCompileError::TypeLibraryMemberAmbiguous {
                    target: target_name,
                });
            }
        };
    let supported_assignment_shape = if explicit_set {
        member_spec.invoke_kind == TypeLibMemberInvokeKind::PropertyPutRef
    } else {
        member_spec.invoke_kind == TypeLibMemberInvokeKind::PropertyPut
    };
    if !supported_assignment_shape {
        return Err(ProjectCompileError::TypeLibraryMemberShapeUnsupported {
            target: format!("{}.{}", binding.qualified_type, member_spec.name),
            shape: render_typelib_invoke_kind(member_spec.invoke_kind).to_string(),
        });
    }
    let has_named_args = args.iter().any(|arg| arg.contains(":="));
    if has_named_args {
        let value_name = member_spec
            .parameter_names
            .last()
            .cloned()
            .unwrap_or_else(|| "value".to_string());
        args.push(format!("{value_name} := {rhs}"));
    } else {
        args.push(rhs.to_string());
    }
    let actual_arity = args.iter().filter(|arg| !arg.trim().is_empty()).count();
    let expected_arity = member_spec.parameter_names.len();
    if actual_arity != expected_arity {
        return Err(ProjectCompileError::TypeLibraryInvokeArityUnsupported {
            target: format!("{}.{}", binding.qualified_type, member_spec.name),
            expected: expected_arity,
            actual: actual_arity,
        });
    }
    let rendered_args = args
        .iter()
        .map(|arg| arg.trim())
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "{}Call DispatchInvoke({}, {}, {})",
        &line[..leading],
        var_name,
        member_token,
        rendered_args
    ))
}

fn rewrite_early_bound_property_read_assignment(
    line: &str,
    early_bound: &BTreeMap<String, EarlyBoundBinding>,
) -> Result<String, ProjectCompileError> {
    if early_bound.is_empty() || class_state_line_is_non_executable(line) {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lowered = trimmed.to_ascii_lowercase();
    let explicit_set = lowered.starts_with("set ");
    let explicit_let = lowered.starts_with("let ");
    let payload = if explicit_set || explicit_let {
        trimmed[4..].trim_start()
    } else {
        trimmed
    };
    let Some(eq_idx) = payload.find('=') else {
        return Ok(line.to_string());
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() || rhs.contains('(') || rhs.contains(char::is_whitespace) {
        return Ok(line.to_string());
    }
    let Some(dot_idx) = rhs.find('.') else {
        return Ok(line.to_string());
    };
    let var_name = rhs[..dot_idx].trim();
    let Some(binding) = early_bound.get(&normalize_identifier(var_name)) else {
        return Ok(line.to_string());
    };
    let member_name = rhs[dot_idx + 1..].trim();
    if member_name.is_empty() {
        return Ok(line.to_string());
    }
    let target_name = format!("{}.{}", binding.qualified_type, member_name);
    let (member_token, member_spec) =
        match resolve_early_bound_binding_member_token_and_spec(binding, member_name) {
            KnownTypeLibMemberResolution::Resolved(member_token, member_spec) => {
                (member_token, member_spec)
            }
            KnownTypeLibMemberResolution::Unsupported => {
                return Err(ProjectCompileError::TypeLibraryMemberUnsupported {
                    member_name: member_name.to_string(),
                });
            }
            KnownTypeLibMemberResolution::Missing => {
                return Err(ProjectCompileError::TypeLibraryMemberNotFound {
                    target: target_name,
                });
            }
            KnownTypeLibMemberResolution::Ambiguous => {
                return Err(ProjectCompileError::TypeLibraryMemberAmbiguous {
                    target: target_name,
                });
            }
        };
    if !matches!(
        member_spec.invoke_kind,
        TypeLibMemberInvokeKind::PropertyGet | TypeLibMemberInvokeKind::Method
    ) || !member_spec.parameter_names.is_empty()
    {
        return Ok(line.to_string());
    }
    Ok(format!(
        "{}{}{} = DispatchInvoke({}, {})",
        &line[..leading],
        if explicit_set {
            "Set "
        } else if explicit_let {
            "Let "
        } else {
            ""
        },
        lhs,
        var_name,
        member_token
    ))
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
        let (target, instance_arg) = if let Some(dot_idx) = raw_name.find('.') {
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
            (target, instance_arg)
        } else {
            let receiver = normalize_identifier(raw_name);
            if receiver.is_empty() {
                cursor = close + 1;
                continue;
            }
            let Some((target, instance_arg)) =
                resolve_internal_class_default_member_target_of_kinds(
                    &receiver,
                    active_project,
                    current_project,
                    current_module,
                    procedures,
                    internal_class_bindings,
                    &[ProcedureDeclKind::PropertyGet],
                )?
            else {
                cursor = close + 1;
                continue;
            };
            (target, instance_arg)
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

    let rewritten = rewrite_internal_class_call_statement_without_parens(
        &rewritten,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
    )?;

    rewrite_internal_class_statement_invoke_without_parentheses(
        &rewritten,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
    )
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_property_reads(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    if internal_class_bindings.is_empty() || class_state_line_is_non_executable(line) {
        return Ok(line.to_string());
    }
    let rewritten = rewrite_internal_class_property_expression_reads(
        line,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
    )?;
    rewrite_internal_class_default_member_statement_reads(
        &rewritten,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
    )
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_default_member_statement_reads(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    if trimmed.is_empty()
        || trimmed.contains(char::is_whitespace)
        || trimmed.contains('.')
        || trimmed.contains('=')
        || trimmed.contains('(')
        || trimmed.contains(')')
        || trimmed.contains(',')
    {
        return Ok(line.to_string());
    }
    let receiver = normalize_identifier(trimmed);
    if receiver.is_empty() {
        return Ok(line.to_string());
    }
    let Some((target, instance_arg)) = resolve_internal_class_default_member_target_of_kinds(
        &receiver,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
        &[ProcedureDeclKind::PropertyGet],
    )?
    else {
        return Ok(line.to_string());
    };
    Ok(format!("{}{}({})", &line[..leading], target, instance_arg))
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_property_expression_reads(
    text: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    let bytes = text.as_bytes();
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if !is_identifier_byte(bytes[index]) || (index > 0 && is_identifier_byte(bytes[index - 1]))
        {
            index += 1;
            continue;
        }
        let receiver_start = index;
        while index < bytes.len() && is_identifier_byte(bytes[index]) {
            index += 1;
        }
        let receiver_end = index;
        if index >= bytes.len() || bytes[index] != b'.' {
            continue;
        }
        let member_start = index + 1;
        if member_start >= bytes.len() || !is_identifier_byte(bytes[member_start]) {
            continue;
        }
        index = member_start;
        while index < bytes.len() && is_identifier_byte(bytes[index]) {
            index += 1;
        }
        let member_end = index;
        let mut next = member_end;
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if text[..receiver_start].trim().is_empty()
            && next < bytes.len()
            && is_identifier_byte(bytes[next])
        {
            continue;
        }
        if next < bytes.len() && (bytes[next] == b'(' || bytes[next] == b'=') {
            continue;
        }
        let receiver = normalize_identifier(&text[receiver_start..receiver_end]);
        let member = normalize_identifier(&text[member_start..member_end]);
        if receiver.is_empty() || member.is_empty() {
            continue;
        }
        let raw_name = &text[receiver_start..member_end];
        if let Some((target, instance_arg)) = resolve_internal_class_member_target_of_kinds(
            &receiver,
            &member,
            raw_name,
            active_project,
            current_project,
            current_module,
            procedures,
            internal_class_bindings,
            &[ProcedureDeclKind::PropertyGet],
        )? {
            replacements.push((
                receiver_start,
                member_end,
                format!("{}({})", target, instance_arg),
            ));
        }
    }
    if replacements.is_empty() {
        return Ok(text.to_string());
    }
    let mut out = String::with_capacity(text.len() + 32);
    let mut previous = 0usize;
    for (start, end, replacement) in replacements {
        if start < previous || end > text.len() || start >= end {
            continue;
        }
        out.push_str(&text[previous..start]);
        out.push_str(&replacement);
        previous = end;
    }
    out.push_str(&text[previous..]);
    Ok(out)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[allow(clippy::too_many_arguments)]
fn resolve_internal_class_member_target_of_kinds(
    receiver: &str,
    member: &str,
    raw_name: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
    allowed_kinds: &[ProcedureDeclKind],
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
                && (allowed_kinds.is_empty() || allowed_kinds.contains(&decl.kind))
                && is_visible_from_active_project(
                    decl,
                    active_project,
                    current_project,
                    current_module,
                )
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return if allowed_kinds.is_empty() {
            Err(ProjectCompileError::NameResolutionNotFound {
                name: raw_name.to_string(),
            })
        } else {
            Ok(None)
        };
    }
    candidates.sort_by_key(|decl| decl.lowered_name.clone());
    Ok(Some((candidates[0].lowered_name.clone(), instance_arg)))
}

#[allow(clippy::too_many_arguments)]
fn resolve_internal_class_default_member_target_of_kinds(
    receiver: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
    allowed_kinds: &[ProcedureDeclKind],
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
                && decl.is_default_member
                && (allowed_kinds.is_empty() || allowed_kinds.contains(&decl.kind))
                && is_visible_from_active_project(
                    decl,
                    active_project,
                    current_project,
                    current_module,
                )
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        candidates = procedures
            .iter()
            .filter(|decl| {
                decl.project_name == target_project
                    && decl.module_name == target_module
                    && (allowed_kinds.is_empty() || allowed_kinds.contains(&decl.kind))
                    && is_visible_from_active_project(
                        decl,
                        active_project,
                        current_project,
                        current_module,
                    )
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(ProjectCompileError::DefaultMemberResolutionMissing {
                name: receiver.to_string(),
            });
        }
        if candidates.len() != 1 {
            return Err(ProjectCompileError::DefaultMemberResolutionAmbiguous {
                name: receiver.to_string(),
            });
        }
    }
    candidates.sort_by_key(|decl| decl.lowered_name.clone());
    Ok(Some((candidates[0].lowered_name.clone(), instance_arg)))
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
    resolve_internal_class_member_target_of_kinds(
        receiver,
        member,
        raw_name,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
        &[],
    )
}
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
    let (target, instance_arg) = if let Some(dot_idx) = callee.find('.') {
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
        (target, instance_arg)
    } else {
        let receiver = normalize_identifier(callee);
        if receiver.is_empty() {
            return Ok(line.to_string());
        }
        let Some((target, instance_arg)) = resolve_internal_class_default_member_target_of_kinds(
            &receiver,
            active_project,
            current_project,
            current_module,
            procedures,
            internal_class_bindings,
            &[ProcedureDeclKind::PropertyGet],
        )?
        else {
            return Ok(line.to_string());
        };
        (target, instance_arg)
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
fn rewrite_internal_class_statement_invoke_without_parentheses(
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
    if trimmed.is_empty()
        || lower.starts_with("call ")
        || trimmed.contains('(')
        || trimmed.contains('=')
    {
        return Ok(line.to_string());
    }
    let callee_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    if callee_end == trimmed.len() {
        return Ok(line.to_string());
    }
    let callee = trimmed[..callee_end].trim();
    let args_tail = trimmed[callee_end..].trim();
    if args_tail.is_empty() {
        return Ok(line.to_string());
    }
    let (target, instance_arg) = if let Some(dot_idx) = callee.find('.') {
        let receiver = normalize_identifier(callee[..dot_idx].trim());
        let member = normalize_identifier(callee[dot_idx + 1..].trim());
        if receiver.is_empty() || member.is_empty() {
            return Ok(line.to_string());
        }
        let Some((target, instance_arg)) = resolve_internal_class_member_target_of_kinds(
            &receiver,
            &member,
            callee,
            active_project,
            current_project,
            current_module,
            procedures,
            internal_class_bindings,
            &[ProcedureDeclKind::PropertyGet],
        )?
        else {
            return Ok(line.to_string());
        };
        (target, instance_arg)
    } else {
        let receiver = normalize_identifier(callee);
        if receiver.is_empty() {
            return Ok(line.to_string());
        }
        let Some((target, instance_arg)) = resolve_internal_class_default_member_target_of_kinds(
            &receiver,
            active_project,
            current_project,
            current_module,
            procedures,
            internal_class_bindings,
            &[ProcedureDeclKind::PropertyGet],
        )?
        else {
            return Ok(line.to_string());
        };
        (target, instance_arg)
    };
    Ok(format!(
        "{}{}({}, {})",
        &line[..leading],
        target,
        instance_arg,
        args_tail
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
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
    withevents_bindings: &BTreeSet<String>,
) -> Result<String, ProjectCompileError> {
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("set ") {
        return Ok(line.to_string());
    }
    let payload = trimmed[4..].trim_start();
    let Some(eq_idx) = payload.find('=') else {
        return Ok(line.to_string());
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return Ok(line.to_string());
    }
    if let Some(dot_idx) = lhs.find('.') {
        let receiver = normalize_identifier(lhs[..dot_idx].trim());
        let member_expr = lhs[dot_idx + 1..].trim();
        let (member, mut indexed_args) = if let Some(open_idx) = member_expr.find('(') {
            let Some(close_idx) = find_matching_paren(member_expr, open_idx) else {
                return Ok(line.to_string());
            };
            if close_idx != member_expr.len().saturating_sub(1) {
                return Ok(line.to_string());
            }
            let member_name = normalize_identifier(member_expr[..open_idx].trim());
            if member_name.is_empty() {
                return Ok(line.to_string());
            }
            let args_raw = member_expr[open_idx + 1..close_idx].trim();
            let args = split_top_level_args(args_raw)?
                .into_iter()
                .filter(|arg| !arg.trim().is_empty())
                .collect::<Vec<_>>();
            (member_name, args)
        } else {
            let member_name = normalize_identifier(member_expr);
            if member_name.is_empty() {
                return Ok(line.to_string());
            }
            (member_name, Vec::new())
        };
        if !receiver.is_empty()
            && let Some((target, instance_arg)) = resolve_internal_class_member_target_of_kinds(
                &receiver,
                &member,
                lhs,
                active_project,
                current_project,
                current_module,
                procedures,
                internal_class_bindings,
                &[ProcedureDeclKind::PropertySet],
            )?
        {
            let mut lowered_args = vec![instance_arg];
            lowered_args.append(&mut indexed_args);
            lowered_args.push(rhs.to_string());
            return Ok(format!(
                "{}{}({})",
                &line[..leading],
                target,
                lowered_args.join(", ")
            ));
        }
    }
    let (normalized_lhs, mut indexed_args) = if let Some(open_idx) = lhs.find('(') {
        let Some(close_idx) = find_matching_paren(lhs, open_idx) else {
            return Ok(line.to_string());
        };
        if close_idx != lhs.len().saturating_sub(1) {
            return Ok(line.to_string());
        }
        let receiver_name = normalize_identifier(lhs[..open_idx].trim());
        if receiver_name.is_empty() {
            return Ok(line.to_string());
        }
        let args_raw = lhs[open_idx + 1..close_idx].trim();
        let args = split_top_level_args(args_raw)?
            .into_iter()
            .filter(|arg| !arg.trim().is_empty())
            .collect::<Vec<_>>();
        (receiver_name, args)
    } else {
        let receiver_name = normalize_identifier(lhs);
        if receiver_name.is_empty() {
            return Ok(line.to_string());
        }
        (receiver_name, Vec::new())
    };
    if !internal_class_bindings.contains_key(&normalized_lhs) {
        return Ok(line.to_string());
    }
    if withevents_bindings.contains(&normalized_lhs) {
        let binding_token = withevents_binding_token(current_project, current_module, lhs);
        return Ok(format!(
            "{}{} = __oxvba_withevents_set(__oxvba_this_instance, {}, {})",
            &line[..leading],
            lhs,
            binding_token,
            rhs
        ));
    }
    if let Some((target, instance_arg)) = resolve_internal_class_default_member_target_of_kinds(
        &normalized_lhs,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
        &[ProcedureDeclKind::PropertySet],
    )? {
        let mut lowered_args = vec![instance_arg];
        lowered_args.append(&mut indexed_args);
        lowered_args.push(rhs.to_string());
        return Ok(format!(
            "{}{}({})",
            &line[..leading],
            target,
            lowered_args.join(", ")
        ));
    }
    Ok(format!("{}{} = {}", &line[..leading], lhs, rhs))
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_property_assignment(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    if internal_class_bindings.is_empty() || class_state_line_is_non_executable(line) {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("set ") {
        return Ok(line.to_string());
    }
    let payload = if lowered.starts_with("let ") {
        trimmed[4..].trim_start()
    } else {
        trimmed
    };
    let Some(eq_idx) = payload.find('=') else {
        return Ok(line.to_string());
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return Ok(line.to_string());
    }
    let Some(dot_idx) = lhs.find('.') else {
        return Ok(line.to_string());
    };
    let receiver = normalize_identifier(lhs[..dot_idx].trim());
    let member_expr = lhs[dot_idx + 1..].trim();
    let (member, mut indexed_args) = if let Some(open_idx) = member_expr.find('(') {
        let Some(close_idx) = find_matching_paren(member_expr, open_idx) else {
            return Ok(line.to_string());
        };
        if close_idx != member_expr.len().saturating_sub(1) {
            return Ok(line.to_string());
        }
        let member_name = normalize_identifier(member_expr[..open_idx].trim());
        if member_name.is_empty() {
            return Ok(line.to_string());
        }
        let args_raw = member_expr[open_idx + 1..close_idx].trim();
        let args = split_top_level_args(args_raw)?
            .into_iter()
            .filter(|arg| !arg.trim().is_empty())
            .collect::<Vec<_>>();
        (member_name, args)
    } else {
        let member_name = normalize_identifier(member_expr);
        if member_name.is_empty() {
            return Ok(line.to_string());
        }
        (member_name, Vec::new())
    };
    if receiver.is_empty() {
        return Ok(line.to_string());
    }
    let Some((target, instance_arg)) = resolve_internal_class_member_target_of_kinds(
        &receiver,
        &member,
        lhs,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
        &[ProcedureDeclKind::PropertyLet],
    )?
    else {
        return Ok(line.to_string());
    };
    let mut lowered_args = vec![instance_arg];
    lowered_args.append(&mut indexed_args);
    lowered_args.push(rhs.to_string());
    Ok(format!(
        "{}{}({})",
        &line[..leading],
        target,
        lowered_args.join(", ")
    ))
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_default_member_assignment(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    if internal_class_bindings.is_empty() || class_state_line_is_non_executable(line) {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("set ") {
        return Ok(line.to_string());
    }
    let payload = if lowered.starts_with("let ") {
        trimmed[4..].trim_start()
    } else {
        trimmed
    };
    let Some(eq_idx) = payload.find('=') else {
        return Ok(line.to_string());
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() || lhs.contains('.') {
        return Ok(line.to_string());
    }
    if rhs.starts_with("__oxvba_withevents_set(") {
        return Ok(line.to_string());
    }
    let (receiver, mut indexed_args) = if let Some(open_idx) = lhs.find('(') {
        let Some(close_idx) = find_matching_paren(lhs, open_idx) else {
            return Ok(line.to_string());
        };
        if close_idx != lhs.len().saturating_sub(1) {
            return Ok(line.to_string());
        }
        let receiver_name = normalize_identifier(lhs[..open_idx].trim());
        if receiver_name.is_empty() {
            return Ok(line.to_string());
        }
        let args_raw = lhs[open_idx + 1..close_idx].trim();
        let args = split_top_level_args(args_raw)?
            .into_iter()
            .filter(|arg| !arg.trim().is_empty())
            .collect::<Vec<_>>();
        (receiver_name, args)
    } else {
        let receiver_name = normalize_identifier(lhs);
        if receiver_name.is_empty() {
            return Ok(line.to_string());
        }
        (receiver_name, Vec::new())
    };
    if receiver.is_empty() {
        return Ok(line.to_string());
    }
    if let Some(binding) = internal_class_bindings.get(&receiver)
        && indexed_args.is_empty()
        && let Ok(instance_id) = rhs.parse::<i32>()
        && binding.generated_instance_id == Some(instance_id)
    {
        return Ok(line.to_string());
    }
    let Some((target, instance_arg)) = resolve_internal_class_default_member_target_of_kinds(
        &receiver,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
        &[ProcedureDeclKind::PropertyLet],
    )?
    else {
        return Ok(line.to_string());
    };
    let mut lowered_args = vec![instance_arg];
    lowered_args.append(&mut indexed_args);
    lowered_args.push(rhs.to_string());
    Ok(format!(
        "{}{}({})",
        &line[..leading],
        target,
        lowered_args.join(", ")
    ))
}

#[allow(clippy::too_many_arguments)]
fn rewrite_internal_class_default_member_read_assignment(
    line: &str,
    active_project: &str,
    current_project: &str,
    current_module: &str,
    procedures: &[ProcedureDecl],
    internal_class_bindings: &BTreeMap<String, InternalClassBinding>,
) -> Result<String, ProjectCompileError> {
    if internal_class_bindings.is_empty() || class_state_line_is_non_executable(line) {
        return Ok(line.to_string());
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let lowered = trimmed.to_ascii_lowercase();
    let explicit_set = lowered.starts_with("set ");
    let explicit_let = lowered.starts_with("let ");
    let payload = if explicit_set || explicit_let {
        trimmed[4..].trim_start()
    } else {
        trimmed
    };
    let Some(eq_idx) = payload.find('=') else {
        return Ok(line.to_string());
    };
    let lhs = payload[..eq_idx].trim();
    let rhs = payload[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() || lhs.contains('.') || rhs.contains('.') {
        return Ok(line.to_string());
    }
    let normalized_lhs = normalize_identifier(lhs);
    if procedures.iter().any(|procedure| {
        procedure.project_name.eq_ignore_ascii_case(current_project)
            && procedure.module_name.eq_ignore_ascii_case(current_module)
            && normalize_identifier(&procedure.procedure_name) == normalized_lhs
            && procedure.kind.has_return_value()
    }) {
        return Ok(line.to_string());
    }
    let (rhs_name, indexed_args) = if let Some(open_idx) = rhs.find('(') {
        let Some(close_idx) = find_matching_paren(rhs, open_idx) else {
            return Ok(line.to_string());
        };
        if close_idx != rhs.len().saturating_sub(1) {
            return Ok(line.to_string());
        }
        let receiver_name = normalize_identifier(rhs[..open_idx].trim());
        if receiver_name.is_empty() {
            return Ok(line.to_string());
        }
        let args_raw = rhs[open_idx + 1..close_idx].trim();
        let args = split_top_level_args(args_raw)?
            .into_iter()
            .filter(|arg| !arg.trim().is_empty())
            .collect::<Vec<_>>();
        (receiver_name, args)
    } else {
        let receiver_name = normalize_identifier(rhs);
        if receiver_name.is_empty() {
            return Ok(line.to_string());
        }
        (receiver_name, Vec::new())
    };
    if rhs_name.is_empty() {
        return Ok(line.to_string());
    }
    let resolved_target = resolve_internal_class_default_member_target_of_kinds(
        &rhs_name,
        active_project,
        current_project,
        current_module,
        procedures,
        internal_class_bindings,
        &[ProcedureDeclKind::PropertyGet],
    )?;
    let Some((target, instance_arg)) = resolved_target else {
        return Ok(line.to_string());
    };
    let mut lowered_args = vec![instance_arg];
    lowered_args.extend(indexed_args);
    Ok(format!(
        "{}{}{} = {}({})",
        &line[..leading],
        if explicit_set {
            "Set "
        } else if explicit_let {
            "Let "
        } else {
            ""
        },
        lhs,
        target,
        lowered_args.join(", ")
    ))
}
fn collect_class_state_bindings(
    module: &ModuleUnit,
    current_project: &str,
    current_module: &str,
) -> BTreeMap<String, i32> {
    if module.module_kind != ModuleKind::Class {
        return BTreeMap::new();
    }

    let mut bindings = BTreeMap::new();
    let mut in_procedure = false;
    for line in module.source.lines() {
        let normalized = normalize_visibility_prefixed_procedure_signature(line);
        if parse_procedure_signature_line(&normalized).is_some() {
            in_procedure = true;
            continue;
        }
        let lower = line.trim().to_ascii_lowercase();
        if lower == "end sub" || lower == "end function" || lower == "end property" {
            in_procedure = false;
            continue;
        }
        if in_procedure {
            continue;
        }
        for field_name in parse_class_state_field_names(line) {
            bindings.insert(
                normalize_identifier(&field_name),
                class_state_binding_token(current_project, current_module, &field_name),
            );
        }
    }
    bindings
}

fn parse_class_state_field_names(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    let payload = if lower.starts_with("private ") {
        &trimmed[8..]
    } else if lower.starts_with("public ") {
        &trimmed[7..]
    } else if lower.starts_with("dim ") {
        &trimmed[4..]
    } else {
        return Vec::new();
    };
    let payload_lower = payload.trim_start().to_ascii_lowercase();
    if payload_lower.starts_with("withevents ")
        || payload_lower.starts_with("sub ")
        || payload_lower.starts_with("function ")
        || payload_lower.starts_with("property ")
        || payload_lower.starts_with("event ")
        || payload_lower.starts_with("declare ")
        || payload_lower.starts_with("const ")
    {
        return Vec::new();
    }
    payload
        .split(',')
        .filter_map(|part| {
            let token = part
                .trim()
                .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
                .next()
                .unwrap_or_default();
            normalize_procedure_name(token)
        })
        .collect()
}

fn rewrite_internal_class_state_assignment(
    line: &str,
    class_state_bindings: &BTreeMap<String, i32>,
) -> String {
    if class_state_bindings.is_empty() || class_state_line_is_non_executable(line) {
        return line.to_string();
    }
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let Some(eq_idx) = trimmed.find('=') else {
        return line.to_string();
    };
    let lhs = trimmed[..eq_idx].trim();
    let rhs = trimmed[eq_idx + 1..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return line.to_string();
    }
    let normalized_lhs = normalize_identifier(lhs);
    let Some(binding_token) = class_state_bindings.get(&normalized_lhs).copied() else {
        return line.to_string();
    };
    let rewritten_rhs = rewrite_internal_class_state_expression_reads(rhs, class_state_bindings);
    format!(
        "{}{} = __oxvba_withevents_set(__oxvba_this_instance, {}, {})",
        &line[..leading],
        lhs,
        binding_token,
        rewritten_rhs
    )
}

fn rewrite_internal_class_state_reads(
    line: &str,
    class_state_bindings: &BTreeMap<String, i32>,
) -> String {
    if class_state_bindings.is_empty() || class_state_line_is_non_executable(line) {
        return line.to_string();
    }
    rewrite_internal_class_state_expression_reads(line, class_state_bindings)
}

fn rewrite_internal_class_state_expression_reads(
    text: &str,
    class_state_bindings: &BTreeMap<String, i32>,
) -> String {
    let mut rewritten = text.to_string();
    for (field_name, binding_token) in class_state_bindings {
        let replacement = format!(
            "__oxvba_withevents_get(__oxvba_this_instance, {})",
            binding_token
        );
        rewritten = rewrite_bare_identifier(&rewritten, field_name, &replacement);
    }
    rewritten
}

fn class_state_line_is_non_executable(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("attribute ")
        || lower.starts_with("option ")
        || lower.starts_with("implements ")
        || lower.starts_with("public ")
        || lower.starts_with("private ")
        || lower.starts_with("dim ")
        || lower.starts_with("const ")
        || lower.starts_with("event ")
        || lower.starts_with("declare ")
        || lower.starts_with("sub ")
        || lower.starts_with("function ")
        || lower.starts_with("property ")
        || lower.starts_with("end ")
}

fn class_state_binding_token(project: &str, module: &str, field_name: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    let key = format!(
        "field|{}|{}|{}",
        normalize_identifier(project),
        normalize_identifier(module),
        normalize_identifier(field_name)
    );
    for byte in key.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    let signed = (hash & 0x7fff_ffff) as i32;
    if signed == 0 { 1 } else { signed }
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

fn find_top_level_assignment_eq(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut idx = 0usize;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
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
            '=' if depth == 0 => {
                if idx > 0 && bytes[idx - 1] == b':' {
                    idx += 1;
                    continue;
                }
                return Some(idx);
            }
            _ => {}
        }
        idx += 1;
    }
    None
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

#[cfg(test)]
fn known_typelib_create_object_selector(qualified_type: &str) -> Option<i32> {
    let identity = known_typelib_identity_for_prog_id_name(qualified_type)?;
    let metadata = build_typelib_metadata(&identity);
    create_object_selector_from_typelib_metadata(&metadata)
}

enum KnownTypeLibMemberResolution {
    Resolved(i32, ComMemberSpec),
    Unsupported,
    Missing,
    Ambiguous,
}

fn resolve_early_bound_binding_member_token_and_spec(
    binding: &EarlyBoundBinding,
    member_name: &str,
) -> KnownTypeLibMemberResolution {
    let Some(metadata) = binding.typelib_metadata.as_ref() else {
        return KnownTypeLibMemberResolution::Unsupported;
    };
    match resolve_member_token_and_spec_from_typelib_metadata_name(metadata, member_name) {
        TypeLibMemberLookupResult::Resolved(token, spec) => {
            KnownTypeLibMemberResolution::Resolved(token.raw(), spec)
        }
        TypeLibMemberLookupResult::Missing => KnownTypeLibMemberResolution::Missing,
        TypeLibMemberLookupResult::Ambiguous => KnownTypeLibMemberResolution::Ambiguous,
    }
}

fn resolve_early_bound_binding_default_member_token_and_spec(
    binding: &EarlyBoundBinding,
) -> KnownTypeLibMemberResolution {
    let Some(metadata) = binding.typelib_metadata.as_ref() else {
        return KnownTypeLibMemberResolution::Unsupported;
    };
    match resolve_default_member_token_and_spec_from_typelib_metadata(metadata) {
        TypeLibMemberLookupResult::Resolved(token, spec) => {
            KnownTypeLibMemberResolution::Resolved(token.raw(), spec)
        }
        TypeLibMemberLookupResult::Missing => KnownTypeLibMemberResolution::Missing,
        TypeLibMemberLookupResult::Ambiguous => KnownTypeLibMemberResolution::Ambiguous,
    }
}

fn render_typelib_invoke_kind(invoke_kind: TypeLibMemberInvokeKind) -> &'static str {
    match invoke_kind {
        TypeLibMemberInvokeKind::PropertyGet => "property-get",
        TypeLibMemberInvokeKind::Method => "method",
        TypeLibMemberInvokeKind::PropertyPut => "property-put",
        TypeLibMemberInvokeKind::PropertyPutRef => "property-putref",
    }
}

#[cfg(test)]
fn known_typelib_member_token(qualified_type: &str, member_name: &str) -> Option<i32> {
    known_typelib_member_token_and_arity(qualified_type, member_name).map(|(token, _)| token)
}

#[cfg(test)]
fn resolve_known_typelib_member_token_and_spec(
    qualified_type: &str,
    member_name: &str,
) -> KnownTypeLibMemberResolution {
    let Some(identity) = known_typelib_identity_for_prog_id_name(qualified_type) else {
        return KnownTypeLibMemberResolution::Unsupported;
    };
    let metadata = build_typelib_metadata(&identity);
    match resolve_member_token_and_spec_from_typelib_metadata_name(&metadata, member_name) {
        TypeLibMemberLookupResult::Resolved(token, spec) => {
            KnownTypeLibMemberResolution::Resolved(token.raw(), spec)
        }
        TypeLibMemberLookupResult::Missing => KnownTypeLibMemberResolution::Missing,
        TypeLibMemberLookupResult::Ambiguous => KnownTypeLibMemberResolution::Ambiguous,
    }
}

#[cfg(test)]
fn known_typelib_member_token_and_spec(
    qualified_type: &str,
    member_name: &str,
) -> Option<(i32, ComMemberSpec)> {
    match resolve_known_typelib_member_token_and_spec(qualified_type, member_name) {
        KnownTypeLibMemberResolution::Resolved(token, spec) => Some((token, spec)),
        KnownTypeLibMemberResolution::Unsupported
        | KnownTypeLibMemberResolution::Missing
        | KnownTypeLibMemberResolution::Ambiguous => None,
    }
}

#[cfg(test)]
fn resolve_known_typelib_default_member_token_and_spec(
    qualified_type: &str,
) -> KnownTypeLibMemberResolution {
    let Some(identity) = known_typelib_identity_for_prog_id_name(qualified_type) else {
        return KnownTypeLibMemberResolution::Unsupported;
    };
    let metadata = build_typelib_metadata(&identity);
    match resolve_default_member_token_and_spec_from_typelib_metadata(&metadata) {
        TypeLibMemberLookupResult::Resolved(token, spec) => {
            KnownTypeLibMemberResolution::Resolved(token.raw(), spec)
        }
        TypeLibMemberLookupResult::Missing => KnownTypeLibMemberResolution::Missing,
        TypeLibMemberLookupResult::Ambiguous => KnownTypeLibMemberResolution::Ambiguous,
    }
}

#[cfg(test)]
fn known_typelib_default_member_token_and_spec(
    qualified_type: &str,
) -> Option<(i32, ComMemberSpec)> {
    match resolve_known_typelib_default_member_token_and_spec(qualified_type) {
        KnownTypeLibMemberResolution::Resolved(token, spec) => Some((token, spec)),
        KnownTypeLibMemberResolution::Unsupported
        | KnownTypeLibMemberResolution::Missing
        | KnownTypeLibMemberResolution::Ambiguous => None,
    }
}

#[cfg(test)]
fn known_typelib_member_token_and_arity(
    qualified_type: &str,
    member_name: &str,
) -> Option<(i32, usize)> {
    known_typelib_member_token_and_spec(qualified_type, member_name)
        .map(|(token, spec)| (token, spec.parameter_names.len()))
}

/// Transitional native/internal PMR token map.
///
/// Imported COM early-bound lowering must not route through this helper; the external path now
/// resolves authoritative member tokens from `oxvba-com` synthetic typelib metadata instead.
fn known_internal_dynamic_dispatch_member_token(member_name: &str) -> Option<i32> {
    match normalize_identifier(member_name).as_str() {
        "count" => Some(1),
        "exists" => Some(2),
        "firechanged" => Some(3),
        "firechangedpair" => Some(4),
        "firechangedsourceinterface" => Some(11),
        "ping" => Some(5),
        "lookup" => Some(6),
        "setvalue" => Some(7),
        "setvalueref" => Some(8),
        "value" => Some(9),
        "quit" => Some(10),
        "sumpair" => Some(12),
        "lookuppair" => Some(13),
        "setindexedvalue" => Some(14),
        "setindexedvalueref" => Some(15),
        "echovariant" => Some(16),
        "raiseexception" => Some(17),
        "returnsmallint" => Some(18),
        "returnunsignedword" => Some(19),
        "returnsmallintarray" => Some(20),
        "returnboolarray" => Some(21),
        "returnstringarray" => Some(22),
        "returnselfdispatch" => Some(23),
        "returnselfunknown" => Some(24),
        "classifyvariantarg" => Some(25),
        "classifyvariantarrayfirstelementarg" => Some(26),
        "returnselfdispatcharray" => Some(27),
        "returnselftypeddispatcharray" => Some(28),
        "returnselftypedunknownarray" => Some(29),
        "returnsmallintmatrix" => Some(30),
        "returnplainunknown" => Some(31),
        "returnplainunknownarray" => Some(32),
        "returnlongarray" => Some(33),
        "returnunsignedlongarray" => Some(34),
        "returnlong" => Some(35),
        "returnunsignedlong" => Some(36),
        "returnbyte" => Some(37),
        "returnbytearray" => Some(38),
        "returnsignedbyte" => Some(39),
        "returnsignedbytearray" => Some(40),
        "returnplatformint" => Some(41),
        "returnplatformuint" => Some(42),
        "returnplatformintarray" => Some(43),
        "returnplatformuintarray" => Some(44),
        "returnhyper" => Some(45),
        "returnunsignedhyper" => Some(46),
        "returnhyperarray" => Some(47),
        "returnunsignedhyperarray" => Some(48),
        "returndouble" => Some(49),
        "returndoublearray" => Some(50),
        "returnsingle" => Some(51),
        "returnsinglearray" => Some(52),
        "returndate" => Some(53),
        "returndatearray" => Some(54),
        "returncurrency" => Some(55),
        "returncurrencyarray" => Some(56),
        "returndecimal" => Some(57),
        "returndecimalarray" => Some(58),
        "returnwideunsignedlong" => Some(59),
        "returnwideunsignedlongarray" => Some(60),
        "returnwideplatformuint" => Some(61),
        "returnwideplatformuintarray" => Some(62),
        "returnbool" => Some(63),
        "returnstring" => Some(64),
        "returnmissingmembername" => Some(76),
        "returnpingmembername" => Some(77),
        "returnlookupmembername" => Some(78),
        "returnsumpairmembername" => Some(79),
        "returnlookuppairmembername" => Some(80),
        "returnsetvaluemembername" => Some(81),
        "returnsetvaluerefmembername" => Some(82),
        "returnsetindexedvaluemembername" => Some(83),
        "returnsetindexedvaluerefmembername" => Some(84),
        "returnvaluemembername" => Some(85),
        "returndefaultmembername" => Some(86),
        "returnempty" => Some(65),
        "returnnull" => Some(66),
        "returnerror" => Some(67),
        "returnbyreflong" => Some(68),
        "returnbyreflongarray" => Some(69),
        "returnwidehyper" => Some(70),
        "returnwidehyperarray" => Some(71),
        "returnwideunsignedhyper" => Some(72),
        "returnwideunsignedhyperarray" => Some(73),
        "returnvariantmatrix" => Some(74),
        "returnplainunknownvariantarray" => Some(75),
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

fn build_project_dynamic_object_routes(
    bindings: &[ProjectDynamicInstanceBindingDraft],
    procedures: &[ProcedureDecl],
    runtime_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<ProjectDynamicObjectRoute> {
    let mut out = Vec::new();
    for binding in bindings {
        let mut members = procedures
            .iter()
            .filter(|decl| {
                decl.project_name == binding.project_name
                    && decl.module_name == binding.module_name
                    && decl.module_kind == ModuleKind::Class
                    && decl.is_public
            })
            .filter_map(|decl| {
                runtime_metadata
                    .get(&decl.lowered_name)
                    .map(|metadata| ProjectDynamicMemberRoute {
                        member_name: decl.procedure_name.clone(),
                        lowered_name: decl.lowered_name.clone(),
                        known_dispatch_token: known_internal_dynamic_dispatch_member_token(
                            &decl.procedure_name,
                        ),
                        is_default_member: decl.is_default_member,
                        kind: decl.kind.dynamic_member_kind(),
                        visible_param_count: decl.param_count,
                        entry_pc: metadata.entry_pc,
                        param_slots: metadata.param_slots.clone(),
                        return_slot: metadata.return_slot,
                    })
            })
            .collect::<Vec<_>>();
        members.sort_by(|lhs, rhs| {
            lhs.member_name
                .cmp(&rhs.member_name)
                .then(lhs.lowered_name.cmp(&rhs.lowered_name))
        });
        out.push(ProjectDynamicObjectRoute {
            object_handle: binding.object_handle,
            project_name: binding.project_name.clone(),
            module_name: binding.module_name.clone(),
            members,
        });
    }
    out.sort_by_key(|route| route.object_handle);
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
                let call_args = if handler_param_count == 0 {
                    "__oxvba_owner_instance".to_string()
                } else if event_arg_count == 0 {
                    "__oxvba_owner_instance, 0".to_string()
                } else {
                    "__oxvba_owner_instance, __oxvba_arg0".to_string()
                };
                let wrapper_body = if event_arg_count == 0 {
                    format!(
                        "Sub {wrapper}(Optional ByVal __oxvba_source_instance = 0)\nDim __oxvba_owner_instance\n__oxvba_owner_instance = __oxvba_withevents_first_owner(__oxvba_source_instance, {binding_token})\nDo While __oxvba_owner_instance <> 0\nCall {}({call_args})\n__oxvba_owner_instance = __oxvba_withevents_next_owner()\nLoop\nEnd Sub",
                        route.handler_symbol,
                    )
                } else {
                    format!(
                        "Sub {wrapper}(Optional ByVal __oxvba_source_instance = 0, Optional ByVal __oxvba_arg0 = 0)\nDim __oxvba_owner_instance\n__oxvba_owner_instance = __oxvba_withevents_first_owner(__oxvba_source_instance, {binding_token})\nDo While __oxvba_owner_instance <> 0\nCall {}({call_args})\n__oxvba_owner_instance = __oxvba_withevents_next_owner()\nLoop\nEnd Sub",
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
    if let Some((proc_name, kind, _)) = parse_procedure_signature_line(&normalized)
        && let Some(decl) = find_decl_by_signature(
            procedures,
            current_project,
            current_module,
            &proc_name,
            kind,
        )
    {
        let mut rewritten = rewrite_signature_name(&normalized, lowered_proc_signature_name(decl));
        if module.module_kind == ModuleKind::Class {
            rewritten = inject_hidden_instance_param(&rewritten);
            rewritten = strip_signature_param_types(&rewritten);
        }
        let next_function_result = if decl.kind.has_return_value() {
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

#[allow(clippy::too_many_arguments)]
fn rewrite_module_source(
    manifest: &ProjectManifest,
    active_project: &str,
    module: &ModuleUnit,
    current_project: &str,
    procedures: &[ProcedureDecl],
    reference_order: &BTreeMap<String, usize>,
    event_dispatch_plan: &EventDispatchPlan,
    next_internal_instance_id: &mut i32,
    dynamic_instance_bindings: &mut Vec<ProjectDynamicInstanceBindingDraft>,
) -> Result<(String, BTreeMap<String, BTreeSet<String>>), ProjectCompileError> {
    let current_module = normalize_identifier(&module.module_name);
    let mut out = Vec::new();
    let mut active_function_result: Option<(String, String)> = None;
    let mut active_procedure_name: Option<String> = None;
    let mut early_bound = BTreeMap::<String, EarlyBoundBinding>::new();
    let mut internal_class_bindings = BTreeMap::<String, InternalClassBinding>::new();
    let mut forced_object_locals_by_proc = ForcedObjectLocalsByProc::new();
    let mut withevents_bindings = BTreeSet::<String>::new();
    let source_lines = module_source_lines_with_class_terminate_cleanup(module);
    for line in &source_lines {
        record_internal_class_object_local(
            &mut forced_object_locals_by_proc,
            &active_procedure_name,
            line,
            manifest,
            current_project,
            reference_order,
        );
        let expanded = expand_bound_source_line(
            line,
            manifest,
            current_project,
            reference_order,
            procedures,
            &mut early_bound,
            &mut internal_class_bindings,
            &mut withevents_bindings,
            next_internal_instance_id,
            dynamic_instance_bindings,
        )?;
        for expanded_line in expanded {
            let expanded_line = rewrite_internal_class_set_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
                &withevents_bindings,
            )?;
            let expanded_line = rewrite_internal_class_property_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let expanded_line = rewrite_internal_class_default_member_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let expanded_line = rewrite_internal_class_default_member_read_assignment(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
            let expanded_line = rewrite_internal_class_property_reads(
                &expanded_line,
                active_project,
                current_project,
                &current_module,
                procedures,
                &internal_class_bindings,
            )?;
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
            if let Some((proc_name, kind, _)) = parse_procedure_signature_line(&normalized)
                && let Some(decl) = find_decl_by_signature(
                    procedures,
                    current_project,
                    &current_module,
                    &proc_name,
                    kind,
                )
            {
                let mut rewritten =
                    rewrite_signature_name(&normalized, lowered_proc_signature_name(decl));
                if module.module_kind == ModuleKind::Class {
                    rewritten = inject_hidden_instance_param(&rewritten);
                    rewritten = strip_signature_param_types(&rewritten);
                }
                if decl.kind.has_return_value() {
                    active_function_result = Some((proc_name, decl.lowered_name.clone()));
                } else {
                    active_function_result = None;
                }
                active_procedure_name = Some(decl.lowered_name.clone());
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
            if lower.starts_with("end function") || lower.starts_with("end property") {
                active_function_result = None;
            }
            clear_active_procedure_name_if_end(&mut active_procedure_name, &rewritten);
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
    Ok((out.join("\n"), forced_object_locals_by_proc))
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
        let decl = find_decl_by_name(procedures, current_project, module_name, proc_name);
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

    let Some(decl) = find_decl_by_name(procedures, project_name, module_name, proc_name) else {
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
            let Some(kind) = procedure.kind.export_kind() else {
                continue;
            };
            exports.push(HostProcedureExport {
                project_name: active_project.clone(),
                module_name: module_name.clone(),
                procedure_name: procedure.procedure_name.clone(),
                kind,
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
            let Some(kind) = procedure.kind.export_kind() else {
                continue;
            };
            exports.push(HostProcedureExport {
                project_name: active_project.clone(),
                module_name: module_name.clone(),
                procedure_name: procedure.procedure_name.clone(),
                kind,
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

fn collect_member_attributes(source: &str) -> BTreeMap<String, MemberAttributes> {
    let mut attributes = BTreeMap::<String, MemberAttributes>::new();
    for line in source.lines() {
        let Some((member_name, attr_name, attr_value)) = parse_member_attribute_line(line) else {
            continue;
        };
        let entry = attributes.entry(member_name).or_default();
        if attr_name.eq_ignore_ascii_case("vb_usermemid") {
            entry.vb_user_mem_id = attr_value.parse::<i32>().ok();
        }
    }
    attributes
}

fn parse_member_attribute_line(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    let payload = trimmed
        .strip_prefix("Attribute ")
        .or_else(|| trimmed.strip_prefix("attribute "))?;
    let (lhs, rhs) = payload.split_once('=')?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    let (member_name, attr_name) = lhs.rsplit_once('.')?;
    let member_name = normalize_procedure_name(member_name.trim())?;
    let attr_name = normalize_identifier(attr_name.trim());
    Some((member_name, attr_name, rhs.to_string()))
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

fn parse_procedure_signature_line(line: &str) -> Option<(String, ProcedureDeclKind, bool)> {
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
        (ProcedureDeclKind::Sub, tail[4..].trim())
    } else if lower_tail.starts_with("function ") {
        (ProcedureDeclKind::Function, tail[9..].trim())
    } else if lower_tail.starts_with("property get ") {
        (ProcedureDeclKind::PropertyGet, tail[13..].trim())
    } else if lower_tail.starts_with("property let ") {
        (ProcedureDeclKind::PropertyLet, tail[13..].trim())
    } else if lower_tail.starts_with("property set ") {
        (ProcedureDeclKind::PropertySet, tail[13..].trim())
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
        expand_bound_source_line, module_unit_from_source, validate_compiled_project_contract,
    };
    use std::collections::{BTreeMap, BTreeSet};

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
            lowered.contains("call pmr_projecta_sinka_em_changed(__oxvba_owner_instance)"),
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
        for binding in &compiled.event_dispatch_bindings {
            let metadata = compiled
                .procedure_runtime_metadata
                .get(&binding.handler_symbol)
                .expect("event handler symbol should map to procedure runtime metadata");
            assert!(
                metadata.entry_pc < compiled.bytecode.instructions.len(),
                "handler entry point should be within bytecode bounds"
            );
        }
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
            lowered.contains("__oxvba_withevents_set(__oxvba_this_instance,"),
            "WithEvents Set assignment should route through runtime binding setter"
        );
        assert!(
            lowered.contains("__oxvba_withevents_first_owner(__oxvba_source_instance,"),
            "event guard wrapper should enumerate runtime owner bindings through first-owner intrinsic"
        );
    }

    #[test]
    fn compile_project_injects_withevents_owner_cleanup_in_class_terminate() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module parses");
        let sink = module_unit_from_source(
            "Sink",
            ModuleKind::Class,
            "Attribute VB_Name = \"Sink\"\nPrivate WithEvents src As Emitter\nPublic Sub Class_Terminate()\nDim x\nx = 1\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, sink],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("__oxvba_withevents_clear_owner(__oxvba_this_instance)"),
            "Class_Terminate should inject owner-wide WithEvents cleanup call"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_property_get_read_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_object_property_get_read_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_does_not_inject_runtime_validation_for_rewritten_internal_class_object_locals()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        assert!(
            !compiled.bytecode.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::Instruction::ValidateRuntimeAssignment {
                        target_name,
                        ..
                    } if target_name == "property_get_pmr_projecta_widget_value"
                )
            }),
            "rewritten internal-class object locals should preserve object typing instead of injecting runtime Variant validation"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_native_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_object_property_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject object property-get result on Object target");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_object_property_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject object property-get result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_object_property_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject object property-get result on scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_object_property_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject object property-get result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_parenthesized_object_property_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_parenthesized_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_parenthesized_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_native_parenthesized_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_parenthesized_object_property_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject parenthesized object property-get result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_parenthesized_object_property_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject parenthesized object property-get result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_parenthesized_object_property_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject parenthesized object property-get result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_parenthesized_object_property_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject parenthesized object property-get result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_default_member_get_read_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_parenthesized_property_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_parenthesized_default_member_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_indexed_property_get_read_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nLet valueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_indexed_object_property_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nSet childOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_indexed_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nSet valueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_indexed_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nLet valueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_native_indexed_object_property_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nvalueOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_indexed_object_property_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nLet childOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject indexed object property-get result on Object target");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_indexed_object_property_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nchildOut = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject indexed object property-get result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_indexed_object_property_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nLet n = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject indexed object property-get result on scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_indexed_object_property_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nn = widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject indexed object property-get result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_object_default_member_get_read_assignment()
    {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_object_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_object_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_native_object_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_object_default_member_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject object default-member result on Object target");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_object_default_member_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_object_default_member_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject object default-member result on scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_object_default_member_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_parenthesized_object_default_member_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_parenthesized_object_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_parenthesized_object_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_native_parenthesized_object_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_indexed_default_member_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_parenthesized_object_default_member_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject parenthesized object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_parenthesized_object_default_member_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject parenthesized object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_parenthesized_object_default_member_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject parenthesized object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_parenthesized_object_default_member_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject parenthesized object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_indexed_default_member_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nSet childOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_set_for_native_indexed_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nSet valueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_native_indexed_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nLet valueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_native_indexed_default_member_get_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_indexed_default_member_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nLet childOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject indexed object default-member result on Object target");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_indexed_default_member_get_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nchildOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject indexed object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_native_indexed_default_member_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nLet n = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("Let should reject indexed object default-member result on scalar target");
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_native_indexed_default_member_get_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nn = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject indexed object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_set_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_set_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_let_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_implicit_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_default_member_let_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject non-authoritative object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_default_member_implicit_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject non-authoritative object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }
    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_default_member_let_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject non-authoritative object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_default_member_implicit_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject non-authoritative object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_property_set() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget = x\nafterValue = x\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByRef target)\ntarget = target + 7\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_set_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_parenthesized_default_member_set_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_parenthesized_default_member_set_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_parenthesized_default_member_let_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_parenthesized_default_member_implicit_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 4\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_parenthesized_default_member_get_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nLet valueOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 4\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_default_member_let() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim beforeValue\nDim afterValue\nbeforeValue = widget\nwidget = 9\nafterValue = widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 4\nEnd Property\nPublic Property Let Value(ByVal n)\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("beforevalue = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("property_let_pmr_projecta_widget_value(widget, 9)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("aftervalue = property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_let() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim beforeValue\nDim afterValue\nx = 2\nbeforeValue = widget(x)\nwidget(x) = 9\nafterValue = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = 4\nEnd Property\nPublic Property Let Value(ByVal index, ByVal n)\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("beforevalue = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("property_let_pmr_projecta_widget_value(widget, x, 9)"),
            "{lowered}"
        );
        assert!(
            lowered.contains("aftervalue = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_set_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nSet childOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set childout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_set_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nSet valueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("set valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_let_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nLet valueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("let valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_implicit_read_assignment_to_variant_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 5\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_indexed_default_member_let_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nLet childOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject non-authoritative indexed object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_indexed_default_member_implicit_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 5\nchildOut = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject non-authoritative indexed object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }
    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_indexed_default_member_let_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nLet n = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject non-authoritative indexed object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_indexed_default_member_implicit_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 5\nn = widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject non-authoritative indexed object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_explicit_set_for_native_object_property_get_read_assignment_to_scalar_target_lanes()
     {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Object\nindex = index + 7\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(
                err.to_string()
                    .contains("Set requires Object or Variant target, got Long variable n"),
                "{label}: {err}"
            );
        }
    }

    #[test]
    fn compile_project_rejects_explicit_set_for_native_object_default_member_get_read_assignment_to_scalar_target_lanes()
     {
        let cases = [
            (
                "bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Object\nindex = index + 7\nDim c As New Child\nSet Value = c\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(
                err.to_string()
                    .contains("Set requires Object or Variant target, got Long variable n"),
                "{label}: {err}"
            );
        }
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_default_member_set_read_assignment_to_scalar_target_lanes()
     {
        let cases = [
            (
                "bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
            (
                "indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Object\nindex = index + 7\nDim c As New Child\nSet Value = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(
                err.to_string()
                    .contains("Set requires Object or Variant target, got Long variable n"),
                "{label}: {err}"
            );
        }
    }

    #[test]
    fn compile_project_rejects_explicit_set_for_scalar_getter_result_to_variant_target_lanes() {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable valueout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable valueout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable valueout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable valueout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable valueout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.to_string().contains(expected_message), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_explicit_set_for_scalar_getter_result_to_object_target_lanes() {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable childout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable childout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires object value for variable childout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires object value for variable childout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires object value for variable childout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.to_string().contains(expected_message), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_explicit_set_for_scalar_getter_result_to_scalar_target_lanes() {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "Set requires Object or Variant target, got Long variable n",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            assert!(err.to_string().contains(expected_message), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_scalar_getter_result_to_variant_target_lanes() {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let compiled = compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");
        }
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_scalar_getter_result_to_scalar_target_lanes() {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let compiled = compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");
        }
    }

    #[test]
    fn compile_project_rejects_explicit_let_for_scalar_getter_result_to_object_target_lanes() {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let cannot assign to object variable childout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let cannot assign to object variable childout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let cannot assign to object variable childout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let cannot assign to object variable childout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let cannot assign to object variable childout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "let cannot assign to object variable childout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let cannot assign to object variable childout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "let cannot assign to object variable childout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "let cannot assign to object variable childout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = compile_project(&manifest)
                .expect_err("Let should reject scalar getter result on Object target");
            assert!(
                err.to_string()
                    .to_ascii_lowercase()
                    .contains(expected_message),
                "{label}: {err}"
            );
        }
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_scalar_getter_result_to_variant_target_lanes()
     {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "valueout = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let compiled = compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");
        }
    }

    #[test]
    fn compile_project_preserves_implicit_assignment_for_scalar_getter_result_to_scalar_target_lanes()
     {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget)",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "n = property_get_pmr_projecta_widget_value(widget, x)",
            ),
        ];

        for (label, main_source, widget_source, expected_lowered) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let compiled = compile_project(&manifest).expect("compile should succeed");
            let lowered = compiled.rewritten_source.to_ascii_lowercase();
            assert!(lowered.contains(expected_lowered), "{label}: {lowered}");
        }
    }

    #[test]
    fn compile_project_rejects_implicit_assignment_for_scalar_getter_result_to_object_target_lanes()
    {
        let cases = [
            (
                "named property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign long to object variable childout",
            ),
            (
                "parenthesized property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget.Value()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign long to object variable childout",
            ),
            (
                "indexed property",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget.Value(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "cannot assign long to object variable childout",
            ),
            (
                "authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "cannot assign long to object variable childout",
            ),
            (
                "authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "cannot assign long to object variable childout",
            ),
            (
                "authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
                "cannot assign long to object variable childout",
            ),
            (
                "non-authoritative bare default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign long to object variable childout",
            ),
            (
                "non-authoritative parenthesized default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Long\nValue = 9\nEnd Property",
                "cannot assign long to object variable childout",
            ),
            (
                "non-authoritative indexed default member",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index) As Long\nindex = index + 7\nValue = index\nEnd Property",
                "cannot assign long to object variable childout",
            ),
        ];

        for (label, main_source, widget_source, expected_message) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = compile_project(&manifest).expect_err(
                "implicit assignment should reject scalar getter result on Object target",
            );
            assert!(
                err.to_string()
                    .to_ascii_lowercase()
                    .contains(expected_message),
                "{label}: {err}"
            );
        }
    }

    #[test]
    fn compile_project_infers_non_authoritative_single_candidate_indexed_default_member_property_set()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget(1) = x\nafterValue = x\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByVal index, ByRef target)\ntarget = target + 7\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_set_pmr_projecta_widget_value(widget, 1, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_internal_class_indexed_property_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget.Value(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_internal_class_indexed_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_non_authoritative_single_candidate_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_non_authoritative_single_candidate_indexed_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_no_paren_non_authoritative_single_candidate_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget x\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_no_paren_getter_read_assignment_lanes() {
        let cases = [
            (
                "named property explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            let message = err.to_string().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn compile_project_rejects_no_paren_getter_read_assignment_to_object_target_lanes() {
        let cases = [
            (
                "named property explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            let message = err.to_string().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn compile_project_rejects_no_paren_getter_read_assignment_to_scalar_target_lanes() {
        let cases = [
            (
                "named property explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member explicit Let",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member implicit",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            let message = err.to_string().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn compile_project_rejects_no_paren_getter_explicit_set_read_assignment_lanes() {
        let cases = [
            (
                "named property Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "named property scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget.Value x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "authoritative default member Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "authoritative default member scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property\nAttribute Value.VB_UserMemId = 0",
            ),
            (
                "non-authoritative default member Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
            (
                "non-authoritative default member scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget x\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should reject deterministically"),
                Err(err) => err,
            };
            let message = err.to_string().to_ascii_lowercase();
            assert!(
                message.contains("unsupported statement"),
                "{label}: {message}"
            );
        }
    }

    #[test]
    fn compile_project_rewrites_statement_context_non_authoritative_single_candidate_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_statement_context_non_authoritative_single_candidate_indexed_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x)\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByRef index)\nindex = index + 7\nValue = index\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget, x)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_internal_class_parenthesized_property_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_internal_class_parenthesized_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_call_statement_parenthesized_non_authoritative_single_candidate_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("call property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_statement_context_internal_class_parenthesized_property_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget.Value()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_statement_context_internal_class_parenthesized_default_member_get()
    {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property\nAttribute Value.VB_UserMemId = 0",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rewrites_statement_context_parenthesized_non_authoritative_single_candidate_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let compiled = compile_project(&manifest).expect("rewrite should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            lowered.contains("property_get_pmr_projecta_widget_value(widget)"),
            "{lowered}"
        );
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("ambiguous non-authoritative default-member fallback should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_default_member_let() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget = 9\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Let Value(ByVal n)\nEnd Property\nPublic Property Let Observe(ByVal n)\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("ambiguous non-authoritative default-member let should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_indexed_default_member_let() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x) = 9\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Let Value(ByVal index, ByVal n)\nEnd Property\nPublic Property Let Observe(ByVal index, ByVal n)\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("ambiguous indexed non-authoritative default-member let should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_default_member_property_set() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget = x\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByRef target)\nEnd Property\nPublic Property Set Observe(ByRef target)\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("ambiguous non-authoritative default-member property set should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_indexed_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("ambiguous indexed non-authoritative default-member getter should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_indexed_default_member_property_set() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget(1) = x\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Set Value(ByVal index, ByRef target)\nEnd Property\nPublic Property Set Observe(ByVal index, ByRef target)\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous indexed non-authoritative default-member property set should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_object_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "bare implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_variant_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "bare implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_scalar_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "bare implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_explicit_set_object_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_explicit_set_variant_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_explicit_set_scalar_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "parenthesized Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe() As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
            (
                "indexed Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index) As Object\nDim c As New Child\nSet Value = c\nEnd Property\nPublic Property Get Observe(ByVal index) As Object\nDim c As New Child\nSet Observe = c\nEnd Property",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let child = module_unit_from_source(
                "Child",
                ModuleKind::Class,
                "Attribute VB_Name = \"Child\"",
            )
            .expect("child module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget, child],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_call_statement_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous non-authoritative call-statement default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_call_statement_indexed_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget(x)\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous indexed non-authoritative call-statement default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_parenthesized_default_member_let_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject non-authoritative parenthesized object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("let cannot assign to object variable childout")
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_parenthesized_default_member_implicit_read_assignment_to_object_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject non-authoritative parenthesized object default-member result on Object target",
        );
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("set required for object variable childout")
        );
    }
    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_parenthesized_default_member_let_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "Let should reject non-authoritative parenthesized object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_non_authoritative_single_candidate_parenthesized_default_member_implicit_read_assignment_to_scalar_target()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
        )
        .expect("module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value() As Object\nDim c As New Child\nSet Value = c\nEnd Property",
        )
        .expect("module parses");
        let child =
            module_unit_from_source("Child", ModuleKind::Class, "Attribute VB_Name = \"Child\"")
                .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget, child],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "implicit assignment should reject non-authoritative parenthesized object default-member result on scalar target",
        );
        assert!(
            err.to_string()
                .contains("cannot assign Object to Long variable n")
        );
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_call_statement_parenthesized_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget()\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous parenthesized non-authoritative call-statement default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_no_paren_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget x\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("ambiguous non-authoritative no-paren default-member getter should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_statement_context_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous non-authoritative statement-context default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_statement_context_indexed_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x)\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property\nPublic Property Get Observe(ByVal index)\nObserve = index + 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous indexed non-authoritative statement-context default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_ambiguous_non_authoritative_statement_context_parenthesized_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget()\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property\nPublic Property Get Observe()\nObserve = 2\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "ambiguous parenthesized non-authoritative statement-context default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("missing non-authoritative default-member getter should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_default_member_let() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget = 9\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("missing non-authoritative default-member let should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_indexed_default_member_let() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x) = 9\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value(ByVal index)\nValue = index\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("missing indexed non-authoritative default-member let should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_default_member_property_set() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget = x\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("missing non-authoritative default-member property set should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_indexed_default_member_property_set() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nSet widget(1) = x\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 1\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing non-authoritative indexed default-member property set should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_indexed_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut\nx = 2\nvalueOut = widget(x)\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("missing non-authoritative indexed default-member getter should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_object_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "bare implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nLet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nchildOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Let to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nLet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed implicit to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nchildOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_variant_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "bare implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nLet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nvalueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Let to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nLet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed implicit to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nvalueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_scalar_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "bare implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nLet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nn = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Let to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nLet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed implicit to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nn = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_explicit_set_object_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim childOut As Object\nSet childOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Set to Object target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim childOut As Object\nx = 2\nSet childOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_explicit_set_variant_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut As Variant\nSet valueOut = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Set to Variant target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim valueOut As Variant\nx = 2\nSet valueOut = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_explicit_set_scalar_target_default_member_read_assignment_lanes()
     {
        let cases = [
            (
                "bare Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "parenthesized Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim n As Long\nSet n = widget()\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
            (
                "indexed Set to scalar target",
                "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nDim n As Long\nx = 2\nSet n = widget(x)\nEnd Sub",
                "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
            ),
        ];

        for (label, main_source, widget_source) in cases {
            let main_module =
                module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
                    .expect("main module parses");
            let widget = module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                .expect("widget module parses");
            let manifest = ProjectManifest {
                project_name: "ProjectA".to_string(),
                project_kind: ProjectKind::Source,
                modules: vec![main_module, widget],
                references: Vec::new(),
                reference_projects: Vec::new(),
                conditional_constants: BTreeMap::new(),
            };

            let err = match compile_project(&manifest) {
                Ok(_) => panic!("{label} should fail deterministically"),
                Err(err) => err,
            };
            assert_eq!(
                err.code(),
                "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING",
                "{label}: {err}"
            );
            assert!(err.to_string().contains("widget"), "{label}: {err}");
        }
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_call_statement_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing non-authoritative call-statement default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_call_statement_indexed_default_member_get()
    {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nCall widget(x)\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing non-authoritative indexed call-statement default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_statement_context_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing non-authoritative statement-context default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_statement_context_indexed_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget(x)\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing non-authoritative indexed statement-context default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_call_statement_parenthesized_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nCall widget()\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing parenthesized non-authoritative call-statement default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_no_paren_default_member_get() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nDim x\nx = 2\nwidget x\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest)
            .expect_err("missing non-authoritative no-paren default-member getter should fail");
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn compile_project_rejects_missing_non_authoritative_statement_context_parenthesized_default_member_get()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nwidget()\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Sub Touch()\nEnd Sub",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let err = compile_project(&manifest).expect_err(
            "missing parenthesized non-authoritative statement-context default-member getter should fail",
        );
        assert_eq!(err.code(), "PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING");
        assert!(err.to_string().contains("widget"));
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
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.Count()\nEnd Sub",
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
    fn known_typelib_member_token_reads_external_member_metadata() {
        assert_eq!(
            super::known_typelib_member_token("OxVba.TestDispatch", "Count"),
            Some(1)
        );
        assert_eq!(
            super::known_typelib_member_token("OxVba.TestDispatch", "UnknownMember"),
            None
        );
    }

    #[test]
    fn known_typelib_member_token_and_arity_reads_external_member_metadata() {
        assert_eq!(
            super::known_typelib_member_token_and_arity("OxVba.TestDispatch", "Count"),
            Some((1, 0))
        );
        assert_eq!(
            super::known_typelib_member_token_and_arity("OxVba.TestDispatch", "SumPair"),
            Some((12, 2))
        );
    }

    #[test]
    fn known_typelib_member_token_and_spec_reads_external_member_shape_metadata() {
        let (token, spec) =
            super::known_typelib_member_token_and_spec("OxVba.TestDispatch", "SetValueRef")
                .expect("setter shape metadata should resolve");
        assert_eq!(token, 8);
        assert_eq!(
            spec.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        assert_eq!(spec.parameter_names, vec!["value".to_string()]);
    }

    #[test]
    fn known_typelib_default_member_token_and_spec_reads_external_metadata() {
        let (token, spec) =
            super::known_typelib_default_member_token_and_spec("OxVba.TestDispatch")
                .expect("default member metadata should resolve");
        assert_eq!(token, 16);
        assert_eq!(spec.name, "EchoVariant");
        assert_eq!(spec.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        assert!(spec.is_default_member);
    }

    #[test]
    fn known_typelib_create_object_selector_reads_external_activation_metadata() {
        assert_eq!(
            super::known_typelib_create_object_selector("OxVba.TestDispatch"),
            Some(4)
        );
        assert_eq!(
            super::known_typelib_create_object_selector("Excel.Application"),
            None
        );
    }

    #[test]
    fn compile_project_internal_dynamic_routes_use_internal_dispatch_token_table() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim widget As New Widget\nEnd Sub",
        )
        .expect("main module parses");
        let widget = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            "Attribute VB_Name = \"Widget\"\nPublic Property Get Value()\nValue = 9\nEnd Property",
        )
        .expect("widget module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module, widget],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("native internal route should compile");
        assert!(
            compiled.project_dynamic_objects[0]
                .members
                .iter()
                .any(|member| {
                    member.member_name.eq_ignore_ascii_case("Value")
                        && member.known_dispatch_token == Some(9)
                }),
            "expected native internal dynamic route to keep its transitional token table"
        );
    }

    #[test]
    fn expand_bound_source_line_stores_imported_typelib_metadata_in_early_bound_binding() {
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: Vec::new(),
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let mut early_bound = BTreeMap::new();
        let mut internal_class_bindings = BTreeMap::new();
        let mut withevents_bindings = BTreeSet::new();
        let mut next_internal_instance_id = 1;
        let mut dynamic_instance_bindings = Vec::new();
        let expanded = expand_bound_source_line(
            "Dim obj As New OxVba.TestDispatch",
            &manifest,
            "projecta",
            &BTreeMap::new(),
            &[],
            &mut early_bound,
            &mut internal_class_bindings,
            &mut withevents_bindings,
            &mut next_internal_instance_id,
            &mut dynamic_instance_bindings,
        )
        .expect("external dim should bind through metadata");

        assert_eq!(
            expanded,
            vec![
                "Dim obj As Object".to_string(),
                "Set obj = CreateObject(4)".to_string()
            ]
        );
        let binding = early_bound.get("obj").expect("binding should be recorded");
        assert_eq!(binding.qualified_type, "OxVba.TestDispatch");
        assert_eq!(binding.create_selector, Some(4));
        let metadata = binding
            .typelib_metadata
            .as_ref()
            .expect("supported imported binding should carry metadata");
        assert!(
            metadata
                .members
                .iter()
                .any(|member| member.name == "EchoVariant" && member.is_default_member),
            "expected imported binding metadata to carry authoritative default-member identity"
        );
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
        assert!(lowered.contains("set obj = createobject(4)"));
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
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.UnknownMember()\nEnd Sub",
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
        let err = compile_project(&manifest).expect_err("missing member should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-MEMBER-NOT-FOUND");
    }

    #[test]
    fn compile_project_rewrites_multi_arg_external_member_call_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.SumPair(1, 2)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("multi-arg external member call should compile in widened subset");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 12, 1, 2)"));
    }

    #[test]
    fn compile_project_rewrites_property_get_external_member_call_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.Lookup(41)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("property-get external member call should compile in supported subset");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 6, 41)"));
    }

    #[test]
    fn compile_project_rewrites_named_arg_external_member_calls_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim sumPair\nDim lookupPair\nDim echoValue\nSet obj = CreateObject(4)\nsumPair = obj.SumPair(rhs := 14, lhs := 3)\nlookupPair = obj.LookupPair(rhs := 9, lhs := 5)\nechoValue = obj(value := 41)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("named-argument imported member calls should compile in supported subset");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("sumpair = dispatchinvoke(obj, 12, rhs := 14, lhs := 3)"));
        assert!(lowered.contains("lookuppair = dispatchinvoke(obj, 13, rhs := 9, lhs := 5)"));
        assert!(lowered.contains("echovalue = dispatchinvoke(obj, 16, value := 41)"));
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_named_arg_external_member_calls_to_dispatchinvoke()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim sumPair\nDim lookupPair\nDim echoValue\nSet obj = CreateObject(4)\nLet sumPair = obj.SumPair(rhs := 14, lhs := 3)\nLet lookupPair = obj.LookupPair(rhs := 9, lhs := 5)\nLet echoValue = obj(value := 41)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("explicit Let named-argument imported member calls should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("let sumpair = dispatchinvoke(obj, 12, rhs := 14, lhs := 3)"));
        assert!(lowered.contains("let lookuppair = dispatchinvoke(obj, 13, rhs := 9, lhs := 5)"));
        assert!(lowered.contains("let echovalue = dispatchinvoke(obj, 16, value := 41)"));
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_positional_external_member_calls_to_dispatchinvoke()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim countValue\nDim existsValue\nDim lookupValue\nDim echoValue\nSet obj = CreateObject(4)\nLet countValue = obj.Count()\nLet existsValue = obj.Exists(42)\nLet lookupValue = obj.Lookup(42)\nLet echoValue = obj(42)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("explicit Let imported positional calls should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("let countvalue = dispatchinvoke(obj, 1)"));
        assert!(lowered.contains("let existsvalue = dispatchinvoke(obj, 2, 42)"));
        assert!(lowered.contains("let lookupvalue = dispatchinvoke(obj, 6, 42)"));
        assert!(lowered.contains("let echovalue = dispatchinvoke(obj, 16, 42)"));
    }

    #[test]
    fn compile_project_rewrites_call_statements_for_positional_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nCall obj.Count()\nCall obj.Exists(42)\nCall obj.Lookup(42)\nCall obj.Value()\nCall obj(42)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("Call-form positional imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 1)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 2, 42)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 6, 42)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 9)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 16, 42)"));
    }

    #[test]
    fn compile_project_rewrites_call_statements_for_named_arg_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nCall obj.SumPair(rhs := 14, lhs := 3)\nCall obj.LookupPair(rhs := 9, lhs := 5)\nCall obj(value := 41)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("Call-form named-argument imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 12, rhs := 14, lhs := 3)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 13, rhs := 9, lhs := 5)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 16, value := 41)"));
    }

    #[test]
    fn compile_project_rewrites_no_paren_call_statements_for_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nCall obj.Count\nCall obj.Exists 42\nCall obj.Lookup 42\nCall obj.Value\nCall obj 42\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("no-paren Call-form imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 1)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 2, 42)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 6, 42)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 9)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 16, 42)"));
    }

    #[test]
    fn compile_project_rewrites_no_paren_named_arg_call_statements_for_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nCall obj.SumPair rhs := 14, lhs := 3\nCall obj.LookupPair rhs := 9, lhs := 5\nCall obj value := 41\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("no-paren named-argument Call-form imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 12, rhs := 14, lhs := 3)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 13, rhs := 9, lhs := 5)"));
        assert!(lowered.contains("call dispatchinvoke(obj, 16, value := 41)"));
    }

    #[test]
    fn compile_project_rewrites_statement_context_for_positional_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.Count()\nobj.Exists(42)\nobj.Lookup(42)\nobj.Value()\nobj(42)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("statement-context positional imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 1)"));
        assert!(lowered.contains("dispatchinvoke(obj, 2, 42)"));
        assert!(lowered.contains("dispatchinvoke(obj, 6, 42)"));
        assert!(lowered.contains("dispatchinvoke(obj, 9)"));
        assert!(lowered.contains("dispatchinvoke(obj, 16, 42)"));
    }

    #[test]
    fn compile_project_rewrites_statement_context_for_named_arg_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.SumPair(rhs := 14, lhs := 3)\nobj.LookupPair(rhs := 9, lhs := 5)\nobj(value := 41)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("statement-context named-argument imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 12, rhs := 14, lhs := 3)"));
        assert!(lowered.contains("dispatchinvoke(obj, 13, rhs := 9, lhs := 5)"));
        assert!(lowered.contains("dispatchinvoke(obj, 16, value := 41)"));
    }

    #[test]
    fn compile_project_rewrites_no_paren_statement_context_for_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.Exists 42\nobj.Lookup 42\nobj 42\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("no-paren statement-context positional imported member invokes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 2, 42)"));
        assert!(lowered.contains("dispatchinvoke(obj, 6, 42)"));
        assert!(lowered.contains("dispatchinvoke(obj, 16, 42)"));
    }

    #[test]
    fn compile_project_rewrites_no_paren_named_arg_statement_context_for_external_member_invokes() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.SumPair rhs := 14, lhs := 3\nobj.LookupPair rhs := 9, lhs := 5\nobj value := 41\nEnd Sub",
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
        let compiled = compile_project(&manifest).expect(
            "no-paren statement-context named-argument imported member invokes should compile",
        );
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 12, rhs := 14, lhs := 3)"));
        assert!(lowered.contains("dispatchinvoke(obj, 13, rhs := 9, lhs := 5)"));
        assert!(lowered.contains("dispatchinvoke(obj, 16, value := 41)"));
    }

    #[test]
    fn compile_project_rewrites_external_object_member_call_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim child As Object\nSet obj = CreateObject(4)\nSet child = obj.ReturnSelfDispatch()\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("object-valued imported member call should compile in supported subset");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("set child = dispatchinvoke(obj, 23)"));
    }

    #[test]
    fn compile_project_rewrites_external_unknown_member_call_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim wrapped\nSet obj = CreateObject(4)\nwrapped = obj.ReturnSelfUnknown()\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("IUnknown-valued imported member call should compile in supported subset");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("wrapped = dispatchinvoke(obj, 24)"));
    }

    #[test]
    fn compile_project_preserves_imported_object_result_assignment_intents_across_dispatch_and_unknown_members()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim childDispatch As Object\nDim childUnknown As Object\nDim wrappedDispatch\nDim wrappedUnknown\nSet obj = CreateObject(4)\nSet childDispatch = obj.ReturnSelfDispatch()\nSet childUnknown = obj.ReturnSelfUnknown()\nwrappedDispatch = obj.ReturnSelfDispatch()\nLet wrappedUnknown = obj.ReturnSelfUnknown()\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("imported object-result assignment-intent lanes should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("set childdispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("set childunknown = dispatchinvoke(obj, 24)"));
        assert!(lowered.contains("wrappeddispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("let wrappedunknown = dispatchinvoke(obj, 24)"));
    }

    #[test]
    fn compile_project_preserves_imported_zero_arg_object_result_assignment_intents_without_parentheses()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim childDispatch As Object\nDim childUnknown As Object\nDim wrappedDispatch\nDim wrappedUnknown\nSet obj = CreateObject(4)\nSet childDispatch = obj.ReturnSelfDispatch\nSet childUnknown = obj.ReturnSelfUnknown\nwrappedDispatch = obj.ReturnSelfDispatch\nLet wrappedUnknown = obj.ReturnSelfUnknown\nEnd Sub",
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
        let compiled = compile_project(&manifest).expect(
            "imported zero-arg object-result assignment intents without parentheses should compile",
        );
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("set childdispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("set childunknown = dispatchinvoke(obj, 24)"));
        assert!(lowered.contains("wrappeddispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("let wrappedunknown = dispatchinvoke(obj, 24)"));
    }

    #[test]
    fn compile_project_rewrites_object_property_get_external_read_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim childDispatch As Object\nDim childUnknown As Object\nDim wrappedDispatch\nDim wrappedUnknown\nSet obj = CreateObject(4)\nSet childDispatch = obj.SelfDispatch\nSet childUnknown = obj.SelfUnknown\nwrappedDispatch = obj.SelfDispatch\nLet wrappedUnknown = obj.SelfUnknown\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("object-valued imported property-get read-assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("set childdispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("set childunknown = dispatchinvoke(obj, 24)"));
        assert!(lowered.contains("wrappeddispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("let wrappedunknown = dispatchinvoke(obj, 24)"));
    }

    #[test]
    fn compile_project_rewrites_parenthesized_object_property_get_external_read_assignment_to_dispatchinvoke()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim childDispatch As Object\nDim childUnknown As Object\nDim wrappedDispatch\nDim wrappedUnknown\nSet obj = CreateObject(4)\nSet childDispatch = obj.SelfDispatch()\nSet childUnknown = obj.SelfUnknown()\nwrappedDispatch = obj.SelfDispatch()\nLet wrappedUnknown = obj.SelfUnknown()\nEnd Sub",
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
        let compiled = compile_project(&manifest).expect(
            "parenthesized object-valued imported property-get read-assignment should compile",
        );
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("set childdispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("set childunknown = dispatchinvoke(obj, 24)"));
        assert!(lowered.contains("wrappeddispatch = dispatchinvoke(obj, 23)"));
        assert!(lowered.contains("let wrappedunknown = dispatchinvoke(obj, 24)"));
    }

    #[test]
    fn compile_project_rewrites_zero_arg_property_get_external_read_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.Value\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("zero-arg property-get imported read-assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("x = dispatchinvoke(obj, 9)"));
    }

    #[test]
    fn compile_project_rewrites_zero_arg_method_external_read_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.Ping\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("zero-arg method imported read-assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("x = dispatchinvoke(obj, 5)"));
    }

    #[test]
    fn compile_project_rewrites_parenthesized_zero_arg_property_get_external_read_assignment_to_dispatchinvoke()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.Value()\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("parenthesized zero-arg property-get imported read-assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("x = dispatchinvoke(obj, 9)"));
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_zero_arg_property_get_external_read_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nLet x = obj.Value\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("explicit Let zero-arg property-get imported read-assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("let x = dispatchinvoke(obj, 9)"));
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_zero_arg_method_external_read_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nLet x = obj.Ping\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("explicit Let zero-arg method imported read-assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("let x = dispatchinvoke(obj, 5)"));
    }

    #[test]
    fn compile_project_preserves_explicit_let_for_parenthesized_zero_arg_property_get_external_read_assignment()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nLet x = obj.Value()\nEnd Sub",
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
        let compiled = compile_project(&manifest).expect(
            "explicit Let parenthesized zero-arg property-get imported read-assignment should compile",
        );
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("let x = dispatchinvoke(obj, 9)"));
    }

    #[test]
    fn compile_project_rewrites_external_default_member_call_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj(41)\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("external default-member call should compile in supported subset");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("dispatchinvoke(obj, 16, 41)"));
    }

    #[test]
    fn compile_project_rejects_wrong_arity_for_zero_arg_external_member() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.Count(1)\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("wrong zero-arg member arity should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED");
    }

    #[test]
    fn compile_project_rejects_wrong_arity_for_multi_arg_external_member() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj.SumPair(1)\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("wrong multi-arg member arity should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED");
    }

    #[test]
    fn compile_project_rejects_wrong_arity_for_external_default_member() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim x\nSet obj = CreateObject(4)\nx = obj()\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("wrong default-member arity should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED");
    }

    #[test]
    fn compile_project_rejects_missing_external_default_member() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatchNoDefault\nDim x\nSet obj = CreateObject(4)\nx = obj(41)\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("missing default member should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-MEMBER-NOT-FOUND");
    }

    #[test]
    fn compile_project_rejects_ambiguous_external_default_member() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatchAmbiguousDefault\nDim x\nSet obj = CreateObject(4)\nx = obj(41)\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("ambiguous default member should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-MEMBER-AMBIGUOUS");
    }

    #[test]
    fn compile_project_rejects_property_put_external_member_shape() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nCall obj.SetValue(9)\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("property-put imported member shape should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED");
    }

    #[test]
    fn compile_project_rewrites_property_put_external_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.SetValue = 9\nEnd Sub",
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
        let compiled =
            compile_project(&manifest).expect("property-put imported assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 7, 9)"));
    }

    #[test]
    fn compile_project_rewrites_property_putref_external_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim other As OxVba.TestDispatch\nSet obj = CreateObject(4)\nSet other = CreateObject(4)\nSet obj.SetValueRef = other\nEnd Sub",
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
        let compiled =
            compile_project(&manifest).expect("property-putref imported assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 8, other)"));
    }

    #[test]
    fn compile_project_rewrites_indexed_property_put_external_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.SetIndexedValue(7) = 11\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("indexed property-put imported assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 14, 7, 11)"));
    }

    #[test]
    fn compile_project_rewrites_named_arg_indexed_property_put_external_assignment_to_dispatchinvoke()
     {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.SetIndexedValue(lhs := 7) = 11\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("named-argument indexed property-put imported assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 14, lhs := 7, value := 11)"));
    }

    #[test]
    fn compile_project_rewrites_indexed_property_putref_external_assignment_to_dispatchinvoke() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim other As OxVba.TestDispatch\nSet obj = CreateObject(4)\nSet other = CreateObject(4)\nSet obj.SetIndexedValueRef(8) = other\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("indexed property-putref imported assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 15, 8, other)"));
    }

    #[test]
    fn compile_project_rewrites_named_arg_indexed_property_putref_external_assignment_shape() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim other As OxVba.TestDispatch\nSet obj = CreateObject(4)\nSet other = CreateObject(4)\nSet obj.SetIndexedValueRef(lhs := 8) = other\nEnd Sub",
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
        let compiled = compile_project(&manifest)
            .expect("named-argument indexed property-putref imported assignment should compile");
        let lowered = compiled.rewritten_source.to_ascii_lowercase();
        assert!(lowered.contains("call dispatchinvoke(obj, 15, lhs := 8, value := other)"));
    }

    #[test]
    fn compile_project_rejects_wrong_arity_for_property_put_external_assignment() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nSet obj = CreateObject(4)\nobj.SetIndexedValue = 11\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("indexed property-put missing index should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED");
    }

    #[test]
    fn compile_project_rejects_property_putref_external_member_shape() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As OxVba.TestDispatch\nDim other As OxVba.TestDispatch\nSet obj = CreateObject(4)\nSet other = CreateObject(4)\nCall obj.SetValueRef(other)\nEnd Sub",
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
        let err = compile_project(&manifest)
            .expect_err("property-putref imported member shape should reject compilation");
        assert_eq!(err.code(), "BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED");
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
