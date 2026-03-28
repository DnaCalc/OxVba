use std::sync::{Arc, Mutex};

use oxvba_hal::{
    callbacks::HostCallbacks,
    model::{HostPolicy, UnsupportedFeatureMode},
    project::{
        HostExtensionModuleChange, ProjectCallbackResult, ProjectDescriptor, ProjectDescriptorKind,
        ProjectReferenceDescriptor, ProjectReferenceKind,
    },
};
use oxvba_host::{Engine, HostConfig};

struct ProjectAwareCallbacks {
    mutations: Mutex<Vec<String>>,
}

impl HostCallbacks for ProjectAwareCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }

    fn on_status_bar(&self, _text: &str) {}

    fn on_debug_print(&self, _text: &str) {}

    fn supports_project_catalog(&self) -> bool {
        true
    }

    fn supports_project_references(&self) -> bool {
        true
    }

    fn supports_project_mutation(&self) -> bool {
        true
    }

    fn on_list_projects(&self) -> ProjectCallbackResult<Vec<ProjectDescriptor>> {
        Ok(vec![ProjectDescriptor {
            project_name: "Workbook".to_string(),
            kind: ProjectDescriptorKind::Host,
            supports_extension_modules: true,
        }])
    }

    fn on_get_project(&self, project_name: &str) -> ProjectCallbackResult<ProjectDescriptor> {
        Ok(ProjectDescriptor {
            project_name: project_name.to_string(),
            kind: ProjectDescriptorKind::Host,
            supports_extension_modules: true,
        })
    }

    fn on_list_project_references(
        &self,
        project_name: &str,
    ) -> ProjectCallbackResult<Vec<ProjectReferenceDescriptor>> {
        Ok(vec![ProjectReferenceDescriptor {
            project_name: project_name.to_string(),
            referenced_name: "HostExt".to_string(),
            kind: ProjectReferenceKind::Project,
        }])
    }

    fn on_attach_host_extension_module(
        &self,
        change: &HostExtensionModuleChange,
    ) -> ProjectCallbackResult<()> {
        self.mutations
            .lock()
            .expect("mutation log lock")
            .push(format!("{}::{}", change.project_name, change.module_name));
        Ok(())
    }
}

#[test]
fn engine_public_api_exposes_callback_backed_project_services() {
    let callbacks = Arc::new(ProjectAwareCallbacks {
        mutations: Mutex::new(Vec::new()),
    });
    let mut engine = Engine::new(HostConfig::default()).with_host_callbacks(callbacks.clone());
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine.set_unsupported_feature_mode(UnsupportedFeatureMode::Runtime);

    let host = engine.host_services();
    let projects = host
        .project_catalog()
        .expect("project catalog should be exposed")
        .list_projects()
        .expect("project list should succeed");
    assert_eq!(projects[0].project_name, "Workbook");

    let references = host
        .project_references()
        .expect("project references should be exposed")
        .list_references("Workbook")
        .expect("reference list should succeed");
    assert_eq!(references[0].referenced_name, "HostExt");

    host.project_mutation()
        .expect("project mutation should be exposed")
        .attach_host_extension_module(&HostExtensionModuleChange {
            project_name: "Workbook".to_string(),
            module_name: "HostExt".to_string(),
            source: "Public Sub Sync()\nEnd Sub".to_string(),
        })
        .expect("host extension attach should succeed");

    assert_eq!(
        callbacks
            .mutations
            .lock()
            .expect("mutation log lock")
            .as_slice(),
        ["Workbook::HostExt"]
    );
}
