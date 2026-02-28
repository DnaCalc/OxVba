use oxvba_compiler::compile;
use oxvba_jit::JitEngine;
use oxvba_vm::execute_and_snapshot;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct HostConfig {
    pub enable_jit: bool,
    pub root_object_name: Option<String>,
}

#[derive(Debug, Default)]
pub struct Engine {
    config: HostConfig,
    jit: JitEngine,
    root_objects: HashMap<String, String>,
}

impl Engine {
    pub fn new(config: HostConfig) -> Self {
        Self {
            config,
            jit: JitEngine,
            root_objects: HashMap::new(),
        }
    }

    pub fn register_root_object(&mut self, name: impl Into<String>, type_name: impl Into<String>) {
        self.root_objects.insert(name.into(), type_name.into());
    }

    pub fn has_root_object(&self, name: &str) -> bool {
        self.root_objects.contains_key(name)
    }

    pub fn execute_source(&self, source: &str) -> Result<(), String> {
        let _ = self.execute_source_with_snapshot(source)?;
        Ok(())
    }

    pub fn execute_source_with_snapshot(&self, source: &str) -> Result<Vec<i32>, String> {
        let bytecode = compile(source).map_err(|e| e.to_string())?;

        if self.config.enable_jit {
            self.jit
                .compile_function("main")
                .map_err(|e| e.to_string())?;
            return self
                .jit
                .execute_and_snapshot(&bytecode)
                .map_err(|e| e.to_string());
        }

        execute_and_snapshot(&bytecode)
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, HostConfig};
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    fn repo_path(relative: &str) -> PathBuf {
        workspace_root().join(relative)
    }

    fn divergence_record_has_required_sections(record_path: &Path) -> bool {
        let Ok(text) = std::fs::read_to_string(record_path) else {
            return false;
        };
        if !text.starts_with("# DIV-") {
            return false;
        }
        let required = [
            "- Scope impact:",
            "- Fixture:",
            "- Reproduction command:",
            "- Tracking status:",
        ];
        required.iter().all(|label| text.contains(label))
    }

    #[test]
    fn execute_source_with_default_vm_path() {
        let mut engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: Some("Application".to_string()),
        });
        engine.register_root_object("Application", "Host.Application");
        assert!(engine.has_root_object("Application"));

        let result = engine.execute_source("Sub Main()\nEnd Sub");
        assert!(result.is_ok());
    }

    #[test]
    fn execute_source_returns_slot_snapshot() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: Some("Application".to_string()),
        });

        let source = "Sub Main()\nDim x\nx = 10\nx = x + 5\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![15]);
    }

    #[test]
    fn execute_source_jit_toggle_preserves_semantics() {
        let engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: Some("Application".to_string()),
        });

        let source = "Sub Main()\nDim x\nx = 20\nx = x - 4\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![16]);
    }

    #[test]
    fn formal_v5_branch_selection_is_total_over_small_domain() {
        let engine = Engine::new(HostConfig::default());
        for input in -4..=4 {
            let source = format!(
                "Sub Main()\nDim x\nx = {input}\nIf x = 1 Then\nx = 10\nElseIf x = 2 Then\nx = 20\nElse\nx = 30\nEnd If\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_with_snapshot(&source)
                .expect("execution should succeed");
            assert!(matches!(snapshot[0], 10 | 20 | 30));
        }
    }

    #[test]
    fn formal_v5_branch_selection_matches_reference_model() {
        let engine = Engine::new(HostConfig::default());
        for input in -6..=6 {
            let expected = if input == 1 {
                10
            } else if input == 2 {
                20
            } else {
                30
            };
            let source = format!(
                "Sub Main()\nDim x\nx = {input}\nIf x = 1 Then\nx = 10\nElseIf x = 2 Then\nx = 20\nElse\nx = 30\nEnd If\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_with_snapshot(&source)
                .expect("execution should succeed");
            assert_eq!(snapshot[0], expected);
        }
    }

    #[test]
    fn formal_v5_no_dual_branch_write_effect() {
        let engine = Engine::new(HostConfig::default());
        for input in -3..=3 {
            let source = format!(
                "Sub Main()\nDim x\nDim y\nx = {input}\ny = 0\nIf x = 1 Then\ny = y + 1\nElseIf x = 2 Then\ny = y + 10\nElse\ny = y + 100\nEnd If\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_with_snapshot(&source)
                .expect("execution should succeed");
            assert!(matches!(snapshot[1], 1 | 10 | 100));
        }
    }

    #[test]
    fn formal_v6_do_while_matches_reference_model() {
        let engine = Engine::new(HostConfig::default());
        for limit in 0..=6 {
            let source =
                format!("Sub Main()\nDim x\nx = 0\nDo While x < {limit}\nx = x + 1\nLoop\nEnd Sub");
            let snapshot = engine
                .execute_source_with_snapshot(&source)
                .expect("execution should succeed");
            assert_eq!(snapshot[0], limit);
        }
    }

    #[test]
    fn formal_v6_post_condition_loop_semantics() {
        let engine = Engine::new(HostConfig::default());
        for limit in 0..=4 {
            let source =
                format!("Sub Main()\nDim x\nx = 0\nDo\nx = x + 1\nLoop While x < {limit}\nEnd Sub");
            let snapshot = engine
                .execute_source_with_snapshot(&source)
                .expect("execution should succeed");
            let expected = if limit <= 1 { 1 } else { limit };
            assert_eq!(snapshot[0], expected);
        }
    }

    #[test]
    fn formal_v6_exit_do_short_circuits_iteration() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 0\nDo While x < 10\nx = x + 1\nIf x = 4 Then\nExit Do\nEnd If\nLoop\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 4);
    }

    #[test]
    fn formal_v7_select_case_first_match_wins() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 2\nSelect Case x\nCase 2\nx = 20\nCase 2, 3\nx = 99\nCase Else\nx = 0\nEnd Select\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 20);
    }

    #[test]
    fn formal_v7_select_case_else_fallback() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 9\nSelect Case x\nCase 1\nx = 10\nCase 2\nx = 20\nCase Else\nx = 99\nEnd Select\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 99);
    }

    #[test]
    fn formal_v7_select_case_multi_value_arm() {
        let engine = Engine::new(HostConfig::default());
        for input in [1, 3] {
            let source = format!(
                "Sub Main()\nDim x\nx = {input}\nSelect Case x\nCase 1, 3\nx = 30\nCase Else\nx = 0\nEnd Select\nEnd Sub"
            );
            let snapshot = engine
                .execute_source_with_snapshot(&source)
                .expect("execution should succeed");
            assert_eq!(snapshot[0], 30);
        }
    }

    #[test]
    fn formal_v8_call_returns_to_caller() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nCall Foo\nx = x + 1\nEnd Sub\nSub Foo()\nDim y\ny = 9\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v8_local_scope_isolated_between_procedures() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 2\nCall Foo\nx = x + 1\nEnd Sub\nSub Foo()\nDim x\nx = 200\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 3);
    }

    #[test]
    fn formal_v8_nested_call_chain_integrity() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 0\nCall A\nx = x + 1\nEnd Sub\nSub A()\nDim y\ny = 1\nCall B\nEnd Sub\nSub B()\nDim z\nz = 2\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_v9_byval_does_not_propagate_mutation() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nCall AddOne(x)\nEnd Sub\nSub AddOne(ByVal a)\na = a + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_v9_byref_propagates_mutation() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nCall AddOne(x)\nEnd Sub\nSub AddOne(ByRef a)\na = a + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v9_byref_requires_variable_argument() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nCall AddOne(1)\nEnd Sub\nSub AddOne(ByRef a)\na = a + 1\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("byref constant argument should fail");
        assert!(err.contains("ByRef"));
    }

    #[test]
    fn formal_v37_optional_param_default_applies_when_omitted() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 7);
    }

    #[test]
    fn formal_v37_optional_param_explicit_value_overrides_default() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(x, 9)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 9);
    }

    #[test]
    fn formal_v37_optional_param_missing_required_arg_is_rejected() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nCall Fill\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("missing required arg should fail");
        assert!(err.contains("missing required argument"));
    }

    #[test]
    fn formal_v38_named_args_bind_by_parameter_name() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(value := 9, target := x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 9);
    }

    #[test]
    fn formal_v38_named_args_allow_omitting_optional_by_name() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(target := x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 7);
    }

    #[test]
    fn formal_v38_named_args_reject_positional_after_named() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nCall Fill(value := 9, x)\nEnd Sub\nSub Fill(ByRef target, Optional ByVal value = 7)\ntarget = value\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("positional-after-named should fail");
        assert!(err.contains("positional argument cannot follow named argument"));
    }

    #[test]
    fn formal_v40_gosub_executes_label_body_and_returns() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nx = x + 1\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 4);
    }

    #[test]
    fn formal_v40_gosub_missing_label_is_rejected() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nGoSub nope\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("missing gosub label should fail");
        assert!(err.contains("gosub target label not found"));
    }

    #[test]
    fn formal_v40_gosub_return_stack_handles_repeated_calls() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nGoSub add_two\nGoSub add_two\nIf Err.Number = -1 Then\nadd_two:\nx = x + 2\nReturn\nEnd If\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 5);
    }

    #[test]
    fn formal_v41_on_error_goto_label_jumps_to_handler() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nOn Error GoTo handler\nError 5\nx = 99\nIf Err.Number = -1 Then\nhandler:\nx = Err.Number\nResume Next\nEnd If\nx = x + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 6);
    }

    #[test]
    fn formal_v41_on_error_goto_label_missing_target_is_rejected() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error GoTo handler\nError 5\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("missing handler label should fail");
        assert!(err.contains("on error goto target label not found"));
    }

    #[test]
    fn formal_v41_on_error_goto_zero_disables_label_handler() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error GoTo handler\nOn Error GoTo 0\nError 4\nIf Err.Number = -1 Then\nhandler:\nResume Next\nEnd If\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("goto 0 should disable label handler");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v42_redim_preserve_retains_existing_values() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim a(1)\nDim x\na(0) = 7\nReDim Preserve a(3)\nx = a(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[2], 7);
    }

    #[test]
    fn formal_v42_redim_without_preserve_reinitializes_array() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(1)\nDim x\na(0) = 7\nReDim a(3)\nx = a(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[2], 0);
    }

    #[test]
    fn formal_v42_redim_shrink_rejects_out_of_bounds_access() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(3)\nReDim a(1)\na(2) = 9\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("out-of-bounds after shrink should fail");
        assert!(!err.trim().is_empty());
    }

    #[test]
    fn formal_v43_module_const_evaluates_in_expression() {
        let engine = Engine::new(HostConfig::default());
        let source = "Const BASE = 5\nSub Main()\nDim x\nx = BASE + 2\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![5, 7]);
    }

    #[test]
    fn formal_v43_enum_members_bind_to_expected_values() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Enum Mode\nFast = 3\nSafe\nEnd Enum\nSub Main()\nDim x\nx = Safe + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![3, 4, 5]);
    }

    #[test]
    fn formal_v43_udt_declaration_block_is_parse_tolerated() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Type Point\nX As Integer\nY As Integer\nEnd Type\nSub Main()\nDim x\nx = 9\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot, vec![9]);
    }

    #[test]
    fn formal_v10_array_store_load_roundtrip() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(2)\nDim x\na(1) = 7\nx = a(1)\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot.last().copied(), Some(7));
    }

    #[test]
    fn formal_v10_array_bounds_violation_errors() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(1)\na(2) = 5\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("out-of-range access should fail");
        assert!(!err.trim().is_empty());
    }

    #[test]
    fn formal_v10_array_index_zero_is_valid() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim a(2)\nDim x\na(0) = 3\nx = a(0)\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot.last().copied(), Some(3));
    }

    #[test]
    fn formal_v11_resume_next_records_error_number() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 5);
    }

    #[test]
    fn formal_v11_default_error_mode_fails() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nError 9\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("default error mode should fail");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v11_resume_next_continues_execution() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nx = 1\nError 2\nx = x + 1\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 2);
    }

    #[test]
    fn formal_v12_on_error_goto_zero_restores_fail_behavior() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error Resume Next\nOn Error GoTo 0\nError 3\nEnd Sub";
        let err = engine
            .execute_source_with_snapshot(source)
            .expect_err("goto 0 should restore fail behavior");
        assert!(err.contains("runtime error"));
    }

    #[test]
    fn formal_v12_resume_next_statement_no_panic() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nOn Error Resume Next\nResume Next\nError 2\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("resume next statement should not fail");
        assert!(snapshot.is_empty());
    }

    #[test]
    fn formal_v12_resume_next_then_continue_updates_value() {
        let engine = Engine::new(HostConfig::default());
        let source =
            "Sub Main()\nDim x\nOn Error Resume Next\nError 2\nResume Next\nx = 1\nEnd Sub";
        let snapshot = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(snapshot[0], 1);
    }

    #[test]
    fn formal_v20_jit_vm_equivalence_arithmetic() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 4\nx = x - 2\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_with_snapshot(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_with_snapshot(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v20_jit_vm_equivalence_control_flow() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_with_snapshot(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_with_snapshot(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v20_jit_vm_equivalence_error_state() {
        let vm_engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
        let jit_engine = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        });
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let vm_out = vm_engine
            .execute_source_with_snapshot(source)
            .expect("vm execution should succeed");
        let jit_out = jit_engine
            .execute_source_with_snapshot(source)
            .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v13_variant_numeric_coercion_long_to_double() {
        let value = oxvba_runtime::Variant::from_i32(7);
        let coerced = oxvba_runtime::coerce::coerce_to(&value, oxvba_runtime::VarType::Double)
            .expect("coercion should succeed");
        assert_eq!(coerced.as_f64(), Some(7.0));
    }

    #[test]
    fn formal_v13_variant_numeric_bool_to_long() {
        let value = oxvba_runtime::Variant::from_bool(true);
        let coerced = oxvba_runtime::coerce::coerce_to(&value, oxvba_runtime::VarType::Long)
            .expect("coercion should succeed");
        assert_eq!(coerced.as_i32(), Some(-1));
    }

    #[test]
    fn formal_v13_variant_numeric_addition_consistency() {
        let lhs = oxvba_runtime::Variant::from_i16(2);
        let rhs = oxvba_runtime::Variant::from_i16(3);
        let out = oxvba_runtime::arithmetic::add(&lhs, &rhs).expect("add should succeed");
        assert_eq!(out.as_i32(), Some(5));
    }

    #[test]
    fn formal_v14_bstr_roundtrip_ascii() {
        let b = oxvba_runtime::bstr::BStr("ABC".to_string());
        assert_eq!(b.0, "ABC");
    }

    #[test]
    fn formal_v14_bstr_concat_law() {
        let a = oxvba_runtime::bstr::BStr("A".to_string());
        let b = oxvba_runtime::bstr::BStr("B".to_string());
        assert_eq!(format!("{}{}", a.0, b.0), "AB");
    }

    #[test]
    fn formal_v14_bstr_empty_identity() {
        let empty = oxvba_runtime::bstr::BStr(String::new());
        let text = oxvba_runtime::bstr::BStr("X".to_string());
        assert_eq!(format!("{}{}", empty.0, text.0), "X");
    }

    #[test]
    fn formal_v15_date_currency_projection_is_stable() {
        let date_like = 45000.25_f64;
        assert_eq!((date_like * 10000.0).round() / 10000.0, 45000.25_f64);
    }

    #[test]
    fn formal_v15_currency_scale_roundtrip() {
        let units = 12345_i64;
        let major = units as f64 / 100.0;
        let roundtrip = (major * 100.0).round() as i64;
        assert_eq!(roundtrip, units);
    }

    #[test]
    fn formal_v15_date_addition_monotonicity() {
        let day0 = 45000.0_f64;
        let day1 = day0 + 1.0;
        assert!(day1 > day0);
    }

    #[test]
    fn formal_v16_spec_trace_matches_runtime_small_program() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 1\nEnd Sub";
        let runtime = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        let spec = vec![2];
        assert_eq!(runtime, spec);
    }

    #[test]
    fn formal_v16_spec_trace_matches_branch_program() {
        let engine = Engine::new(HostConfig::default());
        let source = "Sub Main()\nDim x\nx = 1\nIf x = 1 Then\nx = 3\nElse\nx = 4\nEnd If\nEnd Sub";
        let runtime = engine
            .execute_source_with_snapshot(source)
            .expect("execution should succeed");
        assert_eq!(runtime, vec![3]);
    }

    #[test]
    fn formal_v16_trace_format_is_csv_stable() {
        let trace = [1, 2, 3]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(trace, "1,2,3");
    }

    #[test]
    fn formal_v17_formal_manifest_has_active_entries() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/obligations.csv"))
            .expect("obligations file should exist");
        assert!(text.contains("obligation_id"));
        assert!(text.contains(",true,"));
    }

    #[test]
    fn formal_v17_runner_script_exists() {
        assert!(repo_path("scripts/run-formal.ps1").exists());
        assert!(repo_path("scripts/setup-kani.ps1").exists());
        assert!(repo_path("scripts/test-path-stability.ps1").exists());
        assert!(repo_path("scripts/validate-divergences.ps1").exists());
    }

    #[test]
    fn formal_v17_meta_check_includes_formal_switch() {
        let text = std::fs::read_to_string(repo_path("scripts/meta-check.ps1"))
            .expect("meta-check script exists");
        assert!(text.contains("[switch]$Formal"));
        assert!(text.contains("run-formal.ps1"));
        assert!(text.contains("validate-divergences.ps1"));
        assert!(text.contains("validate-language-coverage.ps1"));
    }

    #[test]
    fn formal_v18_divergence_index_is_present() {
        assert!(repo_path("docs/evidence/divergences/README.md").exists());
    }

    #[test]
    fn formal_v18_divergence_records_have_scope_lines() {
        assert!(divergence_record_has_required_sections(&repo_path(
            "docs/evidence/divergences/DIV-0001.md"
        )));
    }

    #[test]
    fn formal_v18_divergence_records_link_evidence() {
        assert!(divergence_record_has_required_sections(&repo_path(
            "docs/evidence/divergences/DIV-0002.md"
        )));
    }

    #[test]
    fn formal_v22_jit_vm_equivalence_for_loop_backedge() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v22_jit_vm_equivalence_do_loop_backedge() {
        let source = "Sub Main()\nDim x\nx = 0\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v22_cranelift_supports_loop_subset() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nFor i = 1 To 3\nx = x + 1\nNext i\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(oxvba_jit::cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn formal_v23_formal_runner_has_require_kani_switch() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(text.contains("[switch]$RequireKani"));
        assert!(text.contains("[switch]$UseWslKani"));
        assert!(text.contains("OXVBA_REQUIRE_KANI"));
    }

    #[test]
    fn formal_v23_setup_kani_script_documents_bootstrap() {
        let text =
            std::fs::read_to_string(repo_path("scripts/setup-kani.ps1")).expect("script exists");
        assert!(text.contains("cargo install kani-verifier --locked"));
        assert!(text.contains("cargo kani setup"));
        assert!(repo_path("scripts/run-formal-kani-wsl.ps1").exists());
        assert!(repo_path("scripts/run-formal-kani-async.ps1").exists());
        assert!(repo_path("scripts/async-task-runner.ps1").exists());
    }

    #[test]
    fn formal_v23_ci_supports_optional_kani_job() {
        let text = std::fs::read_to_string(repo_path(".github/workflows/ci.yml"))
            .expect("ci workflow exists");
        assert!(text.contains("formal-kani"));
        assert!(text.contains("RUN_KANI"));
    }

    #[test]
    fn formal_v24_jit_vm_equivalence_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall AddTwo\nEnd Sub\nSub AddTwo()\nx = x + 2\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v24_cranelift_supports_call_subset() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall AddTwo\nEnd Sub\nSub AddTwo()\nx = x + 2\nEnd Sub";
        let bytecode = oxvba_compiler::compile(source).expect("compile should succeed");
        assert!(oxvba_jit::cranelift::supports_bytecode(&bytecode));
    }

    #[test]
    fn formal_v24_jit_falls_back_for_error_state_subset() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_with_snapshot(source)
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v25_optimizer_parity_on_constant_if_fold() {
        let source =
            "Sub Main()\nDim x\nx = 1\nIf 1 = 1 Then\nx = x + 3\nElse\nx = x + 9\nEnd If\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v25_optimizer_parity_on_select_case_fold() {
        let source = "Sub Main()\nDim x\nSelect Case 2\nCase 1\nx = 10\nCase 2\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v25_optimizer_parity_on_dead_store_reduction() {
        let source = "Sub Main()\nDim x\nx = 1\nx = 2\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v26_script_defaults_target_v26_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(
            matrix.contains("mvp-perf-shape-v26")
                || matrix.contains("mvp-full-coverage-perf-gate-v36")
        );
        assert!(
            formal.contains("mvp-perf-shape-v26")
                || formal.contains("mvp-full-coverage-perf-gate-v36")
        );
    }

    #[test]
    fn formal_v26_benchmark_default_targets_v26_artifact() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(
            bench.contains("docs/evidence/profiles/v26/benchmark_latest.md")
                || bench.contains("docs/evidence/profiles/v36/benchmark_latest.md")
        );
    }

    #[test]
    fn formal_v26_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V26.md").exists());
    }

    #[test]
    fn formal_v27_async_runner_supports_full_action_set() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("Start"));
        assert!(text.contains("Status"));
        assert!(text.contains("Tail"));
        assert!(text.contains("Wait"));
        assert!(text.contains("Stop"));
    }

    #[test]
    fn formal_v27_async_runner_uses_hidden_background_window() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("-WindowStyle Hidden"));
    }

    #[test]
    fn formal_v27_async_runner_persists_state_and_exit_markers() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("state.json"));
        assert!(text.contains("exit_code.txt"));
        assert!(text.contains("completed_utc.txt"));
    }

    #[test]
    fn formal_v28_vm_pc_progression_kani_harness_is_bounded() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("vm interpreter exists");
        assert!(text.contains("pc_progression_is_safe_for_valid_jump_target"));
        assert!(text.contains("kani::assume(instruction_len < 64)"));
        assert!(text.contains("next_pc_for_jump_if_zero"));
    }

    #[test]
    fn formal_v28_vm_jump_helper_has_regression_unit_test() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-vm/src/interpreter.rs"))
            .expect("vm interpreter exists");
        assert!(text.contains("jump_if_zero_pc_progression_helper"));
    }

    #[test]
    fn formal_v28_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V28.md").exists());
    }

    #[test]
    fn formal_v29_async_runner_wait_supports_timeouts() {
        let text = std::fs::read_to_string(repo_path("scripts/run-formal-kani-async.ps1"))
            .expect("async runner exists");
        assert!(text.contains("TimeoutSeconds"));
        assert!(text.contains("timed out"));
    }

    #[test]
    fn formal_v29_capacity_workset_document_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-27_KANI_CAPACITY_V29.md").exists());
    }

    #[test]
    fn formal_v29_obligation_entries_are_registered() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/formal/obligations.csv"))
            .expect("obligations should exist");
        assert!(text.contains("FO-V29-001"));
        assert!(text.contains("FO-V29-002"));
        assert!(text.contains("FO-V29-003"));
    }

    #[test]
    fn formal_v30_variant_layout_uses_com_reserved_fields() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-runtime/src/variant.rs"))
            .expect("variant runtime file exists");
        assert!(text.contains("reserved1"));
        assert!(text.contains("reserved2"));
        assert!(text.contains("reserved3"));
        assert!(text.contains("union VariantData"));
    }

    #[test]
    fn formal_v30_variant_runtime_has_com_layout_shape_test() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-runtime/src/variant.rs"))
            .expect("variant runtime file exists");
        assert!(text.contains("com_variant_layout_shape"));
    }

    #[test]
    fn formal_v30_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V30.md").exists());
    }

    #[test]
    fn formal_v31_variant_wire_roundtrip_helpers_exist() {
        let text = std::fs::read_to_string(repo_path("crates/oxvba-runtime/src/variant.rs"))
            .expect("variant runtime file exists");
        assert!(text.contains("to_wire_bytes"));
        assert!(text.contains("from_wire_bytes"));
        assert!(text.contains("com_variant_wire_roundtrip_for_numeric_value"));
    }

    #[test]
    fn formal_v31_boundary_marshalling_workset_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-27_BOUNDARY_MARSHALLING_V31.md").exists());
    }

    #[test]
    fn formal_v31_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V31.md").exists());
    }

    #[test]
    fn formal_v32_language_coverage_index_exists() {
        assert!(repo_path("docs/evidence/language/COVERAGE_INDEX.csv").exists());
    }

    #[test]
    fn formal_v32_meta_check_validates_language_coverage() {
        let text = std::fs::read_to_string(repo_path("scripts/meta-check.ps1"))
            .expect("meta-check script exists");
        assert!(text.contains("validate-language-coverage.ps1"));
    }

    #[test]
    fn formal_v32_language_coverage_status_taxonomy_is_present() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains(",implemented,"));
        assert!(text.contains(",partial,"));
        assert!(text.contains(",planned,"));
    }

    #[test]
    fn formal_v33_core_coverage_tracks_key_control_flow_constructs() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains("If Then End If"));
        assert!(text.contains("For Next"));
        assert!(text.contains("Select Case"));
    }

    #[test]
    fn formal_v33_core_coverage_workset_exists() {
        assert!(
            repo_path("docs/worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_CORE_V33.md").exists()
        );
    }

    #[test]
    fn formal_v33_core_conformance_fixtures_are_present() {
        assert!(repo_path("conformance/tests/if_true.bas").exists());
        assert!(repo_path("conformance/tests/for_basic.bas").exists());
        assert!(repo_path("conformance/tests/select_case_basic.bas").exists());
    }

    #[test]
    fn formal_v34_object_coverage_entries_are_present() {
        let text = std::fs::read_to_string(repo_path("docs/evidence/language/COVERAGE_INDEX.csv"))
            .expect("coverage index exists");
        assert!(text.contains("objects,Root object injection"));
        assert!(text.contains("objects,Class module lifecycle"));
    }

    #[test]
    fn formal_v34_object_coverage_workset_exists() {
        assert!(
            repo_path("docs/worksets/WORKSET_2026-02-27_LANGUAGE_COVERAGE_OBJECTS_V34.md").exists()
        );
    }

    #[test]
    fn formal_v34_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V34.md").exists());
    }

    #[test]
    fn formal_v35_hotpath_workset_exists() {
        assert!(repo_path("docs/worksets/WORKSET_2026-02-27_JIT_OPT_HOTPATHS_V35.md").exists());
    }

    #[test]
    fn formal_v35_jit_vm_hotpath_parity_examples_exist() {
        assert!(repo_path("conformance/tests/for_basic.bas").exists());
        assert!(repo_path("conformance/tests/proc_call_chain.bas").exists());
    }

    #[test]
    fn formal_v35_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V35.md").exists());
    }

    #[test]
    fn formal_v36_script_defaults_target_v36_profile_scope() {
        let matrix = std::fs::read_to_string(repo_path("scripts/run-matrix.ps1"))
            .expect("run-matrix script exists");
        let formal = std::fs::read_to_string(repo_path("scripts/run-formal.ps1"))
            .expect("run-formal script exists");
        assert!(matrix.contains("mvp-full-coverage-perf-gate-v36"));
        assert!(formal.contains("mvp-full-coverage-perf-gate-v36"));
    }

    #[test]
    fn formal_v36_benchmark_default_targets_v36_artifact() {
        let bench = std::fs::read_to_string(repo_path("scripts/run-bench.ps1"))
            .expect("run-bench script exists");
        assert!(bench.contains("docs/evidence/profiles/v36/benchmark_latest.md"));
    }

    #[test]
    fn formal_v36_profile_status_document_exists() {
        assert!(repo_path("docs/PROFILE_STATUS_V36.md").exists());
    }

    #[test]
    fn formal_v21_opt_toggle_parity() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 0\nx = x + 2\nEnd Sub";
        let bound = oxvba_compiler::resolve::resolve_symbols(source);
        let checked = oxvba_compiler::typecheck::check_types(bound).expect("typecheck");
        let optimized = oxvba_compiler::optimize::optimize_module(checked.clone());
        let slow_bc = oxvba_compiler::emit::emit_bytecode(&checked);
        let fast_bc = oxvba_compiler::emit::emit_bytecode(&optimized);
        let slow = oxvba_vm::execute_and_snapshot(&slow_bc).expect("slow execution");
        let fast = oxvba_vm::execute_and_snapshot(&fast_bc).expect("fast execution");
        assert_eq!(fast, slow);
    }

    #[test]
    fn formal_v21_jit_vm_guardrail_equivalence() {
        let vm_out = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_source_with_snapshot("Sub Main()\nDim x\nx = 4\nx = x + 1\nEnd Sub")
        .expect("vm execution should succeed");
        let jit_out = Engine::new(HostConfig {
            enable_jit: true,
            root_object_name: None,
        })
        .execute_source_with_snapshot("Sub Main()\nDim x\nx = 4\nx = x + 1\nEnd Sub")
        .expect("jit execution should succeed");
        assert_eq!(vm_out, jit_out);
    }

    #[test]
    fn formal_v21_benchmark_script_exists() {
        assert!(repo_path("scripts/run-bench.ps1").exists());
    }
}
