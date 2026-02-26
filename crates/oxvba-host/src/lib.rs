//! oxvba-host: engine orchestration and host integration scaffolding.

pub mod engine;
pub mod events;
pub mod project;

pub use engine::{Engine, HostConfig};
pub use project::Project;
