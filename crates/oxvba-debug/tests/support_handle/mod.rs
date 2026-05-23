#![allow(dead_code)]

use std::{collections::BTreeMap, sync::Arc};

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::{
    DebugAttachConfig, DebugSessionAttach, DebugSessionHandle, attach_debug_session,
};
use oxvba_host::{Engine, HostConfig};

pub fn make_manifest(source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "DebugHandleCatalog".to_string(),
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

pub fn multi_module_manifest() -> ProjectManifest {
    ProjectManifest {
        project_name: "DebugHandleCatalog".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                "Sub Main()\nCall Foo(4)\nEnd Sub",
            )
            .expect("module1"),
            module_unit_from_source(
                "Module2",
                ModuleKind::Procedural,
                "Sub Foo(ByVal y As Long)\nDim z As Long\nz = y + 1\nEnd Sub",
            )
            .expect("module2"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

pub fn attach(manifest: ProjectManifest) -> DebugSessionAttach {
    attach_with_config(manifest, DebugAttachConfig::default())
}

pub fn attach_with_config(
    manifest: ProjectManifest,
    config: DebugAttachConfig,
) -> DebugSessionAttach {
    attach_debug_session(
        Arc::new(Engine::new(HostConfig::default())),
        manifest,
        config,
    )
    .expect("debug handle attach")
}

pub fn attach_handle() -> DebugSessionHandle {
    attach(call_manifest()).handle
}
