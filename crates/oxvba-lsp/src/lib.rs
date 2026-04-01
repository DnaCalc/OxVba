//! Thin LSP transport bootstrap over the direct OxVba language-service API.
//!
//! This crate intentionally owns transport/session concerns only. Semantic
//! parsing, binding, and query behavior remain in `oxvba-languageservice`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use oxvba_compiler::ProjectManifest;
use oxvba_languageservice::{
    DocumentId, LanguageService, SemanticProvenance, SpannedDiagnostic, SymbolProvenanceKind,
    Workspace,
};
use oxvba_project::load_workspace_target as load_project_workspace_target;
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
        state.uri_documents.clear();
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

    pub fn workspace_documents(&self) -> Vec<DocumentId> {
        let state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        let mut documents = state
            .service
            .workspace
            .document_ids()
            .cloned()
            .collect::<Vec<_>>();
        documents.sort_by(|left, right| left.0.cmp(&right.0));
        documents
    }

    pub fn document_diagnostics(&self, id: &DocumentId) -> Vec<SpannedDiagnostic> {
        let state = self
            .state
            .lock()
            .expect("oxvba-lsp transport state mutex poisoned");
        state.service.diagnostics(id)
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

    if exact_matches.is_empty() && suffix_matches.is_empty() {
        return Err(format!(
            "uri `{uri}` does not map to a loaded workspace document; add the file to the project or load the correct workspace first"
        ));
    }

    Err(format!(
        "uri `{uri}` is ambiguous within the loaded workspace for candidate `{candidate}`"
    ))
}

fn uri_module_candidate(uri: &Url) -> Option<String> {
    let segment = uri
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .or_else(|| {
            let path = uri.path().trim_matches('/');
            (!path.is_empty()).then_some(path)
        })?;

    let stem = segment
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(segment);
    (!stem.is_empty()).then(|| stem.to_string())
}

fn load_workspace_target(path: &Path) -> Result<oxvba_project::LoadedProject, String> {
    load_project_workspace_target(path).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{OxvbaLspCore, server_capabilities, server_info};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
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
    fn open_change_close_tracks_loaded_workspace_document() {
        let temp_root = unique_temp_dir("oxvba_lsp_sync_loaded_workspace");
        fs::create_dir_all(&temp_root).expect("temp dir");
        let module_path = temp_root.join("Module1.bas");
        fs::write(&module_path, "Sub Main()\nEnd Sub\n").expect("module write");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj write");

        let core = OxvbaLspCore::new();
        core.load_workspace_path(&temp_root)
            .expect("load workspace");
        let uri = Url::from_file_path(&module_path).expect("uri");

        let opened = core
            .open_text_document(&uri, "Sub Main()\n    Print 1\nEnd Sub\n")
            .expect("open");
        assert_eq!(opened.0, "Module1");
        assert_eq!(core.document_count(), 2);
        assert_eq!(
            core.document_source(&opened).as_deref(),
            Some("Sub Main()\n    Print 1\nEnd Sub\n")
        );

        core.change_text_document(&uri, "Sub Main()\n    Print 2\nEnd Sub\n")
            .expect("change");
        assert_eq!(
            core.document_source(&opened).as_deref(),
            Some("Sub Main()\n    Print 2\nEnd Sub\n")
        );

        core.close_text_document(&uri).expect("close");
        assert_eq!(
            core.document_source(&opened).as_deref(),
            Some("Sub Main()\nEnd Sub\n")
        );
        assert!(core.synchronized_document_id(&uri).is_none());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn open_rejects_uri_outside_loaded_workspace() {
        let temp_root = unique_temp_dir("oxvba_lsp_sync_reject");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module write");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj write");

        let core = OxvbaLspCore::new();
        core.load_workspace_path(&temp_root)
            .expect("load workspace");

        let stray_uri = Url::from_file_path(temp_root.join("Module2.bas")).expect("stray uri");
        let err = core
            .open_text_document(&stray_uri, "Sub Main()\nEnd Sub\n")
            .expect_err("untracked document should fail");
        assert!(err.contains("does not map to a loaded workspace document"));

        let _ = fs::remove_dir_all(&temp_root);
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
        core.load_workspace_path(&temp_root)
            .expect("load workspace");
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

    #[test]
    fn reload_clears_stale_uri_document_mappings() {
        let temp_root = unique_temp_dir("oxvba_lsp_reload_workspace");
        let first_dir = temp_root.join("First");
        let second_dir = temp_root.join("Second");
        fs::create_dir_all(&first_dir).expect("first dir");
        fs::create_dir_all(&second_dir).expect("second dir");

        let first_module_path = first_dir.join("Module1.bas");
        fs::write(&first_module_path, "Sub Main()\nEnd Sub\n").expect("first module");
        fs::write(
            first_dir.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>First</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("first basproj");

        fs::write(second_dir.join("Module2.bas"), "Sub Main()\nEnd Sub\n").expect("second module");
        fs::write(
            second_dir.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>Second</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module2.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("second basproj");

        let core = OxvbaLspCore::new();
        core.load_workspace_path(&first_dir)
            .expect("first workspace load");
        let first_uri = Url::from_file_path(&first_module_path).expect("first uri");
        core.open_text_document(&first_uri, "Sub Main()\n    Print 1\nEnd Sub\n")
            .expect("open first module");
        assert!(core.synchronized_document_id(&first_uri).is_some());

        core.load_workspace_path(&second_dir)
            .expect("second workspace load");
        assert!(core.synchronized_document_id(&first_uri).is_none());

        let documents = core
            .workspace_documents()
            .into_iter()
            .map(|document| document.0)
            .collect::<Vec<_>>();
        assert!(
            documents.iter().any(|document| document == "Module2"),
            "expected reloaded workspace document set to include Module2, got: {documents:?}"
        );
        assert!(
            documents.iter().all(|document| document != "Module1"),
            "expected reloaded workspace document set to drop Module1, got: {documents:?}"
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn workspace_load_includes_referenced_project_documents() {
        let temp_root = unique_temp_dir("oxvba_lsp_ref_workspace");
        let app_dir = temp_root.join("App");
        let lib_dir = temp_root.join("Lib");
        fs::create_dir_all(&app_dir).expect("app dir");
        fs::create_dir_all(&lib_dir).expect("lib dir");

        fs::write(
            lib_dir.join("Helpers.bas"),
            "Public Function DoubleIt(ByVal x As Long) As Long\n    DoubleIt = x * 2\nEnd Function\n",
        )
        .expect("lib module");
        fs::write(
            lib_dir.join("Lib.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Library</OutputType>\n    <ProjectName>Lib</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Helpers.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("lib basproj");

        fs::write(
            app_dir.join("Module1.bas"),
            "Sub Main()\n    Print DoubleIt(2)\nEnd Sub\n",
        )
        .expect("app module");
        fs::write(
            app_dir.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n    <ProjectReference Include=\"..\\Lib\\Lib.basproj\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("app basproj");

        let core = OxvbaLspCore::new();
        core.load_workspace_path(&app_dir)
            .expect("load app workspace");

        let documents = core
            .workspace_documents()
            .into_iter()
            .map(|document| document.0)
            .collect::<Vec<_>>();
        assert!(
            documents.iter().any(|document| document == "Module1"),
            "expected root module in workspace, got: {documents:?}"
        );
        assert!(
            documents.iter().any(|document| document == "Lib::Helpers"),
            "expected referenced-project module in workspace, got: {documents:?}"
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn sync_round_trip_stays_within_local_editor_budget() {
        let temp_root = unique_temp_dir("oxvba_lsp_perf_workspace");
        fs::create_dir_all(&temp_root).expect("temp dir");

        let mut modules_xml = String::new();
        for index in 0..25 {
            let module_name = format!("Module{index}");
            let module_path = temp_root.join(format!("{module_name}.bas"));
            fs::write(
                &module_path,
                format!(
                    "Public Function F{index}() As Long\n    F{index} = {index}\nEnd Function\n"
                ),
            )
            .expect("module write");
            modules_xml.push_str(&format!("    <Module Include=\"{module_name}.bas\" />\n"));
        }
        fs::write(
            temp_root.join("Perf.basproj"),
            format!(
                "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Library</OutputType>\n    <ProjectName>Perf</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n{modules_xml}  </ItemGroup>\n</Project>\n"
            ),
        )
        .expect("basproj write");

        let core = OxvbaLspCore::new();
        let load_start = Instant::now();
        core.load_workspace_path(&temp_root)
            .expect("load workspace");
        let load_elapsed = load_start.elapsed();

        let uri = Url::from_file_path(temp_root.join("Module0.bas")).expect("uri");
        let change_start = Instant::now();
        core.open_text_document(
            &uri,
            "Public Function F0() As Long\n    F0 = 10\nEnd Function\n",
        )
        .expect("open");
        core.change_text_document(
            &uri,
            "Public Function F0() As Long\n    F0 = 20\nEnd Function\n",
        )
        .expect("change");
        core.close_text_document(&uri).expect("close");
        let change_elapsed = change_start.elapsed();

        let budget = Duration::from_secs(3);
        assert!(
            load_elapsed < budget,
            "workspace load exceeded local editor budget: {:?}",
            load_elapsed
        );
        assert!(
            change_elapsed < budget,
            "sync round-trip exceeded local editor budget: {:?}",
            change_elapsed
        );

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
