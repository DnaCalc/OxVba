//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod engine;
pub mod events;
pub mod project;
pub mod runner;

pub use engine::{Engine, HostConfig};
pub use project::{
    GraphPublicSymbolResolution, HostExportKind, HostProcedureExport, ModuleAttributes, ModuleKind,
    ModuleNode, Project, ProjectGraph, ProjectKind, ProjectModelError, ProjectNode,
    ProjectReference, PublicSymbolResolution, ReferenceBindingState, ReferenceKind,
    TypeLibraryBindingRecord, TypeLibraryBindingStatus, TypeLibraryCatalogEntry,
};
pub use runner::{
    PolicyOverrides, ResolvedRunnerBootstrap, RunnerBootstrapOptions, RuntimeProfileId,
    resolve_runner_bootstrap,
};
