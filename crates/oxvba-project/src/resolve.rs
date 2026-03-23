//! Reference resolution for `.basproj` projects.
//!
//! Resolves project references (recursive with cycle detection), COM references
//! (bridged to `TypeLibraryCatalogEntry`), and native references (path validation).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use oxvba_compiler::ReferencedProjectManifest;
use oxvba_host::TypeLibraryCatalogEntry;

use crate::error::BasProjError;
use crate::load::load_basproj;
use crate::model::*;

/// Resolve `<ProjectReference>` items by loading each referenced `.basproj`
/// recursively and extracting their public modules.
///
/// `ancestors` tracks the current resolution chain for cycle detection.
/// `seen` tracks all projects that have already been fully resolved (for
/// diamond deduplication — a project referenced by multiple paths is only
/// included once).
pub fn resolve_project_references(
    basproj: &BasProj,
    project_dir: &Path,
    ancestors: &mut HashSet<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<Vec<ReferencedProjectManifest>, BasProjError> {
    let mut results = Vec::new();

    for pr in &basproj.project_references {
        let ref_path = project_dir.join(&pr.include);
        let canonical = ref_path.canonicalize().map_err(|_| {
            BasProjError::ProjectReferenceNotFound {
                include: pr.include.clone(),
            }
        })?;

        // Cycle detection: is this project an ancestor in the current chain?
        if ancestors.contains(&canonical) {
            let cycle: Vec<String> =
                ancestors.iter().map(|p| p.display().to_string()).collect();
            return Err(BasProjError::CyclicProjectReference {
                path: canonical.display().to_string(),
                cycle,
            });
        }

        // Diamond dedup: already resolved this project via another path
        if seen.contains(&canonical) {
            continue;
        }

        seen.insert(canonical.clone());
        ancestors.insert(canonical.clone());

        let loaded = load_basproj(&canonical)?;

        // Recursively resolve the referenced project's own references
        let ref_project_dir = crate::model::project_dir(&canonical);
        let ref_basproj_xml =
            std::fs::read_to_string(&canonical).map_err(|e| BasProjError::Io {
                path: canonical.display().to_string(),
                source: e,
            })?;
        let ref_basproj = crate::parse::parse_basproj_xml(&ref_basproj_xml)?;
        let nested =
            resolve_project_references(&ref_basproj, &ref_project_dir, ancestors, seen)?;
        results.extend(nested);

        // Remove from ancestors after recursion (no longer on the current path)
        ancestors.remove(&canonical);

        // Extract public modules from the referenced project
        let public_modules: Vec<_> = loaded
            .manifest
            .modules
            .into_iter()
            .filter(|m| {
                m.attributes.vb_exposed
                    || m.module_kind == oxvba_compiler::ModuleKind::Procedural
            })
            .collect();

        results.push(ReferencedProjectManifest {
            project_name: loaded.manifest.project_name,
            modules: public_modules,
        });
    }

    Ok(results)
}

/// Bridge `BasProjComReference` items to `TypeLibraryCatalogEntry` values.
pub fn resolve_com_references(
    com_refs: &[BasProjComReference],
) -> Vec<TypeLibraryCatalogEntry> {
    com_refs
        .iter()
        .map(|cr| TypeLibraryCatalogEntry {
            library_name: cr.include.clone(),
            importlib: cr.import_lib.clone().unwrap_or_default(),
            libid: cr.guid.clone(),
            major_version: cr.version_major.unwrap_or(0),
            minor_version: cr.version_minor.unwrap_or(0),
            lcid: cr.lcid,
        })
        .collect()
}

/// Validate that all `<NativeReference>` paths exist on disk.
pub fn resolve_native_references(
    native_refs: &[BasProjNativeReference],
    project_dir: &Path,
) -> Result<(), BasProjError> {
    for nr in native_refs {
        if let Some(ref path_str) = nr.path {
            let resolved = project_dir.join(path_str);
            if !resolved.exists() {
                return Err(BasProjError::NativeReferenceNotFound {
                    include: nr.include.clone(),
                    resolved_path: resolved.display().to_string(),
                });
            }
        }
    }
    Ok(())
}
