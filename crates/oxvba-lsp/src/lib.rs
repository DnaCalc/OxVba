//! Thin LSP transport bootstrap over the direct OxVba language-service API.
//!
//! This crate intentionally owns transport/session concerns only. Semantic
//! parsing, binding, and query behavior remain in `oxvba-languageservice`.

use std::sync::{Arc, Mutex};

use oxvba_languageservice::{LanguageService, Workspace};
use tower_lsp::lsp_types::{ServerCapabilities, ServerInfo};

/// Direct language-service core owned by the transport shell.
#[derive(Clone)]
pub struct OxvbaLspCore {
    service: Arc<Mutex<LanguageService>>,
}

impl Default for OxvbaLspCore {
    fn default() -> Self {
        Self::new()
    }
}

impl OxvbaLspCore {
    pub fn new() -> Self {
        let workspace = Workspace::new();
        let service = LanguageService::new(workspace);
        Self {
            service: Arc::new(Mutex::new(service)),
        }
    }

    pub fn service(&self) -> Arc<Mutex<LanguageService>> {
        Arc::clone(&self.service)
    }
}

/// Current server info for the bootstrap transport shell.
pub fn server_info() -> ServerInfo {
    ServerInfo {
        name: "oxvba-lsp".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

/// Current advertised capabilities.
///
/// The bootstrap slice intentionally advertises no document sync or query
/// features yet. Those will land in later transport beads once the workspace
/// synchronization boundary is implemented.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities::default()
}

#[cfg(test)]
mod tests {
    use super::{OxvbaLspCore, server_capabilities, server_info};

    #[test]
    fn bootstrap_server_info_is_stable() {
        let info = server_info();
        assert_eq!(info.name, "oxvba-lsp");
        assert!(info.version.is_some());
    }

    #[test]
    fn bootstrap_server_advertises_no_editor_features_yet() {
        let capabilities = server_capabilities();
        assert!(capabilities.text_document_sync.is_none());
        assert!(capabilities.hover_provider.is_none());
        assert!(capabilities.definition_provider.is_none());
        assert!(capabilities.references_provider.is_none());
        assert!(capabilities.completion_provider.is_none());
        assert!(capabilities.signature_help_provider.is_none());
        assert!(capabilities.rename_provider.is_none());
        assert!(capabilities.code_action_provider.is_none());
        assert!(capabilities.document_symbol_provider.is_none());
        assert!(capabilities.workspace_symbol_provider.is_none());
        assert!(capabilities.semantic_tokens_provider.is_none());
    }

    #[test]
    fn bootstrap_core_owns_a_direct_language_service_instance() {
        let core = OxvbaLspCore::new();
        let service = core.service();
        let _guard = service.lock().expect("language service mutex poisoned");
    }
}
