use std::collections::BTreeMap;

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, compile_project, module_unit_from_source,
};

#[test]
fn compiled_project_contains_structured_source_maps() {
    let manifest = ProjectManifest {
        project_name: "CompilerSourceMap".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                "Sub Main()\nDim x As Long\nx = 1\nEnd Sub",
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
    assert!(compiled.source_maps.module("Module1").is_some());
    assert!(compiled.source_maps.module("Module2").is_some());
    assert_eq!(
        compiled
            .source_maps
            .module("Module1")
            .unwrap()
            .file_to_runtime(2),
        Some(2)
    );
    assert!(
        compiled
            .source_maps
            .module("Module1")
            .unwrap()
            .executable_file_lines()
            .contains(&2)
    );
}
