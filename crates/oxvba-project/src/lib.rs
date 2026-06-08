//! OxVBA project system: `.basproj`/`.vbp` parsing and clean-path loading.
//!
//! This crate turns an on-disk project (and its transitive `<ProjectReference>`
//! graph) into the in-memory shape the clean compiler stack consumes.
//!
//! # Architecture
//!
//! ```text
//! .basproj / .vbp file
//!     ↓  (parse_basproj_xml / parse_vbp)
//! BasProj  (intermediate XML model)
//!     ↓  (load_basproj / load_workspace_target)
//! LoadedProject { ProjectManifest, ... }   ← single-project view
//!
//! .basproj graph
//!     ↓  (load_project_closure)
//! Vec<oxvba_symbol::manifest::SymbolProjectManifest>   ← leaf-first closure
//!     ↓  (oxvba_bind::bind_projects → linearize → oxvba_vm2::Vm::link)
//! ```
//!
//! [`manifest`] holds the loader's own project-manifest types (relocated from the
//! deleted `oxvba-compiler`); [`closure`] adapts them into `oxvba-symbol` types.

pub mod closure;
pub mod error;
pub mod load;
pub mod manifest;
pub mod model;
pub mod parse;
pub mod vbp;
pub mod workspace_target;

pub use closure::{load_project_closure, load_project_closure_with_entry};
pub use error::BasProjError;
pub use load::{
    LoadedProject, infer_project_name_from_path, load_basproj, load_basproj_from_str,
    override_loaded_project_entry_point,
};
pub use manifest::{
    DeclareParamType, ExportKind, ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind,
    ProjectManifest, ProjectReference, ReferenceKind, ReferencedProjectManifest,
};
pub use model::{
    BasProj, BasProjComReference, BasProjModule, BasProjModuleKind, BasProjNativeExport,
    BasProjNativeReference, BasProjProjectReference, BasProjProjectReferenceKind,
    BasProjProperties, BuildTarget, CallingConvention, ClassModuleMetadata, Instancing,
    NativeExportDescriptor, OutputType, RuntimeFlavor,
};
pub use parse::parse_basproj_xml;
pub use vbp::{generate_basproj_from_vbp, load_vbp, load_vbp_from_str, parse_vbp};
pub use workspace_target::{
    discover_project_file_in_dir, load_convention_project, load_workspace_target,
};
