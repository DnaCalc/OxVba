#![allow(dead_code)]

use std::{collections::BTreeMap, sync::Arc};

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::{DebugCoreConfig, DebugSessionCore, prepare_debug_session_core};
use oxvba_host::{Engine, HostConfig, ProjectRuntimeSession};

pub fn make_manifest(source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "DebugCatalog".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source("Module1", ModuleKind::Procedural, source)
                .expect("module unit"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

pub fn call_manifest() -> ProjectManifest {
    make_manifest(
        "Sub Main()\n\
         Call Foo(4)\n\
         End Sub\n\
         \n\
         Sub Foo(ByVal y As Long)\n\
         Dim z As Long\n\
         z = y + 1\n\
         End Sub",
    )
}

pub fn prepare(manifest: &ProjectManifest) -> DebugSessionCore {
    let engine = Arc::new(Engine::new(HostConfig::default()));
    prepare_debug_session_core(engine, manifest.clone(), DebugCoreConfig::default())
        .expect("debug session should prepare")
}

pub fn prepared_runtime(manifest: &ProjectManifest) -> ProjectRuntimeSession {
    Engine::new(HostConfig::default())
        .compile_and_prepare_session(manifest)
        .expect("runtime session should prepare")
}
