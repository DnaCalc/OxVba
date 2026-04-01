//! OxVBA project system: `.basproj` parsing, loading, and generation.
//!
//! This crate provides the project file layer that sits above `oxvba-compiler`
//! and `oxvba-host`. It parses `.basproj` XML project files, resolves module
//! sources from the filesystem, and produces `ProjectManifest` values suitable
//! for compilation via `oxvba_compiler::compile_project`.
//!
//! # Architecture
//!
//! ```text
//! .basproj file
//!     ↓  (parse_basproj_xml)
//! BasProj  (intermediate XML model)
//!     ↓  (load_basproj / load_basproj_from_str)
//! LoadedProject { ProjectManifest, Vec<NativeExportDescriptor>, ... }
//!     ↓  (oxvba-host: Engine::execute_project_with_snapshot_phased)
//! CompiledProject → Execution
//! ```

pub mod com_selection;
pub mod error;
pub mod generate;
pub mod host_helpers;
pub mod load;
pub mod model;
pub mod parse;
pub mod resolve;
pub mod validate;
pub mod vbp;
pub mod workspace_target;

// Re-exports for convenience
pub use com_selection::{
    ComProjectEditPlan, ComProjectEditPlanKind, ComProjectSelection, ComProjectSelectionStatus,
    ComSelectionCandidate,
    ComSelectionCarrierKind, ComSelectionConfidence, ComSelectionDiscoveryError,
    ComSelectionIdentity, ComSelectionSourceKind, FileBackedComSelectionQuery,
    RegisteredComSelectionQuery,
    assess_project_com_selections, basproj_reference_from_candidate,
    candidate_from_catalog_entry, candidate_from_project_reference, candidate_from_resolved_identity,
    discover_file_backed_com_candidates, discover_prog_id_com_candidates,
    discover_registered_com_candidates, plan_add_com_candidate, plan_remove_com_reference,
    plan_repair_project_selection, plan_replace_com_reference,
};
pub use error::BasProjError;
pub use generate::generate_basproj_xml;
pub use host_helpers::{
    HostProjectEdit, HostProjectModuleInfo, HostProjectReferenceInfo, HostProjectReferenceKind,
    HostProjectSurface, HostWorkspaceTargetKind, ModuleIdentityInfo, ModuleIdentityRewrite,
    PlannedModule, VbNameAttributeAction, add_com_reference_edit, add_module_edit,
    add_project_reference_edit, inspect_module_identity, inspect_workspace_target,
    plan_new_module, reconcile_module_identity, remove_com_reference_edit, remove_module_edit,
    remove_project_reference_edit,
};
pub use load::{
    LoadedProject, infer_project_name_from_path, load_basproj, load_basproj_from_str,
    override_loaded_project_entry_point,
};
pub use model::{
    BasProj, BasProjComReference, BasProjModule, BasProjModuleKind, BasProjNativeExport,
    BasProjNativeReference, BasProjProjectReference, BasProjProperties, CallingConvention,
    ClassModuleMetadata, Instancing, NativeExportDescriptor, OutputType, RuntimeFlavor,
};
pub use parse::parse_basproj_xml;
pub use validate::{ComClassExportDescriptor, DispatchMemberInfo};
pub use vbp::{generate_basproj_from_vbp, load_vbp, load_vbp_from_str, parse_vbp};
pub use workspace_target::{
    discover_project_file_in_dir, load_convention_project, load_workspace_target,
};
