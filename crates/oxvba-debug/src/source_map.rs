/// Debugger-facing source-map wrapper placeholder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugSourceMap;

impl DebugSourceMap {
    pub fn file_to_runtime(&self, _module: &str, file_line: u32) -> Option<u32> {
        Some(file_line)
    }

    pub fn runtime_to_file(&self, _module: &str, runtime_line: u32) -> Option<u32> {
        Some(runtime_line)
    }

    pub fn nearest_executable_file_line(&self, _module: &str, file_line: u32) -> Option<u32> {
        Some(file_line)
    }
}
