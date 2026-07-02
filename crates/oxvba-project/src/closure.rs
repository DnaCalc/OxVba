//! Clean-path project loading: a `.basproj` (or adapted `.vbp`) and its transitive
//! `<ProjectReference>` graph → a leaf-first, unit-deduped closure of
//! [`oxvba_symbol::manifest::SymbolProjectManifest`]s — exactly the input
//! [`oxvba_bind::bind_projects`] consumes (each project compiles to its own bundle;
//! the entry project is last).
//!
//! Each project carries **all** its own modules (it compiles to its own bundle)
//! plus its **direct** references' public modules (so surface synthesis can bind
//! against them). Diamonds collapse to one manifest (one bundle); cycles are
//! rejected. This is the on-disk-graph → in-memory-closure plumbing; the runtime
//! (`Vm::link`) is origin-agnostic.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::manifest as legacy;
use oxvba_symbol::manifest as sym;

use crate::error::BasProjError;
use crate::model::{self, BasProj, BasProjModule, BasProjProjectReferenceKind};

/// Load a `.basproj`/`.vbp` and its transitive project-reference graph into a
/// leaf-first, unit-deduped closure of `SymbolProjectManifest`s (entry last).
pub fn load_project_closure(path: &Path) -> Result<Vec<sym::SymbolProjectManifest>, BasProjError> {
    load_project_closure_with_entry(path, None)
}

/// As [`load_project_closure`], but overriding the **entry** (root) project's
/// startup procedure (the `oxvba run-project --entry Module.Proc` override). The
/// override applies only to the root; referenced projects keep their own entry
/// resolution (they are libraries with no entry).
pub fn load_project_closure_with_entry(
    path: &Path,
    entry_override: Option<&str>,
) -> Result<Vec<sym::SymbolProjectManifest>, BasProjError> {
    // The entry project itself — a missing file here is the project, not a missing
    // *reference*, so report it as I/O rather than `ProjectReferenceNotFound`.
    let root = path.canonicalize().map_err(|e| BasProjError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut order: Vec<PathBuf> = Vec::new();
    let mut nodes: HashMap<PathBuf, ProjectNode> = HashMap::new();
    let mut ancestors: HashSet<PathBuf> = HashSet::new();
    walk(
        &root,
        &mut order,
        &mut nodes,
        &mut ancestors,
        entry_override,
    )?;

    let mut manifests = Vec::with_capacity(order.len());
    for canonical_path in &order {
        let node = &nodes[canonical_path];
        // A project's direct references contribute their public modules as the
        // surface a referrer binds against.
        let reference_projects = node
            .direct_refs
            .iter()
            .filter_map(|dep| nodes.get(dep))
            .map(|dep| sym::ReferencedProjectManifest {
                project_name: dep.project_name.clone(),
                project_kind: sym::ProjectKind::Library,
                modules: public_modules(&dep.modules),
            })
            .collect();
        manifests.push(sym::SymbolProjectManifest {
            project_name: node.project_name.clone(),
            project_kind: node.project_kind,
            modules: node.modules.clone(),
            references: node.references.clone(),
            reference_projects,
            conditional_constants: node.conditional_constants.clone(),
            conditional_compilation_target: Default::default(),
        });
    }
    Ok(manifests)
}

/// One fully-loaded project: its own modules (clean-path shape) + the canonical
/// paths of its direct `<ProjectReference kind="Project">` dependencies.
struct ProjectNode {
    project_name: String,
    project_kind: sym::ProjectKind,
    modules: Vec<sym::ModuleUnit>,
    references: Vec<sym::ProjectReference>,
    conditional_constants: BTreeMap<String, i32>,
    direct_refs: Vec<PathBuf>,
}

/// Post-order walk: load each project once (diamond dedup via `nodes`), recurse
/// into its direct references first (so dependencies land in `order` before their
/// dependents — leaf-first, entry last), reject cycles via `ancestors`.
fn walk(
    path: &Path,
    order: &mut Vec<PathBuf>,
    nodes: &mut HashMap<PathBuf, ProjectNode>,
    ancestors: &mut HashSet<PathBuf>,
    entry_override: Option<&str>,
) -> Result<(), BasProjError> {
    if nodes.contains_key(path) {
        return Ok(()); // already resolved via another path (diamond)
    }
    if ancestors.contains(path) {
        let cycle = ancestors.iter().map(|p| p.display().to_string()).collect();
        return Err(BasProjError::CyclicProjectReference {
            path: path.display().to_string(),
            cycle,
        });
    }

    let basproj = parse_project_file(path)?;
    let project_dir = model::project_dir(path);

    ancestors.insert(path.to_path_buf());
    let mut direct_refs = Vec::new();
    for pr in &basproj.project_references {
        // A `HostInjected` reference is the ambient host object model, resolved by
        // the host provider at bind time — not a peer bundle in the link set.
        if pr.kind != BasProjProjectReferenceKind::Project {
            continue;
        }
        let dep_path = project_dir.join(&pr.include);
        let dep = canonical(&dep_path).map_err(|_| BasProjError::ProjectReferenceNotFound {
            include: pr.include.clone(),
        })?;
        // A referenced project is a library; it keeps its own entry resolution.
        walk(&dep, order, nodes, ancestors, None)?;
        direct_refs.push(dep);
    }
    ancestors.remove(path);

    let node = build_node(&basproj, &project_dir, direct_refs, entry_override)?;
    nodes.insert(path.to_path_buf(), node);
    order.push(path.to_path_buf());
    Ok(())
}

/// Parse a project file (`.basproj`, or a `.vbp` adapted to `.basproj` shape) with
/// its `<Import>`s merged.
fn parse_project_file(path: &Path) -> Result<BasProj, BasProjError> {
    let text = std::fs::read_to_string(path).map_err(|e| BasProjError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let project_dir = model::project_dir(path);
    let is_vbp = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("vbp"));
    let xml = if is_vbp {
        let vbp = crate::vbp::parse_vbp(&text).map_err(BasProjError::VbpParse)?;
        crate::vbp::generate_basproj_from_vbp(&vbp).map_err(BasProjError::VbpUnsupported)?
    } else {
        text
    };
    let mut basproj = crate::parse::parse_basproj_xml(&xml)?;
    crate::load::process_imports(&mut basproj, &xml, &project_dir)?;
    Ok(basproj)
}

/// Build one project's clean-path node: reuse the legacy module loader (file I/O,
/// attribute parsing, entry-point shim + top-level-mainline rewrite — all
/// source-shape concerns), then adapt the loaded manifest into `oxvba-symbol`
/// types, merging per-class metadata (`instancing`/`prog_id`/etc.) the legacy
/// `ModuleAttributes` does not carry.
fn build_node(
    basproj: &BasProj,
    project_dir: &Path,
    direct_refs: Vec<PathBuf>,
    entry_override: Option<&str>,
) -> Result<ProjectNode, BasProjError> {
    let mut loaded = crate::load::build_loaded_project(basproj, project_dir)?;
    // A CLI `--entry` override re-points the root project's startup shim, reusing
    // the loader's own shim logic (so the clean path's entry matches the legacy one).
    if let Some(entry) = entry_override {
        crate::load::override_loaded_project_entry_point(&mut loaded, entry)?;
    }
    let meta = module_metadata_by_name(basproj);

    let modules = loaded
        .manifest
        .modules
        .iter()
        .map(|m| adapt_module(m, &meta))
        .collect();
    let references = build_references(basproj);

    Ok(ProjectNode {
        project_name: loaded.manifest.project_name,
        project_kind: adapt_project_kind(loaded.manifest.project_kind),
        modules,
        references,
        conditional_constants: loaded.manifest.conditional_constants,
        direct_refs,
    })
}

/// Per-module COM/document metadata keyed by module name (file stem), recovered
/// from the `.basproj` items — the legacy `ModuleAttributes` drops these.
fn module_metadata_by_name(basproj: &BasProj) -> HashMap<String, &BasProjModule> {
    basproj
        .modules
        .iter()
        .map(|bm| (module_name_of(&bm.include), bm))
        .collect()
}

fn module_name_of(include: &str) -> String {
    Path::new(include)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(include)
        .to_string()
}

fn adapt_module(m: &legacy::ModuleUnit, meta: &HashMap<String, &BasProjModule>) -> sym::ModuleUnit {
    let bm = meta.get(&m.module_name);
    sym::ModuleUnit {
        module_name: m.module_name.clone(),
        module_kind: adapt_module_kind(m.module_kind),
        attributes: sym::ModuleAttributes {
            vb_name: m.attributes.vb_name.clone(),
            vb_global_namespace: m.attributes.vb_global_namespace,
            vb_creatable: m.attributes.vb_creatable,
            vb_predeclared_id: m.attributes.vb_predeclared_id,
            vb_exposed: m.attributes.vb_exposed,
            option_private_module: m.attributes.option_private_module,
            host_document_type: bm.and_then(|b| b.host_document_type.clone()),
            instancing: bm
                .and_then(|b| b.instancing)
                .map(adapt_model_instancing)
                .or_else(|| m.attributes.instancing.map(adapt_manifest_instancing)),
            prog_id: bm.and_then(|b| b.prog_id.clone()),
            description: bm.and_then(|b| b.description.clone()),
        },
        source: m.source.clone(),
    }
}

/// A project's public modules — the surface a referrer sees: exposed classes and
/// (always-public) standard modules. Mirrors the legacy reference resolver's filter.
fn public_modules(modules: &[sym::ModuleUnit]) -> Vec<sym::ModuleUnit> {
    modules
        .iter()
        .filter(|m| m.attributes.vb_exposed || m.module_kind == sym::ModuleKind::Procedural)
        .cloned()
        .collect()
}

/// `<ProjectReference>` / `<COMReference>` / `<NativeReference>` → the binder's
/// reference vocabulary (project source refs are resolved via `reference_projects`;
/// COM refs drive the typelib provider; native refs target `Declare Lib`).
fn build_references(basproj: &BasProj) -> Vec<sym::ProjectReference> {
    let mut references = Vec::new();
    for pr in &basproj.project_references {
        references.push(match pr.kind {
            BasProjProjectReferenceKind::Project => sym::ProjectReference::Project {
                referenced_project_name: module_name_of(&pr.include),
            },
            BasProjProjectReferenceKind::HostInjected => sym::ProjectReference::HostInjected {
                referenced_project_name: pr.include.clone(),
            },
        });
    }
    for cr in &basproj.com_references {
        references.push(sym::ProjectReference::TypeLibrary {
            name: cr.include.clone(),
            guid: cr.guid.clone(),
            version_major: cr.version_major,
            version_minor: cr.version_minor,
            lcid: cr.lcid,
            import_lib: cr.import_lib.clone(),
        });
    }
    for nr in &basproj.native_references {
        references.push(sym::ProjectReference::Native {
            name: nr.include.clone(),
            path: nr.path.clone(),
        });
    }
    references
}

fn adapt_project_kind(kind: legacy::ProjectKind) -> sym::ProjectKind {
    match kind {
        legacy::ProjectKind::Source => sym::ProjectKind::Source,
        legacy::ProjectKind::Host => sym::ProjectKind::Host,
        legacy::ProjectKind::Library => sym::ProjectKind::Library,
    }
}

fn adapt_module_kind(kind: legacy::ModuleKind) -> sym::ModuleKind {
    match kind {
        legacy::ModuleKind::Procedural => sym::ModuleKind::Procedural,
        legacy::ModuleKind::Class => sym::ModuleKind::Class,
        legacy::ModuleKind::Document => sym::ModuleKind::Document,
        legacy::ModuleKind::Form => sym::ModuleKind::Form,
        legacy::ModuleKind::Extension => sym::ModuleKind::Extension,
    }
}

fn adapt_model_instancing(instancing: model::Instancing) -> sym::Instancing {
    match instancing {
        model::Instancing::Private => sym::Instancing::Private,
        model::Instancing::PublicNotCreatable => sym::Instancing::PublicNotCreatable,
        model::Instancing::MultiUse => sym::Instancing::MultiUse,
        model::Instancing::GlobalMultiUse => sym::Instancing::GlobalMultiUse,
        model::Instancing::SingleUse => sym::Instancing::SingleUse,
        model::Instancing::GlobalSingleUse => sym::Instancing::GlobalSingleUse,
    }
}

fn adapt_manifest_instancing(instancing: legacy::Instancing) -> sym::Instancing {
    match instancing {
        legacy::Instancing::Private => sym::Instancing::Private,
        legacy::Instancing::PublicNotCreatable => sym::Instancing::PublicNotCreatable,
        legacy::Instancing::MultiUse => sym::Instancing::MultiUse,
        legacy::Instancing::GlobalMultiUse => sym::Instancing::GlobalMultiUse,
        legacy::Instancing::SingleUse => sym::Instancing::SingleUse,
        legacy::Instancing::GlobalSingleUse => sym::Instancing::GlobalSingleUse,
    }
}

fn canonical(path: &Path) -> Result<PathBuf, BasProjError> {
    path.canonicalize()
        .map_err(|_| BasProjError::ProjectReferenceNotFound {
            include: path.display().to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxvba_closure_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp root");
        dir
    }

    /// Write a project directory `root/<name>/` with a `.basproj` (referencing the
    /// given relative project includes) and the given `(file, source)` modules.
    fn write_project(
        root: &Path,
        name: &str,
        output_type: &str,
        entry_point: Option<&str>,
        modules: &[(&str, &str)],
        project_refs: &[&str],
    ) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).expect("create project dir");
        let module_items: String = modules
            .iter()
            .map(|(file, _)| {
                if file.ends_with(".cls") {
                    format!("    <ClassModule Include=\"{file}\" />\n")
                } else {
                    format!("    <Module Include=\"{file}\" />\n")
                }
            })
            .collect();
        let ref_items: String = project_refs
            .iter()
            .map(|inc| format!("    <ProjectReference Include=\"{inc}\" />\n"))
            .collect();
        let entry = entry_point
            .map(|e| format!("    <EntryPoint>{e}</EntryPoint>\n"))
            .unwrap_or_default();
        let xml = format!(
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n\
             <PropertyGroup>\n\
             <OutputType>{output_type}</OutputType>\n\
             <ProjectName>{name}</ProjectName>\n\
             {entry}</PropertyGroup>\n\
             <ItemGroup>\n{module_items}</ItemGroup>\n\
             <ItemGroup>\n{ref_items}</ItemGroup>\n\
             </Project>\n"
        );
        let basproj_path = dir.join(format!("{name}.basproj"));
        std::fs::write(&basproj_path, xml).expect("write basproj");
        for (file, source) in modules {
            std::fs::write(dir.join(file), source).expect("write module");
        }
        basproj_path
    }

    #[test]
    fn single_project_closure_has_one_manifest() {
        let root = unique_root("single");
        let app = write_project(
            &root,
            "App",
            "Exe",
            Some("Main.Run"),
            &[("Main.bas", "Public Sub Run()\nEnd Sub\n")],
            &[],
        );
        let closure = load_project_closure(&app).expect("closure");
        assert_eq!(closure.len(), 1);
        assert_eq!(closure[0].project_name, "App");
        assert!(closure[0].reference_projects.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn app_to_lib_closure_is_leaf_first_with_direct_reference_source() {
        let root = unique_root("app_lib");
        write_project(
            &root,
            "Lib",
            "Library",
            None,
            &[(
                "LibMod.bas",
                "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\nAdd = a + b\nEnd Function\n",
            )],
            &[],
        );
        let app = write_project(
            &root,
            "App",
            "Exe",
            Some("Main.Run"),
            &[("Main.bas", "Public Sub Run()\nEnd Sub\n")],
            &["../Lib/Lib.basproj"],
        );

        let closure = load_project_closure(&app).expect("closure");
        // Leaf-first: Lib precedes App; the entry project (App) is last.
        assert_eq!(closure.len(), 2);
        assert_eq!(closure[0].project_name, "Lib");
        assert_eq!(closure[1].project_name, "App");

        // App carries Lib's public source as a direct reference + a Project reference.
        let app_m = &closure[1];
        assert_eq!(app_m.reference_projects.len(), 1);
        assert_eq!(app_m.reference_projects[0].project_name, "Lib");
        assert!(
            app_m.reference_projects[0]
                .modules
                .iter()
                .any(|m| m.module_name == "LibMod"),
            "Lib's public module should be visible to App"
        );
        assert!(app_m.references.iter().any(|r| matches!(
            r,
            sym::ProjectReference::Project { referenced_project_name } if referenced_project_name == "Lib"
        )));
        // Lib (a library) carries no references.
        assert!(closure[0].reference_projects.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn diamond_dedups_shared_dependency_to_one_manifest() {
        // App → B → D and App → C → D: D must appear once, leaf-first before B/C.
        let root = unique_root("diamond");
        write_project(
            &root,
            "D",
            "Library",
            None,
            &[(
                "DMod.bas",
                "Public Function DVal() As Long\nDVal = 50\nEnd Function\n",
            )],
            &[],
        );
        write_project(
            &root,
            "B",
            "Library",
            None,
            &[(
                "BMod.bas",
                "Public Function BFromD() As Long\nBFromD = DVal()\nEnd Function\n",
            )],
            &["../D/D.basproj"],
        );
        write_project(
            &root,
            "C",
            "Library",
            None,
            &[(
                "CMod.bas",
                "Public Function CFromD() As Long\nCFromD = DVal()\nEnd Function\n",
            )],
            &["../D/D.basproj"],
        );
        let app = write_project(
            &root,
            "App",
            "Exe",
            Some("Main.Run"),
            &[("Main.bas", "Public Sub Run()\nEnd Sub\n")],
            &["../B/B.basproj", "../C/C.basproj"],
        );

        let closure = load_project_closure(&app).expect("closure");
        let names: Vec<&str> = closure.iter().map(|m| m.project_name.as_str()).collect();
        assert_eq!(
            names.iter().filter(|n| **n == "D").count(),
            1,
            "D appears once"
        );
        assert_eq!(names.last(), Some(&"App"), "entry project is last");
        // D precedes both B and C (leaf-first).
        let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
        assert!(pos("D") < pos("B") && pos("D") < pos("C"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn referenced_exported_class_header_instancing_reaches_surface_metadata() {
        let root = unique_root("class_header_instancing");
        write_project(
            &root,
            "Lib",
            "Library",
            None,
            &[(
                "Widget.cls",
                "VERSION 1.0 CLASS\n\
                 BEGIN\n\
                   MultiUse = -1  'True\n\
                 END\n\
                 Attribute VB_Name = \"Widget\"\n\
                 Attribute VB_GlobalNameSpace = False\n\
                 Attribute VB_Creatable = False\n\
                 Attribute VB_PredeclaredId = False\n\
                 Attribute VB_Exposed = True\n\
                 Public Function Value() As Long\n\
                 Value = 7\n\
                 End Function\n",
            )],
            &[],
        );
        let app = write_project(
            &root,
            "App",
            "Exe",
            Some("Main.Run"),
            &[("Main.bas", "Public Sub Run()\nEnd Sub\n")],
            &["../Lib/Lib.basproj"],
        );

        let closure = load_project_closure(&app).expect("closure");
        let app_manifest = closure.last().expect("app manifest");
        let widget = app_manifest.reference_projects[0]
            .modules
            .iter()
            .find(|module| module.module_name == "Widget")
            .expect("referenced Widget class should be public");

        assert_eq!(
            widget.attributes.instancing,
            Some(sym::Instancing::MultiUse),
            "exported .cls MultiUse header metadata should survive normalization"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cyclic_project_reference_is_rejected() {
        let root = unique_root("cycle");
        // A → B → A.
        write_project(
            &root,
            "A",
            "Library",
            None,
            &[("AMod.bas", "Public Sub A1()\nEnd Sub\n")],
            &["../B/B.basproj"],
        );
        let b = write_project(
            &root,
            "B",
            "Library",
            None,
            &[("BMod.bas", "Public Sub B1()\nEnd Sub\n")],
            &["../A/A.basproj"],
        );
        // Start from B so the cycle B→A→B is on the active chain.
        let err = load_project_closure(&b).expect_err("cycle must be rejected");
        assert!(
            matches!(err, BasProjError::CyclicProjectReference { .. }),
            "{err:?}"
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
