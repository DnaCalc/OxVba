//! High-level `.basproj` loading: parse XML, resolve filesystem, produce `ProjectManifest`.

use std::collections::BTreeMap;
use std::path::Path;

use oxvba_com::{TypeLibResolveRequest, build_typelib_metadata, resolve_known_typelib_identity};
use oxvba_compiler::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectManifest, ProjectReference,
    ReferenceKind, ReferencedProjectManifest, module_unit_from_source,
    project::project_typelib_as_manifest,
};
use oxvba_host::TypeLibraryCatalogEntry;

use crate::error::BasProjError;
use crate::model::*;
use crate::parse::{merge_import, parse_basproj_xml};
use crate::resolve;

/// Result of loading a `.basproj` file: a `ProjectManifest` for compilation plus
/// any native export descriptors declared in the project.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub manifest: ProjectManifest,
    pub native_exports: Vec<NativeExportDescriptor>,
    pub output_type: OutputType,
    pub runtime_flavor: RuntimeFlavor,
    pub default_runtime_profile: String,
    pub default_policy_preset: String,
    pub default_root_object: String,
    pub entry_point: Option<String>,
    pub type_library_catalog: Vec<TypeLibraryCatalogEntry>,
    /// Per-class metadata keyed by module name (instancing, prog_id, description).
    pub class_module_metadata: BTreeMap<String, ClassModuleMetadata>,
}

const TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME: &str = "__OxVbaTypeLibBindingDiagnostic";
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

    let mut loaded = build_loaded_project(&basproj, project_dir)?;

    // Resolve project references (recursive with cycle detection)
    if !basproj.project_references.is_empty() {
        let mut ancestors = std::collections::HashSet::new();
        let mut seen = std::collections::HashSet::new();
        loaded.manifest.reference_projects =
            resolve::resolve_project_references(&basproj, project_dir, &mut ancestors, &mut seen)
                .unwrap_or_default();
    }
    inject_type_library_reference_projects(&mut loaded);

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
        .unwrap_or_else(|| dir_name_or_default(project_dir));

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
            reference_kind: ReferenceKind::Project,
        });
    }
    for cr in &basproj.com_references {
        references.push(ProjectReference {
            referenced_project_name: cr.include.clone(),
            reference_kind: ReferenceKind::TypeLibrary,
        });
    }

    // Build type library catalog entries
    let type_library_catalog: Vec<TypeLibraryCatalogEntry> = basproj
        .com_references
        .iter()
        .map(|cr| TypeLibraryCatalogEntry {
            library_name: cr.include.clone(),
            importlib: cr.import_lib.clone().unwrap_or_default(),
            libid: cr.guid.clone(),
            major_version: cr.version_major.unwrap_or(0),
            minor_version: cr.version_minor.unwrap_or(0),
            lcid: cr.lcid,
        })
        .collect();

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
        runtime_flavor: props.runtime_flavor.unwrap_or(RuntimeFlavor::Lite),
        default_runtime_profile: props
            .default_runtime_profile
            .clone()
            .unwrap_or_else(|| "windows-headless".to_string()),
        default_policy_preset: props
            .default_policy_preset
            .clone()
            .unwrap_or_else(|| "deterministic-runtime".to_string()),
        default_root_object: props
            .default_root_object
            .clone()
            .unwrap_or_else(|| "Application".to_string()),
        entry_point: effective_entry_point,
        type_library_catalog,
        class_module_metadata,
    })
}

fn resolve_effective_entry_point(
    output_type: OutputType,
    configured_entry_point: Option<&str>,
    modules: &mut Vec<ModuleUnit>,
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
    modules: &mut Vec<ModuleUnit>,
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
    } else if lower.strip_prefix("private ").is_some() {
        return false;
    } else if lower.strip_prefix("friend ").is_some() {
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

fn inject_type_library_reference_projects(loaded: &mut LoadedProject) {
    for reference in &loaded.manifest.references {
        if reference.reference_kind != ReferenceKind::TypeLibrary {
            continue;
        }

        let Some(catalog_entry) = loaded.type_library_catalog.iter().find(|entry| {
            entry
                .library_name
                .eq_ignore_ascii_case(&reference.referenced_project_name)
        }) else {
            continue;
        };

        let request = TypeLibResolveRequest {
            reference_name: reference.referenced_project_name.clone(),
            importlib_hint: non_empty_trimmed(&catalog_entry.importlib),
            libid_hint: catalog_entry.libid.clone(),
            major_version_hint: Some(catalog_entry.major_version),
            minor_version_hint: Some(catalog_entry.minor_version),
            lcid_hint: catalog_entry.lcid,
        };
        let importlib_missing_on_filesystem = request
            .importlib_hint
            .as_deref()
            .is_some_and(is_missing_filesystem_importlib_hint);
        let identity = resolve_known_typelib_identity(&request);
        if importlib_missing_on_filesystem {
            let Some(identity) = identity else {
                let diagnostic =
                    build_typelib_binding_diagnostic_project_for_missing_importlib(&request);
                if loaded.manifest.reference_projects.iter().any(|project| {
                    project
                        .project_name
                        .eq_ignore_ascii_case(&diagnostic.project_name)
                }) {
                    continue;
                }
                loaded.manifest.reference_projects.push(diagnostic);
                continue;
            };
            let mut synthetic = project_typelib_as_manifest(&build_typelib_metadata(&identity));
            append_typelib_binding_diagnostic_module(
                &mut synthetic,
                build_typelib_binding_diagnostic_module_for_missing_importlib(&request),
            );
            if loaded.manifest.reference_projects.iter().any(|project| {
                project
                    .project_name
                    .eq_ignore_ascii_case(&synthetic.project_name)
            }) {
                continue;
            }
            loaded.manifest.reference_projects.push(synthetic);
            continue;
        }
        let Some(identity) = identity else {
            let diagnostic = build_typelib_binding_diagnostic_project(&request);
            if loaded.manifest.reference_projects.iter().any(|project| {
                project
                    .project_name
                    .eq_ignore_ascii_case(&diagnostic.project_name)
            }) {
                continue;
            }
            loaded.manifest.reference_projects.push(diagnostic);
            continue;
        };

        let synthetic = project_typelib_as_manifest(&build_typelib_metadata(&identity));
        if loaded.manifest.reference_projects.iter().any(|project| {
            project
                .project_name
                .eq_ignore_ascii_case(&synthetic.project_name)
        }) {
            continue;
        }
        loaded.manifest.reference_projects.push(synthetic);
    }
}

fn build_typelib_binding_diagnostic_project(
    request: &TypeLibResolveRequest,
) -> ReferencedProjectManifest {
    let (code, message) = match (
        request.libid_hint.as_deref(),
        request.importlib_hint.as_deref(),
    ) {
        (Some(libid), _) => (
            "PMR-E-TYPELIB-LIBID-UNRESOLVED",
            format!(
                "type-library reference `{}` with LIBID `{}` could not be resolved",
                request.reference_name, libid
            ),
        ),
        (None, Some(importlib)) => (
            "PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED",
            format!(
                "type-library reference `{}` with importlib `{}` could not be resolved",
                request.reference_name, importlib
            ),
        ),
        (None, None) => (
            "PMR-E-TYPELIB-IMPORTLIB-MISSING",
            format!(
                "type-library reference `{}` is missing an importlib hint",
                request.reference_name
            ),
        ),
    };
    ReferencedProjectManifest {
        project_name: request.reference_name.clone(),
        modules: vec![ModuleUnit {
            module_name: TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME.to_string(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes {
                vb_name: TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME.to_string(),
                ..ModuleAttributes::default()
            },
            source: format!(
                "Attribute VB_Name = \"{TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME}\"\ncode={code}\nmessage={message}\n"
            ),
        }],
    }
}

fn build_typelib_binding_diagnostic_project_for_missing_importlib(
    request: &TypeLibResolveRequest,
) -> ReferencedProjectManifest {
    ReferencedProjectManifest {
        project_name: request.reference_name.clone(),
        modules: vec![build_typelib_binding_diagnostic_module_for_missing_importlib(request)],
    }
}

fn build_typelib_binding_diagnostic_module_for_missing_importlib(
    request: &TypeLibResolveRequest,
) -> ModuleUnit {
    let importlib = request.importlib_hint.as_deref().unwrap_or_default();
    ModuleUnit {
        module_name: TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME.to_string(),
        module_kind: ModuleKind::Procedural,
        attributes: ModuleAttributes {
            vb_name: TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME.to_string(),
            ..ModuleAttributes::default()
        },
        source: format!(
            "Attribute VB_Name = \"{TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME}\"\ncode=PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED\nmessage=type-library reference `{}` with importlib `{}` could not be resolved\n",
            request.reference_name, importlib
        ),
    }
}

fn append_typelib_binding_diagnostic_module(
    project: &mut ReferencedProjectManifest,
    diagnostic: ModuleUnit,
) {
    if project
        .modules
        .iter()
        .any(|module| module.module_name == TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME)
    {
        return;
    }
    project.modules.push(diagnostic);
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_missing_filesystem_importlib_hint(importlib_hint: &str) -> bool {
    let trimmed = importlib_hint.trim();
    if trimmed.is_empty() {
        return false;
    }
    let path = Path::new(trimmed);
    let looks_like_filesystem_path =
        path.is_absolute() || trimmed.contains('\\') || trimmed.contains('/');
    looks_like_filesystem_path && !path.exists()
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
        let mut unit = module_unit_from_source(&module_name, module_kind, source).map_err(|err| {
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
fn dir_name_or_default(dir: &Path) -> String {
    dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_libid_with_missing_filesystem_importlib_stays_diagnostic() {
        let unique = format!(
            "oxvba_project_load_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        let main_path = temp_root.join("Main.bas");
        std::fs::write(&main_path, "Public Sub Main()\nEnd Sub\n").expect("write main module");
        let missing_importlib = temp_root.join("missing").join("OxVba.TestEventServer.tlb");
        let xml = format!(
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n\
  <PropertyGroup>\n\
    <OutputType>Exe</OutputType>\n\
    <ProjectName>ProjectA</ProjectName>\n\
    <EntryPoint>Main.Main</EntryPoint>\n\
  </PropertyGroup>\n\
  <ItemGroup>\n\
    <Module Include=\"Main.bas\" />\n\
  </ItemGroup>\n\
  <ItemGroup>\n\
    <COMReference Include=\"OxVbaMissingBase\">\n\
      <Guid>{{E2A30001-0001-0001-0001-000000000001}}</Guid>\n\
      <VersionMajor>1</VersionMajor>\n\
      <VersionMinor>0</VersionMinor>\n\
      <Lcid>0</Lcid>\n\
      <ImportLib>{}</ImportLib>\n\
    </COMReference>\n\
  </ItemGroup>\n\
</Project>\n",
            missing_importlib.display()
        );

        let loaded = load_basproj_from_str(&xml, &temp_root).expect("basproj should load");
        let diagnostic = loaded
            .manifest
            .reference_projects
            .iter()
            .find(|project| {
                project.modules.iter().any(|module| {
                    module.module_name == TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME
                        && module
                            .source
                            .contains("code=PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED")
                })
            })
            .expect("expected synthetic diagnostic project for broken importlib");
        let diagnostic_module = diagnostic
            .modules
            .iter()
            .find(|module| module.module_name == TYPELIB_BINDING_DIAGNOSTIC_MODULE_NAME)
            .expect("diagnostic module present");
        assert!(
            diagnostic_module
                .source
                .contains("code=PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
            "expected unresolved importlib code, got: {}",
            diagnostic_module.source
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
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
    fn loaded_project_preserves_default_member_lowering_for_class_property_get() {
        let unique = format!(
            "oxvba_project_load_default_member_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        );
        let temp_root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(
            temp_root.join("Main.bas"),
            "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub\n",
        )
        .expect("write main module");
        std::fs::write(
            temp_root.join("Widget.cls"),
            concat!(
                "Attribute VB_Name = \"Widget\"\n",
                "Option Explicit\n",
                "Private stored As Long\n",
                "Public Sub Class_Initialize()\n",
                "stored = 41\n",
                "End Sub\n",
                "Public Property Get Value() As Long\n",
                "Value = stored + 1\n",
                "End Property\n",
                "Attribute Value.VB_UserMemId = 0\n"
            ),
        )
        .expect("write widget module");

        let main_source = "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub\n";
        let widget_source = concat!(
            "Attribute VB_Name = \"Widget\"\n",
            "Option Explicit\n",
            "Private stored As Long\n",
            "Public Sub Class_Initialize()\n",
            "stored = 41\n",
            "End Sub\n",
            "Public Property Get Value() As Long\n",
            "Value = stored + 1\n",
            "End Property\n",
            "Attribute Value.VB_UserMemId = 0\n"
        );
        let short_manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Main", ModuleKind::Procedural, main_source)
                    .expect("short main parses"),
                module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                    .expect("short widget parses"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let short_compiled =
            oxvba_compiler::compile_project(&short_manifest).expect("short manifest compiles");
        let short_lowered = short_compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            short_lowered.contains("valueout = property_get_pmr_projecta_widget_value(widget)"),
            "{short_lowered}"
        );

        let direct_manifest = ProjectManifest {
            project_name: "AttributeOracleProject".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Main", ModuleKind::Procedural, main_source)
                    .expect("direct main parses"),
                module_unit_from_source("Widget", ModuleKind::Class, widget_source)
                    .expect("direct widget parses"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let direct_compiled =
            oxvba_compiler::compile_project(&direct_manifest).expect("direct manifest compiles");
        let direct_lowered = direct_compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            direct_lowered.contains("valueout = property_get_pmr_"),
            "{direct_lowered}"
        );
        assert!(
            direct_lowered.contains("property get pmr_"),
            "{direct_lowered}"
        );

        let loaded = load_basproj_from_str(
            "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>AttributeOracleProject</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Main.bas\" />
    <ClassModule Include=\"Widget.cls\" />
  </ItemGroup>
</Project>
",
            &temp_root,
        )
        .expect("load project");

        let loaded_compiled =
            oxvba_compiler::compile_project(&loaded.manifest).expect("loaded manifest compiles");
        let loaded_lowered = loaded_compiled.rewritten_source.to_ascii_lowercase();
        assert!(
            loaded_lowered.contains("valueout = property_get_pmr_"),
            "{loaded_lowered}"
        );
        assert!(
            loaded_lowered.contains("property get pmr_"),
            "{loaded_lowered}"
        );
        assert!(
            loaded_lowered.contains("pmr_attributeoracleproject_widget_value ="),
            "{loaded_lowered}"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    }
}
