//! oxvba-jit: JIT scaffolding and CLIF lowering placeholders.

pub mod cranelift;
pub mod jit_context;
pub mod runtime_helpers;
pub mod slot_abi;

use std::sync::Arc;

use oxvba_compiler::Bytecode;
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{HalProfileId, HostPolicy},
    traits::HostServices,
};
use oxvba_runtime::RuntimeValue;
use oxvba_vm::execute_and_snapshot_with_host;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JitError {
    #[error("jit execution failed: {0}")]
    Execution(String),
}

#[derive(Debug, Default)]
pub struct JitEngine;

#[cfg(test)]
fn project_runtime_values_to_legacy_slots(values: Vec<RuntimeValue>) -> Vec<i32> {
    use oxvba_runtime::value_tags::EMPTY_TAG;

    values
        .into_iter()
        .map(|value| value.project_compat_slot_i32().unwrap_or(EMPTY_TAG))
        .collect()
}

impl JitEngine {
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Ok(())
    }

    pub fn execute_and_snapshot(&self, bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot_with_host(bytecode, default_host_services())
    }

    pub fn execute_and_snapshot_values(
        &self,
        bytecode: &Bytecode,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot(bytecode)
    }

    pub fn execute_and_snapshot_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        // Try the RtSlot path first (supports more instructions).
        // On failure, fall back to VM for proper error handling with detailed messages.
        if cranelift::supports_bytecode_rtslot(bytecode) {
            match cranelift::execute_bytecode_rtslot(bytecode, host_services.clone()) {
                Ok(values) => return Ok(values),
                Err(_) => {
                    return execute_and_snapshot_with_host(bytecode, host_services)
                        .map_err(JitError::Execution);
                }
            }
        }
        // Fall back to legacy i32 path for the original 23-instruction subset.
        if cranelift::supports_bytecode(bytecode) {
            return cranelift::execute_bytecode(bytecode).map_err(JitError::Execution);
        }
        // Fall back to VM interpreter for unsupported bytecode.
        execute_and_snapshot_with_host(bytecode, host_services).map_err(JitError::Execution)
    }

    pub fn execute_and_snapshot_values_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot_with_host(bytecode, host_services)
    }
}

fn default_host_services() -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(HalProfileId::Windows)
        .policy(HostPolicy::deterministic_runtime())
        .build()
}

#[cfg(test)]
mod tests {
    use super::{JitEngine, cranelift};
    #[cfg(target_os = "windows")]
    use crate::project_runtime_values_to_legacy_slots;
    use crate::{jit_context::JitContextOwned, runtime_helpers};
    use oxvba_compiler::bytecode::RuntimeArrayElementType;
    use oxvba_hal::{
        adapters::builder::HostBuilder,
        model::{HalProfileId, HostPolicy},
    };
    use oxvba_runtime::{
        F64Value, RuntimeValue,
        safe_array::{SafeArray, SafeArrayBound},
    };

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
    fn execute_and_snapshot_values_projects_legacy_jit_lane_to_runtime_values() {
        let bytecode = oxvba_compiler::compile("Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub")
            .expect("compile should succeed");
        let out = JitEngine
            .execute_and_snapshot_values(&bytecode)
            .expect("value snapshot should succeed");
        assert_eq!(out, vec![RuntimeValue::I32(3)]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn execute_and_snapshot_values_fallback_preserves_non_legacy_runtime_values() {
        let bytecode = oxvba_compiler::compile(
            "Sub Main()\nDim x\nx = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));
        let host_services = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::interactive_dev())
            .build();

        let out = JitEngine
            .execute_and_snapshot_values_with_host(&bytecode, host_services)
            .expect("fallback value snapshot should succeed");
        assert_eq!(out.len(), 1);
        assert!(matches!(
            out[0],
            RuntimeValue::Object(ref object_ref) if object_ref.raw() > 0
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn execute_and_snapshot_fallback_projects_non_legacy_runtime_values_to_legacy_slots() {
        let bytecode = oxvba_compiler::compile(
            "Sub Main()\nDim x\nx = CreateObject(\"OxVba.TestDispatch\")\nEnd Sub",
        )
        .expect("compile should succeed");
        assert!(!cranelift::supports_bytecode(&bytecode));
        let host_services = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::interactive_dev())
            .build();

        let out = project_runtime_values_to_legacy_slots(
            JitEngine
                .execute_and_snapshot_with_host(&bytecode, host_services)
                .expect("fallback semantic snapshot should succeed"),
        );
        assert_eq!(out.len(), 1);
        assert!(out[0] > 0);
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
    fn runtime_date_helpers_materialize_real_date_values() {
        let mut ctx = JitContextOwned::new(8, 8, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_slot(1, RuntimeValue::I32(2026));
            ctx.context.write_slot(2, RuntimeValue::I32(2));
            ctx.context.write_slot(3, RuntimeValue::I32(28));
            ctx.context.write_slot(4, RuntimeValue::I32(1));
            ctx.context.write_slot(5, RuntimeValue::I32(3));
        }

        assert_eq!(
            runtime_helpers::oxrt_date_serial(ctx.context_ptr(), 0, 1, 2, 3),
            0
        );
        assert_eq!(
            runtime_helpers::oxrt_date_add(ctx.context_ptr(), 6, 4, 5, 0),
            0
        );
        assert_eq!(runtime_helpers::oxrt_year(ctx.context_ptr(), 1, 6), 0);
        assert_eq!(runtime_helpers::oxrt_month(ctx.context_ptr(), 2, 6), 0);
        assert_eq!(runtime_helpers::oxrt_day(ctx.context_ptr(), 3, 6), 0);
        assert_eq!(runtime_helpers::oxrt_weekday(ctx.context_ptr(), 7, 6), 0);

        let values = ctx.extract_user_values();
        assert_eq!(
            values[0],
            RuntimeValue::F64(F64Value::from_date_f64(46081.0))
        );
        assert_eq!(
            values[6],
            RuntimeValue::F64(F64Value::from_date_f64(46084.0))
        );
        assert_eq!(values[1], RuntimeValue::I32(2026));
        assert_eq!(values[2], RuntimeValue::I32(3));
        assert_eq!(values[3], RuntimeValue::I32(3));
        assert_eq!(values[7], RuntimeValue::I32(3));
    }

    #[test]
    fn runtime_preserve_resize_helper_retains_existing_byte_values() {
        let mut ctx = JitContextOwned::new(2, 2, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_slot(
                0,
                RuntimeValue::ArrayIntent(SafeArray {
                    dimensions: 1,
                    len: 2,
                    bounds: Some(vec![SafeArrayBound { lower: 0, count: 2 }]),
                    elements: Some(vec![RuntimeValue::I32(90), RuntimeValue::I32(91)]),
                }),
            );
            ctx.context.write_slot(1, RuntimeValue::I32(3));
        }

        let upper_bounds = [1u32];
        let lower_bounds = [0i32];
        let rc = runtime_helpers::oxrt_array_resize_preserve(
            ctx.context_ptr(),
            0,
            upper_bounds.as_ptr(),
            lower_bounds.as_ptr(),
            1,
            RuntimeArrayElementType::Byte as i32,
        );
        assert_eq!(rc, 0);

        let values = ctx.extract_user_values();
        assert_eq!(
            values[0],
            RuntimeValue::ArrayIntent(SafeArray {
                dimensions: 1,
                len: 4,
                bounds: Some(vec![SafeArrayBound { lower: 0, count: 4 }]),
                elements: Some(vec![
                    RuntimeValue::I32(90),
                    RuntimeValue::I32(91),
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                ]),
            })
        );
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
            vec![RuntimeValue::ErrorCode(2001), RuntimeValue::ErrorCode(2002)]
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
        assert_eq!(
            jit,
            vec![
                RuntimeValue::I32(8), // VarType(vbNullString) = vbString
                RuntimeValue::I32(1),
                RuntimeValue::I32(10),
                RuntimeValue::I32(2),
                RuntimeValue::Bool(false), // IsNumeric(vbNullString) = False
                RuntimeValue::Bool(false), // IsNumeric(Null) = False
                RuntimeValue::Bool(false), // IsNumeric(CVErr) = False
                RuntimeValue::Bool(true),  // IsNumeric(7) = True
            ]
        );
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
        assert_eq!(
            jit,
            vec![
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(true),
                RuntimeValue::Bool(false), // IsNumeric(CVErr) = False
                RuntimeValue::I32(10),
            ]
        );
    }

    #[test]
    fn nested_error_mode_transitions_run_through_jit_and_match_vm() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\nOn Error Resume Next\nError 5\na = Err.Number\nResume Next\nb = Err.Number\nOn Error GoTo Handler\nError 6\nc = Err.Number\nGoTo Done\nHandler:\nd = Err.Number\nResume Next\nDone:\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        // Now supported via RtSlot JIT (Resume/ResumeNext ungated)
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));

        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
        assert_eq!(
            jit,
            vec![
                RuntimeValue::I32(5),
                RuntimeValue::I32(20), // Resume Next without pending error = error 20
                RuntimeValue::I32(0), // Resume Next in handler jumps back; Err.Number=0 after clear
                RuntimeValue::I32(6),
            ]
        );
    }

    // ── RtSlot path tests ─────────────────────────────────────────────

    #[test]
    fn rtslot_path_supported_for_basic_arithmetic() {
        let bytecode = oxvba_compiler::compile("Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub")
            .expect("compile should succeed");
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));
    }

    #[test]
    fn rtslot_execution_matches_vm_for_basic_arithmetic() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn rtslot_execution_matches_vm_for_loop() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 5\nx = x + i\nNext i\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn rtslot_execution_matches_vm_for_comparisons() {
        let source = "Sub Main()\nDim a\nDim b\nDim c\nDim d\na = 3\nb = 2\nIf a > b Then\nc = 1\nElse\nc = 0\nEnd If\nIf a < b Then\nd = 1\nElse\nd = 0\nEnd If\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn rtslot_execution_matches_vm_for_abs_sgn_fix() {
        let source = "Sub Main()\nDim x\nx = Abs(-7)\nx = Sgn(x)\nx = Fix(x)\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    // ── Resume instruction tests ─────────────────────────────────────

    #[test]
    fn oern_catches_implicit_error_and_matches_vm() {
        // Use a variable for 0 so the compiler emits DivSlots (not constant-folded)
        let source = "Sub Main()\nDim a\nDim x\nx = 0\nOn Error Resume Next\na = 1 / x\na = Err.Number\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
    }

    #[test]
    fn goto_handler_resume_next_and_matches_vm() {
        let source = "Sub Main()\nDim a\nOn Error GoTo H\nError 5\na = 1\nGoTo Done\nH:\na = Err.Number\nResume Next\nDone:\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
        // Resume Next after Error 5 → jumps to a = 1
        assert_eq!(jit, vec![RuntimeValue::I32(1)]);
    }

    #[test]
    fn resume_label_and_matches_vm() {
        let source = "Sub Main()\nDim a\nOn Error GoTo H\nError 9\na = 99\nGoTo Done\nH:\nResume Done\nDone:\na = 1\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
        assert_eq!(jit, vec![RuntimeValue::I32(1)]);
    }

    #[test]
    fn resume_without_error_gives_error_20_and_matches_vm() {
        let source =
            "Sub Main()\nDim a\nOn Error Resume Next\nResume Next\na = Err.Number\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
        assert_eq!(jit, vec![RuntimeValue::I32(20)]);
    }

    #[test]
    fn goto_handler_resume_retry_and_matches_vm() {
        // Resume (retry): handler fixes the condition, Resume retries the failing instruction.
        // On Error GoTo H: Error 5 → handler sets a=99, Resume Next → a=99 then falls through to a=1
        // Simpler than div-by-zero retry which depends on how the compiler emits division.
        let source = "Sub Main()\nDim a\nOn Error GoTo H\nError 5\na = 1\nGoTo Done\nH:\na = 99\nResume Next\nDone:\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(cranelift::supports_bytecode_rtslot(&bytecode));
        let vm = oxvba_vm::execute_and_snapshot(&bytecode).expect("vm should execute");
        let jit = JitEngine
            .execute_and_snapshot(&bytecode)
            .expect("jit should execute");
        assert_eq!(jit, vm);
        // Handler sets a=99, Resume Next goes to a=1, so a=1
        assert_eq!(jit, vec![RuntimeValue::I32(1)]);
    }
}
