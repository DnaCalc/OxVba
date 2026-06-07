#![allow(dead_code)]

use std::collections::BTreeMap;

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, compile_project, module_unit_from_source,
};
use oxvba_debug::DebugSourceMap;

pub fn manifest(module_name: &str, kind: ModuleKind, source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "SourceMapDebug".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module_unit_from_source(module_name, kind, source).expect("module")],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

pub fn debug_map(source: &str) -> DebugSourceMap {
    let compiled = compile_project(&manifest("Module1", ModuleKind::Procedural, source))
        .expect("compile project");
    DebugSourceMap::from_compiled_project(&compiled)
}
