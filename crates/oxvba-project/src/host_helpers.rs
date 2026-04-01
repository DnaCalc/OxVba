use std::path::Path;

use crate::{
    BasProjComReference, BasProjError, BasProjModule, BasProjModuleKind, BasProjProjectReference,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProjectEdit {
    AddModule(BasProjModule),
    RemoveModule { include: String },
    AddProjectReference(BasProjProjectReference),
    RemoveProjectReference { include: String },
    AddComReference(BasProjComReference),
    RemoveComReference { include: String },
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
        VbNameAttributeAction, add_com_reference_edit, add_project_reference_edit,
        inspect_module_identity, plan_new_module, reconcile_module_identity,
    };
    use crate::BasProjModuleKind;
    use std::path::Path;

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
}
