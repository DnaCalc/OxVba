use std::collections::HashMap;
use std::sync::Arc;

use oxvba_compiler::{ProjectManifest, ReferenceKind, ReferencedProjectManifest};

use crate::document::{Document, DocumentId};
use crate::semantic::{SemanticSnapshot, build_semantic_snapshot_with_provenance};
use crate::span::{SymbolInfo, SymbolProvenanceKind};

/// The workspace model: manages a set of documents and their semantic snapshots.
///
/// On `change_document`, only the changed module is re-analyzed. Other modules
/// serve cached results. This gives sub-100ms interactive latency for typical
/// VBA projects (20-200 modules).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkspaceStats {
    pub analysis_builds: u64,
    pub export_rebuilds: u64,
    pub open_operations: u64,
    pub change_operations: u64,
    pub close_operations: u64,
    pub project_loads: u64,
}

pub struct Workspace {
    documents: HashMap<DocumentId, Arc<Document>>,
    project: Option<ProjectManifest>,
    /// Cross-module exports: procedure/variable names → symbols from other modules.
    cross_module_exports: HashMap<String, Vec<SymbolInfo>>,
    stats: WorkspaceStats,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            documents: HashMap::new(),
            project: None,
            cross_module_exports: HashMap::new(),
            stats: WorkspaceStats::default(),
        }
    }

    pub fn with_project(mut self, manifest: ProjectManifest) -> Self {
        self.load_project_manifest(manifest);
        self
    }

    pub fn project(&self) -> Option<&ProjectManifest> {
        self.project.as_ref()
    }

    pub fn stats(&self) -> WorkspaceStats {
        self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = WorkspaceStats::default();
    }

    /// Open a new document in the workspace.
    pub fn open_document(&mut self, id: DocumentId, source: &str) {
        self.open_document_with_origin(id, source, None, SymbolProvenanceKind::SourceModule);
    }

    /// Open a new document in the workspace with explicit provenance.
    pub fn open_document_with_origin(
        &mut self,
        id: DocumentId,
        source: &str,
        project_name: Option<String>,
        provenance_kind: SymbolProvenanceKind,
    ) {
        self.stats.open_operations += 1;
        let doc = Document::new_with_origin(id, source, project_name, provenance_kind);
        self.insert_document(doc);
        self.rebuild_exports();
    }

    /// Update a document's source. Re-analyzes only this module.
    pub fn change_document(&mut self, id: &DocumentId, source: &str) {
        if let Some(existing) = self.documents.get(id) {
            self.stats.change_operations += 1;
            let doc = existing.with_source(source);
            self.stats.analysis_builds += 1;
            let snapshot = Arc::new(build_semantic_snapshot_with_provenance(
                source,
                doc.semantic_provenance(),
            ));
            let doc = doc.with_snapshot(snapshot);
            self.documents.insert(id.clone(), Arc::new(doc));
            self.rebuild_exports();
        }
    }

    /// Close a document, removing it from the workspace.
    pub fn close_document(&mut self, id: &DocumentId) {
        self.stats.close_operations += 1;
        self.documents.remove(id);
        self.rebuild_exports();
    }

    /// Get a document by ID.
    pub fn document(&self, id: &DocumentId) -> Option<&Arc<Document>> {
        self.documents.get(id)
    }

    /// Get the semantic snapshot for a document.
    pub fn snapshot(&self, id: &DocumentId) -> Option<&Arc<SemanticSnapshot>> {
        self.documents.get(id).and_then(|d| d.snapshot.as_ref())
    }

    /// Iterate over all document IDs.
    pub fn document_ids(&self) -> impl Iterator<Item = &DocumentId> {
        self.documents.keys()
    }

    /// Number of open documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Replace the workspace contents from a real OxVba project manifest.
    ///
    /// Root modules keep their plain module-name document identifiers for
    /// compatibility with the existing provider surface. Referenced-project
    /// modules are qualified as `Project::Module` to avoid collisions while
    /// still participating in cross-project queries.
    pub fn load_project_manifest(&mut self, manifest: ProjectManifest) {
        self.stats.project_loads += 1;
        self.project = Some(manifest.clone());
        self.documents.clear();

        for module in &manifest.modules {
            self.open_document_with_origin(
                DocumentId::new(module.module_name.clone()),
                &module.source,
                Some(manifest.project_name.clone()),
                SymbolProvenanceKind::SourceModule,
            );
        }

        let reference_kinds: HashMap<String, ReferenceKind> = manifest
            .references
            .iter()
            .map(|reference| {
                (
                    reference.referenced_project_name.to_ascii_lowercase(),
                    reference.reference_kind,
                )
            })
            .collect();

        for reference in &manifest.reference_projects {
            let provenance_kind = reference_kinds
                .get(&reference.project_name.to_ascii_lowercase())
                .copied()
                .map(|kind| match kind {
                    ReferenceKind::TypeLibrary => {
                        SymbolProvenanceKind::ImportedTypeLibraryProjection
                    }
                    _ => SymbolProvenanceKind::ProjectReference,
                })
                .unwrap_or(SymbolProvenanceKind::ProjectReference);
            self.load_referenced_project(reference, provenance_kind);
        }

        self.rebuild_exports();
    }

    /// Get cross-module exports for a given name (case-insensitive).
    pub fn cross_module_symbols(&self, name: &str) -> &[SymbolInfo] {
        let lower = name.to_ascii_lowercase();
        self.cross_module_exports
            .get(&lower)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Rebuild the cross-module export table from all document snapshots.
    fn rebuild_exports(&mut self) {
        self.stats.export_rebuilds += 1;
        self.cross_module_exports.clear();
        for doc in self.documents.values() {
            if let Some(snap) = &doc.snapshot {
                for sym in &snap.symbols.symbols {
                    // Only module-level (scope=0) public symbols are cross-module
                    if sym.scope == 0 {
                        let lower = sym.name.to_ascii_lowercase();
                        self.cross_module_exports
                            .entry(lower)
                            .or_default()
                            .push(sym.clone());
                    }
                }
            }
        }
    }

    fn load_referenced_project(
        &mut self,
        reference: &ReferencedProjectManifest,
        provenance_kind: SymbolProvenanceKind,
    ) {
        for module in &reference.modules {
            self.open_document_with_origin(
                DocumentId::new(format!(
                    "{}::{}",
                    reference.project_name, module.module_name
                )),
                &module.source,
                Some(reference.project_name.clone()),
                provenance_kind,
            );
        }
    }

    fn insert_document(&mut self, doc: Document) {
        let id = doc.id.clone();
        self.stats.analysis_builds += 1;
        let snapshot = Arc::new(build_semantic_snapshot_with_provenance(
            doc.source.as_ref(),
            doc.semantic_provenance(),
        ));
        let doc = doc.with_snapshot(snapshot);
        self.documents.insert(id, Arc::new(doc));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::span::SymbolProvenanceKind;
    use oxvba_compiler::{
        ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ReferencedProjectManifest,
    };

    #[test]
    fn workspace_open_and_query() {
        let mut ws = Workspace::new();
        let id = DocumentId::new("Module1");
        ws.open_document(id.clone(), "Sub Hello()\nEnd Sub\n");

        assert_eq!(ws.document_count(), 1);
        let snap = ws.snapshot(&id).expect("snapshot should exist");
        assert!(!snap.symbols.symbols.is_empty());
    }

    #[test]
    fn workspace_change_invalidates_snapshot() {
        let mut ws = Workspace::new();
        let id = DocumentId::new("Module1");
        ws.open_document(id.clone(), "Sub Foo()\nEnd Sub\n");

        let snap1 = ws.snapshot(&id).unwrap().clone();

        ws.change_document(&id, "Sub Bar()\nEnd Sub\n");

        let snap2 = ws.snapshot(&id).unwrap().clone();
        // Different source → different snapshot
        assert_ne!(snap1.source.as_ref(), snap2.source.as_ref());
    }

    #[test]
    fn workspace_change_reanalyzes_only_the_changed_document() {
        let manifest = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                ModuleUnit {
                    module_name: "Main".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Main".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub Main()\n    Helper\nEnd Sub\n".to_string(),
                },
                ModuleUnit {
                    module_name: "Helper".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Helper".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub Helper()\nEnd Sub\n".to_string(),
                },
            ],
            references: Vec::new(),
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let mut ws = Workspace::new().with_project(manifest);
        let helper_before = ws
            .snapshot(&DocumentId::new("Helper"))
            .expect("helper snapshot")
            .clone();
        let shared_before = ws
            .snapshot(&DocumentId::new("Core::Shared"))
            .expect("shared snapshot")
            .clone();

        ws.reset_stats();
        ws.change_document(
            &DocumentId::new("Main"),
            "Public Sub Main()\n    Helper\n    SharedProc\nEnd Sub\n",
        );

        let stats = ws.stats();
        assert_eq!(stats.change_operations, 1);
        assert_eq!(stats.analysis_builds, 1);
        assert_eq!(stats.export_rebuilds, 1);

        let helper_after = ws
            .snapshot(&DocumentId::new("Helper"))
            .expect("helper snapshot after change")
            .clone();
        let shared_after = ws
            .snapshot(&DocumentId::new("Core::Shared"))
            .expect("shared snapshot after change")
            .clone();

        assert!(
            Arc::ptr_eq(&helper_before, &helper_after),
            "unchanged helper snapshot should be reused"
        );
        assert!(
            Arc::ptr_eq(&shared_before, &shared_after),
            "unchanged referenced-project snapshot should be reused"
        );
    }

    #[test]
    fn workspace_cross_module_exports() {
        let mut ws = Workspace::new();
        ws.open_document(DocumentId::new("Mod1"), "Sub DoWork()\nEnd Sub\n");
        ws.open_document(DocumentId::new("Mod2"), "Sub Helper()\nEnd Sub\n");

        let exports = ws.cross_module_symbols("dowork");
        assert!(
            !exports.is_empty(),
            "expected cross-module export for DoWork"
        );
    }

    #[test]
    fn workspace_close_removes_document() {
        let mut ws = Workspace::new();
        let id = DocumentId::new("Module1");
        ws.open_document(id.clone(), "Sub Test()\nEnd Sub\n");
        assert_eq!(ws.document_count(), 1);

        ws.close_document(&id);
        assert_eq!(ws.document_count(), 0);
    }

    #[test]
    fn workspace_load_project_manifest_includes_reference_documents() {
        let manifest = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\nEnd Sub\n".to_string(),
            }],
            references: Vec::new(),
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let ws = Workspace::new().with_project(manifest);

        assert_eq!(ws.document_count(), 2);
        assert!(ws.document(&DocumentId::new("Main")).is_some());
        let reference = ws
            .document(&DocumentId::new("Core::Shared"))
            .expect("reference document should be loaded");
        assert_eq!(
            reference.provenance_kind,
            SymbolProvenanceKind::ProjectReference
        );
        assert_eq!(reference.project_name.as_deref(), Some("Core"));
    }
}
