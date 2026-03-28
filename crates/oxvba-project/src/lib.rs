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
pub mod load;
pub mod model;
pub mod parse;
pub mod resolve;
pub mod validate;
pub mod vbp;

// Re-exports for convenience
pub use error::BasProjError;
pub use generate::generate_basproj_xml;
pub use load::{LoadedProject, load_basproj, load_basproj_from_str};
pub use model::{
    BasProj, BasProjComReference, BasProjModule, BasProjModuleKind, BasProjNativeExport,
    BasProjNativeReference, BasProjProjectReference, BasProjProperties, CallingConvention,
    ClassModuleMetadata, Instancing, NativeExportDescriptor, OutputType, RuntimeFlavor,
};
pub use parse::parse_basproj_xml;
pub use validate::{ComClassExportDescriptor, DispatchMemberInfo};
pub use vbp::{generate_basproj_from_vbp, load_vbp, load_vbp_from_str, parse_vbp};
