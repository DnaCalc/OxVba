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
        if cranelift::supports_bytecode(bytecode) {
            return cranelift::execute_bytecode(bytecode).map_err(JitError::Execution);
        }
        execute_and_snapshot(bytecode).map_err(JitError::Execution)
    }
}

#[cfg(test)]
mod tests {
    use super::{JitEngine, cranelift};

    #[test]
    fn supports_subset_bytecode_path() {
        let bytecode = oxvba_compiler::compile("Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub")
            .expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn falls_back_for_unsupported_error_state_bytecode() {
        let bytecode =
            oxvba_compiler::compile("Sub Main()\nOn Error Resume Next\nError 2\nEnd Sub")
                .expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));

        let out = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("fallback execution should succeed");
        assert!(out.is_empty());
    }

    #[test]
    fn clif_execution_matches_vm_for_acyclic_control_flow_subset() {
        let source =
            "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = x + 3\nElse\nx = x + 9\nEnd If\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn falls_back_for_loop_backedge_subset() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));
    }
}
