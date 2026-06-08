//! High-level `.basproj` loading: parse XML, resolve filesystem, produce `ProjectManifest`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::BasProjError;
use crate::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectManifest, ProjectReference,
    ReferenceKind,
};
use crate::model::*;
use crate::parse::{merge_import, parse_basproj_xml};

/// Result of loading a `.basproj` file: a `ProjectManifest` for compilation plus
/// any native export descriptors declared in the project.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub manifest: ProjectManifest,
    pub native_exports: Vec<NativeExportDescriptor>,
    pub output_type: OutputType,
    pub build_target: BuildTarget,
    pub runtime_flavor: RuntimeFlavor,
    pub default_runtime_profile: Option<String>,
    pub default_policy_preset: Option<String>,
    pub default_root_object: String,
    pub entry_point: Option<String>,
    /// Per-class metadata keyed by module name (instancing, prog_id, description).
    pub class_module_metadata: BTreeMap<String, ClassModuleMetadata>,
}

const STARTUP_ENTRY_SHIM_MODULE_PREFIX: &str = "__OxVbaStartupEntryShim";
const TOP_LEVEL_MAINLINE_PROC_PREFIX: &str = "__OxVbaTopLevelMainline";

/// Load a `.basproj` file from disk, resolve module sources, and produce a
/// `LoadedProject` containing the `ProjectManifest` and export descriptors.
pub fn load_basproj(path: &Path) -> Result<LoadedProject, BasProjError> {
    let xml = std::fs::read_to_string(path).map_err(|e| BasProjError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let project_dir = crate::model::project_dir(path);
    load_basproj_from_str(&xml, &project_dir)
}

/// Load a `.basproj` from an XML string with a given project directory for
/// resolving relative paths.
pub fn load_basproj_from_str(xml: &str, project_dir: &Path) -> Result<LoadedProject, BasProjError> {
    let mut basproj = parse_basproj_xml(xml)?;

    // Process <Import> elements by re-parsing the XML to find them
    // (they were collected during parse but we need to resolve and merge them)
    process_imports(&mut basproj, xml, project_dir)?;

    // The clean path resolves the project-reference graph in `crate::closure`
    // (leaf-first, diamond-deduped) rather than here; `build_loaded_project` only
    // loads this project's own modules. `reference_projects` stays empty.
    let loaded = build_loaded_project(&basproj, project_dir)?;
    Ok(loaded)
}

/// Override the effective startup procedure for an already loaded project.
///
/// This is an execution-time override for host/CLI scenarios such as
/// `oxvba run-project --entry Module.Procedure`. It rewrites only the in-memory
/// startup shim and does not mutate project files on disk.
pub fn override_loaded_project_entry_point(
    loaded: &mut LoadedProject,
    entry_point: &str,
) -> Result<(), BasProjError> {
    parse_configured_entry_point(entry_point)?;
    strip_startup_entry_shims(&mut loaded.manifest.modules);
    inject_entry_point_startup_shim(&mut loaded.manifest.modules, entry_point)?;
    loaded.entry_point = Some(entry_point.trim().to_string());
    Ok(())
}

/// Build a `LoadedProject` from a fully-parsed (imports merged) `BasProj`.
/// Does **not** resolve project references — the caller is responsible for
/// populating `manifest.reference_projects` afterwards.
pub(crate) fn build_loaded_project(
    basproj: &BasProj,
    project_dir: &Path,
) -> Result<LoadedProject, BasProjError> {
    let props = &basproj.properties;

    // Output type is required
    let output_type = props
        .output_type
        .ok_or_else(|| BasProjError::MissingProperty("OutputType".to_string()))?;

    // Map OutputType to ProjectKind
    let project_kind = match output_type {
        OutputType::HostModule => ProjectKind::Host,
        OutputType::Library | OutputType::Addin | OutputType::ComServer | OutputType::ComExe => {
            ProjectKind::Library
        }
        OutputType::Exe => ProjectKind::Source,
    };

    // Project name: explicit or directory name
    let project_name = props
        .project_name
        .clone()
        .unwrap_or_else(|| infer_project_name_from_path(project_dir));

    // Entry point validation
    let configured_entry_point = props.entry_point.clone();

    // Conditional constants
    let conditional_constants = props
        .define_constants
        .as_deref()
        .map(parse_define_constants)
        .unwrap_or_default();

    // Load modules
    let mut modules = if basproj.modules.is_empty() {
        // Auto-discovery mode
        discover_modules(project_dir)?
    } else {
        load_explicit_modules(&basproj.modules, project_dir)?
    };
    validate_top_level_mainline_policy(output_type, &modules)?;
    let effective_entry_point = resolve_effective_entry_point(
        output_type,
        configured_entry_point.as_deref(),
        &mut modules,
    )?;
    if let Some(entry_point) = effective_entry_point.as_deref() {
        inject_entry_point_startup_shim(&mut modules, entry_point)?;
    }

    // Build references (order = precedence)
    let mut references = Vec::new();
    for pr in &basproj.project_references {
        references.push(ProjectReference {
            referenced_project_name: project_ref_name(&pr.include),
            reference_kind: match pr.kind {
                BasProjProjectReferenceKind::Project => ReferenceKind::Project,
                BasProjProjectReferenceKind::HostInjected => ReferenceKind::HostInjected,
            },
        });
    }
    for cr in &basproj.com_references {
        references.push(ProjectReference {
            referenced_project_name: cr.include.clone(),
            reference_kind: ReferenceKind::TypeLibrary,
        });
    }

    // Build native export descriptors
    let mut native_exports = Vec::new();
    let mut seen_export_names = std::collections::HashSet::new();
    for ne in &basproj.native_exports {
        if !seen_export_names.insert(ne.include.clone()) {
            return Err(BasProjError::DuplicateExportName(ne.include.clone()));
        }
        let module = ne
            .module
            .as_ref()
            .ok_or_else(|| BasProjError::MissingMetadata {
                element: "NativeExport".to_string(),
                include: ne.include.clone(),
                metadata: "Module".to_string(),
            })?;
        let procedure = ne
            .procedure
            .as_ref()
            .ok_or_else(|| BasProjError::MissingMetadata {
                element: "NativeExport".to_string(),
                include: ne.include.clone(),
                metadata: "Procedure".to_string(),
            })?;
        native_exports.push(NativeExportDescriptor {
            exported_name: ne.include.clone(),
            module_name: module.clone(),
            procedure_name: procedure.clone(),
            calling_convention: ne.calling_convention.unwrap_or(CallingConvention::Stdcall),
            ordinal: ne.ordinal,
            kind: None,
            param_types: None,
            return_type: None,
            category: ne.category.clone(),
            description: ne.description.clone(),
            argument_descriptions: ne.argument_descriptions.clone(),
        });
    }

    // Collect class module metadata (instancing, prog_id, description)
    let mut class_module_metadata = BTreeMap::new();
    for bm in &basproj.modules {
        if bm.kind == BasProjModuleKind::ClassModule
            && (bm.instancing.is_some() || bm.prog_id.is_some() || bm.description.is_some())
        {
            let module_name = Path::new(&bm.include)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
            class_module_metadata.insert(
                module_name,
                ClassModuleMetadata {
                    instancing: bm.instancing,
                    prog_id: bm.prog_id.clone(),
                    description: bm.description.clone(),
                },
            );
        }
    }

    // Note: project references are resolved by the caller (load_basproj_from_str)
    // so that resolve_project_references can call build_loaded_project without
    // triggering re-entrant resolution with fresh ancestor/seen sets.

    let manifest = ProjectManifest {
        project_name,
        project_kind,
        modules,
        references,
        reference_projects: Vec::new(),
        conditional_constants,
    };

    Ok(LoadedProject {
        manifest,
        native_exports,
        output_type,
        build_target: props.build_target.unwrap_or(BuildTarget::Bundle),
        runtime_flavor: props.runtime_flavor.unwrap_or(RuntimeFlavor::Lite),
        default_runtime_profile: props.default_runtime_profile.clone(),
        default_policy_preset: props.default_policy_preset.clone(),
        default_root_object: props
            .default_root_object
            .clone()
            .unwrap_or_else(|| "Application".to_string()),
        entry_point: effective_entry_point,
        class_module_metadata,
    })
}

fn resolve_effective_entry_point(
    output_type: OutputType,
    configured_entry_point: Option<&str>,
    modules: &mut [ModuleUnit],
) -> Result<Option<String>, BasProjError> {
    if let Some(entry_point) = configured_entry_point {
        return Ok(Some(entry_point.to_string()));
    }
    if output_type != OutputType::Exe {
        return Ok(None);
    }
    if let Some(entry_point) = prepare_unique_top_level_mainline_entry(modules)? {
        return Ok(Some(entry_point));
    }
    discover_unique_sub_main_entry_point(modules).map(Some)
}

fn validate_top_level_mainline_policy(
    output_type: OutputType,
    modules: &[ModuleUnit],
) -> Result<(), BasProjError> {
    if !matches!(
        output_type,
        OutputType::Library | OutputType::Addin | OutputType::ComServer | OutputType::ComExe
    ) {
        return Ok(());
    }

    let Some(module_name) = modules
        .iter()
        .filter(|module| module.module_kind == ModuleKind::Procedural)
        .find(|module| !extract_top_level_mainline_lines(&module.source).is_empty())
        .map(|module| module.module_name.clone())
    else {
        return Ok(());
    };

    Err(BasProjError::TopLevelMainlineUnsupported {
        output_type: output_type_name(output_type).to_string(),
        module_name,
    })
}

fn prepare_unique_top_level_mainline_entry(
    modules: &mut [ModuleUnit],
) -> Result<Option<String>, BasProjError> {
    let candidates = modules
        .iter()
        .enumerate()
        .filter(|(_, module)| module.module_kind == ModuleKind::Procedural)
        .filter(|(_, module)| !extract_top_level_mainline_lines(&module.source).is_empty())
        .map(|(idx, module)| (idx, module.module_name.clone()))
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [] => Ok(None),
        [(idx, module_name)] => {
            let (rewritten, proc_name) = rewrite_module_with_top_level_mainline(&modules[*idx])?;
            modules[*idx] = rewritten;
            Ok(Some(format!("{module_name}.{proc_name}")))
        }
        _ => Err(BasProjError::EntryPointAmbiguous(
            "unique top-level mainline fallback for OutputType=Exe".to_string(),
        )),
    }
}

fn inject_entry_point_startup_shim(
    modules: &mut Vec<ModuleUnit>,
    entry_point: &str,
) -> Result<(), BasProjError> {
    let (target_module, target_procedure) = parse_configured_entry_point(entry_point)?;
    if !modules
        .iter()
        .any(|module| module.module_name.eq_ignore_ascii_case(&target_module))
    {
        return Err(BasProjError::EntryPointInvalid(entry_point.to_string()));
    }

    let shim_module_name = next_startup_shim_module_name(modules);
    let shim_source = format!(
        "Attribute VB_Name = \"{shim_module_name}\"\n\
         Option Private Module\n\
         Public Sub Main()\n\
         Call {target_module}.{target_procedure}()\n\
         End Sub"
    );
    let shim_module =
        module_unit_from_source(&shim_module_name, ModuleKind::Procedural, shim_source)
            .map_err(|_| BasProjError::EntryPointInvalid(entry_point.to_string()))?;
    modules.insert(0, shim_module);
    Ok(())
}

fn discover_unique_sub_main_entry_point(modules: &[ModuleUnit]) -> Result<String, BasProjError> {
    let candidates = modules
        .iter()
        .filter(|module| module.module_kind == ModuleKind::Procedural)
        .filter(|module| module_has_public_parameterless_main(module))
        .map(|module| module.module_name.clone())
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [module_name] => Ok(format!("{module_name}.Main")),
        [] => Err(BasProjError::EntryPointNotFound(
            "unique Sub Main fallback for OutputType=Exe".to_string(),
        )),
        _ => Err(BasProjError::EntryPointAmbiguous(
            "unique Sub Main fallback for OutputType=Exe".to_string(),
        )),
    }
}

fn rewrite_module_with_top_level_mainline(
    module: &ModuleUnit,
) -> Result<(ModuleUnit, String), BasProjError> {
    let (retained_lines, mainline_lines) = split_top_level_mainline_lines(&module.source);
    let proc_name = next_top_level_mainline_proc_name(&module.source);
    let mut rewritten = retained_lines.join("\n");
    if !rewritten.is_empty() && !rewritten.ends_with('\n') {
        rewritten.push('\n');
    }
    if !rewritten.is_empty() {
        rewritten.push('\n');
    }
    rewritten.push_str(&format!("Public Sub {proc_name}()\n"));
    for line in mainline_lines {
        rewritten.push_str(&line);
        rewritten.push('\n');
    }
    rewritten.push_str("End Sub\n");
    let rewritten_module =
        module_unit_from_source(&module.module_name, module.module_kind, rewritten).map_err(
            |_| BasProjError::EntryPointInvalid(format!("{}.{}", module.module_name, proc_name)),
        )?;
    Ok((rewritten_module, proc_name))
}

fn parse_configured_entry_point(entry_point: &str) -> Result<(String, String), BasProjError> {
    let Some((module_name, procedure_name)) = entry_point.trim().split_once('.') else {
        return Err(BasProjError::EntryPointInvalid(entry_point.to_string()));
    };
    let module_name = module_name.trim();
    let procedure_name = procedure_name.trim();
    if !is_valid_entry_identifier(module_name) || !is_valid_entry_identifier(procedure_name) {
        return Err(BasProjError::EntryPointInvalid(entry_point.to_string()));
    }
    Ok((module_name.to_string(), procedure_name.to_string()))
}

fn is_valid_entry_identifier(identifier: &str) -> bool {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn module_has_public_parameterless_main(module: &ModuleUnit) -> bool {
    module
        .source
        .lines()
        .any(line_is_public_parameterless_main_sub_signature)
}

fn split_top_level_mainline_lines(source: &str) -> (Vec<String>, Vec<String>) {
    let lines = source
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();
    let mainline = extract_top_level_mainline_lines(source);
    if mainline.is_empty() {
        return (lines, Vec::new());
    }

    let mut retained = Vec::new();
    let mut remaining = mainline.clone();
    for line in lines {
        if let Some(pos) = remaining.iter().position(|candidate| candidate == &line) {
            remaining.remove(pos);
        } else {
            retained.push(line);
        }
    }
    (retained, mainline)
}

fn extract_top_level_mainline_lines(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut active_proc_end: Option<&'static str> = None;
    let mut active_decl_block_end: Option<&'static str> = None;

    for raw in source.lines() {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if let Some(end_term) = active_proc_end {
            if lower == end_term {
                active_proc_end = None;
            }
            continue;
        }

        if let Some(end_term) = active_decl_block_end {
            if lower == end_term {
                active_decl_block_end = None;
            }
            continue;
        }

        if trimmed.is_empty()
            || trimmed.starts_with('\'')
            || trimmed
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rem "))
        {
            continue;
        }
        if let Some(end_term) = procedure_end_term(&lower) {
            active_proc_end = Some(end_term);
            continue;
        }
        if starts_type_block(&lower) {
            active_decl_block_end = Some("end type");
            continue;
        }
        if starts_enum_block(&lower) {
            active_decl_block_end = Some("end enum");
            continue;
        }
        if is_non_mainline_top_level_directive(trimmed) {
            continue;
        }
        out.push(line.to_string());
    }

    out
}

fn next_top_level_mainline_proc_name(source: &str) -> String {
    let mut index = 0usize;
    loop {
        let candidate = if index == 0 {
            TOP_LEVEL_MAINLINE_PROC_PREFIX.to_string()
        } else {
            format!("{TOP_LEVEL_MAINLINE_PROC_PREFIX}{index}")
        };
        if !module_has_proc_named(source, &candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn module_has_proc_named(source: &str, proc_name: &str) -> bool {
    source
        .lines()
        .any(|line| line_declares_proc_name(line, proc_name))
}

fn line_declares_proc_name(line: &str, proc_name: &str) -> bool {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let prefixes = [
        "sub ",
        "public sub ",
        "private sub ",
        "friend sub ",
        "function ",
        "public function ",
        "private function ",
        "friend function ",
        "property get ",
        "public property get ",
        "private property get ",
        "friend property get ",
        "property let ",
        "public property let ",
        "private property let ",
        "friend property let ",
        "property set ",
        "public property set ",
        "private property set ",
        "friend property set ",
    ];
    for prefix in prefixes {
        if let Some(rest) = lower.strip_prefix(prefix)
            && let Some((name, _)) = split_identifier_prefix(rest.trim())
            && name.eq_ignore_ascii_case(proc_name)
        {
            return true;
        }
    }
    false
}

fn procedure_end_term(lower: &str) -> Option<&'static str> {
    if lower.starts_with("sub ")
        || lower.starts_with("public sub ")
        || lower.starts_with("private sub ")
        || lower.starts_with("friend sub ")
    {
        Some("end sub")
    } else if lower.starts_with("function ")
        || lower.starts_with("public function ")
        || lower.starts_with("private function ")
        || lower.starts_with("friend function ")
    {
        Some("end function")
    } else if lower.starts_with("property get ")
        || lower.starts_with("public property get ")
        || lower.starts_with("private property get ")
        || lower.starts_with("friend property get ")
        || lower.starts_with("property let ")
        || lower.starts_with("public property let ")
        || lower.starts_with("private property let ")
        || lower.starts_with("friend property let ")
        || lower.starts_with("property set ")
        || lower.starts_with("public property set ")
        || lower.starts_with("private property set ")
        || lower.starts_with("friend property set ")
    {
        Some("end property")
    } else {
        None
    }
}

fn is_non_mainline_top_level_directive(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    lower.starts_with("attribute ")
        || lower.starts_with("option ")
        || lower.starts_with("#const ")
        || lower.starts_with("dim ")
        || lower.starts_with("global ")
        || lower.starts_with("static ")
        || lower.starts_with("public ")
        || lower.starts_with("private ")
        || lower.starts_with("friend ")
        || lower.starts_with("implements ")
        || lower.starts_with("event ")
        || lower.starts_with("const ")
        || lower.starts_with("public const ")
        || lower.starts_with("private const ")
        || lower.starts_with("friend const ")
        || lower.starts_with("declare ")
        || lower.starts_with("public declare ")
        || lower.starts_with("private declare ")
        || is_def_type_directive(&lower)
}

fn starts_type_block(lower: &str) -> bool {
    lower.starts_with("type ")
        || lower.starts_with("private type ")
        || lower.starts_with("public type ")
}

fn starts_enum_block(lower: &str) -> bool {
    lower.starts_with("enum ")
        || lower.starts_with("private enum ")
        || lower.starts_with("public enum ")
}

fn is_def_type_directive(lower: &str) -> bool {
    [
        "defbool ",
        "defbyte ",
        "defint ",
        "deflng ",
        "deflnglng ",
        "deflngptr ",
        "defsng ",
        "defdbl ",
        "defdec ",
        "defcur ",
        "defdate ",
        "defstr ",
        "defobj ",
        "defvar ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn line_is_public_parameterless_main_sub_signature(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    let lower = if let Some(rest) = lower.strip_prefix("public ") {
        rest
    } else if lower.strip_prefix("private ").is_some() || lower.strip_prefix("friend ").is_some() {
        return false;
    } else {
        lower.as_str()
    };
    let Some(rest) = lower.strip_prefix("sub ") else {
        return false;
    };
    let Some((name, remainder)) = split_identifier_prefix(rest) else {
        return false;
    };
    if name != "main" {
        return false;
    }
    let remainder = remainder.trim();
    if remainder.is_empty() {
        return true;
    }
    if let Some(args) = remainder.strip_prefix('(') {
        return args.trim_end_matches(')').trim().is_empty() && remainder.trim_end().ends_with(')');
    }
    false
}

fn split_identifier_prefix(value: &str) -> Option<(&str, &str)> {
    let end = value
        .char_indices()
        .find(|(_, ch)| !(*ch == '_' || ch.is_ascii_alphanumeric()))
        .map(|(idx, _)| idx)
        .unwrap_or(value.len());
    if end == 0 {
        return None;
    }
    Some(value.split_at(end))
}

fn next_startup_shim_module_name(modules: &[ModuleUnit]) -> String {
    let mut index = 0usize;
    loop {
        let candidate = if index == 0 {
            STARTUP_ENTRY_SHIM_MODULE_PREFIX.to_string()
        } else {
            format!("{STARTUP_ENTRY_SHIM_MODULE_PREFIX}{index}")
        };
        if !modules
            .iter()
            .any(|module| module.module_name.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
        index += 1;
    }
}

fn strip_startup_entry_shims(modules: &mut Vec<ModuleUnit>) {
    modules.retain(|module| !is_startup_shim_module_name(&module.module_name));
}

fn is_startup_shim_module_name(module_name: &str) -> bool {
    module_name
        .to_ascii_lowercase()
        .starts_with(&STARTUP_ENTRY_SHIM_MODULE_PREFIX.to_ascii_lowercase())
}

fn output_type_name(output_type: OutputType) -> &'static str {
    match output_type {
        OutputType::HostModule => "HostModule",
        OutputType::Library => "Library",
        OutputType::Exe => "Exe",
        OutputType::Addin => "Addin",
        OutputType::ComServer => "ComServer",
        OutputType::ComExe => "ComExe",
    }
}

/// Build a [`ModuleUnit`] from raw source, parsing its `Attribute VB_*` header
/// lines into [`ModuleAttributes`]. The module's semantic identity is its
/// `Attribute VB_Name` (falling back to the supplied `module_name`), so a file
/// stem may differ from the VBA module name. Relocated from the deleted
/// `oxvba-compiler` — VBA module-header parsing is a source-shape concern owned
/// by the project loader.
fn module_unit_from_source(
    module_name: impl Into<String>,
    module_kind: ModuleKind,
    source: impl Into<String>,
) -> Result<ModuleUnit, BasProjError> {
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
    let semantic_module_name = attrs.vb_name.clone();
    Ok(ModuleUnit {
        module_name: semantic_module_name,
        module_kind,
        attributes: attrs,
        source,
    })
}

fn parse_attribute_line(
    line: &str,
    attrs: &mut ModuleAttributes,
    module_name: &str,
) -> Result<(), BasProjError> {
    let Some(rest) = line
        .strip_prefix("Attribute ")
        .or_else(|| line.strip_prefix("attribute "))
    else {
        return Ok(());
    };
    let header_invalid = || BasProjError::ModuleSourceInvalid {
        include: module_name.to_string(),
        message: format!("invalid module header line: {line}"),
    };
    let Some((lhs, rhs)) = rest.split_once('=') else {
        return Err(header_invalid());
    };
    let key = lhs.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err(header_invalid());
    }
    let value_text = rhs.trim().trim_matches('"');
    match key.as_str() {
        "vb_name" => attrs.vb_name = value_text.to_string(),
        "vb_globalnamespace" => {
            attrs.vb_global_namespace =
                parse_bool_attribute(value_text).ok_or_else(header_invalid)?
        }
        "vb_creatable" => {
            attrs.vb_creatable = parse_bool_attribute(value_text).ok_or_else(header_invalid)?
        }
        "vb_predeclaredid" => {
            attrs.vb_predeclared_id = parse_bool_attribute(value_text).ok_or_else(header_invalid)?
        }
        "vb_exposed" => {
            attrs.vb_exposed = parse_bool_attribute(value_text).ok_or_else(header_invalid)?
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

/// Process `<Import>` elements by re-scanning the XML for them, loading the
/// imported files, and merging their items into the parent `BasProj`.
pub(crate) fn process_imports(
    basproj: &mut BasProj,
    xml: &str,
    project_dir: &Path,
) -> Result<(), BasProjError> {
    let import_paths = collect_import_paths(xml)?;
    for rel_path in import_paths {
        let abs_path = project_dir.join(&rel_path);
        if !abs_path.exists() {
            return Err(BasProjError::ImportFileNotFound(rel_path));
        }
        let import_xml = std::fs::read_to_string(&abs_path).map_err(|e| BasProjError::Io {
            path: abs_path.display().to_string(),
            source: e,
        })?;
        let imported = parse_basproj_xml(&import_xml)?;
        merge_import(basproj, imported);
    }
    Ok(())
}

/// Scan the XML string for `<Import Project="...">` elements and return their paths.
fn collect_import_paths(xml: &str) -> Result<Vec<String>, BasProjError> {
    let mut reader = quick_xml::reader::Reader::from_str(xml);
    let mut paths = Vec::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Empty(ref e))
            | Ok(quick_xml::events::Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "Import" {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        if key == "Project" {
                            paths.push(String::from_utf8_lossy(&attr.value).to_string());
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                return Err(BasProjError::XmlParse(format!(
                    "XML parse error scanning imports: {e}"
                )));
            }
            _ => {}
        }
    }
    Ok(paths)
}

fn normalize_loaded_module_source(module_kind: ModuleKind, source: String) -> String {
    if !matches!(module_kind, ModuleKind::Class | ModuleKind::Document) {
        return source;
    }

    let lines = source.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return source;
    }

    let first = lines[0].trim().to_ascii_lowercase();
    if !(first.starts_with("version ") && first.ends_with(" class")) {
        return source;
    }

    let mut idx = 1;
    if idx < lines.len() && lines[idx].trim().eq_ignore_ascii_case("BEGIN") {
        idx += 1;
        while idx < lines.len() && !lines[idx].trim().eq_ignore_ascii_case("END") {
            idx += 1;
        }
        if idx < lines.len() {
            idx += 1;
        }
    }

    lines[idx..].join("\n")
}

/// Auto-discover `.bas` and `.cls` files in a project directory.
fn discover_modules(project_dir: &Path) -> Result<Vec<ModuleUnit>, BasProjError> {
    let mut modules = Vec::new();
    discover_modules_recursive(project_dir, project_dir, &mut modules)?;
    // Sort for deterministic ordering
    modules.sort_by(|a, b| a.module_name.cmp(&b.module_name));
    Ok(modules)
}

fn discover_modules_recursive(
    _base_dir: &Path,
    dir: &Path,
    modules: &mut Vec<ModuleUnit>,
) -> Result<(), BasProjError> {
    let entries = std::fs::read_dir(dir).map_err(|e| BasProjError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| BasProjError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            discover_modules_recursive(_base_dir, &path, modules)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let module_kind = match ext {
                "bas" => Some(ModuleKind::Procedural),
                "cls" => Some(ModuleKind::Class),
                _ => None,
            };
            if let Some(kind) = module_kind {
                let module_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string();
                let source = std::fs::read_to_string(&path).map_err(|e| BasProjError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?;
                let source = normalize_loaded_module_source(kind, source);
                let unit = module_unit_from_source(&module_name, kind, source).map_err(|err| {
                    BasProjError::ModuleSourceInvalid {
                        include: path.display().to_string(),
                        message: err.to_string(),
                    }
                })?;
                modules.push(unit);
            }
        }
    }
    Ok(())
}

/// Load explicitly declared modules from the project file.
fn load_explicit_modules(
    basproj_modules: &[BasProjModule],
    project_dir: &Path,
) -> Result<Vec<ModuleUnit>, BasProjError> {
    let mut modules = Vec::new();
    for bm in basproj_modules {
        let source_path = project_dir.join(&bm.include);
        let source = std::fs::read_to_string(&source_path).map_err(|e| BasProjError::Io {
            path: source_path.display().to_string(),
            source: e,
        })?;
        let module_name = Path::new(&bm.include)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
        let module_kind = match bm.kind {
            BasProjModuleKind::Module => ModuleKind::Procedural,
            BasProjModuleKind::ClassModule => ModuleKind::Class,
            BasProjModuleKind::DocumentModule => ModuleKind::Document,
        };
        let source = normalize_loaded_module_source(module_kind, source);
        let mut unit =
            module_unit_from_source(&module_name, module_kind, source).map_err(|err| {
                BasProjError::ModuleSourceInvalid {
                    include: bm.include.clone(),
                    message: err.to_string(),
                }
            })?;
        unit.attributes.vb_global_namespace = bm.vb_global_namespace;
        unit.attributes.vb_creatable = bm.vb_creatable;
        unit.attributes.vb_predeclared_id = bm.vb_predeclared_id;
        unit.attributes.vb_exposed = bm.vb_exposed;
        modules.push(unit);
    }
    Ok(modules)
}

/// Extract a project name from a `ProjectReference` include path.
fn project_ref_name(include: &str) -> String {
    Path::new(include)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(include)
        .to_string()
}

/// Get directory name as string, or "Project" as fallback.
pub fn infer_project_name_from_path(dir: &Path) -> String {
    let raw = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");
    sanitize_inferred_project_name(raw)
}

fn sanitize_inferred_project_name(raw: &str) -> String {
    let mut name = String::new();
    let mut previous_was_underscore = false;
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' {
            previous_was_underscore = false;
            ch
        } else {
            if previous_was_underscore {
                continue;
            }
            previous_was_underscore = true;
            '_'
        };
        name.push(mapped);
    }
    let trimmed = name.trim_matches('_');
    let mut candidate = if trimmed.is_empty() {
        "Project".to_string()
    } else {
        trimmed.to_string()
    };
    if candidate
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        candidate.insert_str(0, "Project_");
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inferred_project_name_sanitizes_common_directory_names() {
        assert_eq!(
            infer_project_name_from_path(Path::new("math-tool")),
            "math_tool"
        );
        assert_eq!(
            infer_project_name_from_path(Path::new("123-demo")),
            "Project_123_demo"
        );
        assert_eq!(infer_project_name_from_path(Path::new("___")), "Project");
    }

    #[test]
    fn configured_entry_point_injects_startup_shim_ahead_of_loaded_modules() {
        let unique = format!(
            "oxvba_project_load_entrypoint_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        let main_path = temp_root.join("MainModule.bas");
        let startup_path = temp_root.join("StartupModule.bas");
        std::fs::write(&main_path, "Public Sub Main()\nEnd Sub\n").expect("write main module");
        std::fs::write(&startup_path, "Public Sub Run()\nEnd Sub\n").expect("write startup module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
    <EntryPoint>StartupModule.Run</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"MainModule.bas\" />
    <Module Include=\"StartupModule.bas\" />
  </ItemGroup>
</Project>
";

        let loaded = load_basproj_from_str(xml, &temp_root).expect("basproj should load");
        assert!(
            loaded.manifest.modules[0]
                .module_name
                .starts_with(STARTUP_ENTRY_SHIM_MODULE_PREFIX),
            "expected startup shim first, got {:?}",
            loaded
                .manifest
                .modules
                .iter()
                .map(|module| module.module_name.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            loaded.manifest.modules[0]
                .source
                .contains("Call StartupModule.Run()"),
            "expected startup shim to target configured entry point, got: {}",
            loaded.manifest.modules[0].source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn exe_without_configured_entry_point_discovers_unique_sub_main() {
        let unique = format!(
            "oxvba_project_load_unique_main_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        let helper_path = temp_root.join("HelperModule.bas");
        let main_path = temp_root.join("MainModule.bas");
        std::fs::write(&helper_path, "Public Sub Warmup()\nEnd Sub\n")
            .expect("write helper module");
        std::fs::write(&main_path, "Public Sub Main()\nEnd Sub\n").expect("write main module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"HelperModule.bas\" />
    <Module Include=\"MainModule.bas\" />
  </ItemGroup>
</Project>
";

        let loaded = load_basproj_from_str(xml, &temp_root).expect("basproj should load");
        assert_eq!(loaded.entry_point.as_deref(), Some("MainModule.Main"));
        assert!(
            loaded.manifest.modules[0]
                .source
                .contains("Call MainModule.Main()"),
            "expected auto-discovered startup shim to target unique Main, got: {}",
            loaded.manifest.modules[0].source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn exe_without_configured_entry_point_discovers_unique_top_level_mainline() {
        let unique = format!(
            "oxvba_project_load_unique_mainline_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        let helper_path = temp_root.join("HelperModule.bas");
        let script_path = temp_root.join("ScriptModule.bas");
        std::fs::write(&helper_path, "Public Sub Warmup()\nEnd Sub\n")
            .expect("write helper module");
        std::fs::write(&script_path, "valueOut = 41\nSub Warmup()\nEnd Sub\n")
            .expect("write script module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"HelperModule.bas\" />
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
";

        let loaded = load_basproj_from_str(xml, &temp_root).expect("basproj should load");
        assert_eq!(
            loaded.entry_point.as_deref(),
            Some("ScriptModule.__OxVbaTopLevelMainline")
        );
        assert!(
            loaded.manifest.modules[0]
                .source
                .contains("Call ScriptModule.__OxVbaTopLevelMainline()"),
            "expected startup shim to target rewritten top-level mainline, got: {}",
            loaded.manifest.modules[0].source
        );
        let script_module = loaded
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "ScriptModule")
            .expect("rewritten script module present");
        assert!(
            script_module
                .source
                .contains("Public Sub __OxVbaTopLevelMainline()"),
            "expected top-level rewrite, got: {}",
            script_module.source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn exe_without_configured_entry_point_rejects_ambiguous_top_level_mainlines() {
        let unique = format!(
            "oxvba_project_load_ambiguous_mainline_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(temp_root.join("First.bas"), "valueOut = 1\n").expect("write first module");
        std::fs::write(temp_root.join("Second.bas"), "valueOut = 2\n")
            .expect("write second module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"First.bas\" />
    <Module Include=\"Second.bas\" />
  </ItemGroup>
</Project>
";

        let err = load_basproj_from_str(xml, &temp_root)
            .expect_err("ambiguous top-level mainlines should fail deterministically");
        assert!(
            matches!(err, BasProjError::EntryPointAmbiguous(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn exe_top_level_mainline_rewrite_preserves_option_private_module_and_module_declarations() {
        let unique = format!(
            "oxvba_project_load_mainline_option_private_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        let script_path = temp_root.join("ScriptModule.bas");
        std::fs::write(
            &script_path,
            "Option Private Module\nPrivate counter As Long\ncounter = 41\nCall Bump\nPublic Sub Bump()\ncounter = counter + 1\nvalueOut = counter\nEnd Sub\n",
        )
        .expect("write script module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
";

        let loaded = load_basproj_from_str(xml, &temp_root)
            .expect("project with Option Private Module should load");
        let script_module = loaded
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "ScriptModule")
            .expect("rewritten script module should be present");
        assert!(
            script_module.source.contains("Option Private Module"),
            "expected Option Private Module to remain at module scope: {}",
            script_module.source
        );
        assert!(
            script_module.source.contains("Private counter As Long"),
            "expected module declaration to remain at module scope: {}",
            script_module.source
        );
        assert!(
            script_module
                .source
                .contains("Public Sub __OxVbaTopLevelMainline"),
            "expected synthetic startup proc, got: {}",
            script_module.source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn exe_top_level_mainline_rewrite_preserves_def_type_and_module_const_directives() {
        let unique = format!(
            "oxvba_project_load_mainline_defconst_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        let script_path = temp_root.join("ScriptModule.bas");
        std::fs::write(
            &script_path,
            "DefObj A-Z\n#Const ENABLE = True\n#If ENABLE Then\nvalueOut = 41\n#Else\nvalueOut = 0\n#End If\n",
        )
        .expect("write script module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
";

        let loaded = load_basproj_from_str(xml, &temp_root)
            .expect("project with DefObj and #Const should load");
        let script_module = loaded
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "ScriptModule")
            .expect("rewritten script module should be present");
        assert!(
            script_module.source.contains("DefObj A-Z"),
            "expected DefObj directive to remain at module scope: {}",
            script_module.source
        );
        assert!(
            script_module.source.contains("#Const ENABLE = True"),
            "expected module constant directive to remain at module scope: {}",
            script_module.source
        );
        assert!(
            script_module
                .source
                .contains("Public Sub __OxVbaTopLevelMainline"),
            "expected synthetic startup proc, got: {}",
            script_module.source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn exe_top_level_mainline_rewrite_preserves_mixed_module_scope_declarations() {
        let unique = format!(
            "oxvba_project_load_mainline_mixed_scope_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(
            temp_root.join("ScriptModule.bas"),
            concat!(
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
            ),
        )
        .expect("write script module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
";

        let loaded =
            load_basproj_from_str(xml, &temp_root).expect("project should load with rewrite");
        assert_eq!(
            loaded.entry_point.as_deref(),
            Some("ScriptModule.__OxVbaTopLevelMainline")
        );
        let script_module = loaded
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "ScriptModule")
            .expect("rewritten script module should remain present");
        assert!(script_module.source.contains("Rem module comment"));
        assert!(script_module.source.contains("Public sharedCount As Long"));
        assert!(script_module.source.contains("Private counter As Long"));
        assert!(script_module.source.contains("Global totalCount As Long"));
        assert!(script_module.source.contains("Static stickyCount As Long"));
        assert!(script_module.source.contains("Private Type CounterState"));
        assert!(script_module.source.contains("Public Enum CounterMode"));
        assert!(
            script_module
                .source
                .contains("Public Sub __OxVbaTopLevelMainline")
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn non_exe_output_types_reject_top_level_mainline_modules() {
        for (output_type, project_name) in [
            ("Library", "ProjectLibrary"),
            ("Addin", "ProjectAddin"),
            ("ComServer", "ProjectComServer"),
            ("ComExe", "ProjectComExe"),
        ] {
            let unique = format!(
                "oxvba_project_load_non_exe_mainline_{}_{}_{}",
                output_type,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("unix epoch")
                    .as_nanos()
            );
            let temp_root = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&temp_root).expect("create temp project root");
            std::fs::write(temp_root.join("ScriptModule.bas"), "valueOut = 41\n")
                .expect("write script module");
            let xml = format!(
                "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>{output_type}</OutputType>\n    <ProjectName>{project_name}</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"ScriptModule.bas\" />\n  </ItemGroup>\n</Project>\n"
            );

            let err = load_basproj_from_str(&xml, &temp_root)
                .expect_err("non-exe output types should reject top-level mainline");
            match err {
                BasProjError::TopLevelMainlineUnsupported {
                    output_type: actual_output_type,
                    module_name,
                } => {
                    assert_eq!(actual_output_type, output_type);
                    assert_eq!(module_name, "ScriptModule");
                }
                other => panic!("unexpected error for {output_type}: {other:?}"),
            }

            std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
        }
    }

    #[test]
    fn addin_without_entry_point_loads_when_no_top_level_mainline_exists() {
        let unique = format!(
            "oxvba_project_load_addin_without_entrypoint_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(
            temp_root.join("Functions.bas"),
            "Public Sub Warmup()\nEnd Sub\n",
        )
        .expect("write addin module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Addin</OutputType>
    <ProjectName>ProjectAddin</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Functions.bas\" />
  </ItemGroup>
</Project>
";

        let loaded = load_basproj_from_str(xml, &temp_root)
            .expect("addin without entry point should load when no top-level mainline exists");
        assert_eq!(loaded.output_type, OutputType::Addin);
        assert!(loaded.entry_point.is_none());

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn override_loaded_entry_point_replaces_existing_startup_shim() {
        let unique = format!(
            "oxvba_project_override_entry_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
            .expect("write main module");
        std::fs::write(
            temp_root.join("Startup.bas"),
            "Public Sub Boot()\nEnd Sub\n",
        )
        .expect("write startup module");
        let xml = "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Main.bas\" />
    <Module Include=\"Startup.bas\" />
  </ItemGroup>
</Project>
";

        let mut loaded = load_basproj_from_str(xml, &temp_root).expect("project should load");
        assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));
        assert_eq!(
            loaded
                .manifest
                .modules
                .iter()
                .filter(|module| is_startup_shim_module_name(&module.module_name))
                .count(),
            1
        );

        override_loaded_project_entry_point(&mut loaded, "Startup.Boot")
            .expect("entry override should inject replacement shim");

        assert_eq!(loaded.entry_point.as_deref(), Some("Startup.Boot"));
        let shim_modules = loaded
            .manifest
            .modules
            .iter()
            .filter(|module| is_startup_shim_module_name(&module.module_name))
            .collect::<Vec<_>>();
        assert_eq!(shim_modules.len(), 1);
        assert!(
            shim_modules[0].source.contains("Call Startup.Boot"),
            "expected startup shim to target override entrypoint, got: {}",
            shim_modules[0].source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }

    #[test]
    fn load_basproj_uses_vb_name_as_semantic_identity_while_preserving_include_path() {
        let unique = format!(
            "oxvba-vb-name-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(
            temp_root.join("Sqlite3_64.bas"),
            "Attribute VB_Name = \"Sqlite3\"\nPublic Sub Main()\nEnd Sub\n",
        )
        .expect("write module");

        let loaded = load_basproj_from_str(
            "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>SqliteFixture</ProjectName>
    <EntryPoint>Sqlite3.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Sqlite3_64.bas\" />
  </ItemGroup>
</Project>
",
            &temp_root,
        )
        .expect("load project");

        let sqlite_module = loaded
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "Sqlite3")
            .expect("loaded manifest should contain semantic Sqlite3 module");
        assert_eq!(sqlite_module.attributes.vb_name, "Sqlite3");

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }
}
