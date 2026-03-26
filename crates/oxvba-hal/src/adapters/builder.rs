//! Host builder: constructs host services from profile, policy, and optional components.

use std::sync::Arc;

use oxvba_com::PortableComProjection;

use crate::{
    callbacks::HostCallbacks,
    model::{HalProfileId, HalRuntimeClass, HostPolicy},
    traits::HostServices,
};

use super::{null, recording::RecordingHostServices, standard, wasm};

/// Builder for constructing host service instances with optional components.
pub struct HostBuilder {
    profile: HalProfileId,
    runtime_class: Option<HalRuntimeClass>,
    policy: HostPolicy,
    callbacks: Option<Arc<dyn HostCallbacks>>,
    portable_objects: Option<Arc<PortableComProjection>>,
    recording: bool,
}

impl HostBuilder {
    pub fn new() -> Self {
        Self {
            profile: HalProfileId::Null,
            runtime_class: None,
            policy: HostPolicy::default(),
            callbacks: None,
            portable_objects: None,
            recording: false,
        }
    }

    pub fn profile(mut self, profile: HalProfileId) -> Self {
        self.profile = profile;
        self
    }

    pub fn runtime_class(mut self, runtime_class: HalRuntimeClass) -> Self {
        self.runtime_class = Some(runtime_class);
        self
    }

    pub fn policy(mut self, policy: HostPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn callbacks(mut self, callbacks: Arc<dyn HostCallbacks>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    /// Set a portable COM projection for host-registered objects.
    pub fn portable_objects(mut self, projection: Arc<PortableComProjection>) -> Self {
        self.portable_objects = Some(projection);
        self
    }

    /// Enable recording of HAL interactions into a journal.
    pub fn recording(mut self, enable: bool) -> Self {
        self.recording = enable;
        self
    }

    pub fn build(self) -> Arc<dyn HostServices> {
        let runtime_class = self.runtime_class.unwrap_or_else(|| {
            HalRuntimeClass::default_for(self.profile, self.policy.wasm_runtime_class)
        });

        let host: Arc<dyn HostServices> = match self.profile {
            HalProfileId::Windows | HalProfileId::Linux | HalProfileId::MacOs => {
                let mut host = standard::StandardHostServices::new_with_runtime_class(
                    self.profile,
                    runtime_class,
                    self.policy,
                );
                if let Some(cb) = self.callbacks {
                    host = host.with_callbacks(cb);
                }
                if let Some(projection) = self.portable_objects {
                    host = host.with_portable_objects(projection);
                }
                Arc::new(host)
            }
            HalProfileId::Wasm => wasm::WasmHostServices::boxed(self.policy),
            HalProfileId::Null => null::NullHostServices::boxed(self.policy),
        };

        if self.recording {
            Arc::new(RecordingHostServices::new(host))
        } else {
            host
        }
    }
}

impl Default for HostBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        callbacks::{DefaultHostCallbacks, HostCallbacks},
        model::HostPolicy,
        project::{
            HostExtensionModuleChange, ProjectCallbackResult, ProjectDescriptor,
            ProjectDescriptorKind, ProjectReferenceDescriptor, ProjectReferenceKind,
            ResolvedProjectReference,
        },
    };
    use std::sync::Mutex;

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

        fn on_resolve_project_reference(
            &self,
            reference: &ProjectReferenceDescriptor,
        ) -> ProjectCallbackResult<ResolvedProjectReference> {
            Ok(ResolvedProjectReference::Project(ProjectDescriptor {
                project_name: reference.referenced_name.clone(),
                kind: ProjectDescriptorKind::Library,
                supports_extension_modules: false,
            }))
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
    fn builder_creates_windows_host() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::deterministic_runtime())
            .build();
        assert_eq!(host.profile(), HalProfileId::Windows);
    }

    #[test]
    fn builder_creates_null_host() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Null)
            .policy(HostPolicy::strict_ci())
            .build();
        assert_eq!(host.profile(), HalProfileId::Null);
    }

    #[test]
    fn builder_accepts_callbacks() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::deterministic_runtime())
            .callbacks(Arc::new(DefaultHostCallbacks))
            .build();
        assert_eq!(host.profile(), HalProfileId::Windows);
    }

    #[test]
    fn builder_with_explicit_runtime_class() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .runtime_class(HalRuntimeClass::WindowsHeadless)
            .policy(HostPolicy::deterministic_runtime())
            .build();
        assert_eq!(host.descriptor().runtime_class, "windows-headless");
    }

    #[test]
    fn builder_exposes_callback_backed_project_services() {
        let callbacks = Arc::new(ProjectAwareCallbacks {
            mutations: Mutex::new(Vec::new()),
        });
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::interactive_dev())
            .callbacks(callbacks.clone())
            .build();

        let projects = host
            .project_catalog()
            .expect("project catalog should be exposed")
            .list_projects()
            .expect("project list should succeed");
        assert_eq!(projects[0].project_name, "Workbook");

        let refs = host
            .project_references()
            .expect("project references should be exposed")
            .list_references("Workbook")
            .expect("reference list should succeed");
        assert_eq!(refs[0].referenced_name, "HostExt");

        host.project_mutation()
            .expect("project mutation should be exposed")
            .attach_host_extension_module(&HostExtensionModuleChange {
                project_name: "Workbook".to_string(),
                module_name: "HostExt".to_string(),
                source: "Public Sub Sync()\nEnd Sub".to_string(),
            })
            .expect("host extension attach should succeed");

        assert_eq!(
            callbacks.mutations.lock().expect("mutation log lock").as_slice(),
            ["Workbook::HostExt"]
        );
    }
}
