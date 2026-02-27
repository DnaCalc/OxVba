//! oxvba-jit: JIT scaffolding and CLIF lowering placeholders.

pub mod cranelift;

use oxvba_compiler::Bytecode;
use oxvba_vm::execute_and_snapshot;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JitError {
    #[error("jit execution failed: {0}")]
    Execution(String),
}

#[derive(Debug, Default)]
pub struct JitEngine;

impl JitEngine {
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Ok(())
    }

    pub fn execute_and_snapshot(&self, bytecode: &Bytecode) -> Result<Vec<i32>, JitError> {
        execute_and_snapshot(bytecode).map_err(JitError::Execution)
    }
}
