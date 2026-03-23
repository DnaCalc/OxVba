use std::sync::Arc;

use crate::SyntaxKind;

/// Immutable concrete syntax tree node.
///
/// Green nodes form the persistent, shareable backbone of the syntax tree.
/// They carry no absolute position — that is computed lazily by the red tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreenNode {
    kind: SyntaxKind,
    width: u32,
    children: Vec<GreenChild>,
}

/// A child of a green node: either a token (leaf) or a sub-node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreenChild {
    Token { kind: SyntaxKind, text: Box<str> },
    Node(Arc<GreenNode>),
}

impl GreenNode {
    pub fn new(kind: SyntaxKind, children: Vec<GreenChild>) -> Self {
        let width = children.iter().map(|c| c.width()).sum();
        GreenNode {
            kind,
            width,
            children,
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn children(&self) -> &[GreenChild] {
        &self.children
    }
}

impl GreenChild {
    pub fn width(&self) -> u32 {
        match self {
            GreenChild::Token { text, .. } => text.len() as u32,
            GreenChild::Node(n) => n.width(),
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        match self {
            GreenChild::Token { kind, .. } => *kind,
            GreenChild::Node(n) => n.kind(),
        }
    }
}

/// An opaque checkpoint for retroactive node wrapping in Pratt parsing.
///
/// Created by `GreenNodeBuilder::checkpoint()`, consumed by `start_node_at()`.
#[derive(Debug, Clone, Copy)]
pub struct Checkpoint(usize);

/// Builder for constructing green trees from parser events.
pub struct GreenNodeBuilder {
    stack: Vec<(SyntaxKind, Vec<GreenChild>)>,
}

impl GreenNodeBuilder {
    pub fn new() -> Self {
        GreenNodeBuilder { stack: Vec::new() }
    }

    /// Record a checkpoint at the current position in the topmost node's
    /// children list. Use with `start_node_at` to retroactively wrap
    /// children that were added after the checkpoint.
    pub fn checkpoint(&self) -> Checkpoint {
        let child_count = self
            .stack
            .last()
            .map(|(_, children)| children.len())
            .unwrap_or(0);
        Checkpoint(child_count)
    }

    /// Start a new composite node retroactively at the given checkpoint.
    ///
    /// All children added to the current frame since the checkpoint are
    /// drained into a new child frame for the new node.
    pub fn start_node_at(&mut self, cp: Checkpoint, kind: SyntaxKind) {
        if let Some(parent) = self.stack.last_mut() {
            let taken: Vec<GreenChild> = parent.1.drain(cp.0..).collect();
            self.stack.push((kind, taken));
        } else {
            self.stack.push((kind, Vec::new()));
        }
    }

    /// Start a new composite node.
    pub fn start_node(&mut self, kind: SyntaxKind) {
        self.stack.push((kind, Vec::new()));
    }

    /// Finish the current composite node, adding it as a child of its parent.
    pub fn finish_node(&mut self) {
        let (kind, children) = self
            .stack
            .pop()
            .expect("finish_node without matching start_node");
        let node = Arc::new(GreenNode::new(kind, children));
        if let Some(parent) = self.stack.last_mut() {
            parent.1.push(GreenChild::Node(node));
        } else {
            // Root node — push back so `finish()` can retrieve it.
            self.stack.push((kind, vec![GreenChild::Node(node)]));
        }
    }

    /// Add a token leaf.
    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        let child = GreenChild::Token {
            kind,
            text: text.into(),
        };
        if let Some(parent) = self.stack.last_mut() {
            parent.1.push(child);
        }
    }

    /// Consume the builder and return the root green node.
    pub fn finish(mut self) -> Arc<GreenNode> {
        assert_eq!(self.stack.len(), 1, "unbalanced start_node/finish_node");
        let (_, mut children) = self.stack.pop().unwrap();
        assert_eq!(children.len(), 1);
        match children.pop().unwrap() {
            GreenChild::Node(n) => n,
            _ => panic!("expected root node"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_round_trip() {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile);
        b.token(SyntaxKind::KwSub, "Sub");
        b.token(SyntaxKind::Whitespace, " ");
        b.token(SyntaxKind::Ident, "Foo");
        b.finish_node();
        let root = b.finish();
        assert_eq!(root.kind(), SyntaxKind::SourceFile);
        assert_eq!(root.width(), 7); // "Sub Foo"
        assert_eq!(root.children().len(), 3);
    }

    #[test]
    fn checkpoint_wraps_retroactively() {
        // Build: SourceFile { BinaryExpr { Ident("a"), Plus, Ident("b") } }
        // using checkpoint to retroactively wrap "a" and "+" and "b".
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile);

        let cp = b.checkpoint(); // before "a"
        b.token(SyntaxKind::Ident, "a");
        b.token(SyntaxKind::Plus, "+");
        b.token(SyntaxKind::Ident, "b");

        // Retroactively wrap everything since cp in a BinaryExpr
        b.start_node_at(cp, SyntaxKind::BinaryExpr);
        b.finish_node();

        b.finish_node(); // SourceFile
        let root = b.finish();
        assert_eq!(root.kind(), SyntaxKind::SourceFile);
        assert_eq!(root.children().len(), 1); // single BinaryExpr child
        if let GreenChild::Node(inner) = &root.children()[0] {
            assert_eq!(inner.kind(), SyntaxKind::BinaryExpr);
            assert_eq!(inner.children().len(), 3); // a, +, b
        } else {
            panic!("expected node child");
        }
    }
}
