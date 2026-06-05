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

    /// The declared name token of a `SubDecl`/`FunctionDecl`/`PropertyDecl`: the
    /// first name token after the `Sub`/`Function`/`Property [Get|Let|Set]`
    /// keywords. Unlike [`name_token`](Self::name_token) this also accepts a
    /// `BracketedIdent` or a keyword used as a name (`Function Name()`), so the
    /// scanner and the binder agree on exactly which decls are named procedures.
    pub fn proc_name_token(&self) -> Option<SyntaxToken<'a>> {
        let mut toks = self.child_tokens().into_iter().filter(|t| !t.kind.is_trivia());
        let mut decl_kw = None;
        for t in toks.by_ref() {
            if matches!(
                t.kind,
                SyntaxKind::KwSub | SyntaxKind::KwFunction | SyntaxKind::KwProperty
            ) {
                decl_kw = Some(t.kind);
                break;
            }
        }
        decl_kw?;
        let mut next = toks.next();
        if decl_kw == Some(SyntaxKind::KwProperty) {
            if let Some(t) = next {
                if matches!(t.kind, SyntaxKind::KwGet | SyntaxKind::KwLet | SyntaxKind::KwSet) {
                    next = toks.next();
                }
            }
        }
        next.filter(|t| {
            matches!(t.kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent) || t.kind.is_keyword()
        })
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

    // ── Structured-statement accessors ──────────────────────

    /// First child node of the given kind.
    pub fn child_node(&self, kind: SyntaxKind) -> Option<SyntaxNode<'a>> {
        self.child_nodes().into_iter().find(|n| n.kind() == kind)
    }

    /// All child nodes of the given kind.
    pub fn children_of(&self, kind: SyntaxKind) -> Vec<SyntaxNode<'a>> {
        self.child_nodes()
            .into_iter()
            .filter(|n| n.kind() == kind)
            .collect()
    }

    /// `VarDeclarator` children of a `DimStmt`/`ConstStmt`.
    pub fn declarators(&self) -> Vec<SyntaxNode<'a>> {
        self.children_of(SyntaxKind::VarDeclarator)
    }

    /// `TypeField` children of a `TypeBlock`.
    pub fn type_fields(&self) -> Vec<SyntaxNode<'a>> {
        self.children_of(SyntaxKind::TypeField)
    }

    /// `EnumMember` children of an `EnumBlock`.
    pub fn enum_members(&self) -> Vec<SyntaxNode<'a>> {
        self.children_of(SyntaxKind::EnumMember)
    }

    /// The declared name token of a `VarDeclarator`/`TypeField`/`EnumMember`
    /// (Ident, BracketedIdent, or a keyword used as a name) — skipping a leading
    /// `WithEvents` and any type suffix.
    pub fn declarator_name(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens().into_iter().find(|t| {
            !matches!(t.kind, SyntaxKind::KwWithEvents | SyntaxKind::TypeSuffix)
                && (t.kind == SyntaxKind::Ident
                    || t.kind == SyntaxKind::BracketedIdent
                    || t.kind.is_keyword())
        })
    }

    /// The `TypeRef` of a `VarDeclarator`/`TypeField` (its declared type).
    pub fn declared_type(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::TypeRef)
    }

    /// The `ArrayBounds` of a `VarDeclarator`/`TypeField`/`ReDimStmt`.
    pub fn array_bounds(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::ArrayBounds)
    }

    /// True if this declarator is `Dim WithEvents …`.
    pub fn is_with_events(&self) -> bool {
        self.child_tokens()
            .iter()
            .any(|t| t.kind == SyntaxKind::KwWithEvents)
    }

    /// The `ParamDefault` of a `Param`, if any.
    pub fn param_default(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::ParamDefault)
    }

    /// The `LabelRef` child (GoTo/GoSub/On Error GoTo/Resume target).
    pub fn label_ref(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::LabelRef)
    }

    /// The first `FileNumber` child of a file-I/O statement.
    pub fn file_number(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::FileNumber)
    }

    /// The string literal of a `Lib`/`Alias` clause on a `DeclareStmt` (quotes trimmed).
    pub fn clause_string(&self, clause: SyntaxKind) -> Option<String> {
        self.child_node(clause).and_then(|n| {
            n.child_tokens()
                .into_iter()
                .find(|t| t.kind == SyntaxKind::StringLiteral)
                .map(|t| t.text.trim_matches('"').to_string())
        })
    }

    /// `Lib "…"` string of a `DeclareStmt`.
    pub fn lib_string(&self) -> Option<String> {
        self.clause_string(SyntaxKind::LibClause)
    }

    /// `Alias "…"` string of a `DeclareStmt`.
    pub fn alias_string(&self) -> Option<String> {
        self.clause_string(SyntaxKind::AliasClause)
    }

    // ── Expression / control-flow navigation (binder input contract) ─────────

    /// First non-trivia direct child token (e.g. the operator of a Binary/Unary
    /// expression, or the name token of an `IdentExpr`).
    pub fn first_significant_token(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens().into_iter().find(|t| !t.kind.is_trivia())
    }

    /// All expression child nodes, in source order.
    pub fn expr_children(&self) -> Vec<SyntaxNode<'a>> {
        self.child_nodes().into_iter().filter(|n| n.kind().is_expr()).collect()
    }

    /// The first expression child node, if any (e.g. a statement's condition,
    /// a `ParenExpr`'s inner expression, a `UnaryExpr`'s operand).
    pub fn first_expr_child(&self) -> Option<SyntaxNode<'a>> {
        self.child_nodes().into_iter().find(|n| n.kind().is_expr())
    }

    // ── Expression accessors ────────────────────────────────

    /// `BinaryExpr` (infix) left operand — the first expression child.
    pub fn binary_lhs(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().next()
    }
    /// `BinaryExpr` (infix) right operand — the second expression child.
    pub fn binary_rhs(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().nth(1)
    }
    /// `BinaryExpr` operator token (the first significant token; `KwTypeOf` for
    /// the `TypeOf … Is …` form — check [`is_typeof`](Self::is_typeof)).
    pub fn binary_op_token(&self) -> Option<SyntaxToken<'a>> {
        self.first_significant_token()
    }
    /// True if this `BinaryExpr` is the `TypeOf <expr> Is <type>` form.
    pub fn is_typeof(&self) -> bool {
        self.first_significant_token().map(|t| t.kind == SyntaxKind::KwTypeOf).unwrap_or(false)
    }
    /// `TypeOf <operand> Is …` — the tested operand (first expression child).
    pub fn typeof_operand(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().next()
    }
    /// `TypeOf … Is <type>` — the type operand (second expression child).
    pub fn typeof_type(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().nth(1)
    }

    /// `UnaryExpr` operator token.
    pub fn unary_op_token(&self) -> Option<SyntaxToken<'a>> {
        self.first_significant_token()
    }
    /// `UnaryExpr` operand (the single expression child).
    pub fn unary_operand(&self) -> Option<SyntaxNode<'a>> {
        self.first_expr_child()
    }

    /// `MemberExpr` receiver (the object before `.`/`!`). `None` for a leading-dot
    /// member (`.Value` inside a `With` block).
    pub fn member_receiver(&self) -> Option<SyntaxNode<'a>> {
        self.first_expr_child()
    }
    /// True if this `MemberExpr` is `obj ! name` (bang access) rather than `obj . name`.
    pub fn member_is_bang(&self) -> bool {
        self.child_tokens().iter().any(|t| t.kind == SyntaxKind::Bang)
    }
    /// True if this is a leading-dot member (`.Value`) with no explicit receiver
    /// (resolves against the enclosing `With`).
    pub fn member_has_leading_dot(&self) -> bool {
        self.member_receiver().is_none()
    }
    /// `MemberExpr` member name token (the `Ident`/keyword after the `.`/`!`).
    pub fn member_name_token(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens().into_iter().rev().find(|t| {
            matches!(t.kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent) || t.kind.is_keyword()
        })
    }

    /// `IndexExpr` base (the indexed expression, before the `(`).
    pub fn index_base(&self) -> Option<SyntaxNode<'a>> {
        self.child_nodes().into_iter().find(|n| n.kind() != SyntaxKind::ArgList)
    }
    /// `IndexExpr` argument list.
    pub fn index_arg_list(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::ArgList)
    }

    /// `IdentExpr` name token (the identifier, or `Me`).
    pub fn ident_name_token(&self) -> Option<SyntaxToken<'a>> {
        self.first_significant_token()
    }
    /// True if this `IdentExpr` is the `Me` keyword.
    pub fn ident_is_me(&self) -> bool {
        self.first_significant_token().map(|t| t.kind == SyntaxKind::KwMe).unwrap_or(false)
    }
    /// The `TypeSuffix` token on an `IdentExpr`/`MemberExpr`, if present (`x%`, `s$`).
    pub fn type_suffix_token(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens().into_iter().find(|t| t.kind == SyntaxKind::TypeSuffix)
    }

    /// `LiteralExpr` value token (skips a leading `#` for a `#n` file-number literal).
    pub fn literal_token(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens()
            .into_iter()
            .find(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::Hash)
    }

    /// `ParenExpr` inner expression.
    pub fn paren_inner(&self) -> Option<SyntaxNode<'a>> {
        self.first_expr_child()
    }

    /// `NewExpr` class name (possibly dotted), reconstructed from its name tokens.
    pub fn new_type_name(&self) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        for t in self.child_tokens() {
            match t.kind {
                SyntaxKind::KwNew | SyntaxKind::Dot => {}
                k if k.is_trivia() => {}
                k if k == SyntaxKind::Ident || k == SyntaxKind::BracketedIdent || k.is_keyword() => {
                    parts.push(t.text)
                }
                _ => {}
            }
        }
        (!parts.is_empty()).then(|| parts.join("."))
    }

    /// The argument items of an `ArgList`, split on top-level commas (named args,
    /// omitted slots, and positional values all recovered from the flat children).
    pub fn arg_items(&self) -> Vec<ArgItem<'a>> {
        let significant: Vec<SyntaxElement<'a>> = self
            .children()
            .into_iter()
            .filter(|el| {
                let k = el.kind();
                !k.is_trivia() && k != SyntaxKind::LParen && k != SyntaxKind::RParen
            })
            .collect();
        if significant.is_empty() {
            return Vec::new();
        }
        let mut segments: Vec<Vec<SyntaxElement<'a>>> = vec![Vec::new()];
        for el in significant {
            if el.kind() == SyntaxKind::Comma {
                segments.push(Vec::new());
            } else {
                segments.last_mut().unwrap().push(el);
            }
        }
        segments
            .into_iter()
            .map(|seg| {
                if seg.is_empty() {
                    return ArgItem::Omitted;
                }
                let value = seg.iter().rev().find_map(|e| match e {
                    SyntaxElement::Node(n) => Some(*n),
                    _ => None,
                });
                let named = seg.iter().any(|e| e.kind() == SyntaxKind::ColonEq);
                let name = named.then(|| {
                    seg.iter().find_map(|e| match e {
                        SyntaxElement::Token(t) if t.kind == SyntaxKind::Ident || t.kind.is_keyword() => {
                            Some(*t)
                        }
                        _ => None,
                    })
                }).flatten();
                match (name, value) {
                    (Some(name), Some(value)) => ArgItem::Named { name, value },
                    (None, Some(value)) => ArgItem::Positional(value),
                    _ => ArgItem::Omitted,
                }
            })
            .collect()
    }

    // ── Control-flow accessors ──────────────────────────────

    /// `IfStmt`/`ElseIfClause`/`WhileStmt`/`WithStmt` condition or object expression.
    pub fn condition_expr(&self) -> Option<SyntaxNode<'a>> {
        self.first_expr_child()
    }
    /// `IfStmt` then-body (the first direct `Block`).
    pub fn if_then_block(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::Block)
    }
    /// `IfStmt` `ElseIf` clauses, in order.
    pub fn if_elseif_clauses(&self) -> Vec<SyntaxNode<'a>> {
        self.children_of(SyntaxKind::ElseIfClause)
    }
    /// `IfStmt` `Else` clause, if present.
    pub fn if_else_clause(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::ElseClause)
    }

    /// True if this `ForStmt` is a `For Each` loop.
    pub fn for_is_each(&self) -> bool {
        self.child_tokens().iter().any(|t| t.kind == SyntaxKind::KwEach)
    }
    /// `For` counter variable token (range form; `None` for `For Each`).
    pub fn for_counter_token(&self) -> Option<SyntaxToken<'a>> {
        self.child_tokens().into_iter().find(|t| {
            !matches!(t.kind, SyntaxKind::KwFor | SyntaxKind::KwEach)
                && (t.kind == SyntaxKind::Ident
                    || t.kind == SyntaxKind::BracketedIdent
                    || t.kind.is_keyword())
        })
    }
    /// `For i = <start> …` (range form): the start expression.
    pub fn for_start(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().next()
    }
    /// `For i = … To <end>` (range form): the end expression.
    pub fn for_end(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().nth(1)
    }
    /// `For … Step <step>` (range form): the step expression, if a `Step` clause is present.
    pub fn for_step(&self) -> Option<SyntaxNode<'a>> {
        self.child_tokens()
            .iter()
            .any(|t| t.kind == SyntaxKind::KwStep)
            .then(|| self.expr_children().into_iter().nth(2))
            .flatten()
    }
    /// `For Each <var> …`: the loop variable expression.
    pub fn foreach_var(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().next()
    }
    /// `For Each … In <collection>`: the collection expression.
    pub fn foreach_collection(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().nth(1)
    }

    /// `DoStmt` pre-test condition (`Do While`/`Do Until …`), if any.
    pub fn do_pre_cond(&self) -> Option<SyntaxNode<'a>> {
        let block = self.body_block().map(|b| b.text_range().0).unwrap_or(u32::MAX);
        self.expr_children().into_iter().find(|e| e.text_range().0 < block)
    }
    /// `DoStmt` post-test condition (`Loop While`/`Loop Until …`), if any.
    pub fn do_post_cond(&self) -> Option<SyntaxNode<'a>> {
        let block = self.body_block().map(|b| b.text_range().0)?;
        self.expr_children().into_iter().find(|e| e.text_range().0 > block)
    }
    /// True if the pre-test keyword is `Until` (vs `While`).
    pub fn do_pre_is_until(&self) -> bool {
        self.do_loop_keyword(true).map(|t| t.kind == SyntaxKind::KwUntil).unwrap_or(false)
    }
    /// True if the post-test keyword is `Until` (vs `While`).
    pub fn do_post_is_until(&self) -> bool {
        self.do_loop_keyword(false).map(|t| t.kind == SyntaxKind::KwUntil).unwrap_or(false)
    }
    fn do_loop_keyword(&self, before_block: bool) -> Option<SyntaxToken<'a>> {
        let block = self.body_block().map(|b| b.text_range().0);
        self.child_tokens().into_iter().find(|t| {
            matches!(t.kind, SyntaxKind::KwWhile | SyntaxKind::KwUntil)
                && match block {
                    Some(b) => {
                        if before_block {
                            t.offset < b
                        } else {
                            t.offset > b
                        }
                    }
                    None => before_block,
                }
        })
    }

    /// `SelectStmt` scrutinee (the `Select Case <expr>` expression).
    pub fn select_scrutinee(&self) -> Option<SyntaxNode<'a>> {
        self.first_expr_child()
    }
    /// `SelectStmt` `Case` clauses, in order.
    pub fn select_case_clauses(&self) -> Vec<SyntaxNode<'a>> {
        self.children_of(SyntaxKind::CaseClause)
    }
    /// The selectors of a `CaseClause` (`Case Else`, `Case Is <op> e`, value, `lo To hi`).
    pub fn case_specs(&self) -> Vec<CaseSpec<'a>> {
        if self.child_tokens().iter().any(|t| t.kind == SyntaxKind::KwElse) {
            return vec![CaseSpec::Else];
        }
        if let Some(is_tok) = self.child_tokens().into_iter().find(|t| t.kind == SyntaxKind::KwIs) {
            let op = self.child_tokens().into_iter().find(|t| {
                t.offset > is_tok.offset && is_comparison_op(t.kind)
            });
            let value = self.expr_children().into_iter().next();
            return match (op, value) {
                (Some(op), Some(value)) => vec![CaseSpec::Is { op, value }],
                (None, Some(value)) => vec![CaseSpec::Value(value)],
                _ => Vec::new(),
            };
        }
        let block = self.body_block().map(|b| b.text_range().0).unwrap_or(u32::MAX);
        let mut specs = Vec::new();
        let mut pending: Option<SyntaxNode<'a>> = None;
        let mut expecting_hi = false;
        for el in self.children() {
            if el.offset() >= block {
                break;
            }
            match el {
                SyntaxElement::Token(t) if t.kind == SyntaxKind::KwTo => expecting_hi = true,
                SyntaxElement::Node(n) if n.kind().is_expr() => {
                    if expecting_hi {
                        if let Some(lo) = pending.take() {
                            specs.push(CaseSpec::Range { lo, hi: n });
                        }
                        expecting_hi = false;
                    } else {
                        if let Some(p) = pending.take() {
                            specs.push(CaseSpec::Value(p));
                        }
                        pending = Some(n);
                    }
                }
                _ => {}
            }
        }
        if let Some(p) = pending {
            specs.push(CaseSpec::Value(p));
        }
        specs
    }

    /// `AssignStmt`/`SetStmt`/`LetStmt` target l-value (first expression child).
    pub fn assign_target(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().next()
    }
    /// `AssignStmt`/`SetStmt`/`LetStmt` assigned value (second expression child).
    pub fn assign_value(&self) -> Option<SyntaxNode<'a>> {
        self.expr_children().into_iter().nth(1)
    }

    /// `CallStmt` callee expression.
    pub fn call_callee(&self) -> Option<SyntaxNode<'a>> {
        self.first_expr_child()
    }
    /// `CallStmt` argument list, if present.
    pub fn call_arg_list(&self) -> Option<SyntaxNode<'a>> {
        self.child_node(SyntaxKind::ArgList)
    }
}

fn is_comparison_op(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Eq
            | SyntaxKind::LtGt
            | SyntaxKind::Lt
            | SyntaxKind::LtEq
            | SyntaxKind::Gt
            | SyntaxKind::GtEq
    )
}

/// One argument of an `ArgList`, as the binder consumes it.
#[derive(Debug, Clone, Copy)]
pub enum ArgItem<'a> {
    /// A positional argument: the value expression.
    Positional(SyntaxNode<'a>),
    /// A named argument `name := value`.
    Named { name: SyntaxToken<'a>, value: SyntaxNode<'a> },
    /// An omitted slot (`f(a, , c)`).
    Omitted,
}

/// One selector of a `CaseClause`.
#[derive(Debug, Clone, Copy)]
pub enum CaseSpec<'a> {
    /// `Case Else`.
    Else,
    /// A single value (`Case 3`).
    Value(SyntaxNode<'a>),
    /// An inclusive range (`Case 1 To 9`).
    Range { lo: SyntaxNode<'a>, hi: SyntaxNode<'a> },
    /// A comparison (`Case Is >= 5`).
    Is { op: SyntaxToken<'a>, value: SyntaxNode<'a> },
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

    // ── Expression / control-flow accessor tests ─────────────

    fn find_first<'a>(node: SyntaxNode<'a>, kind: SyntaxKind) -> Option<SyntaxNode<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        for c in node.child_nodes() {
            if let Some(f) = find_first(c, kind) {
                return Some(f);
            }
        }
        None
    }

    fn in_sub(body: &str) -> String {
        format!("Sub T()\n{body}End Sub\n")
    }

    #[test]
    fn expr_binary_operands_and_op() {
        let p = crate::parser::parse(&in_sub("x = a + b\n"));
        let bin = find_first(p.syntax(), SyntaxKind::BinaryExpr).expect("binary");
        assert_eq!(bin.binary_op_token().unwrap().text, "+");
        assert_eq!(bin.binary_lhs().unwrap().kind(), SyntaxKind::IdentExpr);
        assert_eq!(bin.binary_rhs().unwrap().kind(), SyntaxKind::IdentExpr);
        assert!(!bin.is_typeof());
    }

    #[test]
    fn expr_typeof_is() {
        let p = crate::parser::parse(&in_sub("x = TypeOf obj Is Worksheet\n"));
        let b = find_first(p.syntax(), SyntaxKind::BinaryExpr).expect("typeof");
        assert!(b.is_typeof());
        assert_eq!(b.typeof_operand().unwrap().ident_name_token().unwrap().text, "obj");
        assert_eq!(b.typeof_type().unwrap().ident_name_token().unwrap().text, "Worksheet");
    }

    #[test]
    fn expr_unary() {
        let p = crate::parser::parse(&in_sub("x = -a\n"));
        let u = find_first(p.syntax(), SyntaxKind::UnaryExpr).expect("unary");
        assert_eq!(u.unary_op_token().unwrap().text, "-");
        assert_eq!(u.unary_operand().unwrap().kind(), SyntaxKind::IdentExpr);
    }

    #[test]
    fn expr_member_postfix_bang_and_leading_dot() {
        let p = crate::parser::parse(&in_sub("x = obj.Count\n"));
        let m = find_first(p.syntax(), SyntaxKind::MemberExpr).expect("member");
        assert_eq!(m.member_receiver().unwrap().kind(), SyntaxKind::IdentExpr);
        assert_eq!(m.member_name_token().unwrap().text, "Count");
        assert!(!m.member_is_bang());
        assert!(!m.member_has_leading_dot());

        let p2 = crate::parser::parse(&in_sub("x = rs!Field\n"));
        let bang = find_first(p2.syntax(), SyntaxKind::MemberExpr).expect("bang");
        assert!(bang.member_is_bang());
        assert_eq!(bang.member_name_token().unwrap().text, "Field");

        let p3 = crate::parser::parse(&in_sub("With foo\n.Bar = 1\nEnd With\n"));
        let dot = find_first(p3.syntax(), SyntaxKind::MemberExpr).expect("leading dot");
        assert!(dot.member_has_leading_dot());
        assert_eq!(dot.member_name_token().unwrap().text, "Bar");
    }

    #[test]
    fn expr_index_base_and_args() {
        let p = crate::parser::parse(&in_sub("x = arr(1, 2)\n"));
        let ix = find_first(p.syntax(), SyntaxKind::IndexExpr).expect("index");
        assert_eq!(ix.index_base().unwrap().kind(), SyntaxKind::IdentExpr);
        let args = ix.index_arg_list().unwrap().arg_items();
        assert_eq!(args.len(), 2);
        assert!(matches!(args[0], ArgItem::Positional(_)));
    }

    #[test]
    fn expr_ident_me_suffix_literal_paren_new() {
        let p = crate::parser::parse(&in_sub("x% = Me\n"));
        let assign = find_first(p.syntax(), SyntaxKind::AssignStmt).expect("assign");
        let target = assign.assign_target().unwrap();
        assert_eq!(target.ident_name_token().unwrap().text, "x");
        assert!(target.type_suffix_token().is_some());
        assert!(assign.assign_value().unwrap().ident_is_me());

        let p2 = crate::parser::parse(&in_sub("x = (5)\n"));
        let paren = find_first(p2.syntax(), SyntaxKind::ParenExpr).expect("paren");
        let lit = paren.paren_inner().unwrap();
        assert_eq!(lit.kind(), SyntaxKind::LiteralExpr);
        assert_eq!(lit.literal_token().unwrap().text, "5");

        let p3 = crate::parser::parse(&in_sub("Set x = New Scripting.Dictionary\n"));
        let nw = find_first(p3.syntax(), SyntaxKind::NewExpr).expect("new");
        assert_eq!(nw.new_type_name().as_deref(), Some("Scripting.Dictionary"));
    }

    #[test]
    fn arg_items_named_omitted_positional() {
        let p = crate::parser::parse(&in_sub("x = f(a, , c := 3)\n"));
        let ix = find_first(p.syntax(), SyntaxKind::IndexExpr).expect("call");
        let items = ix.index_arg_list().unwrap().arg_items();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], ArgItem::Positional(_)));
        assert!(matches!(items[1], ArgItem::Omitted));
        match items[2] {
            ArgItem::Named { name, .. } => assert_eq!(name.text, "c"),
            _ => panic!("expected named arg"),
        }
    }

    #[test]
    fn ctrl_if_elseif_else() {
        let p = crate::parser::parse(&in_sub(
            "If a > 1 Then\nx = 1\nElseIf b Then\nx = 2\nElse\nx = 3\nEnd If\n",
        ));
        let iff = find_first(p.syntax(), SyntaxKind::IfStmt).expect("if");
        assert_eq!(iff.condition_expr().unwrap().kind(), SyntaxKind::BinaryExpr);
        assert!(iff.if_then_block().is_some());
        assert_eq!(iff.if_elseif_clauses().len(), 1);
        assert!(iff.if_else_clause().is_some());
        let elif = iff.if_elseif_clauses()[0];
        assert_eq!(elif.condition_expr().unwrap().kind(), SyntaxKind::IdentExpr);
        assert!(elif.body_block().is_some());
    }

    #[test]
    fn ctrl_for_range_and_each() {
        let p = crate::parser::parse(&in_sub("For i = 1 To 10 Step 2\nx = i\nNext\n"));
        let f = find_first(p.syntax(), SyntaxKind::ForStmt).expect("for");
        assert!(!f.for_is_each());
        assert_eq!(f.for_counter_token().unwrap().text, "i");
        assert_eq!(f.for_start().unwrap().literal_token().unwrap().text, "1");
        assert_eq!(f.for_end().unwrap().literal_token().unwrap().text, "10");
        assert_eq!(f.for_step().unwrap().literal_token().unwrap().text, "2");
        assert!(f.body_block().is_some());

        let p2 = crate::parser::parse(&in_sub("For Each it In coll\nx = it\nNext\n"));
        let fe = find_first(p2.syntax(), SyntaxKind::ForStmt).expect("for each");
        assert!(fe.for_is_each());
        assert_eq!(fe.foreach_var().unwrap().ident_name_token().unwrap().text, "it");
        assert_eq!(fe.foreach_collection().unwrap().ident_name_token().unwrap().text, "coll");
    }

    #[test]
    fn ctrl_do_pre_and_post() {
        let p = crate::parser::parse(&in_sub("Do While a < 10\nx = 1\nLoop\n"));
        let d = find_first(p.syntax(), SyntaxKind::DoStmt).expect("do");
        assert!(d.do_pre_cond().is_some());
        assert!(d.do_post_cond().is_none());
        assert!(!d.do_pre_is_until());

        let p2 = crate::parser::parse(&in_sub("Do\nx = 1\nLoop Until a > 5\n"));
        let d2 = find_first(p2.syntax(), SyntaxKind::DoStmt).expect("do");
        assert!(d2.do_pre_cond().is_none());
        assert!(d2.do_post_cond().is_some());
        assert!(d2.do_post_is_until());
    }

    #[test]
    fn ctrl_select_case_specs() {
        let p = crate::parser::parse(&in_sub(
            "Select Case n\nCase 1, 5 To 9\nx = 1\nCase Is > 100\nx = 2\nCase Else\nx = 3\nEnd Select\n",
        ));
        let sel = find_first(p.syntax(), SyntaxKind::SelectStmt).expect("select");
        assert_eq!(sel.select_scrutinee().unwrap().ident_name_token().unwrap().text, "n");
        let clauses = sel.select_case_clauses();
        assert_eq!(clauses.len(), 3);
        let s0 = clauses[0].case_specs();
        assert_eq!(s0.len(), 2);
        assert!(matches!(s0[0], CaseSpec::Value(_)));
        assert!(matches!(s0[1], CaseSpec::Range { .. }));
        match clauses[1].case_specs()[0] {
            CaseSpec::Is { op, .. } => assert_eq!(op.text, ">"),
            _ => panic!("expected Is selector"),
        }
        assert!(matches!(clauses[2].case_specs()[0], CaseSpec::Else));
    }

    #[test]
    fn ctrl_with_set_and_call() {
        let p = crate::parser::parse(&in_sub("With obj\n.X = 1\nEnd With\n"));
        let w = find_first(p.syntax(), SyntaxKind::WithStmt).expect("with");
        assert_eq!(w.condition_expr().unwrap().ident_name_token().unwrap().text, "obj");
        assert!(w.body_block().is_some());

        let p2 = crate::parser::parse(&in_sub("Set a = b\n"));
        let set = find_first(p2.syntax(), SyntaxKind::SetStmt).expect("set");
        assert_eq!(set.assign_target().unwrap().ident_name_token().unwrap().text, "a");
        assert_eq!(set.assign_value().unwrap().ident_name_token().unwrap().text, "b");

        let p3 = crate::parser::parse(&in_sub("DoThing x, y\n"));
        let call = find_first(p3.syntax(), SyntaxKind::CallStmt).expect("call");
        assert_eq!(call.call_callee().unwrap().ident_name_token().unwrap().text, "DoThing");
        assert_eq!(call.call_arg_list().unwrap().arg_items().len(), 2);
    }
}
