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
}
