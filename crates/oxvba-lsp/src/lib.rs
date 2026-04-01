//! Thin LSP transport bootstrap over the direct OxVba language-service API.
//!
//! This crate intentionally owns transport/session concerns only. Semantic
//! parsing, binding, and query behavior remain in `oxvba-languageservice`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use oxvba_compiler::ProjectManifest;
use oxvba_languageservice::{
    DocumentId, LanguageService, SemanticProvenance, SymbolProvenanceKind, Workspace,
};
use tower_lsp::lsp_types::{
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, Url,
};

#[derive(Debug, Clone)]
struct BaselineDocument {
    source: String,
    project_name: Option<String>,
    provenance_kind: SymbolProvenanceKind,
}

struct TransportState {
    service: LanguageService,
    uri_documents: HashMap<Url, DocumentId>,
    baseline_documents: HashMap<DocumentId, BaselineDocument>,
}

impl TransportState {
    fn new() -> Self {
        Self {
            service: LanguageService::new(Workspace::new()),
            uri_documents: HashMap::new(),
            baseline_documents: HashMap::new(),
        }
    }

    fn refresh_baseline_documents(&mut self) {
        self.baseline_documents.clear();
        let document_ids = self
            .service
            .workspace
            .document_ids()
            .cloned()
            .collect::<Vec<_>>();
        for id in document_ids {
            if let Some(doc) = self.service.workspace.document(&id) {
                self.baseline_documents.insert(
                    id,
                    BaselineDocument {
                        source: doc.source.to_string(),
                        project_name: doc.project_name.clone(),
                        provenance_kind: doc.provenance_kind,
                    },
                );
            }
        }
    }
}

/// Direct language-service core owned by the transport shell.
#[derive(Clone)]
pub struct OxvbaLspCore {
    state: Arc<Mutex<TransportState>>,
}

impl Default for OxvbaLspCore {
    fn default() -> Self {
        Self::new()
    }
}

impl OxvbaLspCore {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TransportState::new())),
        }
    }

    pub fn load_project_manifest(&self, manifest: ProjectManifest) {
        let mut state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        state.service = LanguageService::from_project(manifest);
        state.refresh_baseline_documents();
    }

    pub fn load_workspace_path(&self, path: &Path) -> Result<(), String> {
        let loaded = load_workspace_target(path)?;
        self.load_project_manifest(loaded.manifest);
        Ok(())
    }

    pub fn open_text_document(&self, uri: &Url, source: &str) -> Result<DocumentId, String> {
        let mut state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        let id = resolve_document_id(&state, uri)?;
        if state.service.workspace.document(&id).is_some() {
            state.service.workspace.change_document(&id, source);
        } else {
            state.service.workspace.open_document(id.clone(), source);
        }
        state.uri_documents.insert(uri.clone(), id.clone());
        Ok(id)
    }

    pub fn change_text_document(&self, uri: &Url, source: &str) -> Result<DocumentId, String> {
        let mut state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        let id = state
            .uri_documents
            .get(uri)
            .cloned()
            .unwrap_or(resolve_document_id(&state, uri)?);

        if state.service.workspace.document(&id).is_some() {
            state.service.workspace.change_document(&id, source);
        } else {
            state.service.workspace.open_document(id.clone(), source);
        }

        state.uri_documents.insert(uri.clone(), id.clone());
        Ok(id)
    }

    pub fn close_text_document(&self, uri: &Url) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        let Some(id) = state.uri_documents.remove(uri) else {
            return Ok(());
        };

        if let Some(baseline) = state.baseline_documents.get(&id).cloned() {
            state.service.workspace.open_document_with_origin(
                id,
                &baseline.source,
                baseline.project_name,
                baseline.provenance_kind,
            );
        } else {
            state.service.workspace.close_document(&id);
        }

        Ok(())
    }

    pub fn synchronized_document_id(&self, uri: &Url) -> Option<DocumentId> {
        let state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        state.uri_documents.get(uri).cloned()
    }

    pub fn document_source(&self, id: &DocumentId) -> Option<String> {
        let state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        state
            .service
            .workspace
            .document(id)
            .map(|doc| doc.source.to_string())
    }

    pub fn document_count(&self) -> usize {
        let state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        state.service.workspace.document_count()
    }

    pub fn semantic_provenance(&self, id: &DocumentId) -> Option<SemanticProvenance> {
        let state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        state
            .service
            .workspace
            .document(id)
            .map(|doc| doc.semantic_provenance())
    }
}

/// Current server info for the transport shell.
pub fn server_info() -> ServerInfo {
    ServerInfo {
        name: "oxvba-lsp".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

/// Current advertised capabilities.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..TextDocumentSyncOptions::default()
            },
        )),
        ..ServerCapabilities::default()
    }
}

fn resolve_document_id(state: &TransportState, uri: &Url) -> Result<DocumentId, String> {
    if let Some(id) = state.uri_documents.get(uri) {
        return Ok(id.clone());
    }

    let candidate = uri_module_candidate(uri)
        .ok_or_else(|| format!("cannot derive a module identity from uri `{uri}`"))?;
    let candidate_lower = candidate.to_ascii_lowercase();

    let mut exact_matches = state
        .service
        .workspace
        .document_ids()
        .filter(|id| id.0.eq_ignore_ascii_case(&candidate))
        .cloned()
        .collect::<Vec<_>>();
    if exact_matches.len() == 1 {
        return Ok(exact_matches.remove(0));
    }

    let mut suffix_matches = state
        .service
        .workspace
        .document_ids()
        .filter(|id| {
            id.0.rsplit("::")
                .next()
                .map(|suffix| suffix.eq_ignore_ascii_case(&candidate_lower))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    if suffix_matches.len() == 1 {
        return Ok(suffix_matches.remove(0));
    }

    Ok(DocumentId::new(candidate))
}

fn uri_module_candidate(uri: &Url) -> Option<String> {
    let segment = uri
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .or_else(|| {
            let path = uri.path().trim_matches('/');
            (!path.is_empty()).then_some(path)
        })?;

    let stem = segment.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(segment);
    (!stem.is_empty()).then(|| stem.to_string())
}

fn load_workspace_target(path: &Path) -> Result<oxvba_project::LoadedProject, String> {
    if path.is_dir() {
        if let Some(basproj) = discover_project_file_in_dir(path, "basproj")? {
            return oxvba_project::load_basproj(&basproj).map_err(|err| err.to_string());
        }
        if let Some(vbp) = discover_project_file_in_dir(path, "vbp")? {
            return oxvba_project::load_vbp(&vbp).map_err(|err| err.to_string());
        }
        return load_convention_project(path);
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("vbp") => oxvba_project::load_vbp(path).map_err(|err| err.to_string()),
        Some("basproj") | None => oxvba_project::load_basproj(path).map_err(|err| err.to_string()),
        Some(other) => Err(format!(
            "unsupported workspace target `{}` with extension `.{other}`",
            path.display()
        )),
    }
}

fn discover_project_file_in_dir(dir: &Path, extension: &str) -> Result<Option<PathBuf>, String> {
    let entries = fs::read_dir(dir).map_err(|source| {
        oxvba_project::BasProjError::Io {
            path: dir.display().to_string(),
            source,
        }
        .to_string()
    })?;
    let mut matches = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(extension))
        .collect::<Vec<_>>();
    matches.sort();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.into_iter().next()),
        _ => Err(
            oxvba_project::BasProjError::ProjectDiscoveryAmbiguous {
                directory: dir.display().to_string(),
                kind: extension.to_string(),
                candidates: matches
                    .into_iter()
                    .map(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                            .to_string()
                    })
                    .collect(),
            }
            .to_string(),
        ),
    }
}

fn load_convention_project(project_dir: &Path) -> Result<oxvba_project::LoadedProject, String> {
    let project_name = oxvba_project::infer_project_name_from_path(project_dir);
    let xml = format!(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>{}</ProjectName>\n  </PropertyGroup>\n</Project>\n",
        xml_escape(&project_name)
    );
    oxvba_project::load_basproj_from_str(&xml, project_dir).map_err(|err| err.to_string())
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{OxvbaLspCore, server_capabilities, server_info};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{TextDocumentSyncCapability, TextDocumentSyncKind, Url};

    #[test]
    fn bootstrap_server_info_is_stable() {
        let info = server_info();
        assert_eq!(info.name, "oxvba-lsp");
        assert!(info.version.is_some());
    }

    #[test]
    fn server_advertises_full_text_sync() {
        let capabilities = server_capabilities();
        let Some(TextDocumentSyncCapability::Options(options)) = capabilities.text_document_sync
        else {
            panic!("expected explicit text document sync options");
        };
        assert_eq!(options.open_close, Some(true));
        assert_eq!(options.change, Some(TextDocumentSyncKind::FULL));
    }

    #[test]
    fn open_change_close_tracks_unsaved_document() {
        let core = OxvbaLspCore::new();
        let uri = Url::parse("file:///workspace/Module1.bas").expect("uri");

        let opened = core
            .open_text_document(&uri, "Sub Main()\nEnd Sub\n")
            .expect("open");
        assert_eq!(opened.0, "Module1");
        assert_eq!(core.document_count(), 1);
        assert_eq!(
            core.document_source(&opened).as_deref(),
            Some("Sub Main()\nEnd Sub\n")
        );

        core.change_text_document(&uri, "Sub Main()\n    Print 1\nEnd Sub\n")
            .expect("change");
        assert_eq!(
            core.document_source(&opened).as_deref(),
            Some("Sub Main()\n    Print 1\nEnd Sub\n")
        );

        core.close_text_document(&uri).expect("close");
        assert_eq!(core.document_count(), 0);
        assert!(core.synchronized_document_id(&uri).is_none());
    }

    #[test]
    fn close_restores_loaded_project_document_source() {
        let temp_root = unique_temp_dir("oxvba_lsp_sync_project");
        fs::create_dir_all(&temp_root).expect("temp dir");
        let module_path = temp_root.join("Module1.bas");
        fs::write(&module_path, "Sub Main()\nEnd Sub\n").expect("module write");
        let basproj_path = temp_root.join("App.basproj");
        fs::write(
            &basproj_path,
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj write");

        let core = OxvbaLspCore::new();
        core.load_workspace_path(&temp_root).expect("load workspace");
        let uri = Url::from_file_path(&module_path).expect("module uri");

        let module_id = core
            .open_text_document(&uri, "Sub Main()\n    Print 1\nEnd Sub\n")
            .expect("open module");
        assert_eq!(module_id.0, "Module1");
        assert_eq!(
            core.document_source(&module_id).as_deref(),
            Some("Sub Main()\n    Print 1\nEnd Sub\n")
        );

        core.close_text_document(&uri).expect("close module");
        assert_eq!(
            core.document_source(&module_id).as_deref(),
            Some("Sub Main()\nEnd Sub\n")
        );

        let provenance = core
            .semantic_provenance(&module_id)
            .expect("restored document provenance");
        assert_eq!(provenance.project_name.as_deref(), Some("App"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nonce}"))
    }
}
