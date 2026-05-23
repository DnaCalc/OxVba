use oxvba_compiler::{CompiledProject, CompilerSourceMap};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugSourceMap {
    compiler: CompilerSourceMap,
}

impl DebugSourceMap {
    pub fn new(compiler: CompilerSourceMap) -> Self {
        Self { compiler }
    }

    pub fn from_compiled_project(compiled: &CompiledProject) -> Self {
        Self::new(compiled.source_maps.clone())
    }

    pub fn file_to_runtime(&self, module: &str, file_line: u32) -> Option<u32> {
        self.compiler.module(module)?.file_to_runtime(file_line)
    }

    pub fn runtime_to_file(&self, module: &str, runtime_line: u32) -> Option<u32> {
        self.compiler.module(module)?.runtime_to_file(runtime_line)
    }

    pub fn nearest_executable_file_line(&self, module: &str, file_line: u32) -> Option<u32> {
        self.compiler
            .module(module)?
            .nearest_executable_file_line(file_line)
    }

    pub fn executable_file_lines(&self, module: &str) -> Vec<u32> {
        self.compiler
            .module(module)
            .map(|module| module.executable_file_lines())
            .unwrap_or_default()
    }
}
