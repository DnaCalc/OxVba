//! oxvba-host: engine orchestration for the clean execution stack.

pub mod engine;
pub mod runner;

pub use engine::{DiagnosticPhase, Engine, HostConfig, PhaseDiagnostic};
pub use runner::{
    PolicyOverrides, ResolvedRunnerBootstrap, RunnerBootstrapFallbacks, RunnerBootstrapOptions,
    RuntimeProfileId, resolve_runner_bootstrap, resolve_runner_bootstrap_with_fallbacks,
};
