//! Host callback trait for interactive and host-supplied project operations.

use crate::project::{
    HostExtensionModuleChange, ProjectCallbackError, ProjectCallbackResult, ProjectDescriptor,
    ProjectReferenceDescriptor, ResolvedProjectReference,
};

/// Trait implemented by host embeddings to handle interactive operations.
pub trait HostCallbacks: Send + Sync {
    /// Called when VBA executes `MsgBox`. Returns the button code.
    fn on_msg_box(&self, prompt: &str, style: i32) -> i32;
    /// Called when VBA executes `InputBox`. Returns the user input string.
    fn on_input_box(&self, prompt: &str, default: &str) -> String;
    /// Called when VBA sets `Application.StatusBar`.
    fn on_status_bar(&self, text: &str);
    /// Called when VBA executes plain console `Print` on a stdio host.
    fn on_console_print(&self, _text: &str) -> bool {
        false
    }
    /// Called when VBA executes plain console `Input` / `Line Input` on a stdio host.
    fn on_console_input_line(&self) -> Option<String> {
        None
    }
    /// Called when VBA executes `Debug.Print`.
    fn on_debug_print(&self, text: &str);

    /// Whether the host exposes project discovery operations.
    fn supports_project_catalog(&self) -> bool {
        false
    }

    /// Whether the host exposes project reference queries.
    fn supports_project_references(&self) -> bool {
        false
    }

    /// Whether the host exposes host-project mutation operations.
    fn supports_project_mutation(&self) -> bool {
        false
    }

    fn on_list_projects(&self) -> ProjectCallbackResult<Vec<ProjectDescriptor>> {
        Err(ProjectCallbackError::AdapterFault {
            message: "project catalog callbacks unsupported".to_string(),
        })
    }

    fn on_get_project(&self, project_name: &str) -> ProjectCallbackResult<ProjectDescriptor> {
        Err(ProjectCallbackError::ProjectNotFound {
            project_name: project_name.to_string(),
        })
    }

    fn on_list_project_references(
        &self,
        project_name: &str,
    ) -> ProjectCallbackResult<Vec<ProjectReferenceDescriptor>> {
        Err(ProjectCallbackError::ProjectNotFound {
            project_name: project_name.to_string(),
        })
    }

    fn on_resolve_project_reference(
        &self,
        reference: &ProjectReferenceDescriptor,
    ) -> ProjectCallbackResult<ResolvedProjectReference> {
        Err(ProjectCallbackError::ReferenceUnresolved {
            referenced_name: reference.referenced_name.clone(),
        })
    }

    fn on_attach_host_extension_module(
        &self,
        change: &HostExtensionModuleChange,
    ) -> ProjectCallbackResult<()> {
        Err(ProjectCallbackError::AdapterFault {
            message: format!(
                "host project mutation unsupported for `{}` / `{}`",
                change.project_name, change.module_name
            ),
        })
    }
}

/// Default callbacks: returns `style.max(1)` for MsgBox, `default` for InputBox.
pub struct DefaultHostCallbacks;

impl HostCallbacks for DefaultHostCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }

    fn on_status_bar(&self, _text: &str) {}

    fn on_console_print(&self, _text: &str) -> bool {
        false
    }

    fn on_console_input_line(&self) -> Option<String> {
        None
    }

    fn on_debug_print(&self, text: &str) {
        eprintln!("[oxvba-hal] debug.print: {text}");
    }
}
