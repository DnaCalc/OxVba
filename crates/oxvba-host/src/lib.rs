//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod engine;
pub mod events;
pub mod project;
pub mod runner;

pub use engine::{ComEventCallbackDispatch, Engine, HostConfig, ProjectRuntimeSession};
pub use project::{
    GraphPublicSymbolResolution, HostExportKind, HostProcedureExport, ModuleAttributes, ModuleKind,
    ModuleNode, Project, ProjectGraph, ProjectKind, ProjectModelError, ProjectNode,
    ProjectReference, PublicSymbolResolution, ReferenceBindingState, ReferenceKind,
    TypeLibraryBindingRecord, TypeLibraryBindingStatus, TypeLibraryCatalogEntry,
};
pub use runner::{
    PolicyOverrides, ResolvedRunnerBootstrap, RunnerBootstrapFallbacks,
    RunnerBootstrapOptions, RuntimeProfileId, resolve_runner_bootstrap,
    resolve_runner_bootstrap_with_fallbacks,
};
