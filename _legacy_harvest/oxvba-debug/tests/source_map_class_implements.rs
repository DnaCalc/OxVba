use oxvba_compiler::{
    CompilerLineMapping, CompilerModuleSourceMap, CompilerSourceLineKind, CompilerSourceMap,
};
use oxvba_debug::DebugSourceMap;
use std::collections::BTreeMap;

#[test]
fn class_implements_line_is_dropped() {
    let mut modules = BTreeMap::new();
    modules.insert(
        "class1".to_string(),
        CompilerModuleSourceMap {
            module_name: "Class1".to_string(),
            lines: vec![CompilerLineMapping {
                file_line: 1,
                runtime_line: None,
                kind: CompilerSourceLineKind::DroppedImplements,
                executable: false,
            }],
        },
    );
    let map = DebugSourceMap::new(CompilerSourceMap { modules });
    assert_eq!(map.file_to_runtime("Class1", 1), None);
}
