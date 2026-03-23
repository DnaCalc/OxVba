use std::sync::Arc;

use crate::semantic::SemanticSnapshot;

/// Opaque document identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentId(pub String);

impl DocumentId {
    pub fn new(id: impl Into<String>) -> Self {
        DocumentId(id.into())
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An immutable snapshot of a single document.
///
/// Every edit produces a new `Document` with an incremented version.
/// `Arc` sharing means multiple references (background analysis, foreground
/// queries) coexist without locking.
#[derive(Clone)]
pub struct Document {
    pub id: DocumentId,
    pub version: u64,
    pub source: Arc<str>,
    pub snapshot: Option<Arc<SemanticSnapshot>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("id", &self.id)
            .field("version", &self.version)
            .field("source_len", &self.source.len())
            .field("has_snapshot", &self.snapshot.is_some())
            .finish()
    }
}

impl Document {
    pub fn new(id: DocumentId, source: &str) -> Self {
        Document {
            id,
            version: 1,
            source: source.into(),
            snapshot: None,
        }
    }

    /// Create a new version of this document with updated source.
    pub fn with_source(&self, source: &str) -> Self {
        Document {
            id: self.id.clone(),
            version: self.version + 1,
            source: source.into(),
            snapshot: None, // invalidated
        }
    }

    /// Attach a computed semantic snapshot.
    pub fn with_snapshot(mut self, snapshot: Arc<SemanticSnapshot>) -> Self {
        self.snapshot = Some(snapshot);
        self
    }
}
