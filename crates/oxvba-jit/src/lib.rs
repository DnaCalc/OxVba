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
    use oxvba_runtime::value_tags::error_tag_from_code;

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
    fn clif_execution_matches_vm_for_loop_control_flow_subset() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn clif_execution_matches_vm_for_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall AddTwo\nEnd Sub\nSub AddTwo()\nx = x + 2\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn supports_intrinsic_math_subset() {
        let bytecode = oxvba_compiler::compile(
            "Sub Main()\nDim x\nx = Abs(-7)\nx = Sgn(x)\nx = Fix(x)\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn clif_execution_matches_vm_for_intrinsic_math_subset() {
        let source = "Sub Main()\nDim x\nx = Abs(-7)\nx = Sgn(x)\nx = Fix(x)\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn falls_back_for_financial_tolerance_subset_and_matches_vm() {
        let source = "Sub Main()\nDim a\nDim b\na = Rate(0, 0, 0)\nb = NPer(1, 0, 0, 0)\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit fallback should execute");
        assert_eq!(jit, vm);
        assert_eq!(
            jit,
            vec![error_tag_from_code(2001), error_tag_from_code(2002)]
        );
    }

    #[test]
    fn falls_back_for_tag_introspection_subset_and_matches_vm() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\nDim f\nDim g\nDim h\na = VarType(vbNullString)\nb = VarType(Null)\nc = VarType(CVErr(9))\nd = VarType(7)\ne = IsNumeric(vbNullString)\nf = IsNumeric(Null)\ng = IsNumeric(CVErr(9))\nh = IsNumeric(7)\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit fallback should execute");
        assert_eq!(jit, vm);
        assert_eq!(jit, vec![0, 1, 10, 3, 0, 0, 0, 1]);
    }
}
