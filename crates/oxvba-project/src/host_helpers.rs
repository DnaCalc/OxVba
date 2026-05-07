use std::fs;
use std::path::{Path, PathBuf};

use oxvba_host::{
    DirectHostCommandStatus, DirectHostIssue, DirectHostIssueKind, DirectHostRetryability,
};

use crate::model::parse_define_constants;
use crate::vbp::VbpReference;
use crate::{
    BasProj, BasProjComReference, BasProjError, BasProjModule, BasProjModuleKind,
    BasProjProjectReference, BasProjProjectReferenceKind, BuildTarget, OutputType, RuntimeFlavor,
    discover_project_file_in_dir, infer_project_name_from_path, load_workspace_target,
    parse_basproj_xml, parse_vbp, serialize_basproj_xml,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIdentityInfo {
    pub file_stem: String,
    pub declared_vb_name: Option<String>,
    pub effective_name: String,
    pub attribute_required: bool,
    pub attribute_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbNameAttributeAction {
    None,
    Insert,
    Replace,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIdentityRewrite {
    pub logical_name: String,
    pub attribute_action: VbNameAttributeAction,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedModule {
    pub include: String,
    pub basproj_item: BasProjModule,
    pub logical_name: String,
    pub source: String,
    pub identity: ModuleIdentityInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostWorkspaceTargetKind {
    BasProj,
    Vbp,
    ConventionDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectModuleInfo {
    pub kind: BasProjModuleKind,
    pub include: String,
    pub source_path: PathBuf,
    pub identity: ModuleIdentityInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProjectReferenceKind {
    Project,
    HostInjected,
    Com,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectReferenceInfo {
    pub kind: HostProjectReferenceKind,
    pub include: String,
    pub guid: Option<String>,
    pub version_major: Option<u16>,
    pub version_minor: Option<u16>,
    pub lcid: Option<u32>,
    pub import_lib: Option<String>,
    pub path: Option<String>,
    pub referenced_project_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectSurface {
    pub workspace_kind: HostWorkspaceTargetKind,
    pub workspace_target: PathBuf,
    pub project_file: Option<PathBuf>,
    pub project_dir: PathBuf,
    pub project_name: String,
    pub output_type: OutputType,
    pub modules: Vec<HostProjectModuleInfo>,
    pub references: Vec<HostProjectReferenceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectCompileOptionsSurface {
    pub workspace_kind: HostWorkspaceTargetKind,
    pub workspace_target: PathBuf,
    pub project_file: Option<PathBuf>,
    pub project_name: String,
    pub output_type: OutputType,
    pub build_target: Option<BuildTarget>,
    pub runtime_flavor: Option<RuntimeFlavor>,
    pub entry_point: Option<String>,
    pub default_runtime_profile: Option<String>,
    pub default_policy_preset: Option<String>,
    pub default_root_object: Option<String>,
    pub define_constants_raw: Option<String>,
    pub conditional_constants: Vec<HostProjectConditionalConstant>,
    pub build_profiles: Vec<HostProjectBuildProfile>,
    pub source_policies: Vec<HostProjectSourcePolicyOption>,
    pub run_targets: Vec<HostProjectRunTarget>,
    pub build_check_status: DirectHostCommandStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectConditionalConstant {
    pub name: String,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectBuildProfile {
    pub name: String,
    pub display_name: String,
    pub command_status: DirectHostCommandStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProjectSourcePolicyKind {
    DiskOnly,
    WorkspaceOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectSourcePolicyOption {
    pub kind: HostProjectSourcePolicyKind,
    pub display_name: String,
    pub command_status: DirectHostCommandStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProjectRunTargetKind {
    ConfiguredOrDiscoveredEntryPoint,
    MissingOrInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectRunTarget {
    pub id: String,
    pub display_name: String,
    pub kind: HostProjectRunTargetKind,
    pub module_name: Option<String>,
    pub procedure_name: Option<String>,
    pub command_status: DirectHostCommandStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProjectSettingsEdit {
    SetProjectName(String),
    SetEntryPoint(Option<String>),
    SetDefineConstants(Option<String>),
    SetBuildTarget(Option<BuildTarget>),
    SetRuntimeFlavor(Option<RuntimeFlavor>),
    SetDefaultRuntimeProfile(Option<String>),
    SetDefaultPolicyPreset(Option<String>),
    SetDefaultRootObject(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectSettingsEditPlan {
    pub workspace_target: PathBuf,
    pub project_file: PathBuf,
    pub project_name: String,
    pub edits: Vec<HostProjectSettingsEdit>,
    pub validation: HostProjectSettingsEditValidation,
    pub preview: Option<HostProjectCompileOptionsSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectSettingsEditValidation {
    pub can_apply: bool,
    pub issues: Vec<HostProjectSettingsEditIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProjectSettingsEditIssueKind {
    UnsupportedWorkspace,
    InvalidValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectSettingsEditIssue {
    pub edit_index: usize,
    pub kind: HostProjectSettingsEditIssueKind,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectSettingsEditApplication {
    pub workspace_target: PathBuf,
    pub project_file: PathBuf,
    pub project_name: String,
    pub applied_edits: usize,
    pub basproj: BasProj,
    pub surface: HostProjectCompileOptionsSurface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProjectEdit {
    AddModule(BasProjModule),
    RemoveModule { include: String },
    AddProjectReference(BasProjProjectReference),
    RemoveProjectReference { include: String },
    AddComReference(BasProjComReference),
    RemoveComReference { include: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectEditPlan {
    pub workspace_target: PathBuf,
    pub project_file: PathBuf,
    pub project_name: String,
    pub edits: Vec<HostProjectEdit>,
    pub validation: HostProjectEditValidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectEditValidation {
    pub can_apply: bool,
    pub issues: Vec<HostProjectEditIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProjectEditIssueKind {
    AlreadyPresent,
    MissingTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectEditIssue {
    pub edit_index: usize,
    pub kind: HostProjectEditIssueKind,
    pub include: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjectEditApplication {
    pub workspace_target: PathBuf,
    pub project_file: PathBuf,
    pub project_name: String,
    pub applied_edits: usize,
    pub basproj: BasProj,
}

pub fn inspect_workspace_target(path: &Path) -> Result<HostProjectSurface, BasProjError> {
    if path.is_dir() {
        if let Some(project_file) = discover_project_file_in_dir(path, "basproj")? {
            return inspect_basproj_surface(&project_file);
        }
        if let Some(project_file) = discover_project_file_in_dir(path, "vbp")? {
            return inspect_vbp_surface(&project_file);
        }
        return inspect_convention_surface(path);
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("vbp") => inspect_vbp_surface(path),
        Some("basproj") | None => inspect_basproj_surface(path),
        Some(other) => Err(BasProjError::UnsupportedPath {
            path: path.display().to_string(),
            extension: other.to_string(),
        }),
    }
}

pub fn inspect_workspace_compile_options(
    path: &Path,
) -> Result<HostProjectCompileOptionsSurface, BasProjError> {
    let surface = inspect_workspace_target(path)?;
    let properties = surface
        .project_file
        .as_deref()
        .filter(|project_file| {
            project_file
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("basproj"))
                .unwrap_or(false)
        })
        .map(read_basproj_properties)
        .transpose()?
        .unwrap_or_default();
    let load_result = load_workspace_target(path);
    let loaded = load_result.as_ref().ok();
    let load_error = load_result.as_ref().err();

    let entry_point = loaded
        .and_then(|loaded| loaded.entry_point.clone())
        .or_else(|| properties.entry_point.clone());
    let conditional_constants = loaded
        .map(|loaded| loaded.manifest.conditional_constants.clone())
        .unwrap_or_else(|| {
            properties
                .define_constants
                .as_deref()
                .map(parse_define_constants)
                .unwrap_or_default()
        });

    Ok(HostProjectCompileOptionsSurface {
        workspace_kind: surface.workspace_kind,
        workspace_target: surface.workspace_target,
        project_file: surface.project_file,
        project_name: loaded
            .map(|loaded| loaded.manifest.project_name.clone())
            .unwrap_or(surface.project_name),
        output_type: surface.output_type,
        build_target: loaded
            .map(|loaded| loaded.build_target)
            .or(properties.build_target),
        runtime_flavor: loaded
            .map(|loaded| loaded.runtime_flavor)
            .or(properties.runtime_flavor),
        entry_point: entry_point.clone(),
        default_runtime_profile: loaded
            .and_then(|loaded| loaded.default_runtime_profile.clone())
            .or(properties.default_runtime_profile),
        default_policy_preset: loaded
            .and_then(|loaded| loaded.default_policy_preset.clone())
            .or(properties.default_policy_preset),
        default_root_object: loaded
            .map(|loaded| loaded.default_root_object.clone())
            .or(properties.default_root_object),
        define_constants_raw: properties.define_constants,
        conditional_constants: conditional_constants
            .into_iter()
            .map(|(name, value)| HostProjectConditionalConstant { name, value })
            .collect(),
        build_profiles: default_build_profiles(),
        source_policies: default_source_policy_options(),
        run_targets: run_targets_for_entry_point(entry_point.as_deref(), load_error),
        build_check_status: build_check_status(load_error),
    })
}

pub fn prepare_host_project_settings_edit_plan(
    workspace_path: &Path,
    edits: &[HostProjectSettingsEdit],
) -> Result<HostProjectSettingsEditPlan, BasProjError> {
    let surface = inspect_workspace_target(workspace_path)?;
    let project_file = surface.project_file.clone().ok_or_else(|| {
        BasProjError::HostProjectEditUnsupportedWorkspace {
            path: workspace_path.display().to_string(),
            workspace_kind: host_workspace_kind_name(surface.workspace_kind).to_string(),
        }
    })?;
    if surface.workspace_kind != HostWorkspaceTargetKind::BasProj {
        return Err(BasProjError::HostProjectEditUnsupportedWorkspace {
            path: workspace_path.display().to_string(),
            workspace_kind: host_workspace_kind_name(surface.workspace_kind).to_string(),
        });
    }

    let xml = fs::read_to_string(&project_file).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    let mut basproj = parse_basproj_xml(&xml)?;
    let validation = validate_host_project_settings_edits(edits);
    let preview = if validation.can_apply {
        apply_host_project_settings_edits_to_basproj(&mut basproj, edits);
        Some(compile_options_surface_from_basproj(
            workspace_path,
            &surface,
            basproj,
        )?)
    } else {
        None
    };

    Ok(HostProjectSettingsEditPlan {
        workspace_target: workspace_path.to_path_buf(),
        project_file,
        project_name: surface.project_name,
        edits: edits.to_vec(),
        validation,
        preview,
    })
}

pub fn validate_host_project_settings_edits(
    edits: &[HostProjectSettingsEdit],
) -> HostProjectSettingsEditValidation {
    let mut issues = Vec::new();
    for (edit_index, edit) in edits.iter().enumerate() {
        match edit {
            HostProjectSettingsEdit::SetProjectName(value) if value.trim().is_empty() => {
                issues.push(settings_issue(
                    edit_index,
                    "ProjectName",
                    "project name cannot be empty",
                ));
            }
            HostProjectSettingsEdit::SetEntryPoint(Some(value))
                if parse_entry_point_parts(value).is_none() =>
            {
                issues.push(settings_issue(
                    edit_index,
                    "EntryPoint",
                    "entry point must use Module.Procedure form",
                ));
            }
            HostProjectSettingsEdit::SetDefineConstants(Some(value)) => {
                for part in value
                    .split(';')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                {
                    if let Some((key, raw_value)) = part.split_once('=') {
                        if key.trim().is_empty() || raw_value.trim().parse::<i32>().is_err() {
                            issues.push(settings_issue(
                                edit_index,
                                "DefineConstants",
                                "define constants must use Name or Name=<integer> entries separated by semicolons",
                            ));
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    HostProjectSettingsEditValidation {
        can_apply: issues.is_empty(),
        issues,
    }
}

pub fn apply_host_project_settings_edit_plan(
    plan: &HostProjectSettingsEditPlan,
) -> Result<HostProjectSettingsEditApplication, BasProjError> {
    if !plan.validation.can_apply {
        let summary = plan
            .validation
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(BasProjError::HostProjectEditPlanInvalid(summary));
    }

    let xml = fs::read_to_string(&plan.project_file).map_err(|source| BasProjError::Io {
        path: plan.project_file.display().to_string(),
        source,
    })?;
    let mut basproj = parse_basproj_xml(&xml)?;
    apply_host_project_settings_edits_to_basproj(&mut basproj, &plan.edits);
    fs::write(&plan.project_file, serialize_basproj_xml(&basproj)).map_err(|source| {
        BasProjError::Io {
            path: plan.project_file.display().to_string(),
            source,
        }
    })?;
    let surface = inspect_workspace_compile_options(&plan.workspace_target)?;
    Ok(HostProjectSettingsEditApplication {
        workspace_target: plan.workspace_target.clone(),
        project_file: plan.project_file.clone(),
        project_name: plan.project_name.clone(),
        applied_edits: plan.edits.len(),
        basproj,
        surface,
    })
}

pub fn apply_host_project_settings_edits_to_basproj(
    basproj: &mut BasProj,
    edits: &[HostProjectSettingsEdit],
) {
    for edit in edits {
        match edit {
            HostProjectSettingsEdit::SetProjectName(value) => {
                basproj.properties.project_name = Some(value.trim().to_string());
            }
            HostProjectSettingsEdit::SetEntryPoint(value) => {
                basproj.properties.entry_point = non_empty_owned(value.as_deref());
            }
            HostProjectSettingsEdit::SetDefineConstants(value) => {
                basproj.properties.define_constants = non_empty_owned(value.as_deref());
            }
            HostProjectSettingsEdit::SetBuildTarget(value) => {
                basproj.properties.build_target = *value;
            }
            HostProjectSettingsEdit::SetRuntimeFlavor(value) => {
                basproj.properties.runtime_flavor = *value;
            }
            HostProjectSettingsEdit::SetDefaultRuntimeProfile(value) => {
                basproj.properties.default_runtime_profile = non_empty_owned(value.as_deref());
            }
            HostProjectSettingsEdit::SetDefaultPolicyPreset(value) => {
                basproj.properties.default_policy_preset = non_empty_owned(value.as_deref());
            }
            HostProjectSettingsEdit::SetDefaultRootObject(value) => {
                basproj.properties.default_root_object = non_empty_owned(value.as_deref());
            }
        }
    }
}

pub fn inspect_module_identity(
    file_path: &Path,
    source: &str,
) -> Result<ModuleIdentityInfo, BasProjError> {
    let file_stem = module_file_stem(file_path, &file_path.display().to_string())?;
    let declared_vb_name = parse_vb_name_attribute(source);
    let effective_name = declared_vb_name
        .clone()
        .unwrap_or_else(|| file_stem.clone());
    let attribute_matches_file = declared_vb_name
        .as_ref()
        .map(|name| name.eq_ignore_ascii_case(&file_stem))
        .unwrap_or(false);

    Ok(ModuleIdentityInfo {
        file_stem,
        declared_vb_name,
        effective_name,
        attribute_required: !attribute_matches_file
            && declared_vb_name_for_source(source).is_some(),
        attribute_redundant: attribute_matches_file,
    })
}

pub fn reconcile_module_identity(
    file_path: &Path,
    source: &str,
    desired_logical_name: Option<&str>,
) -> Result<ModuleIdentityRewrite, BasProjError> {
    let file_stem = module_file_stem(file_path, &file_path.display().to_string())?;
    let logical_name = desired_logical_name
        .unwrap_or(&file_stem)
        .trim()
        .to_string();
    if logical_name.is_empty() {
        return Err(BasProjError::ModuleSourceInvalid {
            include: file_path.display().to_string(),
            message: "logical module name cannot be empty".to_string(),
        });
    }

    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines = source
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let had_trailing_newline = source.ends_with('\n');
    let existing = vb_name_attribute_line_index(source);
    let needs_attribute = !logical_name.eq_ignore_ascii_case(&file_stem);

    let attribute_action = match (existing, needs_attribute) {
        (Some(index), true) => {
            lines[index] = format!("Attribute VB_Name = \"{logical_name}\"");
            VbNameAttributeAction::Replace
        }
        (Some(index), false) => {
            lines.remove(index);
            VbNameAttributeAction::Remove
        }
        (None, true) => {
            lines.insert(0, format!("Attribute VB_Name = \"{logical_name}\""));
            VbNameAttributeAction::Insert
        }
        (None, false) => VbNameAttributeAction::None,
    };

    let mut rewritten = lines.join(newline);
    if had_trailing_newline && !rewritten.is_empty() {
        rewritten.push_str(newline);
    }

    Ok(ModuleIdentityRewrite {
        logical_name,
        attribute_action,
        source: rewritten,
    })
}

pub fn plan_new_module(
    kind: BasProjModuleKind,
    include: &str,
    logical_name: Option<&str>,
    option_explicit: bool,
) -> Result<PlannedModule, BasProjError> {
    let include = normalize_include_with_extension(include, kind);
    let file_stem = module_file_stem(Path::new(&include), &include)?;
    let logical_name = logical_name.unwrap_or(&file_stem).trim().to_string();
    if logical_name.is_empty() {
        return Err(BasProjError::ModuleSourceInvalid {
            include: include.clone(),
            message: "logical module name cannot be empty".to_string(),
        });
    }

    let mut source = String::new();
    let rewrite = reconcile_module_identity(Path::new(&include), "", Some(&logical_name))?;
    if !rewrite.source.is_empty() {
        source.push_str(&rewrite.source);
        source.push('\n');
    }
    if option_explicit {
        source.push_str("Option Explicit\n");
    }

    let basproj_item = BasProjModule {
        kind,
        include: include.clone(),
        vb_predeclared_id: false,
        vb_exposed: false,
        vb_global_namespace: false,
        vb_creatable: false,
        host_document_type: None,
        instancing: None,
        prog_id: None,
        description: None,
    };
    let identity = inspect_module_identity(Path::new(&include), &source)?;

    Ok(PlannedModule {
        include,
        basproj_item,
        logical_name,
        source,
        identity,
    })
}

pub fn add_module_edit(module: BasProjModule) -> HostProjectEdit {
    HostProjectEdit::AddModule(module)
}

pub fn remove_module_edit(include: impl Into<String>) -> HostProjectEdit {
    HostProjectEdit::RemoveModule {
        include: include.into(),
    }
}

pub fn add_project_reference_edit(include: impl Into<String>) -> HostProjectEdit {
    HostProjectEdit::AddProjectReference(BasProjProjectReference {
        include: include.into(),
        kind: BasProjProjectReferenceKind::Project,
    })
}

pub fn remove_project_reference_edit(include: impl Into<String>) -> HostProjectEdit {
    HostProjectEdit::RemoveProjectReference {
        include: include.into(),
    }
}

pub fn add_com_reference_edit(include: impl Into<String>) -> HostProjectEdit {
    HostProjectEdit::AddComReference(BasProjComReference {
        include: include.into(),
        guid: None,
        version_major: None,
        version_minor: None,
        lcid: None,
        import_lib: None,
    })
}

pub fn remove_com_reference_edit(include: impl Into<String>) -> HostProjectEdit {
    HostProjectEdit::RemoveComReference {
        include: include.into(),
    }
}

pub fn prepare_host_project_edit_plan(
    workspace_path: &Path,
    edits: &[HostProjectEdit],
) -> Result<HostProjectEditPlan, BasProjError> {
    let surface = inspect_workspace_target(workspace_path)?;
    let project_file = surface.project_file.clone().ok_or_else(|| {
        BasProjError::HostProjectEditUnsupportedWorkspace {
            path: workspace_path.display().to_string(),
            workspace_kind: host_workspace_kind_name(surface.workspace_kind).to_string(),
        }
    })?;
    if surface.workspace_kind != HostWorkspaceTargetKind::BasProj {
        return Err(BasProjError::HostProjectEditUnsupportedWorkspace {
            path: workspace_path.display().to_string(),
            workspace_kind: host_workspace_kind_name(surface.workspace_kind).to_string(),
        });
    }

    let xml = fs::read_to_string(&project_file).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    let basproj = parse_basproj_xml(&xml)?;
    let validation = validate_host_project_edits(&basproj, edits);

    Ok(HostProjectEditPlan {
        workspace_target: workspace_path.to_path_buf(),
        project_file,
        project_name: surface.project_name,
        edits: edits.to_vec(),
        validation,
    })
}

pub fn validate_host_project_edits(
    basproj: &BasProj,
    edits: &[HostProjectEdit],
) -> HostProjectEditValidation {
    let mut module_includes = basproj
        .modules
        .iter()
        .map(|module| normalize_include_key(&module.include))
        .collect::<Vec<_>>();
    let mut project_reference_includes = basproj
        .project_references
        .iter()
        .map(|reference| normalize_include_key(&reference.include))
        .collect::<Vec<_>>();
    let mut com_reference_includes = basproj
        .com_references
        .iter()
        .map(|reference| normalize_include_key(&reference.include))
        .collect::<Vec<_>>();

    let mut issues = Vec::new();

    for (edit_index, edit) in edits.iter().enumerate() {
        match edit {
            HostProjectEdit::AddModule(module) => {
                let include = normalize_include_key(&module.include);
                if module_includes.iter().any(|existing| existing == &include) {
                    issues.push(HostProjectEditIssue {
                        edit_index,
                        kind: HostProjectEditIssueKind::AlreadyPresent,
                        include: module.include.clone(),
                        message: format!("module `{}` already exists", module.include),
                    });
                } else {
                    module_includes.push(include);
                }
            }
            HostProjectEdit::RemoveModule { include } => {
                if !remove_first_matching(&mut module_includes, include) {
                    issues.push(HostProjectEditIssue {
                        edit_index,
                        kind: HostProjectEditIssueKind::MissingTarget,
                        include: include.clone(),
                        message: format!("module `{include}` is not present"),
                    });
                }
            }
            HostProjectEdit::AddProjectReference(reference) => {
                let include = normalize_include_key(&reference.include);
                if project_reference_includes
                    .iter()
                    .any(|existing| existing == &include)
                {
                    issues.push(HostProjectEditIssue {
                        edit_index,
                        kind: HostProjectEditIssueKind::AlreadyPresent,
                        include: reference.include.clone(),
                        message: format!(
                            "project reference `{}` already exists",
                            reference.include
                        ),
                    });
                } else {
                    project_reference_includes.push(include);
                }
            }
            HostProjectEdit::RemoveProjectReference { include } => {
                if !remove_first_matching(&mut project_reference_includes, include) {
                    issues.push(HostProjectEditIssue {
                        edit_index,
                        kind: HostProjectEditIssueKind::MissingTarget,
                        include: include.clone(),
                        message: format!("project reference `{include}` is not present"),
                    });
                }
            }
            HostProjectEdit::AddComReference(reference) => {
                let include = normalize_include_key(&reference.include);
                if com_reference_includes
                    .iter()
                    .any(|existing| existing == &include)
                {
                    issues.push(HostProjectEditIssue {
                        edit_index,
                        kind: HostProjectEditIssueKind::AlreadyPresent,
                        include: reference.include.clone(),
                        message: format!("COM reference `{}` already exists", reference.include),
                    });
                } else {
                    com_reference_includes.push(include);
                }
            }
            HostProjectEdit::RemoveComReference { include } => {
                if !remove_first_matching(&mut com_reference_includes, include) {
                    issues.push(HostProjectEditIssue {
                        edit_index,
                        kind: HostProjectEditIssueKind::MissingTarget,
                        include: include.clone(),
                        message: format!("COM reference `{include}` is not present"),
                    });
                }
            }
        }
    }

    HostProjectEditValidation {
        can_apply: issues.is_empty(),
        issues,
    }
}

pub fn apply_host_project_edits_to_basproj(basproj: &mut BasProj, edits: &[HostProjectEdit]) {
    for edit in edits {
        match edit {
            HostProjectEdit::AddModule(module) => {
                upsert_by_include(&mut basproj.modules, module, |item| &item.include);
            }
            HostProjectEdit::RemoveModule { include } => {
                remove_by_include(&mut basproj.modules, include, |item| &item.include);
            }
            HostProjectEdit::AddProjectReference(reference) => {
                upsert_by_include(&mut basproj.project_references, reference, |item| {
                    &item.include
                });
            }
            HostProjectEdit::RemoveProjectReference { include } => {
                remove_by_include(&mut basproj.project_references, include, |item| {
                    &item.include
                });
            }
            HostProjectEdit::AddComReference(reference) => {
                upsert_by_include(&mut basproj.com_references, reference, |item| &item.include);
            }
            HostProjectEdit::RemoveComReference { include } => {
                remove_by_include(&mut basproj.com_references, include, |item| &item.include);
            }
        }
    }
}

pub fn apply_host_project_edits_to_basproj_path(
    project_file: &Path,
    edits: &[HostProjectEdit],
) -> Result<BasProj, BasProjError> {
    let xml = fs::read_to_string(project_file).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    let mut basproj = parse_basproj_xml(&xml)?;
    apply_host_project_edits_to_basproj(&mut basproj, edits);
    let rewritten = serialize_basproj_xml(&basproj);
    fs::write(project_file, rewritten).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    Ok(basproj)
}

pub fn apply_host_project_edit_plan(
    plan: &HostProjectEditPlan,
) -> Result<HostProjectEditApplication, BasProjError> {
    if !plan.validation.can_apply {
        let summary = plan
            .validation
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(BasProjError::HostProjectEditPlanInvalid(summary));
    }

    let basproj = apply_host_project_edits_to_basproj_path(&plan.project_file, &plan.edits)?;
    Ok(HostProjectEditApplication {
        workspace_target: plan.workspace_target.clone(),
        project_file: plan.project_file.clone(),
        project_name: plan.project_name.clone(),
        applied_edits: plan.edits.len(),
        basproj,
    })
}

fn read_basproj_properties(project_file: &Path) -> Result<crate::BasProjProperties, BasProjError> {
    let xml = fs::read_to_string(project_file).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    Ok(parse_basproj_xml(&xml)?.properties)
}

fn compile_options_surface_from_basproj(
    workspace_path: &Path,
    surface: &HostProjectSurface,
    basproj: BasProj,
) -> Result<HostProjectCompileOptionsSurface, BasProjError> {
    let conditional_constants = basproj
        .properties
        .define_constants
        .as_deref()
        .map(parse_define_constants)
        .unwrap_or_default();
    let entry_point = basproj.properties.entry_point.clone();
    Ok(HostProjectCompileOptionsSurface {
        workspace_kind: surface.workspace_kind,
        workspace_target: workspace_path.to_path_buf(),
        project_file: surface.project_file.clone(),
        project_name: basproj
            .properties
            .project_name
            .clone()
            .unwrap_or_else(|| surface.project_name.clone()),
        output_type: basproj
            .properties
            .output_type
            .unwrap_or(surface.output_type),
        build_target: basproj.properties.build_target,
        runtime_flavor: basproj.properties.runtime_flavor,
        entry_point: entry_point.clone(),
        default_runtime_profile: basproj.properties.default_runtime_profile,
        default_policy_preset: basproj.properties.default_policy_preset,
        default_root_object: basproj.properties.default_root_object,
        define_constants_raw: basproj.properties.define_constants,
        conditional_constants: conditional_constants
            .into_iter()
            .map(|(name, value)| HostProjectConditionalConstant { name, value })
            .collect(),
        build_profiles: default_build_profiles(),
        source_policies: default_source_policy_options(),
        run_targets: run_targets_for_entry_point(entry_point.as_deref(), None),
        build_check_status: DirectHostCommandStatus::available(),
    })
}

fn default_build_profiles() -> Vec<HostProjectBuildProfile> {
    vec![HostProjectBuildProfile {
        name: "default".to_string(),
        display_name: "Default".to_string(),
        command_status: DirectHostCommandStatus::available(),
    }]
}

fn default_source_policy_options() -> Vec<HostProjectSourcePolicyOption> {
    vec![
        HostProjectSourcePolicyOption {
            kind: HostProjectSourcePolicyKind::DiskOnly,
            display_name: "Disk only".to_string(),
            command_status: DirectHostCommandStatus::available(),
        },
        HostProjectSourcePolicyOption {
            kind: HostProjectSourcePolicyKind::WorkspaceOverlay,
            display_name: "Workspace overlay".to_string(),
            command_status: DirectHostCommandStatus::available(),
        },
    ]
}

fn run_targets_for_entry_point(
    entry_point: Option<&str>,
    load_error: Option<&BasProjError>,
) -> Vec<HostProjectRunTarget> {
    if let Some(entry_point) = entry_point {
        let (module_name, procedure_name) = parse_entry_point_parts(entry_point)
            .unwrap_or_else(|| (entry_point.to_string(), String::new()));
        return vec![HostProjectRunTarget {
            id: entry_point.to_string(),
            display_name: entry_point.to_string(),
            kind: HostProjectRunTargetKind::ConfiguredOrDiscoveredEntryPoint,
            module_name: Some(module_name),
            procedure_name: Some(procedure_name),
            command_status: DirectHostCommandStatus::available(),
        }];
    }

    let reason = load_error
        .map(run_target_issue_for_load_error)
        .unwrap_or_else(|| DirectHostIssue::new(DirectHostIssueKind::RunTargetMissing));
    vec![HostProjectRunTarget {
        id: "<missing>".to_string(),
        display_name: "No run target".to_string(),
        kind: HostProjectRunTargetKind::MissingOrInvalid,
        module_name: None,
        procedure_name: None,
        command_status: DirectHostCommandStatus::disabled(reason),
    }]
}

fn build_check_status(load_error: Option<&BasProjError>) -> DirectHostCommandStatus {
    match load_error {
        Some(error) => DirectHostCommandStatus::disabled(
            DirectHostIssue::new(DirectHostIssueKind::ProjectInvalid)
                .with_technical_detail(error.to_string())
                .with_retryability(DirectHostRetryability::NotRetryable),
        ),
        None => DirectHostCommandStatus::available(),
    }
}

fn run_target_issue_for_load_error(error: &BasProjError) -> DirectHostIssue {
    let kind = match error {
        BasProjError::EntryPointRequired(_)
        | BasProjError::EntryPointInvalid(_)
        | BasProjError::EntryPointNotFound(_)
        | BasProjError::EntryPointAmbiguous(_) => DirectHostIssueKind::RunTargetMissing,
        _ => DirectHostIssueKind::ProjectInvalid,
    };
    DirectHostIssue::new(kind)
        .with_technical_detail(error.to_string())
        .with_retryability(DirectHostRetryability::NotRetryable)
}

fn parse_entry_point_parts(entry_point: &str) -> Option<(String, String)> {
    let (module, procedure) = entry_point.trim().split_once('.')?;
    let module = module.trim();
    let procedure = procedure.trim();
    if module.is_empty() || procedure.is_empty() {
        return None;
    }
    Some((module.to_string(), procedure.to_string()))
}

fn settings_issue(edit_index: usize, field: &str, message: &str) -> HostProjectSettingsEditIssue {
    HostProjectSettingsEditIssue {
        edit_index,
        kind: HostProjectSettingsEditIssueKind::InvalidValue,
        field: field.to_string(),
        message: message.to_string(),
    }
}

fn non_empty_owned(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn inspect_basproj_surface(project_file: &Path) -> Result<HostProjectSurface, BasProjError> {
    let xml = fs::read_to_string(project_file).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    let basproj = parse_basproj_xml(&xml)?;
    let project_dir = project_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_name = basproj
        .properties
        .project_name
        .clone()
        .unwrap_or_else(|| infer_project_name_from_path(&project_dir));
    let output_type = basproj
        .properties
        .output_type
        .ok_or_else(|| BasProjError::MissingProperty("OutputType".to_string()))?;

    let mut modules = basproj
        .modules
        .iter()
        .map(|module| host_project_module_info(&project_dir, module.kind, &module.include))
        .collect::<Result<Vec<_>, _>>()?;
    modules.sort_by(|left, right| left.include.cmp(&right.include));

    let mut references = Vec::new();
    references.extend(basproj.project_references.iter().map(|reference| {
        HostProjectReferenceInfo {
            kind: match reference.kind {
                BasProjProjectReferenceKind::Project => HostProjectReferenceKind::Project,
                BasProjProjectReferenceKind::HostInjected => HostProjectReferenceKind::HostInjected,
            },
            include: reference.include.clone(),
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
            import_lib: None,
            path: None,
            referenced_project_name: None,
        }
    }));
    references.extend(
        basproj
            .com_references
            .iter()
            .map(|reference| HostProjectReferenceInfo {
                kind: HostProjectReferenceKind::Com,
                include: reference.include.clone(),
                guid: reference.guid.clone(),
                version_major: reference.version_major,
                version_minor: reference.version_minor,
                lcid: reference.lcid,
                import_lib: reference.import_lib.clone(),
                path: None,
                referenced_project_name: None,
            }),
    );
    references.extend(
        basproj
            .native_references
            .iter()
            .map(|reference| HostProjectReferenceInfo {
                kind: HostProjectReferenceKind::Native,
                include: reference.include.clone(),
                guid: None,
                version_major: None,
                version_minor: None,
                lcid: None,
                import_lib: None,
                path: reference.path.clone(),
                referenced_project_name: None,
            }),
    );
    references.sort_by(|left, right| {
        left.include.cmp(&right.include).then_with(|| {
            host_reference_sort_key(left.kind).cmp(&host_reference_sort_key(right.kind))
        })
    });

    Ok(HostProjectSurface {
        workspace_kind: HostWorkspaceTargetKind::BasProj,
        workspace_target: project_file.to_path_buf(),
        project_file: Some(project_file.to_path_buf()),
        project_dir,
        project_name,
        output_type,
        modules,
        references,
    })
}

fn inspect_vbp_surface(project_file: &Path) -> Result<HostProjectSurface, BasProjError> {
    let content = fs::read_to_string(project_file).map_err(|source| BasProjError::Io {
        path: project_file.display().to_string(),
        source,
    })?;
    let vbp = parse_vbp(&content).map_err(BasProjError::VbpParse)?;
    let project_dir = project_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_name = vbp.project_name.clone();
    let output_type = output_type_from_vbp_type(&vbp.project_type)?;

    let mut modules = Vec::new();
    for module in &vbp.modules {
        modules.push(host_project_module_info(
            &project_dir,
            BasProjModuleKind::Module,
            &module.path,
        )?);
    }
    for class in &vbp.classes {
        modules.push(host_project_module_info(
            &project_dir,
            BasProjModuleKind::ClassModule,
            &class.path,
        )?);
    }
    modules.sort_by(|left, right| left.include.cmp(&right.include));

    let mut references = Vec::new();
    for reference in &vbp.references {
        match reference {
            VbpReference::Project(reference) => references.push(HostProjectReferenceInfo {
                kind: HostProjectReferenceKind::Project,
                include: reference.include.clone(),
                guid: None,
                version_major: None,
                version_minor: None,
                lcid: None,
                import_lib: None,
                path: None,
                referenced_project_name: reference.referenced_project_name.clone(),
            }),
            VbpReference::TypeLibrary(reference) => {
                let (version_major, version_minor) = parse_version_pair(&reference.version);
                references.push(HostProjectReferenceInfo {
                    kind: HostProjectReferenceKind::Com,
                    include: reference.name.clone(),
                    guid: Some(reference.guid.clone()),
                    version_major,
                    version_minor,
                    lcid: None,
                    import_lib: None,
                    path: None,
                    referenced_project_name: None,
                });
            }
        }
    }
    references.sort_by(|left, right| {
        left.include.cmp(&right.include).then_with(|| {
            host_reference_sort_key(left.kind).cmp(&host_reference_sort_key(right.kind))
        })
    });

    Ok(HostProjectSurface {
        workspace_kind: HostWorkspaceTargetKind::Vbp,
        workspace_target: project_file.to_path_buf(),
        project_file: Some(project_file.to_path_buf()),
        project_dir,
        project_name,
        output_type,
        modules,
        references,
    })
}

fn inspect_convention_surface(project_dir: &Path) -> Result<HostProjectSurface, BasProjError> {
    let mut modules = Vec::new();
    collect_convention_modules(project_dir, project_dir, &mut modules)?;
    modules.sort_by(|left, right| left.include.cmp(&right.include));

    Ok(HostProjectSurface {
        workspace_kind: HostWorkspaceTargetKind::ConventionDirectory,
        workspace_target: project_dir.to_path_buf(),
        project_file: None,
        project_dir: project_dir.to_path_buf(),
        project_name: infer_project_name_from_path(project_dir),
        output_type: OutputType::Exe,
        modules,
        references: Vec::new(),
    })
}

fn host_project_module_info(
    project_dir: &Path,
    kind: BasProjModuleKind,
    include: &str,
) -> Result<HostProjectModuleInfo, BasProjError> {
    let source_path = project_dir.join(include);
    let source = fs::read_to_string(&source_path).map_err(|source| BasProjError::Io {
        path: source_path.display().to_string(),
        source,
    })?;
    let identity = inspect_module_identity(&source_path, &source)?;
    Ok(HostProjectModuleInfo {
        kind,
        include: include.replace('\\', "/"),
        source_path,
        identity,
    })
}

fn collect_convention_modules(
    base_dir: &Path,
    dir: &Path,
    modules: &mut Vec<HostProjectModuleInfo>,
) -> Result<(), BasProjError> {
    let entries = fs::read_dir(dir).map_err(|source| BasProjError::Io {
        path: dir.display().to_string(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| BasProjError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_convention_modules(base_dir, &path, modules)?;
            continue;
        }

        let kind = match path.extension().and_then(|ext| ext.to_str()) {
            Some("bas") => Some(BasProjModuleKind::Module),
            Some("cls") => Some(BasProjModuleKind::ClassModule),
            _ => None,
        };
        let Some(kind) = kind else {
            continue;
        };

        let include = path
            .strip_prefix(base_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        modules.push(host_project_module_info(base_dir, kind, &include)?);
    }

    Ok(())
}

fn parse_version_pair(version: &str) -> (Option<u16>, Option<u16>) {
    let Some((major, minor)) = version.split_once('.') else {
        return (None, None);
    };
    (major.parse().ok(), minor.parse().ok())
}

fn host_reference_sort_key(kind: HostProjectReferenceKind) -> u8 {
    match kind {
        HostProjectReferenceKind::Project => 0,
        HostProjectReferenceKind::HostInjected => 1,
        HostProjectReferenceKind::Com => 2,
        HostProjectReferenceKind::Native => 3,
    }
}

fn output_type_from_vbp_type(project_type: &str) -> Result<OutputType, BasProjError> {
    match project_type {
        "HostModule" => Ok(OutputType::HostModule),
        "Library" => Ok(OutputType::Library),
        "Exe" => Ok(OutputType::Exe),
        "Addin" => Ok(OutputType::Addin),
        "ComServer" => Ok(OutputType::ComServer),
        "ComExe" => Ok(OutputType::ComExe),
        other => Err(BasProjError::VbpUnsupported(format!(
            "unsupported VBP output type `{other}` for host inspection"
        ))),
    }
}

fn normalize_include_with_extension(include: &str, kind: BasProjModuleKind) -> String {
    let path = Path::new(include);
    if path.extension().is_some() {
        return include.to_string();
    }

    let extension = match kind {
        BasProjModuleKind::Module => "bas",
        BasProjModuleKind::ClassModule | BasProjModuleKind::DocumentModule => "cls",
    };
    format!("{include}.{extension}")
}

fn module_file_stem(path: &Path, include: &str) -> Result<String, BasProjError> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .map(|stem| stem.to_string())
        .ok_or_else(|| BasProjError::ModuleSourceInvalid {
            include: include.to_string(),
            message: "cannot derive module file stem".to_string(),
        })
}

fn host_workspace_kind_name(kind: HostWorkspaceTargetKind) -> &'static str {
    match kind {
        HostWorkspaceTargetKind::BasProj => "BasProj",
        HostWorkspaceTargetKind::Vbp => "Vbp",
        HostWorkspaceTargetKind::ConventionDirectory => "ConventionDirectory",
    }
}

fn normalize_include_key(include: &str) -> String {
    include.replace('\\', "/").to_ascii_lowercase()
}

fn remove_first_matching(items: &mut Vec<String>, target_include: &str) -> bool {
    let target = normalize_include_key(target_include);
    if let Some(index) = items.iter().position(|item| item == &target) {
        items.remove(index);
        true
    } else {
        false
    }
}

fn upsert_by_include<T, F>(items: &mut Vec<T>, new_item: &T, include: F)
where
    T: Clone,
    F: Fn(&T) -> &str,
{
    if let Some(index) = items
        .iter()
        .position(|item| include(item).eq_ignore_ascii_case(include(new_item)))
    {
        items[index] = new_item.clone();
    } else {
        items.push(new_item.clone());
    }
}

fn remove_by_include<T, F>(items: &mut Vec<T>, target_include: &str, include: F)
where
    F: Fn(&T) -> &str,
{
    items.retain(|item| !include(item).eq_ignore_ascii_case(target_include));
}

fn declared_vb_name_for_source(source: &str) -> Option<String> {
    parse_vb_name_attribute(source)
}

fn parse_vb_name_attribute(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let remainder = trimmed.strip_prefix("Attribute VB_Name = \"")?;
        remainder.strip_suffix('"').map(|value| value.to_string())
    })
}

fn vb_name_attribute_line_index(source: &str) -> Option<usize> {
    source.lines().position(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("Attribute VB_Name = \"") && trimmed.ends_with('"')
    })
}

#[cfg(test)]
mod tests {
    use super::{
        HostProjectEditIssueKind, HostProjectReferenceKind, HostProjectRunTargetKind,
        HostProjectSettingsEdit, HostProjectSettingsEditIssueKind, HostWorkspaceTargetKind,
        VbNameAttributeAction, add_com_reference_edit, add_project_reference_edit,
        apply_host_project_edit_plan, apply_host_project_edits_to_basproj,
        apply_host_project_edits_to_basproj_path, apply_host_project_settings_edit_plan,
        inspect_module_identity, inspect_workspace_compile_options, inspect_workspace_target,
        plan_new_module, prepare_host_project_edit_plan, prepare_host_project_settings_edit_plan,
        reconcile_module_identity, validate_host_project_edits,
        validate_host_project_settings_edits,
    };
    use crate::{BasProjModuleKind, BuildTarget, RuntimeFlavor, parse_basproj_xml};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn inspect_module_identity_marks_matching_vb_name_as_redundant() {
        let info = inspect_module_identity(
            Path::new("Widget.cls"),
            "Attribute VB_Name = \"Widget\"\nOption Explicit\n",
        )
        .expect("inspect");

        assert_eq!(info.file_stem, "Widget");
        assert_eq!(info.effective_name, "Widget");
        assert_eq!(info.declared_vb_name.as_deref(), Some("Widget"));
        assert!(info.attribute_redundant);
        assert!(!info.attribute_required);
    }

    #[test]
    fn reconcile_module_identity_inserts_vb_name_when_logical_name_differs() {
        let rewrite = reconcile_module_identity(
            Path::new("Widget.cls"),
            "Option Explicit\n",
            Some("Application"),
        )
        .expect("rewrite");

        assert_eq!(rewrite.attribute_action, VbNameAttributeAction::Insert);
        assert_eq!(
            rewrite.source,
            "Attribute VB_Name = \"Application\"\nOption Explicit\n"
        );
    }

    #[test]
    fn reconcile_module_identity_removes_redundant_vb_name() {
        let rewrite = reconcile_module_identity(
            Path::new("Widget.cls"),
            "Attribute VB_Name = \"Widget\"\nOption Explicit\n",
            Some("Widget"),
        )
        .expect("rewrite");

        assert_eq!(rewrite.attribute_action, VbNameAttributeAction::Remove);
        assert_eq!(rewrite.source, "Option Explicit\n");
    }

    #[test]
    fn plan_new_module_appends_kind_extension_and_scaffolds_source() {
        let planned = plan_new_module(BasProjModuleKind::Module, "MathHelpers", None, true)
            .expect("planned module");

        assert_eq!(planned.include, "MathHelpers.bas");
        assert_eq!(planned.logical_name, "MathHelpers");
        assert_eq!(planned.basproj_item.include, "MathHelpers.bas");
        assert_eq!(planned.source, "Option Explicit\n");
    }

    #[test]
    fn plan_new_module_emits_vb_name_when_logical_name_diverges() {
        let planned = plan_new_module(
            BasProjModuleKind::ClassModule,
            "Widget",
            Some("Application"),
            true,
        )
        .expect("planned class module");

        assert_eq!(planned.include, "Widget.cls");
        assert!(
            planned
                .source
                .contains("Attribute VB_Name = \"Application\"")
        );
        assert!(planned.identity.attribute_required);
    }

    #[test]
    fn project_reference_edit_helpers_create_typed_edits() {
        let project_edit = add_project_reference_edit("../Lib/Lib.basproj");
        let com_edit = add_com_reference_edit("Scripting");

        assert!(matches!(
            project_edit,
            super::HostProjectEdit::AddProjectReference(_)
        ));
        assert!(matches!(
            com_edit,
            super::HostProjectEdit::AddComReference(_)
        ));
    }

    #[test]
    fn inspect_workspace_target_reports_basproj_modules_and_references() {
        let temp_root = unique_temp_dir("oxvba_project_host_surface_basproj");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Widget.cls"),
            "Attribute VB_Name = \"Application\"\nOption Explicit\n",
        )
        .expect("class module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <ClassModule Include=\"Widget.cls\" />\n    <ProjectReference Include=\"../Lib/Lib.basproj\" />\n    <COMReference Include=\"Scripting\">\n      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>\n      <VersionMajor>1</VersionMajor>\n      <VersionMinor>0</VersionMinor>\n    </COMReference>\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let surface = inspect_workspace_target(&temp_root).expect("surface");
        assert_eq!(surface.workspace_kind, HostWorkspaceTargetKind::BasProj);
        assert_eq!(surface.project_name, "App");
        assert_eq!(surface.modules.len(), 1);
        assert_eq!(surface.modules[0].include, "Widget.cls");
        assert_eq!(surface.modules[0].identity.effective_name, "Application");
        assert_eq!(surface.references.len(), 2);
        assert!(surface.references.iter().any(|reference| {
            reference.kind == HostProjectReferenceKind::Project
                && reference.include == "../Lib/Lib.basproj"
        }));
        assert!(surface.references.iter().any(|reference| {
            reference.kind == HostProjectReferenceKind::Com
                && reference.include == "Scripting"
                && reference.guid.as_deref() == Some("{420B2830-E718-11CF-893D-00A0C9054228}")
        }));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn inspect_workspace_target_reports_vbp_and_convention_rosters() {
        let temp_root = unique_temp_dir("oxvba_project_host_surface_vbp");
        let vbp_dir = temp_root.join("VbpApp");
        let convention_dir = temp_root.join("Convention");
        fs::create_dir_all(&vbp_dir).expect("vbp dir");
        fs::create_dir_all(convention_dir.join("Nested")).expect("convention dir");

        fs::write(vbp_dir.join("Main.bas"), "Sub Main()\nEnd Sub\n").expect("main");
        fs::write(
            vbp_dir.join("Thing.cls"),
            "Attribute VB_Name = \"Thing\"\nOption Explicit\n",
        )
        .expect("thing");
        fs::write(
            vbp_dir.join("App.vbp"),
            "Type=Exe\nName=\"App\"\nModule=Main; Main.bas\nClass=Thing; Thing.cls\nReference=*\\A{11111111-2222-3333-4444-555555555555}#1.0#0#..\\LibScale\\LibScale.vbp#LibScale\n",
        )
        .expect("vbp");

        fs::write(
            convention_dir.join("Nested").join("Widget.bas"),
            "Sub WidgetMain()\nEnd Sub\n",
        )
        .expect("widget");

        let vbp_surface = inspect_workspace_target(&vbp_dir).expect("vbp surface");
        assert_eq!(vbp_surface.workspace_kind, HostWorkspaceTargetKind::Vbp);
        assert_eq!(vbp_surface.modules.len(), 2);
        assert!(
            vbp_surface
                .modules
                .iter()
                .any(|module| module.include == "Main.bas")
        );
        assert!(vbp_surface.references.iter().any(|reference| {
            reference.kind == HostProjectReferenceKind::Project
                && reference.include == "../LibScale/LibScale.vbp"
                && reference.referenced_project_name.as_deref() == Some("LibScale")
        }));

        let convention_surface =
            inspect_workspace_target(&convention_dir).expect("convention surface");
        assert_eq!(
            convention_surface.workspace_kind,
            HostWorkspaceTargetKind::ConventionDirectory
        );
        assert_eq!(convention_surface.references.len(), 0);
        assert!(convention_surface.modules.iter().any(|module| {
            module.include == "Nested/Widget.bas"
                && module.kind == BasProjModuleKind::Module
                && module.identity.effective_name == "Widget"
        }));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn inspect_workspace_compile_options_reports_run_target_and_settings() {
        let temp_root = unique_temp_dir("oxvba_project_compile_options");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Main.bas"),
            "Public Sub Main()\nEnd Sub\nPublic Sub Boot()\nEnd Sub\n",
        )
        .expect("main");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <BuildTarget>WrapperExe</BuildTarget>\n    <ProjectName>App</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n    <RuntimeFlavor>Jit</RuntimeFlavor>\n    <DefaultRuntimeProfile>windows-native</DefaultRuntimeProfile>\n    <DefaultPolicyPreset>standard</DefaultPolicyPreset>\n    <DefaultRootObject>Application</DefaultRootObject>\n    <DefineConstants>Win64=1;Debug</DefineConstants>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let surface = inspect_workspace_compile_options(&temp_root).expect("compile options");
        assert_eq!(surface.project_name, "App");
        assert_eq!(surface.build_target, Some(BuildTarget::WrapperExe));
        assert_eq!(surface.runtime_flavor, Some(RuntimeFlavor::Jit));
        assert_eq!(surface.entry_point.as_deref(), Some("Main.Main"));
        assert_eq!(
            surface.default_runtime_profile.as_deref(),
            Some("windows-native")
        );
        assert_eq!(surface.default_policy_preset.as_deref(), Some("standard"));
        assert_eq!(surface.default_root_object.as_deref(), Some("Application"));
        assert!(surface.build_check_status.is_available());
        assert_eq!(surface.build_profiles[0].name, "default");
        assert_eq!(surface.source_policies.len(), 2);
        assert!(
            surface
                .conditional_constants
                .iter()
                .any(|constant| { constant.name == "Win64" && constant.value == 1 })
        );
        assert!(
            surface
                .conditional_constants
                .iter()
                .any(|constant| { constant.name == "Debug" && constant.value == 1 })
        );
        assert_eq!(surface.run_targets.len(), 1);
        assert_eq!(
            surface.run_targets[0].kind,
            HostProjectRunTargetKind::ConfiguredOrDiscoveredEntryPoint
        );
        assert_eq!(surface.run_targets[0].module_name.as_deref(), Some("Main"));
        assert_eq!(
            surface.run_targets[0].procedure_name.as_deref(),
            Some("Main")
        );
        assert!(surface.run_targets[0].command_status.is_available());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn project_settings_edit_plan_validates_previews_and_applies_compile_options() {
        let temp_root = unique_temp_dir("oxvba_project_settings_plan");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Main.bas"),
            "Public Sub Main()\nEnd Sub\nPublic Sub Boot()\nEnd Sub\n",
        )
        .expect("main");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n    <EntryPoint>Main.Main</EntryPoint>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let invalid =
            validate_host_project_settings_edits(&[HostProjectSettingsEdit::SetEntryPoint(Some(
                "Main".to_string(),
            ))]);
        assert!(!invalid.can_apply);
        assert_eq!(
            invalid.issues[0].kind,
            HostProjectSettingsEditIssueKind::InvalidValue
        );

        let plan = prepare_host_project_settings_edit_plan(
            &temp_root,
            &[
                HostProjectSettingsEdit::SetProjectName("RenamedApp".to_string()),
                HostProjectSettingsEdit::SetEntryPoint(Some("Main.Boot".to_string())),
                HostProjectSettingsEdit::SetDefineConstants(Some("Trace=1".to_string())),
                HostProjectSettingsEdit::SetRuntimeFlavor(Some(RuntimeFlavor::Lite)),
                HostProjectSettingsEdit::SetBuildTarget(Some(BuildTarget::Bundle)),
            ],
        )
        .expect("settings plan");
        assert!(plan.validation.can_apply);
        let preview = plan.preview.as_ref().expect("preview");
        assert_eq!(preview.project_name, "RenamedApp");
        assert_eq!(preview.entry_point.as_deref(), Some("Main.Boot"));
        assert_eq!(preview.runtime_flavor, Some(RuntimeFlavor::Lite));
        assert_eq!(preview.build_target, Some(BuildTarget::Bundle));

        let application = apply_host_project_settings_edit_plan(&plan).expect("apply plan");
        assert_eq!(application.applied_edits, 5);
        assert_eq!(application.surface.project_name, "RenamedApp");
        assert_eq!(
            application.surface.entry_point.as_deref(),
            Some("Main.Boot")
        );
        let written = fs::read_to_string(temp_root.join("App.basproj")).expect("read basproj");
        assert!(written.contains("<ProjectName>RenamedApp</ProjectName>"));
        assert!(written.contains("<EntryPoint>Main.Boot</EntryPoint>"));
        assert!(written.contains("<DefineConstants>Trace=1</DefineConstants>"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn apply_host_project_edits_updates_com_references_in_memory() {
        let mut basproj = parse_basproj_xml(
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <COMReference Include=\"Legacy\">\n      <Guid>{11111111-1111-1111-1111-111111111111}</Guid>\n      <VersionMajor>1</VersionMajor>\n      <VersionMinor>0</VersionMinor>\n      <ImportLib>legacy.tlb</ImportLib>\n    </COMReference>\n  </ItemGroup>\n</Project>\n",
        )
        .expect("parse basproj");

        apply_host_project_edits_to_basproj(
            &mut basproj,
            &[
                add_com_reference_edit("Scripting"),
                super::HostProjectEdit::RemoveComReference {
                    include: "Legacy".to_string(),
                },
            ],
        );

        assert_eq!(basproj.com_references.len(), 1);
        assert_eq!(basproj.com_references[0].include, "Scripting");
    }

    #[test]
    fn apply_host_project_edits_to_basproj_path_round_trips_com_reference_xml() {
        let temp_root = unique_temp_dir("oxvba_project_apply_com_edits");
        fs::create_dir_all(&temp_root).expect("temp dir");
        let project_file = temp_root.join("App.basproj");
        fs::write(
            &project_file,
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let edited = apply_host_project_edits_to_basproj_path(
            &project_file,
            &[super::HostProjectEdit::AddComReference(
                crate::BasProjComReference {
                    include: "Scripting".to_string(),
                    guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
                    version_major: Some(1),
                    version_minor: Some(0),
                    lcid: Some(0),
                    import_lib: Some("scrrun.dll".to_string()),
                },
            )],
        )
        .expect("apply edits");

        assert_eq!(edited.com_references.len(), 1);
        let written = fs::read_to_string(&project_file).expect("read rewritten basproj");
        assert!(written.contains("<COMReference Include=\"Scripting\">"));
        assert!(written.contains("<ImportLib>scrrun.dll</ImportLib>"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn validate_host_project_edits_reports_duplicate_add_and_missing_remove() {
        let basproj = parse_basproj_xml(
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n    <ProjectReference Include=\"../Lib/Lib.basproj\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("parse basproj");

        let validation = validate_host_project_edits(
            &basproj,
            &[
                super::add_module_edit(crate::BasProjModule {
                    kind: BasProjModuleKind::Module,
                    include: "Main.bas".to_string(),
                    vb_predeclared_id: false,
                    vb_exposed: false,
                    vb_global_namespace: false,
                    vb_creatable: false,
                    host_document_type: None,
                    instancing: None,
                    prog_id: None,
                    description: None,
                }),
                super::remove_project_reference_edit("../Missing/Thing.basproj"),
            ],
        );

        assert!(!validation.can_apply);
        assert_eq!(validation.issues.len(), 2);
        assert_eq!(
            validation.issues[0].kind,
            HostProjectEditIssueKind::AlreadyPresent
        );
        assert_eq!(
            validation.issues[1].kind,
            HostProjectEditIssueKind::MissingTarget
        );
    }

    #[test]
    fn prepare_host_project_edit_plan_reports_validation_and_apply_round_trips() {
        let temp_root = unique_temp_dir("oxvba_project_host_edit_plan");
        fs::create_dir_all(&temp_root).expect("temp dir");
        let project_file = temp_root.join("App.basproj");
        fs::write(temp_root.join("Main.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            &project_file,
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Main.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("write basproj");

        let plan = prepare_host_project_edit_plan(
            &temp_root,
            &[
                add_project_reference_edit("../Lib/Lib.basproj"),
                add_com_reference_edit("Scripting"),
            ],
        )
        .expect("plan");
        assert!(plan.validation.can_apply);
        assert_eq!(plan.project_name, "App");
        assert_eq!(plan.edits.len(), 2);

        let application = apply_host_project_edit_plan(&plan).expect("apply plan");
        assert_eq!(application.applied_edits, 2);
        assert_eq!(application.basproj.project_references.len(), 1);
        assert_eq!(application.basproj.com_references.len(), 1);

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn prepare_host_project_edit_plan_rejects_convention_directory_targets() {
        let temp_root = unique_temp_dir("oxvba_project_host_edit_plan_convention");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Main.bas"), "Sub Main()\nEnd Sub\n").expect("module");

        let error = prepare_host_project_edit_plan(
            &temp_root,
            &[add_project_reference_edit("../Lib/Lib.basproj")],
        )
        .expect_err("expected unsupported workspace");

        assert!(matches!(
            error,
            crate::BasProjError::HostProjectEditUnsupportedWorkspace { .. }
        ));

        let _ = fs::remove_dir_all(&temp_root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nonce}"))
    }
}
