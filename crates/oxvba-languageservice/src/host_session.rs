use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxvba_project::load_workspace_target;

use crate::document::DocumentId;
use crate::service::{
    CompletionItem, DocumentSymbol, HoverInfo, LanguageService, Location, Position,
    WorkspaceSymbol,
};
use crate::span::{SemanticProvenance, SpannedDiagnostic, SymbolProvenanceKind};
use crate::workspace::WorkspaceStats;

#[derive(Debug, Clone)]
struct BaselineDocument {
    source: String,
    project_name: Option<String>,
    provenance_kind: SymbolProvenanceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostWorkspaceDocument {
    pub id: DocumentId,
    pub project_name: Option<String>,
    pub provenance_kind: SymbolProvenanceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostSessionError {
    WorkspaceLoad {
        path: PathBuf,
        message: String,
    },
    DocumentNotFound {
        document: DocumentId,
    },
}

impl std::fmt::Display for HostSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostSessionError::WorkspaceLoad { path, message } => {
                write!(f, "failed to load workspace `{}`: {message}", path.display())
            }
            HostSessionError::DocumentNotFound { document } => {
                write!(f, "document `{document}` is not part of the loaded workspace")
            }
        }
    }
}

impl std::error::Error for HostSessionError {}

/// Direct host-facing workspace/document session over the real OxVba project model.
///
/// This is intended for direct-embed IDE hosts such as OxIde. It owns no second
/// parser or semantic model; it layers typed session behavior over canonical
/// workspace loading and the existing `LanguageService` query surface.
pub struct HostWorkspaceSession {
    workspace_target: PathBuf,
    service: LanguageService,
    baseline_documents: HashMap<DocumentId, BaselineDocument>,
}

impl HostWorkspaceSession {
    pub fn load_workspace_path(path: &Path) -> Result<Self, HostSessionError> {
        let loaded = load_workspace_target(path).map_err(|err| HostSessionError::WorkspaceLoad {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;
        let service = LanguageService::from_project(loaded.manifest);
        let baseline_documents = collect_baseline_documents(&service);
        Ok(Self {
            workspace_target: path.to_path_buf(),
            service,
            baseline_documents,
        })
    }

    pub fn workspace_target(&self) -> &Path {
        &self.workspace_target
    }

    pub fn reload_workspace(&mut self) -> Result<(), HostSessionError> {
        let reloaded = Self::load_workspace_path(&self.workspace_target)?;
        *self = reloaded;
        Ok(())
    }

    pub fn workspace_stats(&self) -> WorkspaceStats {
        self.service.workspace.stats()
    }

    pub fn documents(&self) -> Vec<HostWorkspaceDocument> {
        let mut docs = self
            .service
            .workspace
            .document_ids()
            .filter_map(|id| self.service.workspace.document(id))
            .map(|doc| HostWorkspaceDocument {
                id: doc.id.clone(),
                project_name: doc.project_name.clone(),
                provenance_kind: doc.provenance_kind,
            })
            .collect::<Vec<_>>();
        docs.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        docs
    }

    pub fn document_source(&self, document: &DocumentId) -> Result<String, HostSessionError> {
        self.service
            .workspace
            .document(document)
            .map(|doc| doc.source.to_string())
            .ok_or_else(|| HostSessionError::DocumentNotFound {
                document: document.clone(),
            })
    }

    pub fn set_document_text(
        &mut self,
        document: &DocumentId,
        source: &str,
    ) -> Result<(), HostSessionError> {
        if self.service.workspace.document(document).is_none() {
            return Err(HostSessionError::DocumentNotFound {
                document: document.clone(),
            });
        }
        self.service.workspace.change_document(document, source);
        Ok(())
    }

    pub fn close_document(&mut self, document: &DocumentId) -> Result<(), HostSessionError> {
        let baseline = self.baseline_documents.get(document).cloned().ok_or_else(|| {
            HostSessionError::DocumentNotFound {
                document: document.clone(),
            }
        })?;
        self.service.workspace.open_document_with_origin(
            document.clone(),
            &baseline.source,
            baseline.project_name,
            baseline.provenance_kind,
        );
        Ok(())
    }

    pub fn diagnostics(&self, document: &DocumentId) -> Result<Vec<SpannedDiagnostic>, HostSessionError> {
        self.ensure_document(document)?;
        Ok(self.service.diagnostics(document))
    }

    pub fn document_symbols(
        &self,
        document: &DocumentId,
    ) -> Result<Vec<DocumentSymbol>, HostSessionError> {
        self.ensure_document(document)?;
        Ok(self.service.document_symbols(document))
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.service.workspace_symbols(query)
    }

    pub fn completions(
        &self,
        document: &DocumentId,
        position: Position,
    ) -> Result<Vec<CompletionItem>, HostSessionError> {
        self.ensure_document(document)?;
        Ok(self.service.completions(document, position))
    }

    pub fn hover(
        &self,
        document: &DocumentId,
        position: Position,
    ) -> Result<Option<HoverInfo>, HostSessionError> {
        self.ensure_document(document)?;
        Ok(self.service.hover(document, position))
    }

    pub fn go_to_definition(
        &self,
        document: &DocumentId,
        position: Position,
    ) -> Result<Option<Location>, HostSessionError> {
        self.ensure_document(document)?;
        Ok(self.service.go_to_definition(document, position))
    }

    pub fn find_references(
        &self,
        document: &DocumentId,
        position: Position,
    ) -> Result<Vec<Location>, HostSessionError> {
        self.ensure_document(document)?;
        Ok(self.service.find_references(document, position))
    }

    pub fn semantic_provenance(
        &self,
        document: &DocumentId,
    ) -> Result<SemanticProvenance, HostSessionError> {
        self.service
            .workspace
            .document(document)
            .map(|doc| doc.semantic_provenance())
            .ok_or_else(|| HostSessionError::DocumentNotFound {
                document: document.clone(),
            })
    }

    fn ensure_document(&self, document: &DocumentId) -> Result<(), HostSessionError> {
        if self.service.workspace.document(document).is_some() {
            Ok(())
        } else {
            Err(HostSessionError::DocumentNotFound {
                document: document.clone(),
            })
        }
    }
}

fn collect_baseline_documents(
    service: &LanguageService,
) -> HashMap<DocumentId, BaselineDocument> {
    let mut baseline_documents = HashMap::new();
    let document_ids = service.workspace.document_ids().cloned().collect::<Vec<_>>();
    for id in document_ids {
        if let Some(doc) = service.workspace.document(&id) {
            baseline_documents.insert(
                id,
                BaselineDocument {
                    source: doc.source.to_string(),
                    project_name: doc.project_name.clone(),
                    provenance_kind: doc.provenance_kind,
                },
            );
        }
    }
    baseline_documents
}

#[cfg(test)]
mod tests {
    use super::{HostSessionError, HostWorkspaceSession};
    use crate::document::DocumentId;
    use crate::span::SymbolProvenanceKind;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn host_session_loads_workspace_documents() {
        let temp_root = unique_temp_dir("oxvba_host_session_load");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let session = HostWorkspaceSession::load_workspace_path(&temp_root).expect("session");
        let documents = session.documents();
        let module = documents
            .iter()
            .find(|document| document.id == DocumentId::new("Module1"))
            .expect("root module should be present");
        assert_eq!(module.project_name.as_deref(), Some("App"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_restores_baseline_document_source_on_close() {
        let temp_root = unique_temp_dir("oxvba_host_session_restore");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let mut session = HostWorkspaceSession::load_workspace_path(&temp_root).expect("session");
        let module = DocumentId::new("Module1");
        session
            .set_document_text(&module, "Sub Main()\n    Print 1\nEnd Sub\n")
            .expect("change");
        assert_eq!(
            session.document_source(&module).as_deref(),
            Ok("Sub Main()\n    Print 1\nEnd Sub\n")
        );

        session.close_document(&module).expect("restore");
        assert_eq!(
            session.document_source(&module).as_deref(),
            Ok("Sub Main()\nEnd Sub\n")
        );
        assert_eq!(
            session.semantic_provenance(&module).map(|p| p.project_name),
            Ok(Some("App".to_string()))
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_rejects_documents_outside_loaded_workspace() {
        let temp_root = unique_temp_dir("oxvba_host_session_missing");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let session = HostWorkspaceSession::load_workspace_path(&temp_root).expect("session");
        let err = session
            .diagnostics(&DocumentId::new("Module2"))
            .expect_err("unknown module should fail");
        assert_eq!(
            err,
            HostSessionError::DocumentNotFound {
                document: DocumentId::new("Module2")
            }
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_includes_referenced_project_documents() {
        let temp_root = unique_temp_dir("oxvba_host_session_refs");
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

        let session = HostWorkspaceSession::load_workspace_path(&app_dir).expect("session");
        let documents = session.documents();
        assert!(documents.iter().any(|document| document.id == DocumentId::new("Module1")));
        let referenced = documents
            .iter()
            .find(|document| document.id == DocumentId::new("Lib::Helpers"))
            .expect("referenced document");
        assert_eq!(referenced.project_name.as_deref(), Some("Lib"));
        assert_eq!(referenced.provenance_kind, SymbolProvenanceKind::ProjectReference);

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
