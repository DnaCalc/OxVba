//! Post-compilation validation for native exports and COM class exports.

use oxvba_compiler::{CompiledProject, DeclareParamType};

use crate::error::BasProjError;
use crate::model::*;

/// A descriptor for a COM-exposed class, derived from project metadata + compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComClassExportDescriptor {
    pub class_name: String,
    pub prog_id: Option<String>,
    pub instancing: Option<Instancing>,
    pub description: Option<String>,
    pub members: Vec<DispatchMemberInfo>,
}

/// Information about a dispatch member on a COM-exposed class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchMemberInfo {
    pub member_name: String,
    pub kind: oxvba_compiler::ProjectDynamicMemberKind,
    pub param_count: usize,
}

/// Validate native export descriptors against a compiled project.
///
/// For each export, finds the matching procedure in `compiled.host_exports`,
/// verifies it's in a procedural module, and populates the `kind`,
/// `param_types`, and `return_type` fields on the descriptor.
pub fn validate_native_exports(
    exports: &mut [NativeExportDescriptor],
    compiled: &CompiledProject,
) -> Result<(), BasProjError> {
    for export in exports.iter_mut() {
        // Find matching host export
        let host_export = compiled
            .host_exports
            .iter()
            .find(|he| {
                he.module_name.eq_ignore_ascii_case(&export.module_name)
                    && he.procedure_name.eq_ignore_ascii_case(&export.procedure_name)
            })
            .ok_or_else(|| BasProjError::ExportProcedureNotFound {
                exported_name: export.exported_name.clone(),
                module_name: export.module_name.clone(),
                procedure_name: export.procedure_name.clone(),
            })?;

        export.kind = Some(host_export.kind);

        // Look up procedure runtime metadata by lowered key
        let key = format!(
            "{}.{}",
            export.module_name.to_ascii_lowercase(),
            export.procedure_name.to_ascii_lowercase()
        );
        if let Some(metadata) = compiled.procedure_runtime_metadata.get(&key) {
            // Populate param_types from param_slots (as Variant since we don't
            // have bound type info at this level — full type info requires
            // re-analyzing the HIR, which is deferred)
            let param_types: Vec<DeclareParamType> = metadata
                .param_slots
                .iter()
                .map(|_| DeclareParamType::Variant)
                .collect();
            export.param_types = Some(param_types);
            export.return_type = Some(if metadata.return_slot.is_some() {
                Some(DeclareParamType::Variant)
            } else {
                None
            });
        }
    }

    Ok(())
}

/// Validate COM class exports for a ComServer project.
///
/// For each class module with `vb_exposed` and `vb_creatable`, builds a
/// `ComClassExportDescriptor` with its dispatch member inventory from the
/// compiled `ProjectDynamicObjectRoute` data.
pub fn validate_com_class_exports(
    modules: &[BasProjModule],
    compiled: &CompiledProject,
    class_metadata: &std::collections::BTreeMap<String, ClassModuleMetadata>,
    _project_name: &str,
) -> Result<Vec<ComClassExportDescriptor>, BasProjError> {
    let mut results = Vec::new();

    for bm in modules {
        if bm.kind != BasProjModuleKind::ClassModule {
            continue;
        }
        if !bm.vb_exposed || !bm.vb_creatable {
            continue;
        }

        let module_name = std::path::Path::new(&bm.include)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Find the dynamic object route for this class
        let members: Vec<DispatchMemberInfo> = compiled
            .project_dynamic_objects
            .iter()
            .find(|route| route.module_name.eq_ignore_ascii_case(&module_name))
            .map(|route| {
                route
                    .members
                    .iter()
                    .map(|m| DispatchMemberInfo {
                        member_name: m.member_name.clone(),
                        kind: m.kind,
                        param_count: m.visible_param_count,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let meta = class_metadata.get(&module_name);
        results.push(ComClassExportDescriptor {
            class_name: module_name,
            prog_id: meta.and_then(|m| m.prog_id.clone()).or_else(|| bm.prog_id.clone()),
            instancing: meta
                .and_then(|m| m.instancing)
                .or(bm.instancing),
            description: meta
                .and_then(|m| m.description.clone())
                .or_else(|| bm.description.clone()),
            members,
        });
    }

    Ok(results)
}
