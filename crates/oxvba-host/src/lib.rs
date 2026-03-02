//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod engine;
pub mod events;
pub mod project;
pub mod runner;

pub use engine::{Engine, HostConfig};
pub use project::Project;
pub use runner::{
    PolicyOverrides, ResolvedRunnerBootstrap, RunnerBootstrapOptions, RuntimeProfileId,
    resolve_runner_bootstrap,
};
