use std::collections::BTreeMap;

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, compile_project, module_unit_from_source,
};
use oxvba_debug::DebugSourceMap;

#[test]
fn module_maps_are_independent() {
    let manifest = ProjectManifest {
        project_name: "MultiMap".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                "Attribute VB_Name = \"Module1\"\nSub Main()\nEnd Sub",
            )
            .unwrap(),
            module_unit_from_source("Module2", ModuleKind::Procedural, "Sub Helper()\nEnd Sub")
                .unwrap(),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let compiled = compile_project(&manifest).expect("compile");
    let map = DebugSourceMap::from_compiled_project(&compiled);
    assert_eq!(map.file_to_runtime("Module1", 1), None);
    assert_eq!(map.file_to_runtime("Module2", 1), Some(1));
}
