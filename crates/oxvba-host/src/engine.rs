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
            let _ = self.jit.compile_function("main");
        }

        execute_and_snapshot(&bytecode)
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, HostConfig};

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
}
