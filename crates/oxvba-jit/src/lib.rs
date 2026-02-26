//! oxvba-jit: JIT scaffolding and CLIF lowering placeholders.

pub mod cranelift;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum JitError {
    #[error("jit is not yet enabled")]
    NotEnabled,
}

#[derive(Debug, Default)]
pub struct JitEngine;

impl JitEngine {
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Err(JitError::NotEnabled)
    }
}
