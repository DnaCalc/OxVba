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
use oxvba_runtime::{RuntimeValue, Variant};
use oxvba_vm::execute_and_snapshot_variants_with_host;
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

/// Compatibility projection for legacy callers that still consume semantic
/// `RuntimeValue` snapshots. The retained execution carrier is `Variant`.
fn project_variants_to_compat_values(values: Vec<Variant>) -> Result<Vec<RuntimeValue>, JitError> {
    values
        .into_iter()
        .map(|value| value.to_runtime_value().map_err(JitError::Execution))
        .collect()
}

impl JitEngine {
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Ok(())
    }

    /// Legacy snapshot alias. Prefer `execute_and_snapshot_variants` for
    /// retained value-model work.
    pub fn execute_and_snapshot(&self, bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot_compat_values(bytecode)
    }

    /// Compatibility snapshot boundary that projects retained `Variant` slots
    /// to `RuntimeValue` for older tests and host surfaces.
    pub fn execute_and_snapshot_compat_values(
        &self,
        bytecode: &Bytecode,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        project_variants_to_compat_values(self.execute_and_snapshot_variants(bytecode)?)
    }

    /// Retained value-model snapshot API.
    pub fn execute_and_snapshot_variants(
        &self,
        bytecode: &Bytecode,
    ) -> Result<Vec<Variant>, JitError> {
        self.execute_and_snapshot_variants_with_host(bytecode, default_host_services())
    }

    /// Legacy snapshot alias. Prefer `execute_and_snapshot_variants`.
    pub fn execute_and_snapshot_values(
        &self,
        bytecode: &Bytecode,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot_compat_values(bytecode)
    }

    /// Legacy host-backed snapshot alias. Prefer
    /// `execute_and_snapshot_variants_with_host`.
    pub fn execute_and_snapshot_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot_compat_values_with_host(bytecode, host_services)
    }

    /// Compatibility host-backed snapshot boundary.
    pub fn execute_and_snapshot_compat_values_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        project_variants_to_compat_values(
            self.execute_and_snapshot_variants_with_host(bytecode, host_services)?,
        )
    }

    /// Retained value-model host-backed snapshot API.
    pub fn execute_and_snapshot_variants_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<Variant>, JitError> {
        // Try the RtSlot path first (supports more instructions).
        // On failure, fall back to VM for proper error handling with detailed messages.
        if cranelift::supports_bytecode_rtslot(bytecode) {
            match cranelift::execute_bytecode_rtslot_variants(bytecode, host_services.clone()) {
                Ok(values) => return Ok(values),
                Err(_) => {
                    return execute_and_snapshot_variants_with_host(bytecode, host_services)
                        .map_err(JitError::Execution);
                }
            }
        }
        // Fall back to legacy i32 path for the original 23-instruction subset.
        if cranelift::supports_bytecode(bytecode) {
            return cranelift::execute_bytecode_variants(bytecode).map_err(JitError::Execution);
        }
        // Fall back to VM interpreter for unsupported bytecode.
        execute_and_snapshot_variants_with_host(bytecode, host_services)
            .map_err(JitError::Execution)
    }

    /// Legacy host-backed snapshot alias. Prefer
    /// `execute_and_snapshot_variants_with_host`.
    pub fn execute_and_snapshot_values_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, JitError> {
        self.execute_and_snapshot_compat_values_with_host(bytecode, host_services)
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
        F64Value, RuntimeValue, VarType, Variant,
        bstr::BStr,
        safe_array::{SafeArray, SafeArrayBound, VT_UI1_VALUE},
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

    #[test]
    fn execute_and_snapshot_variants_exposes_jit_results_before_projection() {
        let bytecode = oxvba_compiler::compile("Sub Main()\nDim x\nx = \"ABC\"\nEnd Sub")
            .expect("compile should succeed");
        let out = JitEngine
            .execute_and_snapshot_variants(&bytecode)
            .expect("variant snapshot should succeed");

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].vtype(), VarType::String);
        assert_eq!(out[0].as_bstr(), Some(BStr::from("ABC")));
    }

    #[test]
    fn execute_and_snapshot_compat_values_projects_variant_results() {
        let bytecode = oxvba_compiler::compile("Sub Main()\nDim x\nx = \"ABC\"\nEnd Sub")
            .expect("compile should succeed");
        let variants = JitEngine
            .execute_and_snapshot_variants(&bytecode)
            .expect("variant snapshot should succeed");
        let compat = JitEngine
            .execute_and_snapshot_compat_values(&bytecode)
            .expect("compat snapshot should succeed");

        assert_eq!(compat, vec![RuntimeValue::String(BStr::from("ABC"))]);
        assert_eq!(
            compat,
            variants
                .into_iter()
                .map(|value| value.to_runtime_value().expect("variant projection"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn execute_bytecode_variants_legacy_subset_reads_compat_slots_directly() {
        let bytecode = oxvba_compiler::compile("Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub")
            .expect("compile should succeed");
        assert!(cranelift::supports_bytecode(&bytecode));

        let out = cranelift::execute_bytecode_variants(&bytecode)
            .expect("legacy variant execution should succeed");

        assert_eq!(out, vec![Variant::from_compat_slot_i32(3)]);
        assert_eq!(out[0].as_i32(), Some(3));
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
    fn runtime_array_resize_paths_preserve_variant_slot_carriers() {
        let mut ctx = JitContextOwned::new(2, 2, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_slot(1, RuntimeValue::I32(2));
        }

        let resized_upper_bounds = [1u32];
        let resized_lower_bounds = [0i32];
        let rc = runtime_helpers::oxrt_array_resize(
            ctx.context_ptr(),
            0,
            resized_upper_bounds.as_ptr(),
            resized_lower_bounds.as_ptr(),
            1,
            RuntimeArrayElementType::Byte as i32,
        );
        assert_eq!(rc, 0);

        let resized = unsafe { ctx.context.read_variant_slot(0) };
        let resized_array = resized
            .as_safearray()
            .expect("array resize should keep a SAFEARRAY-backed variant carrier");
        assert_eq!(resized_array.element_vartype(), VT_UI1_VALUE);
        assert_eq!(
            resized_array.elements().as_deref(),
            Some(
                &[
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                ][..]
            )
        );

        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_safearray(
                    SafeArray::from_typed_values_nd(
                        vec![SafeArrayBound { lower: 0, count: 2 }],
                        VT_UI1_VALUE,
                        vec![RuntimeValue::I32(90), RuntimeValue::I32(91)],
                    )
                    .expect("byte SAFEARRAY setup should succeed"),
                ),
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

        let preserved = unsafe { ctx.context.read_variant_slot(0) };
        let preserved_array = preserved
            .as_safearray()
            .expect("array resize preserve should keep a SAFEARRAY-backed variant carrier");
        assert_eq!(preserved_array.element_vartype(), VT_UI1_VALUE);
        assert_eq!(
            preserved_array.elements().as_deref(),
            Some(
                &[
                    RuntimeValue::I32(90),
                    RuntimeValue::I32(91),
                    RuntimeValue::I32(0),
                    RuntimeValue::I32(0),
                ][..]
            )
        );
    }

    #[test]
    fn runtime_lbound_and_ubound_read_variant_array_carriers() {
        let mut ctx = JitContextOwned::new(3, 3, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_safearray(
                    SafeArray::from_typed_values_nd(
                        vec![SafeArrayBound { lower: 2, count: 4 }],
                        VT_UI1_VALUE,
                        vec![
                            RuntimeValue::I32(10),
                            RuntimeValue::I32(20),
                            RuntimeValue::I32(30),
                            RuntimeValue::I32(40),
                        ],
                    )
                    .expect("byte SAFEARRAY setup should succeed"),
                ),
            );
        }

        assert_eq!(runtime_helpers::oxrt_lbound(ctx.context_ptr(), 1, 0), 0);
        assert_eq!(runtime_helpers::oxrt_ubound(ctx.context_ptr(), 2, 0), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[1], RuntimeValue::I32(2));
        assert_eq!(values[2], RuntimeValue::I32(5));
    }

    #[test]
    fn runtime_string_slice_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(7, 7, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("hello")));
            ctx.context.write_variant_slot(1, Variant::from_i32(2));
            ctx.context.write_variant_slot(2, Variant::from_i32(3));
        }

        assert_eq!(runtime_helpers::oxrt_len(ctx.context_ptr(), 3, 0), 0);
        assert_eq!(runtime_helpers::oxrt_left(ctx.context_ptr(), 4, 0, 1), 0);
        assert_eq!(runtime_helpers::oxrt_right(ctx.context_ptr(), 5, 0, 2), 0);
        assert_eq!(runtime_helpers::oxrt_mid(ctx.context_ptr(), 6, 0, 1, 2), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[3], RuntimeValue::I32(5));
        assert_eq!(values[4], RuntimeValue::String(BStr::from("he")));
        assert_eq!(values[5], RuntimeValue::String(BStr::from("llo")));
        assert_eq!(values[6], RuntimeValue::String(BStr::from("ell")));
    }

    #[test]
    fn runtime_text_transform_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(15, 15, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("Hello World")));
            ctx.context
                .write_variant_slot(1, Variant::from_string(BStr::from("lo")));
            ctx.context
                .write_variant_slot(2, Variant::from_string(BStr::from("LO")));
            ctx.context
                .write_variant_slot(3, Variant::from_string(BStr::from("  spaced  ")));
            ctx.context
                .write_variant_slot(4, Variant::from_string(BStr::from("abc")));
            ctx.context
                .write_variant_slot(5, Variant::from_string(BStr::from("ABC")));
        }

        assert_eq!(runtime_helpers::oxrt_instr(ctx.context_ptr(), 6, 0, 1, 0), 0);
        assert_eq!(
            runtime_helpers::oxrt_instrrev(ctx.context_ptr(), 7, 0, 1, 0),
            0
        );
        assert_eq!(runtime_helpers::oxrt_lower(ctx.context_ptr(), 8, 5), 0);
        assert_eq!(runtime_helpers::oxrt_upper(ctx.context_ptr(), 9, 4), 0);
        assert_eq!(
            runtime_helpers::oxrt_replace(ctx.context_ptr(), 10, 0, 1, 2),
            0
        );
        assert_eq!(runtime_helpers::oxrt_trim(ctx.context_ptr(), 11, 3), 0);
        assert_eq!(
            runtime_helpers::oxrt_strcomp(ctx.context_ptr(), 12, 4, 5, 1),
            0
        );
        assert_eq!(runtime_helpers::oxrt_ltrim(ctx.context_ptr(), 13, 3), 0);
        assert_eq!(runtime_helpers::oxrt_rtrim(ctx.context_ptr(), 14, 3), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[6], RuntimeValue::I32(4));
        assert_eq!(values[7], RuntimeValue::I32(4));
        assert_eq!(values[8], RuntimeValue::String(BStr::from("abc")));
        assert_eq!(values[9], RuntimeValue::String(BStr::from("ABC")));
        assert_eq!(
            values[10],
            RuntimeValue::String(BStr::from("HelLO World"))
        );
        assert_eq!(values[11], RuntimeValue::String(BStr::from("spaced")));
        assert_eq!(values[12], RuntimeValue::I32(0));
        assert_eq!(values[13], RuntimeValue::String(BStr::from("spaced  ")));
        assert_eq!(values[14], RuntimeValue::String(BStr::from("  spaced")));
    }

    #[test]
    fn runtime_char_format_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(14, 14, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(0, Variant::from_i32(65));
            ctx.context
                .write_variant_slot(1, Variant::from_string(BStr::from("123")));
            ctx.context.write_variant_slot(2, Variant::from_i32(3));
            ctx.context
                .write_variant_slot(3, Variant::from_string(BStr::from("Z")));
            ctx.context.write_variant_slot(4, Variant::from_i32(255));
            ctx.context.write_variant_slot(5, Variant::from_i32(8));
            ctx.context
                .write_variant_slot(6, Variant::from_string(BStr::from("3")));
        }

        assert_eq!(runtime_helpers::oxrt_chr(ctx.context_ptr(), 7, 0), 0);
        assert_eq!(runtime_helpers::oxrt_asc(ctx.context_ptr(), 8, 1), 0);
        assert_eq!(runtime_helpers::oxrt_space(ctx.context_ptr(), 9, 2), 0);
        assert_eq!(
            runtime_helpers::oxrt_string_repeat(ctx.context_ptr(), 10, 2, 3),
            0
        );
        assert_eq!(runtime_helpers::oxrt_hex(ctx.context_ptr(), 11, 4), 0);
        assert_eq!(runtime_helpers::oxrt_oct(ctx.context_ptr(), 12, 5), 0);
        assert_eq!(runtime_helpers::oxrt_month_name(ctx.context_ptr(), 13, 6), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[7], RuntimeValue::String(BStr::from("A")));
        assert_eq!(values[8], RuntimeValue::I32('1' as i32));
        assert_eq!(values[9], RuntimeValue::String(BStr::from("   ")));
        assert_eq!(values[10], RuntimeValue::String(BStr::from("ZZZ")));
        assert_eq!(values[11], RuntimeValue::String(BStr::from("FF")));
        assert_eq!(values[12], RuntimeValue::String(BStr::from("10")));
        assert_eq!(values[13], RuntimeValue::String(BStr::from("March")));
    }

    #[test]
    fn runtime_format_helper_reads_variant_carriers() {
        let mut ctx = JitContextOwned::new(3, 3, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_f64(std::f64::consts::PI));
            ctx.context
                .write_variant_slot(1, Variant::from_string(BStr::from("0.00")));
        }

        assert_eq!(runtime_helpers::oxrt_format(ctx.context_ptr(), 2, 0, 1), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[2], RuntimeValue::String(BStr::from("3.14")));
    }

    #[test]
    fn runtime_date_time_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(13, 13, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(0, Variant::from_i32(2026));
            ctx.context.write_variant_slot(1, Variant::from_i32(2));
            ctx.context.write_variant_slot(2, Variant::from_i32(28));
            ctx.context.write_variant_slot(3, Variant::from_i32(1));
            ctx.context.write_variant_slot(4, Variant::from_i32(2));
            ctx.context.write_variant_slot(5, Variant::from_i32(3));
            ctx.context.write_variant_slot(6, Variant::from_i32(1));
            ctx.context.write_variant_slot(7, Variant::from_i32(3));
        }

        assert_eq!(
            runtime_helpers::oxrt_date_serial(ctx.context_ptr(), 8, 0, 1, 2),
            0
        );
        assert_eq!(
            runtime_helpers::oxrt_time_serial(ctx.context_ptr(), 9, 3, 4, 5),
            0
        );
        assert_eq!(
            runtime_helpers::oxrt_date_add(ctx.context_ptr(), 10, 6, 7, 8),
            0
        );
        assert_eq!(
            runtime_helpers::oxrt_date_diff(ctx.context_ptr(), 11, 6, 8, 10),
            0
        );
        assert_eq!(runtime_helpers::oxrt_year(ctx.context_ptr(), 12, 8), 0);

        let values = ctx.extract_user_values();
        assert_eq!(
            values[8],
            RuntimeValue::F64(oxvba_runtime::F64Value::from_date_f64(46081.0))
        );
        assert_eq!(
            values[9],
            RuntimeValue::F64(oxvba_runtime::F64Value::from_date_f64(3723.0 / 86400.0))
        );
        assert_eq!(
            values[10],
            RuntimeValue::F64(oxvba_runtime::F64Value::from_date_f64(46084.0))
        );
        assert_eq!(values[11], RuntimeValue::I32(3));
        assert_eq!(values[12], RuntimeValue::I32(2026));
    }

    #[test]
    fn runtime_math_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(17, 17, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("-7")));
            ctx.context.write_variant_slot(1, Variant::from_i32(-9));
            ctx.context
                .write_variant_slot(2, Variant::from_string(BStr::from("19")));
            ctx.context
                .write_variant_slot(3, Variant::from_string(BStr::from("-1")));
            ctx.context.write_variant_slot(4, Variant::from_i32(81));
            ctx.context.write_variant_slot(5, Variant::from_i32(0));
            ctx.context.write_variant_slot(6, Variant::from_i32(1));
        }

        assert_eq!(runtime_helpers::oxrt_abs(ctx.context_ptr(), 7, 0), 0);
        assert_eq!(runtime_helpers::oxrt_sgn(ctx.context_ptr(), 8, 1), 0);
        assert_eq!(runtime_helpers::oxrt_round(ctx.context_ptr(), 9, 2, 3), 0);
        assert_eq!(runtime_helpers::oxrt_sqr(ctx.context_ptr(), 10, 4), 0);
        assert_eq!(runtime_helpers::oxrt_sin(ctx.context_ptr(), 11, 5), 0);
        assert_eq!(runtime_helpers::oxrt_cos(ctx.context_ptr(), 12, 5), 0);
        assert_eq!(runtime_helpers::oxrt_log(ctx.context_ptr(), 13, 6), 0);
        assert_eq!(runtime_helpers::oxrt_exp(ctx.context_ptr(), 14, 5), 0);
        assert_eq!(runtime_helpers::oxrt_atn(ctx.context_ptr(), 15, 6), 0);
        assert_eq!(runtime_helpers::oxrt_tan(ctx.context_ptr(), 16, 6), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[7], RuntimeValue::I32(7));
        assert_eq!(values[8], RuntimeValue::I32(-1));
        assert_eq!(values[9], RuntimeValue::I32(20));
        assert_eq!(values[10], RuntimeValue::I32(9));
        assert_eq!(values[11], RuntimeValue::I32(0));
        assert_eq!(values[12], RuntimeValue::I32(1));
        assert_eq!(values[13], RuntimeValue::I32(0));
        assert_eq!(values[14], RuntimeValue::I32(1));
        assert_eq!(values[15], RuntimeValue::I32(1));
        assert_eq!(values[16], RuntimeValue::I32(2));
    }

    #[test]
    fn runtime_like_and_strconv_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(6, 6, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("ABC")));
            ctx.context
                .write_variant_slot(1, Variant::from_string(BStr::from("abc")));
            ctx.context
                .write_variant_slot(2, Variant::from_string(BStr::from("mixed words")));
            ctx.context.write_variant_slot(3, Variant::from_i32(3));
        }

        assert_eq!(runtime_helpers::oxrt_like(ctx.context_ptr(), 4, 0, 1, 1), 0);
        assert_eq!(runtime_helpers::oxrt_strconv(ctx.context_ptr(), 5, 2, 3), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[4], RuntimeValue::I32(-1));
        assert_eq!(values[5], RuntimeValue::String(BStr::from("Mixed Words")));
    }

    #[test]
    fn runtime_aggregate_string_helpers_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(9, 9, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("ABCDE")));
            ctx.context.write_variant_slot(1, Variant::from_i32(2));
            ctx.context.write_variant_slot(2, Variant::from_i32(2));
            ctx.context
                .write_variant_slot(3, Variant::from_string(BStr::from("99")));
            ctx.context.write_variant_slot(4, Variant::from_i32(123231));
            ctx.context.write_variant_slot(5, Variant::from_i32(23));
            ctx.context.write_variant_slot(
                6,
                Variant::from_safearray(SafeArray::from_values(vec![
                    RuntimeValue::I32(1),
                    RuntimeValue::I32(2),
                    RuntimeValue::I32(3),
                ])),
            );
            ctx.context.write_variant_slot(7, Variant::from_i32(0));
        }

        assert_eq!(runtime_helpers::oxrt_mid_stmt(ctx.context_ptr(), 0, 1, 2, 3), 0);
        assert_eq!(runtime_helpers::oxrt_split(ctx.context_ptr(), 8, 4, 5), 0);
        assert_eq!(runtime_helpers::oxrt_join(ctx.context_ptr(), 7, 6, 7), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[0], RuntimeValue::String(BStr::from("A99DE")));
        assert_eq!(values[8], RuntimeValue::I32(3));
        assert_eq!(values[7], RuntimeValue::I32(3));
    }

    #[test]
    fn runtime_tag_classifiers_read_variant_array_carriers() {
        let mut ctx = JitContextOwned::new(4, 4, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_safearray(SafeArray::from_values(vec![
                    RuntimeValue::I32(1),
                    RuntimeValue::I32(2),
                ])),
            );
        }

        assert_eq!(
            runtime_helpers::oxrt_vartype_tag(ctx.context_ptr(), 1, 0),
            0
        );
        assert_eq!(
            runtime_helpers::oxrt_typename_tag(ctx.context_ptr(), 2, 0),
            0
        );
        assert_eq!(
            runtime_helpers::oxrt_is_numeric_tag(ctx.context_ptr(), 3, 0),
            0
        );

        let values = ctx.extract_user_values();
        assert_eq!(values[1], RuntimeValue::I32(8204));
        assert_eq!(values[2], RuntimeValue::I32(1001));
        assert_eq!(values[3], RuntimeValue::I32(0));
    }

    #[test]
    fn runtime_simple_predicates_read_variant_carriers() {
        let mut ctx = JitContextOwned::new(11, 11, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_safearray(SafeArray::from_values(vec![
                    RuntimeValue::I32(1),
                    RuntimeValue::I32(2),
                ])),
            );
            ctx.context
                .write_variant_slot(1, Variant::from_error_code(9));
            ctx.context.write_variant_slot(2, Variant::null());
            ctx.context.write_variant_slot(3, Variant::empty());
            ctx.context
                .write_variant_slot(4, Variant::from_date_f64(42.0));
            ctx.context
                .write_variant_slot(5, Variant::from_string(BStr::from("abc")));
        }

        assert_eq!(
            runtime_helpers::oxrt_is_array_tag(ctx.context_ptr(), 6, 0),
            0
        );
        assert_eq!(runtime_helpers::oxrt_is_error(ctx.context_ptr(), 7, 1), 0);
        assert_eq!(runtime_helpers::oxrt_is_null(ctx.context_ptr(), 8, 2), 0);
        assert_eq!(runtime_helpers::oxrt_is_empty(ctx.context_ptr(), 9, 3), 0);
        assert_eq!(
            runtime_helpers::oxrt_is_numeric(ctx.context_ptr(), 10, 4),
            0
        );
        assert_eq!(runtime_helpers::oxrt_is_numeric(ctx.context_ptr(), 5, 5), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[6], RuntimeValue::I32(1));
        assert_eq!(values[7], RuntimeValue::Bool(true));
        assert_eq!(values[8], RuntimeValue::Bool(true));
        assert_eq!(values[9], RuntimeValue::Bool(true));
        assert_eq!(values[10], RuntimeValue::Bool(true));
        assert_eq!(values[5], RuntimeValue::Bool(false));
    }

    #[test]
    fn runtime_vartype_reads_variant_carriers_with_existing_compat_heuristics() {
        let mut ctx = JitContextOwned::new(14, 14, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(0, Variant::from_i16(7));
            ctx.context.write_variant_slot(1, Variant::from_i32(7));
            ctx.context.write_variant_slot(2, Variant::from_i32(40000));
            ctx.context.write_variant_slot(3, Variant::from_i64(9));
            ctx.context
                .write_variant_slot(4, Variant::from_date_f64(42.0));
            ctx.context
                .write_variant_slot(5, Variant::from_error_code(9));
            ctx.context.write_variant_slot(
                6,
                Variant::from_safearray(SafeArray::from_values(vec![
                    RuntimeValue::I32(1),
                    RuntimeValue::I32(2),
                ])),
            );
        }

        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 7, 0), 0);
        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 8, 1), 0);
        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 9, 2), 0);
        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 10, 3), 0);
        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 11, 4), 0);
        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 12, 5), 0);
        assert_eq!(runtime_helpers::oxrt_vartype(ctx.context_ptr(), 13, 6), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[7], RuntimeValue::I32(2));
        assert_eq!(values[8], RuntimeValue::I32(2));
        assert_eq!(values[9], RuntimeValue::I32(3));
        assert_eq!(values[10], RuntimeValue::I32(3));
        assert_eq!(values[11], RuntimeValue::I32(7));
        assert_eq!(values[12], RuntimeValue::I32(10));
        assert_eq!(values[13], RuntimeValue::I32(8204));
    }

    #[test]
    fn runtime_is_date_tag_reads_variant_carriers_with_existing_compat_heuristics() {
        let mut ctx = JitContextOwned::new(18, 18, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("2024-01-31")));
            ctx.context.write_variant_slot(1, Variant::from_i32(20240131));
            ctx.context.write_variant_slot(2, Variant::from_i64(20240131));
            ctx.context.write_variant_slot(3, Variant::from_f64(42.0));
            ctx.context
                .write_variant_slot(4, Variant::from_currency_scaled_i64(420_000));
            ctx.context.write_variant_slot(5, Variant::from_u8(7));
            ctx.context
                .write_variant_slot(6, Variant::from_date_f64(42.0));
            ctx.context
                .write_variant_slot(7, Variant::from_string(BStr::from("abc")));
            ctx.context.write_variant_slot(8, Variant::from_bool(true));
        }

        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 9, 0), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 10, 1), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 11, 2), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 12, 3), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 13, 4), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 14, 5), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 15, 6), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 16, 7), 0);
        assert_eq!(runtime_helpers::oxrt_is_date_tag(ctx.context_ptr(), 17, 8), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[9], RuntimeValue::I32(1));
        assert_eq!(values[10], RuntimeValue::I32(1));
        assert_eq!(values[11], RuntimeValue::I32(1));
        assert_eq!(values[12], RuntimeValue::I32(1));
        assert_eq!(values[13], RuntimeValue::I32(1));
        assert_eq!(values[14], RuntimeValue::I32(1));
        assert_eq!(values[15], RuntimeValue::I32(1));
        assert_eq!(values[16], RuntimeValue::I32(0));
        assert_eq!(values[17], RuntimeValue::I32(0));
    }

    #[test]
    fn runtime_is_object_tag_reads_variant_object_carriers() {
        let mut ctx = JitContextOwned::new(6, 6, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_object_ref(oxvba_runtime::ObjectRef::from_compat_identity(42)),
            );
            ctx.context.write_variant_slot(1, Variant::empty());
            ctx.context.write_variant_slot(2, Variant::from_i32(7));
        }

        assert_eq!(runtime_helpers::oxrt_is_object_tag(ctx.context_ptr(), 3, 0), 0);
        assert_eq!(runtime_helpers::oxrt_is_object_tag(ctx.context_ptr(), 4, 1), 0);
        assert_eq!(runtime_helpers::oxrt_is_object_tag(ctx.context_ptr(), 5, 2), 0);

        let values = ctx.extract_user_values();
        assert_eq!(values[3], RuntimeValue::I32(1));
        assert_eq!(values[4], RuntimeValue::I32(0));
        assert_eq!(values[5], RuntimeValue::I32(0));
    }

    #[test]
    fn runtime_array_literal_and_append_preserve_variant_slot_carriers() {
        let mut ctx = JitContextOwned::new(5, 5, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_variant_slot(0, Variant::from_string(BStr::from("A")));
            ctx.context.write_variant_slot(1, Variant::from_i32(7));
            ctx.context
                .write_variant_slot(3, Variant::from_string(BStr::from("B")));
        }

        let items = [0u32, 1u32];
        assert_eq!(
            runtime_helpers::oxrt_array_literal(ctx.context_ptr(), 2, items.as_ptr(), 2),
            0
        );
        let literal = unsafe { ctx.context.read_variant_slot(2) };
        let literal_elements = literal
            .as_safearray()
            .expect("literal should produce SAFEARRAY")
            .variant_elements()
            .expect("SAFEARRAY should expose variant elements");
        assert_eq!(
            literal_elements,
            vec![Variant::from_string(BStr::from("A")), Variant::from_i32(7)]
        );

        assert_eq!(
            runtime_helpers::oxrt_array_append(ctx.context_ptr(), 4, 2, 3),
            0
        );
        let appended = unsafe { ctx.context.read_variant_slot(4) };
        let appended_elements = appended
            .as_safearray()
            .expect("append should preserve SAFEARRAY carrier")
            .variant_elements()
            .expect("SAFEARRAY should expose variant elements");
        assert_eq!(
            appended_elements,
            vec![
                Variant::from_string(BStr::from("A")),
                Variant::from_i32(7),
                Variant::from_string(BStr::from("B"))
            ]
        );
        assert_eq!(
            unsafe { ctx.context.read_slot(4) },
            RuntimeValue::ArrayIntent(SafeArray::from_variants(appended_elements))
        );
    }

    #[test]
    fn runtime_array_get_and_set_preserve_variant_slot_carriers() {
        let mut ctx = JitContextOwned::new(5, 5, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_safearray(SafeArray::from_variants(vec![
                    Variant::from_string(BStr::from("A")),
                    Variant::from_i32(7),
                ])),
            );
            ctx.context.write_slot(1, RuntimeValue::I32(1));
            ctx.context
                .write_variant_slot(3, Variant::from_string(BStr::from("B")));
        }

        let index = [1u32];
        assert_eq!(
            runtime_helpers::oxrt_array_get(ctx.context_ptr(), 2, 0, index.as_ptr(), 1),
            0
        );
        assert_eq!(
            unsafe { ctx.context.read_variant_slot(2) },
            Variant::from_i32(7)
        );
        unsafe {
            ctx.context.write_slot(4, RuntimeValue::I32(0));
        }

        let first_index = [4u32];
        assert_eq!(
            runtime_helpers::oxrt_array_set(ctx.context_ptr(), 0, first_index.as_ptr(), 1, 3),
            0
        );
        let updated = unsafe { ctx.context.read_variant_slot(0) };
        let elements = updated
            .as_safearray()
            .expect("updated value should stay a SAFEARRAY variant")
            .variant_elements()
            .expect("SAFEARRAY should expose variant elements");
        assert_eq!(
            elements,
            vec![Variant::from_string(BStr::from("B")), Variant::from_i32(7)]
        );
    }

    #[test]
    fn runtime_varptr_uses_variant_array_carrier_directly() {
        let mut ctx = JitContextOwned::new(3, 3, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_variant_slot(
                0,
                Variant::from_safearray(SafeArray::from_variants(vec![
                    Variant::from_u8(1),
                    Variant::from_u8(2),
                    Variant::from_u8(3),
                ])),
            );
        }

        assert_eq!(runtime_helpers::oxrt_varptr(ctx.context_ptr(), 1, 0), 0);
        let RuntimeValue::I64(pointer) = (unsafe { ctx.context.read_slot(1) }) else {
            panic!("VarPtr should return LongPtr carrier");
        };
        assert_ne!(pointer, 0);
        let read_back =
            oxvba_runtime::pointer_helpers::read_back_byte_array_payload_variant(pointer)
                .expect("pointer helper should read back byte-array payload");
        let elements = read_back
            .as_safearray()
            .expect("VarPtr(array) should preserve SAFEARRAY payload")
            .variant_elements()
            .expect("SAFEARRAY should expose variant elements");
        assert_eq!(
            elements,
            vec![
                Variant::from_u8(1),
                Variant::from_u8(2),
                Variant::from_u8(3)
            ]
        );
    }

    #[test]
    fn jit_context_extracts_user_variants_before_projection() {
        let mut ctx = JitContextOwned::new(2, 2, super::default_host_services(), &[]);
        unsafe {
            ctx.context.write_slot(0, RuntimeValue::I32(42));
            ctx.context
                .write_slot(1, RuntimeValue::String(BStr::from("ABC")));
        }

        let variants = ctx.extract_user_variants();
        assert_eq!(variants[0].vtype(), VarType::Long);
        assert_eq!(variants[0].as_i32(), Some(42));
        assert_eq!(variants[1].vtype(), VarType::String);
        assert_eq!(
            variants[1].to_runtime_value().expect("string variant"),
            RuntimeValue::String(BStr::from("ABC"))
        );
    }

    #[test]
    fn jit_context_runtime_value_slot_api_projects_through_variant_accessors() {
        let mut ctx = JitContextOwned::new(2, 2, super::default_host_services(), &[]);
        unsafe {
            ctx.context
                .write_slot(0, RuntimeValue::BindingHandle(7.into()));
            assert_eq!(ctx.context.read_variant_slot(0).as_i32(), Some(7));
            assert_eq!(ctx.context.read_slot(0), RuntimeValue::I32(7));

            ctx.context
                .write_variant_slot(1, Variant::from_string(BStr::from("ABC")));
            assert_eq!(
                ctx.context.read_slot(1),
                RuntimeValue::String(BStr::from("ABC"))
            );
        }
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
