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
pub use error::BasProjError;
pub use generate::generate_basproj_xml;
pub use host_helpers::{
    HostProjectEdit, ModuleIdentityInfo, ModuleIdentityRewrite, PlannedModule,
    VbNameAttributeAction, add_com_reference_edit, add_module_edit, add_project_reference_edit,
    inspect_module_identity, plan_new_module, reconcile_module_identity, remove_com_reference_edit,
    remove_module_edit, remove_project_reference_edit,
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
