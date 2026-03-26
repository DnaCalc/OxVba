//! High-level `.basproj` loading: parse XML, resolve filesystem, produce `ProjectManifest`.

use std::collections::BTreeMap;
use std::path::Path;

use oxvba_com::{TypeLibResolveRequest, build_typelib_metadata, resolve_known_typelib_identity};
use oxvba_compiler::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectManifest, ProjectReference,
    ReferenceKind, ReferencedProjectManifest, project::project_typelib_as_manifest,
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
    let entry_point = props.entry_point.clone();
    if matches!(output_type, OutputType::Exe | OutputType::Addin) && entry_point.is_none() {
        // Not an error yet — auto-discovery of Sub Main is allowed for Exe
        // Addin strictly requires it though
        if output_type == OutputType::Addin {
            return Err(BasProjError::EntryPointRequired("Addin".to_string()));
        }
    }

    // Conditional constants
    let conditional_constants = props
        .define_constants
        .as_deref()
        .map(parse_define_constants)
        .unwrap_or_default();

    // Load modules
    let modules = if basproj.modules.is_empty() {
        // Auto-discovery mode
        discover_modules(project_dir)?
    } else {
        load_explicit_modules(&basproj.modules, project_dir)?
    };

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
        entry_point,
        type_library_catalog,
        class_module_metadata,
    })
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
        let Some(identity) = resolve_known_typelib_identity(&request) else {
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
    let (code, message) = match (request.libid_hint.as_deref(), request.importlib_hint.as_deref()) {
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
            attributes: ModuleAttributes::default(),
            source: format!("code={code}\nmessage={message}\n"),
        }],
    }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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
                modules.push(ModuleUnit {
                    module_name,
                    module_kind: kind,
                    attributes: ModuleAttributes::default(),
                    source,
                });
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
        let attributes = ModuleAttributes {
            vb_name: module_name.clone(),
            vb_global_namespace: bm.vb_global_namespace,
            vb_creatable: bm.vb_creatable,
            vb_predeclared_id: bm.vb_predeclared_id,
            vb_exposed: bm.vb_exposed,
            option_private_module: false,
        };
        modules.push(ModuleUnit {
            module_name,
            module_kind,
            attributes,
            source,
        });
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
