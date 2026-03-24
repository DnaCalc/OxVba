use std::collections::HashMap;
use std::sync::Arc;

use oxvba_compiler::ProjectManifest;

use crate::document::{Document, DocumentId};
use crate::semantic::{SemanticSnapshot, build_semantic_snapshot};
use crate::span::SymbolInfo;

/// The workspace model: manages a set of documents and their semantic snapshots.
///
/// On `change_document`, only the changed module is re-analyzed. Other modules
/// serve cached results. This gives sub-100ms interactive latency for typical
/// VBA projects (20-200 modules).
pub struct Workspace {
    documents: HashMap<DocumentId, Arc<Document>>,
    project: Option<ProjectManifest>,
    /// Cross-module exports: procedure/variable names → symbols from other modules.
    cross_module_exports: HashMap<String, Vec<SymbolInfo>>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            documents: HashMap::new(),
            project: None,
            cross_module_exports: HashMap::new(),
        }
    }

    pub fn with_project(mut self, manifest: ProjectManifest) -> Self {
        self.project = Some(manifest);
        self
    }

    pub fn project(&self) -> Option<&ProjectManifest> {
        self.project.as_ref()
    }

    /// Open a new document in the workspace.
    pub fn open_document(&mut self, id: DocumentId, source: &str) {
        let doc = Document::new(id.clone(), source);
        let snapshot = Arc::new(build_semantic_snapshot(source));
        let doc = doc.with_snapshot(snapshot);
        self.documents.insert(id, Arc::new(doc));
        self.rebuild_exports();
    }

    /// Update a document's source. Re-analyzes only this module.
    pub fn change_document(&mut self, id: &DocumentId, source: &str) {
        if let Some(existing) = self.documents.get(id) {
            let doc = existing.with_source(source);
            let snapshot = Arc::new(build_semantic_snapshot(source));
            let doc = doc.with_snapshot(snapshot);
            self.documents.insert(id.clone(), Arc::new(doc));
            self.rebuild_exports();
        }
    }

    /// Close a document, removing it from the workspace.
    pub fn close_document(&mut self, id: &DocumentId) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
