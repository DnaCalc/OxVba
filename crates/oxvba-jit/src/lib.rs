//! oxvba-jit: JIT scaffolding and CLIF lowering placeholders.

pub mod cranelift;

use std::sync::Arc;

use oxvba_compiler::Bytecode;
use oxvba_hal::{
    adapters,
    model::{HalProfileId, HostPolicy},
    traits::HostServices,
};
use oxvba_vm::execute_and_snapshot_with_host;
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
        self.execute_and_snapshot_with_host(bytecode, default_host_services())
    }

    pub fn execute_and_snapshot_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<i32>, JitError> {
        if cranelift::supports_bytecode(bytecode) {
            return cranelift::execute_bytecode(bytecode).map_err(JitError::Execution);
        }
        execute_and_snapshot_with_host(bytecode, host_services).map_err(JitError::Execution)
    }
}

fn default_host_services() -> Arc<dyn HostServices> {
    adapters::for_profile(HalProfileId::Windows, HostPolicy::deterministic_runtime())
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

    #[test]
    fn falls_back_for_cverr_range_predicates_and_matches_vm() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nDim e\nDim f\na = IsError(CVErr(0))\nb = IsError(CVErr(65535))\nc = IsError(CVErr(70000))\nd = IsError(CVErr(-70000))\ne = IsNumeric(CVErr(0))\nf = VarType(CVErr(70000))\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit fallback should execute");
        assert_eq!(jit, vm);
        assert_eq!(jit, vec![1, 1, 1, 1, 0, 10]);
    }

    #[test]
    fn falls_back_for_nested_error_mode_transitions_and_matches_vm() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nOn Error Resume Next\nError 5\na = Err.Number\nResume Next\nb = Err.Number\nOn Error GoTo Handler\nError 6\nc = Err.Number\nGoTo Done\nHandler:\nd = Err.Number\nResume Next\nDone:\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit fallback should execute");
        assert_eq!(jit, vm);
        assert_eq!(jit, vec![5, 0, 0, 6]);
    }
}
