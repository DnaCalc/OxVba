use oxvba_compiler::compile;
use oxvba_jit::JitEngine;
use oxvba_vm::execute;

#[derive(Debug, Clone, Default)]
pub struct HostConfig {
    pub enable_jit: bool,
    pub root_object_name: Option<String>,
}

#[derive(Debug, Default)]
pub struct Engine {
    config: HostConfig,
    jit: JitEngine,
}

impl Engine {
    pub fn new(config: HostConfig) -> Self {
        Self {
            config,
            jit: JitEngine,
        }
    }

    pub fn execute_source(&self, source: &str) -> Result<(), String> {
        let bytecode = compile(source).map_err(|e| e.to_string())?;

        if self.config.enable_jit {
            let _ = self.jit.compile_function("main");
        }

        execute(&bytecode)
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, HostConfig};

    #[test]
    fn execute_source_with_default_vm_path() {
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: Some("Application".to_string()),
        });

        let result = engine.execute_source("Sub Main()\nEnd Sub");
        assert!(result.is_ok());
    }
}
