use oxvba_compiler::{CompilerModuleSourceMap, CompilerSourceMap};
use oxvba_debug::DebugSourceMap;
use std::collections::BTreeMap;

#[test]
fn empty_module_has_no_executable_lines() {
    let mut modules = BTreeMap::new();
    modules.insert(
        "module1".to_string(),
        CompilerModuleSourceMap {
            module_name: "Module1".to_string(),
            lines: Vec::new(),
        },
    );
    let map = DebugSourceMap::new(CompilerSourceMap { modules });
    assert!(map.executable_file_lines("Module1").is_empty());
    assert_eq!(map.nearest_executable_file_line("Module1", 1), None);
}
