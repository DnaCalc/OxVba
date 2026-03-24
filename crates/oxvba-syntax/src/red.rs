use crate::SyntaxKind;
use crate::green::{GreenChild, GreenNode};

/// A position-aware view over a green node.
///
/// Red nodes are lightweight cursors — they borrow the green tree and carry
/// an absolute byte offset. Creating one is O(1), iterating children is
/// O(n_children).
#[derive(Debug, Clone, Copy)]
pub struct SyntaxNode<'a> {
    green: &'a GreenNode,
    offset: u32,
}

/// A positioned token (leaf) in the syntax tree.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxToken<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
    pub offset: u32,
}

/// Either a node or a token encountered during traversal.
#[derive(Debug, Clone, Copy)]
pub enum SyntaxElement<'a> {
    Node(SyntaxNode<'a>),
    Token(SyntaxToken<'a>),
}

impl<'a> SyntaxNode<'a> {
    pub fn new(green: &'a GreenNode, offset: u32) -> Self {
        SyntaxNode { green, offset }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.green.kind()
    }

    pub fn text_range(&self) -> (u32, u32) {
        (self.offset, self.offset + self.green.width())
    }

    pub fn width(&self) -> u32 {
        self.green.width()
    }

    /// Iterate over children as positioned elements.
    pub fn children(&self) -> Vec<SyntaxElement<'a>> {
        let mut offset = self.offset;
        self.green
            .children()
            .iter()
            .map(|child| {
                let elem = match child {
                    GreenChild::Token { kind, text } => {
                        let tok = SyntaxToken {
                            kind: *kind,
                            text,
                            offset,
                        };
                        SyntaxElement::Token(tok)
                    }
                    GreenChild::Node(n) => {
                        let node = SyntaxNode::new(n, offset);
                        SyntaxElement::Node(node)
                    }
                };
                offset += child.width();
                elem
            })
            .collect()
    }

    /// Iterate over child nodes only (skip tokens).
    pub fn child_nodes(&self) -> Vec<SyntaxNode<'a>> {
        let mut offset = self.offset;
        let mut nodes = Vec::new();
        for child in self.green.children() {
            if let GreenChild::Node(n) = child {
                nodes.push(SyntaxNode::new(n, offset));
            }
            offset += child.width();
        }
        nodes
    }

    /// Iterate over child tokens only (skip nodes).
    pub fn child_tokens(&self) -> Vec<SyntaxToken<'a>> {
        let mut offset = self.offset;
        let mut tokens = Vec::new();
        for child in self.green.children() {
            if let GreenChild::Token { kind, text } = child {
                tokens.push(SyntaxToken {
                    kind: *kind,
                    text,
                    offset,
                });
            }
            offset += child.width();
        }
        tokens
    }

    /// Reconstruct the full source text of this node.
    pub fn text(&self) -> String {
        let mut buf = String::with_capacity(self.green.width() as usize);
        collect_text(self.green, &mut buf);
        buf
    }

    /// Find the first non-trivia token.
    pub fn first_token(&self) -> Option<SyntaxToken<'a>> {
        first_token_impl(self.green, self.offset)
    }

    // ── Typed accessor methods ─────────────────────────────

    /// Get the name token (Ident) of a SubDecl, FunctionDecl, or PropertyDecl.
    pub fn name_token(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens()
            .into_iter()
            .find(|t| t.kind == SyntaxKind::Ident)
    }

    /// Get the ParamList child node, if present.
    pub fn param_list(&self) -> Option<SyntaxNode<'a>> {
        self.child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::ParamList)
    }

    /// Get the individual Param nodes from a ParamList node.
    pub fn params(&self) -> Vec<SyntaxNode<'a>> {
        self.child_nodes()
            .into_iter()
            .filter(|n| n.kind() == SyntaxKind::Param)
            .collect()
    }

    /// Get the TypeRef child node (return type), if present.
    pub fn return_type(&self) -> Option<SyntaxNode<'a>> {
        self.child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::TypeRef)
    }

    /// Get all statement-level child nodes from a Block node.
    pub fn statements(&self) -> Vec<SyntaxNode<'a>> {
        self.child_nodes()
            .into_iter()
            .filter(|n| !n.kind().is_trivia() && n.kind() != SyntaxKind::Block)
            .collect()
    }

    /// Get the body Block child node of a SubDecl, FunctionDecl, or PropertyDecl.
    pub fn body_block(&self) -> Option<SyntaxNode<'a>> {
        self.child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::Block)
    }
}

fn collect_text(node: &GreenNode, buf: &mut String) {
    for child in node.children() {
        match child {
            GreenChild::Token { text, .. } => buf.push_str(text),
            GreenChild::Node(n) => collect_text(n, buf),
        }
    }
}

fn first_token_impl(node: &GreenNode, offset: u32) -> Option<SyntaxToken<'_>> {
    let mut off = offset;
    for child in node.children() {
        match child {
            GreenChild::Token { kind, text } if !kind.is_trivia() => {
                return Some(SyntaxToken {
                    kind: *kind,
                    text,
                    offset: off,
                });
            }
            GreenChild::Token { text, .. } => off += text.len() as u32,
            GreenChild::Node(n) => {
                if let Some(tok) = first_token_impl(n, off) {
                    return Some(tok);
                }
                off += n.width();
            }
        }
    }
    None
}

impl<'a> SyntaxElement<'a> {
    pub fn kind(&self) -> SyntaxKind {
        match self {
            SyntaxElement::Node(n) => n.kind(),
            SyntaxElement::Token(t) => t.kind,
        }
    }

    pub fn offset(&self) -> u32 {
        match self {
            SyntaxElement::Node(n) => n.offset,
            SyntaxElement::Token(t) => t.offset,
        }
    }

    pub fn width(&self) -> u32 {
        match self {
            SyntaxElement::Node(n) => n.width(),
            SyntaxElement::Token(t) => t.text.len() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::green::GreenNodeBuilder;

    #[test]
    fn red_tree_positions() {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile);
        b.token(SyntaxKind::KwSub, "Sub");
        b.token(SyntaxKind::Whitespace, " ");
        b.token(SyntaxKind::Ident, "Foo");
        b.finish_node();
        let green = b.finish();

        let root = SyntaxNode::new(&green, 0);
        assert_eq!(root.text(), "Sub Foo");
        assert_eq!(root.text_range(), (0, 7));

        let children = root.children();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].offset(), 0); // Sub
        assert_eq!(children[1].offset(), 3); // space
        assert_eq!(children[2].offset(), 4); // Foo
    }

    #[test]
    fn typed_accessor_name_token() {
        let src = "Sub Foo()\nEnd Sub\n";
        let p = crate::parser::parse(src);
        let root = p.syntax();
        let sub_decl = root
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::SubDecl)
            .expect("expected SubDecl");
        let name = sub_decl.name_token().expect("expected name token");
        assert_eq!(name.text, "Foo");
    }

    #[test]
    fn typed_accessor_param_list() {
        let src = "Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
        let p = crate::parser::parse(src);
        let root = p.syntax();
        let func = root
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::FunctionDecl)
            .expect("expected FunctionDecl");

        let param_list = func.param_list().expect("expected ParamList");
        let params = param_list.params();
        assert_eq!(params.len(), 2, "expected 2 params, got {}", params.len());

        let ret = func.return_type().expect("expected return TypeRef");
        assert!(
            ret.text().contains("Long"),
            "return type should contain Long, got: {}",
            ret.text()
        );
    }

    #[test]
    fn typed_accessor_body_block() {
        let src = "Sub Test()\n    Dim x As Long\n    x = 1\nEnd Sub\n";
        let p = crate::parser::parse(src);
        let root = p.syntax();
        let sub_decl = root
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::SubDecl)
            .expect("expected SubDecl");
        let body = sub_decl.body_block().expect("expected body Block");
        assert_eq!(body.kind(), SyntaxKind::Block);
        let stmts = body.statements();
        assert!(
            stmts.len() >= 2,
            "expected at least 2 statements, got {}",
            stmts.len()
        );
    }

    #[test]
    fn typed_accessor_property_decl() {
        let src = "Public Property Get Value() As Long\n    Value = mValue\nEnd Property\n";
        let p = crate::parser::parse(src);
        let root = p.syntax();
        let prop = root
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::PropertyDecl)
            .expect("expected PropertyDecl");
        let name = prop.name_token().expect("expected name token");
        assert_eq!(name.text, "Value");
        assert!(prop.return_type().is_some(), "expected return type");
        assert!(prop.body_block().is_some(), "expected body block");
    }

    #[test]
    fn typed_accessor_params_from_paramlist() {
        let src =
            "Sub Multi(a As Long, Optional b As String, ParamArray c() As Variant)\nEnd Sub\n";
        let p = crate::parser::parse(src);
        let root = p.syntax();
        let sub_decl = root
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::SubDecl)
            .expect("expected SubDecl");
        let pl = sub_decl.param_list().expect("expected ParamList");
        let params = pl.params();
        assert_eq!(params.len(), 3, "expected 3 params, got {}", params.len());
    }
}
