use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxvba_compiler::ProjectManifest;
use oxvba_host::{
    EmbeddedExecutionSourcePolicy, EmbeddedWorkspaceInput, EmbeddedWorkspaceSnapshot,
};
use oxvba_project::load_workspace_target;

use crate::document::DocumentId;
use crate::service::{
    CompletionItem, DocumentSymbol, HoverInfo, LanguageService, Location, Position, WorkspaceSymbol,
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
    WorkspaceLoad { path: PathBuf, message: String },
    DocumentNotFound { document: DocumentId },
    ProjectSnapshotUnavailable { path: PathBuf },
    WorkspaceTargetMismatch { requested: PathBuf, loaded: PathBuf },
}

impl std::fmt::Display for HostSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostSessionError::WorkspaceLoad { path, message } => {
                write!(
                    f,
                    "failed to load workspace `{}`: {message}",
                    path.display()
                )
            }
            HostSessionError::DocumentNotFound { document } => {
                write!(
                    f,
                    "document `{document}` is not part of the loaded workspace"
                )
            }
            HostSessionError::ProjectSnapshotUnavailable { path } => {
                write!(
                    f,
                    "workspace `{}` does not have a canonical project snapshot available",
                    path.display()
                )
            }
            HostSessionError::WorkspaceTargetMismatch { requested, loaded } => {
                write!(
                    f,
                    "embedded workspace target `{}` does not match loaded workspace `{}`",
                    requested.display(),
                    loaded.display()
                )
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
        let loaded =
            load_workspace_target(path).map_err(|err| HostSessionError::WorkspaceLoad {
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

    pub fn project_manifest_snapshot(&self) -> Result<ProjectManifest, HostSessionError> {
        let mut manifest = self.service.workspace.project().cloned().ok_or_else(|| {
            HostSessionError::ProjectSnapshotUnavailable {
                path: self.workspace_target.clone(),
            }
        })?;
        apply_workspace_sources_to_manifest(&self.service, &mut manifest);
        Ok(manifest)
    }

    pub fn prepare_embedded_workspace_snapshot(
        &self,
        workspace: &EmbeddedWorkspaceInput,
    ) -> Result<EmbeddedWorkspaceSnapshot, HostSessionError> {
        if !same_workspace_target_path(self.workspace_target(), workspace.path()) {
            return Err(HostSessionError::WorkspaceTargetMismatch {
                requested: workspace.workspace_target.clone(),
                loaded: self.workspace_target.clone(),
            });
        }

        let manifest = match workspace.source_policy {
            EmbeddedExecutionSourcePolicy::DiskOnly => {
                self.service.workspace.project().cloned().ok_or_else(|| {
                    HostSessionError::ProjectSnapshotUnavailable {
                        path: self.workspace_target.clone(),
                    }
                })?
            }
            EmbeddedExecutionSourcePolicy::WorkspaceOverlay => self.project_manifest_snapshot()?,
        };

        Ok(EmbeddedWorkspaceSnapshot::new(workspace.clone(), manifest))
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
        let baseline = self
            .baseline_documents
            .get(document)
            .cloned()
            .ok_or_else(|| HostSessionError::DocumentNotFound {
                document: document.clone(),
            })?;
        self.service.workspace.open_document_with_origin(
            document.clone(),
            &baseline.source,
            baseline.project_name,
            baseline.provenance_kind,
        );
        Ok(())
    }

    pub fn diagnostics(
        &self,
        document: &DocumentId,
    ) -> Result<Vec<SpannedDiagnostic>, HostSessionError> {
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

fn collect_baseline_documents(service: &LanguageService) -> HashMap<DocumentId, BaselineDocument> {
    let mut baseline_documents = HashMap::new();
    let document_ids = service
        .workspace
        .document_ids()
        .cloned()
        .collect::<Vec<_>>();
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

fn apply_workspace_sources_to_manifest(service: &LanguageService, manifest: &mut ProjectManifest) {
    for module in &mut manifest.modules {
        let id = DocumentId::new(module.module_name.clone());
        if let Some(doc) = service.workspace.document(&id) {
            module.source = doc.source.to_string();
        }
    }

    for reference in &mut manifest.reference_projects {
        for module in &mut reference.modules {
            let id = DocumentId::new(format!(
                "{}::{}",
                reference.project_name, module.module_name
            ));
            if let Some(doc) = service.workspace.document(&id) {
                module.source = doc.source.to_string();
            }
        }
    }
}

fn same_workspace_target_path(left: &Path, right: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (left.canonicalize(), right.canonicalize()) {
        left == right
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::{HostSessionError, HostWorkspaceSession};
    use crate::document::DocumentId;
    use crate::span::SymbolProvenanceKind;
    use oxvba_compiler::{ModuleKind, module_unit_from_source};
    use oxvba_host::{
        EmbeddedBuildRequest, EmbeddedBuildRunHost, EmbeddedBuildStatus,
        EmbeddedExecutionSourcePolicy, EmbeddedInvokeProcedureRequest, EmbeddedProcedureTarget,
        EmbeddedResetKind, EmbeddedResetRequest, EmbeddedRunRequest, EmbeddedWorkspaceInput,
        Engine, HostConfig,
    };
    use oxvba_runtime::RuntimeValue;
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
        assert!(
            documents
                .iter()
                .any(|document| document.id == DocumentId::new("Module1"))
        );
        let referenced = documents
            .iter()
            .find(|document| document.id == DocumentId::new("Lib::Helpers"))
            .expect("referenced document");
        assert_eq!(referenced.project_name.as_deref(), Some("Lib"));
        assert_eq!(
            referenced.provenance_kind,
            SymbolProvenanceKind::ProjectReference
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_project_manifest_snapshot_uses_live_overlay_source() {
        let temp_root = unique_temp_dir("oxvba_host_session_snapshot");
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
            .set_document_text(&module, "Sub Main()\n    Print 42\nEnd Sub\n")
            .expect("overlay");

        let manifest = session
            .project_manifest_snapshot()
            .expect("manifest snapshot");
        let module = manifest
            .modules
            .iter()
            .find(|module| module.module_name == "Module1")
            .expect("Module1 manifest");
        assert_eq!(module.source, "Sub Main()\n    Print 42\nEnd Sub\n");

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_prepares_embedded_workspace_snapshot_for_disk_and_overlay_modes() {
        let temp_root = unique_temp_dir("oxvba_host_session_embedded_snapshot");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let mut session = HostWorkspaceSession::load_workspace_path(&temp_root).expect("session");
        session
            .set_document_text(
                &DocumentId::new("Module1"),
                "Sub Main()\n    Print 42\nEnd Sub\n",
            )
            .expect("overlay");

        let disk_snapshot = session
            .prepare_embedded_workspace_snapshot(&EmbeddedWorkspaceInput::new(
                temp_root.clone(),
                EmbeddedExecutionSourcePolicy::DiskOnly,
            ))
            .expect("disk snapshot");
        let disk_module = disk_snapshot
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "Module1")
            .expect("disk Module1");
        assert_eq!(disk_module.source, "Sub Main()\nEnd Sub\n");

        let overlay_snapshot = session
            .prepare_embedded_workspace_snapshot(&EmbeddedWorkspaceInput::new(
                temp_root.clone(),
                EmbeddedExecutionSourcePolicy::WorkspaceOverlay,
            ))
            .expect("overlay snapshot");
        let overlay_module = overlay_snapshot
            .manifest
            .modules
            .iter()
            .find(|module| module.module_name == "Module1")
            .expect("overlay Module1");
        assert_eq!(overlay_module.source, "Sub Main()\n    Print 42\nEnd Sub\n");

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_rejects_embedded_snapshot_for_mismatched_target() {
        let temp_root = unique_temp_dir("oxvba_host_session_snapshot_mismatch");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(temp_root.join("Module1.bas"), "Sub Main()\nEnd Sub\n").expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let session = HostWorkspaceSession::load_workspace_path(&temp_root).expect("session");
        let other_target = temp_root.join("Other");
        let err = session
            .prepare_embedded_workspace_snapshot(&EmbeddedWorkspaceInput::new(
                other_target.clone(),
                EmbeddedExecutionSourcePolicy::WorkspaceOverlay,
            ))
            .expect_err("mismatched target should fail");
        assert_eq!(
            err,
            HostSessionError::WorkspaceTargetMismatch {
                requested: other_target,
                loaded: temp_root.clone(),
            }
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_embedded_round_trip_uses_disk_and_overlay_snapshots_independently() {
        let temp_root = unique_temp_dir("oxvba_host_session_embedded_round_trip");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Module1.bas"),
            "Public Sub Main()\nEnd Sub\n\
             Public Function GetValue() As Integer\n    GetValue = 1\nEnd Function\n",
        )
        .expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let mut host_session =
            HostWorkspaceSession::load_workspace_path(&temp_root).expect("host session");
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);

        let disk_snapshot = host_session
            .prepare_embedded_workspace_snapshot(&EmbeddedWorkspaceInput::new(
                temp_root.clone(),
                EmbeddedExecutionSourcePolicy::DiskOnly,
            ))
            .expect("disk snapshot");
        let disk_build = host.build_workspace(&EmbeddedBuildRequest::new(disk_snapshot.clone()));
        assert_eq!(disk_build.status, EmbeddedBuildStatus::Succeeded);
        let mut disk_run = host
            .run_project(&EmbeddedRunRequest::new(disk_snapshot))
            .expect("disk run");
        let disk_value = disk_run
            .invoke_procedure(&EmbeddedInvokeProcedureRequest::new(
                EmbeddedProcedureTarget::new("Module1", "GetValue"),
                Vec::new(),
            ))
            .expect("disk invoke");
        assert_eq!(disk_value.return_value, Some(RuntimeValue::I32(1)));

        host_session
            .set_document_text(
                &DocumentId::new("Module1"),
                "Public Sub Main()\nEnd Sub\n\
                 Public Function GetValue() As Integer\n    GetValue = 2\nEnd Function\n",
            )
            .expect("overlay");
        let overlay_snapshot = host_session
            .prepare_embedded_workspace_snapshot(&EmbeddedWorkspaceInput::new(
                temp_root.clone(),
                EmbeddedExecutionSourcePolicy::WorkspaceOverlay,
            ))
            .expect("overlay snapshot");
        let overlay_build =
            host.build_workspace(&EmbeddedBuildRequest::new(overlay_snapshot.clone()));
        assert_eq!(overlay_build.status, EmbeddedBuildStatus::Succeeded);
        let mut overlay_run = host
            .run_project(&EmbeddedRunRequest::new(overlay_snapshot))
            .expect("overlay run");
        let overlay_value = overlay_run
            .invoke_procedure(&EmbeddedInvokeProcedureRequest::new(
                EmbeddedProcedureTarget::new("Module1", "GetValue"),
                Vec::new(),
            ))
            .expect("overlay invoke");
        assert_eq!(overlay_value.return_value, Some(RuntimeValue::I32(2)));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn host_session_embedded_validation_separates_build_diagnostics_from_runtime_reset_flow() {
        let temp_root = unique_temp_dir("oxvba_host_session_embedded_validation");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Module1.bas"),
            "Public Sub Main()\nEnd Sub\n\
             Dim counter As Integer\n\
             Public Function IncrementCounter() As Integer\n\
                 counter = counter + 1\n\
                 IncrementCounter = counter\n\
             End Function\n",
        )
        .expect("module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <Module Include=\"Module1.bas\" />\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let host_session =
            HostWorkspaceSession::load_workspace_path(&temp_root).expect("host session");
        let engine = Engine::new(HostConfig::default());
        let host = EmbeddedBuildRunHost::new(&engine);

        let valid_snapshot = host_session
            .prepare_embedded_workspace_snapshot(&EmbeddedWorkspaceInput::new(
                temp_root.clone(),
                EmbeddedExecutionSourcePolicy::DiskOnly,
            ))
            .expect("valid snapshot");
        let mut invalid_manifest = valid_snapshot.manifest.clone();
        invalid_manifest.modules.push(
            module_unit_from_source("Module1", ModuleKind::Procedural, "Sub Other()\nEnd Sub\n")
                .expect("duplicate module"),
        );
        let invalid_snapshot = oxvba_host::EmbeddedWorkspaceSnapshot::new(
            valid_snapshot.workspace.clone(),
            invalid_manifest,
        );
        let failed_build = host.build_workspace(&EmbeddedBuildRequest::new(invalid_snapshot));
        assert_eq!(failed_build.status, EmbeddedBuildStatus::Failed);
        assert_eq!(failed_build.diagnostics.len(), 1);
        assert_eq!(
            failed_build.diagnostics[0].phase(),
            oxvba_host::DiagnosticPhase::CompileTime
        );

        let mut run_session = host
            .run_project(&EmbeddedRunRequest::new(valid_snapshot.clone()))
            .expect("run session");
        let first = run_session
            .invoke_procedure(&EmbeddedInvokeProcedureRequest::new(
                EmbeddedProcedureTarget::new("Module1", "IncrementCounter"),
                Vec::new(),
            ))
            .expect("first invoke");
        let second = run_session
            .invoke_procedure(&EmbeddedInvokeProcedureRequest::new(
                EmbeddedProcedureTarget::new("Module1", "IncrementCounter"),
                Vec::new(),
            ))
            .expect("second invoke");
        assert_eq!(first.return_value, Some(RuntimeValue::I32(1)));
        assert_eq!(second.return_value, Some(RuntimeValue::I32(2)));

        let reset = run_session
            .reset_runtime(&EmbeddedResetRequest::new(
                valid_snapshot.clone(),
                EmbeddedResetKind::ClearSessionState,
            ))
            .expect("reset");
        assert_eq!(reset.kind, EmbeddedResetKind::ClearSessionState);
        let after_reset = run_session
            .invoke_procedure(&EmbeddedInvokeProcedureRequest::new(
                EmbeddedProcedureTarget::new("Module1", "IncrementCounter"),
                Vec::new(),
            ))
            .expect("after reset");
        assert_eq!(after_reset.return_value, Some(RuntimeValue::I32(1)));

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
