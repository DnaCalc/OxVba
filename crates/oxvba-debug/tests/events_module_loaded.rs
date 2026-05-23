#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::DebugEvent;
use std::collections::BTreeMap;

#[test]
fn attach_to_two_module_project_emits_two_module_loaded_events() {
    let manifest = ProjectManifest {
        project_name: "TwoModuleDebug".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source("Module1", ModuleKind::Procedural, "Sub Main()\nEnd Sub")
                .expect("module1"),
            module_unit_from_source("Module2", ModuleKind::Procedural, "Sub Helper()\nEnd Sub")
                .expect("module2"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let attach = support_handle::attach(manifest);
    let first = attach.events.recv().expect("module1 loaded");
    let second = attach.events.recv().expect("module2 loaded");
    assert!(matches!(first, DebugEvent::ModuleLoaded { module, .. } if module.name == "Module1"));
    assert!(matches!(second, DebugEvent::ModuleLoaded { module, .. } if module.name == "Module2"));
    attach.handle.detach().expect("detach");
}
