use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    BasProjComReference, BasProjError, BasProjModule, BasProjModuleKind,
    BasProjProjectReference, OutputType, discover_project_file_in_dir, infer_project_name_from_path,
    parse_basproj_xml, parse_vbp,
};
use crate::vbp::VbpReference;

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
pub enum HostProjectEdit {
    AddModule(BasProjModule),
    RemoveModule { include: String },
    AddProjectReference(BasProjProjectReference),
    RemoveProjectReference { include: String },
    AddComReference(BasProjComReference),
    RemoveComReference { include: String },
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
    references.extend(
        basproj
            .project_references
            .iter()
            .map(|reference| HostProjectReferenceInfo {
                kind: HostProjectReferenceKind::Project,
                include: reference.include.clone(),
                guid: None,
                version_major: None,
                version_minor: None,
                lcid: None,
                import_lib: None,
                path: None,
                referenced_project_name: None,
            }),
    );
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
        left.include
            .cmp(&right.include)
            .then_with(|| {
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
        left.include
            .cmp(&right.include)
            .then_with(|| {
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
        HostProjectReferenceKind::Com => 1,
        HostProjectReferenceKind::Native => 2,
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
        HostProjectReferenceKind, HostWorkspaceTargetKind, VbNameAttributeAction,
        add_com_reference_edit, add_project_reference_edit, inspect_module_identity,
        inspect_workspace_target, plan_new_module, reconcile_module_identity,
    };
    use crate::BasProjModuleKind;
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
        assert!(vbp_surface.modules.iter().any(|module| module.include == "Main.bas"));
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

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nonce}"))
    }
}
