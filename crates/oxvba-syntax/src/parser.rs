use std::sync::Arc;

use crate::green::{Checkpoint, GreenNode, GreenNodeBuilder};
use crate::lexer;
use crate::syntax_kind::SyntaxKind;

/// The result of parsing VBA source code.
pub struct Parse {
    green: Arc<GreenNode>,
    errors: Vec<ParseError>,
}

/// A parse error with a byte offset into the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub offset: u32,
    pub message: String,
}

impl Parse {
    /// Get the root syntax node for red-tree traversal.
    pub fn syntax(&self) -> crate::red::SyntaxNode<'_> {
        crate::red::SyntaxNode::new(&self.green, 0)
    }

    /// Get the root green node (for cheap cloning).
    pub fn green(&self) -> &Arc<GreenNode> {
        &self.green
    }

    /// Parse errors (syntax remains lossless even with errors).
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }
}

/// Parse VBA source into a lossless concrete syntax tree.
///
/// The returned `Parse` always contains a complete tree representing every
/// byte of the input, even when there are syntax errors.
pub fn parse(source: &str) -> Parse {
    let tokens = lexer::tokenize(source);
    let mut p = Parser::new(&tokens);
    p.parse_source_file();
    let (green, errors) = p.finish();
    Parse { green, errors }
}

// ─── Parser internals ───────────────────────────────────────────────────────

struct Parser<'a> {
    tokens: &'a [(SyntaxKind, &'a str)],
    pos: usize,
    builder: GreenNodeBuilder,
    errors: Vec<ParseError>,
    /// Running byte offset (for error reporting).
    offset: u32,
    /// Parenthesis nesting depth for expression parsing.
    paren_depth: u32,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [(SyntaxKind, &'a str)]) -> Self {
        Parser {
            tokens,
            pos: 0,
            builder: GreenNodeBuilder::new(),
            errors: Vec::new(),
            offset: 0,
            paren_depth: 0,
        }
    }

    // ── Token access ────────────────────────────────────────

    fn current(&self) -> SyntaxKind {
        self.tokens
            .get(self.pos)
            .map(|(k, _)| *k)
            .unwrap_or(SyntaxKind::Eof)
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    fn at_any(&self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.current())
    }

    fn at_eof(&self) -> bool {
        self.current() == SyntaxKind::Eof
    }

    /// The next non-trivia token kind.
    fn current_non_trivia(&self) -> SyntaxKind {
        let mut j = self.pos;
        while j < self.tokens.len() {
            let k = self.tokens[j].0;
            if !k.is_trivia() {
                return k;
            }
            j += 1;
        }
        SyntaxKind::Eof
    }

    // ── Token consumption ───────────────────────────────────

    fn bump(&mut self) {
        if self.pos < self.tokens.len() {
            let (kind, text) = self.tokens[self.pos];
            self.builder.token(kind, text);
            self.offset += text.len() as u32;
            self.pos += 1;
        }
    }

    fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: SyntaxKind) {
        if !self.eat(kind) {
            self.error(format!("expected {kind:?}"));
        }
    }

    fn eat_trivia(&mut self) {
        while self.current().is_trivia() {
            self.bump();
        }
    }

    fn eat_whitespace(&mut self) {
        while self.at(SyntaxKind::Whitespace) || self.at(SyntaxKind::LineContinuation) {
            self.bump();
        }
    }

    fn eat_to_eol(&mut self) {
        while !self.at_eof() && !self.at(SyntaxKind::Newline) {
            self.bump();
        }
    }

    fn eat_to_statement_end(&mut self) {
        while !self.at_eof()
            && !self.at(SyntaxKind::Newline)
            && !self.at(SyntaxKind::Comment)
            && !self.at(SyntaxKind::Colon)
        {
            self.bump();
        }
    }

    // ── Error handling ──────────────────────────────────────

    fn error(&mut self, message: String) {
        self.errors.push(ParseError {
            offset: self.offset,
            message,
        });
    }

    /// Wrap remaining tokens up to the next line boundary in an ErrorNode.
    fn error_recover(&mut self, message: &str) {
        self.error(message.to_string());
        self.builder.start_node(SyntaxKind::ErrorNode);
        self.eat_to_eol();
        self.builder.finish_node();
    }

    // ── Node construction ───────────────────────────────────

    fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind);
    }

    fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    fn checkpoint(&self) -> Checkpoint {
        self.builder.checkpoint()
    }

    fn start_node_at(&mut self, cp: Checkpoint, kind: SyntaxKind) {
        self.builder.start_node_at(cp, kind);
    }

    /// Bump a LParen token, incrementing paren depth.
    fn bump_lparen(&mut self) {
        debug_assert!(self.at(SyntaxKind::LParen));
        self.bump();
        self.paren_depth += 1;
    }

    /// Bump a RParen token, decrementing paren depth.
    fn bump_rparen(&mut self) {
        debug_assert!(self.at(SyntaxKind::RParen));
        self.bump();
        self.paren_depth = self.paren_depth.saturating_sub(1);
    }

    /// Eat whitespace and line-continuations; inside parens also eat newlines/comments.
    fn eat_expr_whitespace(&mut self) {
        loop {
            match self.current() {
                SyntaxKind::Whitespace | SyntaxKind::LineContinuation => self.bump(),
                SyntaxKind::Newline | SyntaxKind::Comment if self.paren_depth > 0 => self.bump(),
                _ => break,
            }
        }
    }

    fn finish(self) -> (Arc<GreenNode>, Vec<ParseError>) {
        (self.builder.finish(), self.errors)
    }

    // ── Expression parsing (Pratt parser) ────────────────────

    /// Prefix binding power for unary operators.
    fn prefix_binding_power(kind: SyntaxKind) -> Option<u8> {
        match kind {
            SyntaxKind::KwNot => Some(12),
            SyntaxKind::Minus | SyntaxKind::Plus => Some(26),
            _ => None,
        }
    }

    /// Infix binding power: (left, right). Left < right for left-assoc.
    fn infix_binding_power(kind: SyntaxKind) -> Option<(u8, u8)> {
        match kind {
            SyntaxKind::KwImp => Some((2, 3)),
            SyntaxKind::KwEqv => Some((4, 5)),
            SyntaxKind::KwXor => Some((6, 7)),
            SyntaxKind::KwOr => Some((8, 9)),
            SyntaxKind::KwAnd => Some((10, 11)),
            SyntaxKind::Eq
            | SyntaxKind::LtGt
            | SyntaxKind::Lt
            | SyntaxKind::Gt
            | SyntaxKind::LtEq
            | SyntaxKind::GtEq
            | SyntaxKind::KwLike
            | SyntaxKind::KwIs => Some((14, 15)),
            SyntaxKind::Ampersand => Some((16, 17)),
            SyntaxKind::Plus | SyntaxKind::Minus => Some((18, 19)),
            SyntaxKind::KwMod => Some((20, 21)),
            SyntaxKind::Backslash => Some((22, 23)),
            SyntaxKind::Star | SyntaxKind::Slash => Some((24, 25)),
            SyntaxKind::Caret => Some((28, 27)),
            _ => None,
        }
    }

    fn is_contextual_name_keyword(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::KwDebug
                | SyntaxKind::KwStop
                | SyntaxKind::KwPrint
                | SyntaxKind::KwOpen
                | SyntaxKind::KwClose
                | SyntaxKind::KwInput
                | SyntaxKind::KwOutput
                | SyntaxKind::KwAppend
                | SyntaxKind::KwBinary
                | SyntaxKind::KwRandom
                | SyntaxKind::KwAccess
                | SyntaxKind::KwRead
                | SyntaxKind::KwWrite
                | SyntaxKind::KwLock
                | SyntaxKind::KwShared
                | SyntaxKind::KwLine
                | SyntaxKind::KwName
        )
    }

    fn is_expr_name_token(kind: SyntaxKind) -> bool {
        matches!(kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent)
            || Self::is_contextual_name_keyword(kind)
    }

    /// True if this token stops expression parsing (at zero paren depth).
    fn is_expr_stop(&self, kind: SyntaxKind) -> bool {
        match kind {
            SyntaxKind::Eof => true,
            SyntaxKind::Newline | SyntaxKind::Comment if self.paren_depth == 0 => true,
            SyntaxKind::KwThen | SyntaxKind::KwTo | SyntaxKind::KwStep | SyntaxKind::Colon => true,
            _ => false,
        }
    }

    /// True if the current token can start a primary expression.
    fn is_expr_start(&self) -> bool {
        match self.current() {
            SyntaxKind::IntLiteral
            | SyntaxKind::FloatLiteral
            | SyntaxKind::HexLiteral
            | SyntaxKind::OctLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::DateLiteral
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
            | SyntaxKind::KwEmpty
            | SyntaxKind::KwNull
            | SyntaxKind::KwNothing
            | SyntaxKind::KwMe
            | SyntaxKind::KwNew
            | SyntaxKind::KwTypeOf
            | SyntaxKind::LParen
            | SyntaxKind::Dot
            | SyntaxKind::Hash => true,
            k if Self::is_expr_name_token(k) => true,
            k if Self::prefix_binding_power(k).is_some() => true,
            _ => false,
        }
    }

    /// Parse an expression (public entry point for statements).
    fn parse_expr(&mut self) {
        self.eat_expr_whitespace();
        self.parse_expr_bp(0);
    }

    fn parse_required_expr(&mut self, message: &str) -> bool {
        if self.is_expr_start() {
            self.parse_expr();
            true
        } else {
            self.start_node(SyntaxKind::ErrorNode);
            self.error(message.to_string());
            self.finish_node();
            false
        }
    }

    /// Parse an lvalue expression (only postfix ops — member, index).
    fn parse_lvalue_expr(&mut self) {
        self.eat_expr_whitespace();
        self.parse_expr_bp(29);
    }

    /// Pratt parser core.
    fn parse_expr_bp(&mut self, min_bp: u8) {
        let before = self.pos;

        // ── Prefix / primary ───────────────────────────────
        let cp = self.checkpoint();

        if let Some(r_bp) = Self::prefix_binding_power(self.current()) {
            self.start_node(SyntaxKind::UnaryExpr);
            self.bump(); // operator
            self.eat_expr_whitespace();
            self.parse_expr_bp(r_bp);
            self.finish_node();
        } else {
            self.parse_primary_expr();
        }

        // If primary made no progress, bail out to avoid infinite loop.
        if self.pos == before {
            return;
        }

        // ── Infix / postfix loop ───────────────────────────
        loop {
            self.eat_expr_whitespace();

            let kind = self.current();
            if self.is_expr_stop(kind) {
                break;
            }

            // Postfix: `.`, `!`, `(`
            match kind {
                SyntaxKind::Dot | SyntaxKind::Bang => {
                    if min_bp > 30 {
                        break;
                    }
                    self.start_node_at(cp, SyntaxKind::MemberExpr);
                    self.bump(); // . or !
                    self.eat_expr_whitespace();
                    if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                        self.bump();
                    }
                    // Type suffix after member name
                    if self.at(SyntaxKind::TypeSuffix) {
                        self.bump();
                    }
                    self.finish_node();
                    continue;
                }
                SyntaxKind::LParen => {
                    if min_bp > 30 {
                        break;
                    }
                    self.start_node_at(cp, SyntaxKind::IndexExpr);
                    self.parse_arg_list();
                    self.finish_node();
                    continue;
                }
                _ => {}
            }

            // Infix
            if let Some((l_bp, r_bp)) = Self::infix_binding_power(kind) {
                if l_bp < min_bp {
                    break;
                }
                self.start_node_at(cp, SyntaxKind::BinaryExpr);
                self.bump(); // operator
                self.eat_expr_whitespace();
                self.parse_expr_bp(r_bp);
                self.finish_node();
                continue;
            }

            // Not an operator we recognise — stop
            break;
        }
    }

    /// Parse a primary expression (atom).
    fn parse_primary_expr(&mut self) {
        match self.current() {
            // Literals
            SyntaxKind::IntLiteral
            | SyntaxKind::FloatLiteral
            | SyntaxKind::HexLiteral
            | SyntaxKind::OctLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::DateLiteral => {
                self.start_node(SyntaxKind::LiteralExpr);
                self.bump();
                self.finish_node();
            }
            // Boolean / special values
            SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
            | SyntaxKind::KwEmpty
            | SyntaxKind::KwNull
            | SyntaxKind::KwNothing => {
                self.start_node(SyntaxKind::LiteralExpr);
                self.bump();
                self.finish_node();
            }
            // Me
            SyntaxKind::KwMe => {
                self.start_node(SyntaxKind::IdentExpr);
                self.bump();
                self.finish_node();
            }
            // New expr
            SyntaxKind::KwNew => {
                self.start_node(SyntaxKind::NewExpr);
                self.bump(); // New
                self.eat_expr_whitespace();
                // Type name (possibly qualified)
                if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                    self.bump();
                    while self.at(SyntaxKind::Dot) {
                        self.bump();
                        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                            self.bump();
                        }
                    }
                }
                self.finish_node();
            }
            // TypeOf expr Is type-expr
            SyntaxKind::KwTypeOf => {
                self.start_node(SyntaxKind::BinaryExpr);
                self.bump(); // TypeOf
                self.eat_expr_whitespace();
                self.parse_expr_bp(15);
                self.eat_expr_whitespace();
                if self.at(SyntaxKind::KwIs) {
                    self.bump();
                    self.eat_expr_whitespace();
                    self.parse_expr_bp(15);
                } else {
                    self.error("expected `Is` after `TypeOf` expression".into());
                }
                self.finish_node();
            }
            // Identifier
            k if Self::is_expr_name_token(k) => {
                self.start_node(SyntaxKind::IdentExpr);
                self.bump();
                // Type suffix
                if self.at(SyntaxKind::TypeSuffix) {
                    self.bump();
                }
                self.finish_node();
            }
            // Dot-prefix (With-block member access: .Value)
            SyntaxKind::Dot => {
                self.start_node(SyntaxKind::MemberExpr);
                self.bump(); // .
                self.eat_expr_whitespace();
                if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                    self.bump();
                }
                if self.at(SyntaxKind::TypeSuffix) {
                    self.bump();
                }
                self.finish_node();
            }
            // Parenthesised expression
            SyntaxKind::LParen => {
                self.start_node(SyntaxKind::ParenExpr);
                self.bump_lparen();
                self.eat_expr_whitespace();
                if !self.at(SyntaxKind::RParen) {
                    self.parse_expr_bp(0);
                }
                self.eat_expr_whitespace();
                if self.at(SyntaxKind::RParen) {
                    self.bump_rparen();
                } else {
                    self.error("expected `)`".into());
                }
                self.finish_node();
            }
            // Hash (file number: #1)
            SyntaxKind::Hash => {
                self.start_node(SyntaxKind::LiteralExpr);
                self.bump(); // #
                if self.at(SyntaxKind::IntLiteral) {
                    self.bump();
                }
                self.finish_node();
            }
            _ => {
                // No valid primary — produce an error node
                self.start_node(SyntaxKind::ErrorNode);
                self.error("expected expression".into());
                self.finish_node();
            }
        }
    }

    /// Parse a parenthesised argument list: `(expr, expr, ...)`.
    fn parse_arg_list(&mut self) {
        self.start_node(SyntaxKind::ArgList);
        if self.at(SyntaxKind::LParen) {
            self.bump_lparen();
        }

        self.eat_expr_whitespace();
        if !self.at(SyntaxKind::RParen) && !self.at_eof() {
            // First argument
            if !self.at(SyntaxKind::Comma) {
                self.parse_expr_bp(0);
            }

            while self.at(SyntaxKind::Comma) || {
                self.eat_expr_whitespace();
                self.at(SyntaxKind::Comma)
            } {
                self.bump(); // ,
                self.eat_expr_whitespace();
                // Support empty argument slots: f(a,,b)
                if !self.at(SyntaxKind::Comma) && !self.at(SyntaxKind::RParen) && !self.at_eof() {
                    self.parse_expr_bp(0);
                }
                self.eat_expr_whitespace();
            }
        }

        self.eat_expr_whitespace();
        if self.at(SyntaxKind::RParen) {
            self.bump_rparen();
        } else {
            self.error("expected `)`".into());
        }

        self.finish_node(); // ArgList
    }

    /// Parse a bare argument list (no parens): `expr, expr, ...`
    fn parse_bare_arg_list(&mut self) {
        self.eat_expr_whitespace();
        if self.is_expr_stop(self.current()) {
            return;
        }

        self.start_node(SyntaxKind::ArgList);

        // Named arg: name:=expr — check for ColonEq
        self.parse_bare_arg();

        loop {
            self.eat_expr_whitespace();
            if !self.at(SyntaxKind::Comma) {
                break;
            }
            self.bump(); // ,
            self.eat_expr_whitespace();
            if !self.is_expr_stop(self.current()) && !self.at(SyntaxKind::Comma) {
                self.parse_bare_arg();
            }
        }

        self.finish_node(); // ArgList
    }

    /// Parse a single bare argument, handling name:=expr syntax.
    fn parse_bare_arg(&mut self) {
        // Check for named argument: Ident := expr
        if (self.at(SyntaxKind::Ident) || self.current().is_keyword()) && self.peek_is_colon_eq() {
            self.bump(); // name
            self.eat_expr_whitespace();
            self.bump(); // :=
            self.eat_expr_whitespace();
        }
        self.parse_expr_bp(0);
    }

    /// Check if the next non-trivia token after current is ColonEq.
    fn peek_is_colon_eq(&self) -> bool {
        let mut j = self.pos + 1;
        while j < self.tokens.len() && self.tokens[j].0.is_trivia() {
            j += 1;
        }
        j < self.tokens.len() && self.tokens[j].0 == SyntaxKind::ColonEq
    }

    // ── Top-level parsing ───────────────────────────────────

    fn parse_source_file(&mut self) {
        self.start_node(SyntaxKind::SourceFile);

        // Consume leading trivia
        self.eat_trivia();

        while !self.at_eof() {
            let before = self.pos;
            self.parse_module_level_item();
            // Safety: if we didn't advance, consume one token to avoid infinite loop
            if self.pos == before {
                self.error_recover("unexpected token");
                if self.at(SyntaxKind::Newline) {
                    self.bump();
                }
            }
        }

        // Consume EOF token
        if self.at(SyntaxKind::Eof) {
            self.bump();
        }

        self.finish_node();
    }

    fn parse_module_level_item(&mut self) {
        self.eat_trivia();
        if self.at_eof() {
            return;
        }

        // Check for access modifiers
        let kind = self.current_non_trivia();

        match kind {
            SyntaxKind::KwOption => self.parse_option_stmt(),
            SyntaxKind::KwAttribute => self.parse_attribute_stmt(),
            SyntaxKind::KwImplements => self.parse_implements_stmt(),
            SyntaxKind::KwPublic | SyntaxKind::KwPrivate | SyntaxKind::KwFriend => {
                // Peek past the modifier to see what follows
                self.parse_modified_declaration()
            }
            SyntaxKind::KwStatic => self.parse_modified_declaration(),
            SyntaxKind::KwSub => self.parse_sub_decl(),
            SyntaxKind::KwFunction => self.parse_function_decl(),
            SyntaxKind::KwProperty => self.parse_property_decl(),
            SyntaxKind::KwDeclare => self.parse_declare_stmt(),
            SyntaxKind::KwDim => self.parse_dim_stmt(),
            SyntaxKind::KwConst => self.parse_const_stmt(),
            SyntaxKind::KwType => self.parse_type_block(),
            SyntaxKind::KwEnum => self.parse_enum_block(),
            SyntaxKind::KwEvent => self.parse_event_decl(),
            _ => self.parse_statement(),
        }
    }

    /// Parse a declaration preceded by an access modifier (Public/Private/Friend/Static).
    fn parse_modified_declaration(&mut self) {
        // We don't know yet what follows the modifier, so peek ahead
        let mut j = self.pos;
        // Skip trivia
        while j < self.tokens.len() && self.tokens[j].0.is_trivia() {
            j += 1;
        }
        // Skip the modifier keyword
        if j < self.tokens.len() {
            j += 1;
        }
        // Skip trivia after modifier
        while j < self.tokens.len() && self.tokens[j].0.is_trivia() {
            j += 1;
        }

        let after_modifier = if j < self.tokens.len() {
            self.tokens[j].0
        } else {
            SyntaxKind::Eof
        };

        match after_modifier {
            SyntaxKind::KwSub => self.parse_sub_decl(),
            SyntaxKind::KwFunction => self.parse_function_decl(),
            SyntaxKind::KwProperty => self.parse_property_decl(),
            SyntaxKind::KwDeclare => self.parse_declare_stmt(),
            SyntaxKind::KwConst => self.parse_const_stmt(),
            SyntaxKind::KwType => self.parse_type_block(),
            SyntaxKind::KwEnum => self.parse_enum_block(),
            SyntaxKind::KwEvent => self.parse_event_decl(),
            SyntaxKind::KwStatic => {
                // Public Static Sub ... — skip modifier then recurse
                self.parse_sub_or_function_after_modifier()
            }
            _ => self.parse_dim_stmt(), // Public x As Long
        }
    }

    fn parse_sub_or_function_after_modifier(&mut self) {
        // Already looking at some modifier. We'll wrap it all in the appropriate decl.
        // Peek far enough to find Sub/Function
        let mut j = self.pos;
        let mut found = SyntaxKind::Eof;
        while j < self.tokens.len() {
            let k = self.tokens[j].0;
            if k == SyntaxKind::KwSub {
                found = k;
                break;
            }
            if k == SyntaxKind::KwFunction {
                found = k;
                break;
            }
            if k == SyntaxKind::KwProperty {
                found = k;
                break;
            }
            if k == SyntaxKind::Newline || k == SyntaxKind::Eof {
                break;
            }
            j += 1;
        }
        match found {
            SyntaxKind::KwSub => self.parse_sub_decl(),
            SyntaxKind::KwFunction => self.parse_function_decl(),
            SyntaxKind::KwProperty => self.parse_property_decl(),
            _ => self.parse_dim_stmt(),
        }
    }

    // ── Option statement ────────────────────────────────────

    fn parse_option_stmt(&mut self) {
        self.start_node(SyntaxKind::OptionStmt);
        self.eat_trivia();
        self.bump(); // Option
        self.eat_whitespace();
        // Consume the rest of the line (Explicit, Base 1, Compare Text, etc.)
        self.eat_to_eol();
        self.finish_node();
    }

    // ── Attribute statement (exported module metadata) ──────

    fn parse_attribute_stmt(&mut self) {
        self.start_node(SyntaxKind::AttributeStmt);
        self.eat_trivia();
        self.bump(); // Attribute
        self.eat_to_eol();
        self.finish_node();
    }

    // ── Implements statement ────────────────────────────────

    fn parse_implements_stmt(&mut self) {
        self.start_node(SyntaxKind::ImplementsStmt);
        self.eat_trivia();
        self.bump(); // Implements
        self.eat_whitespace();
        self.eat_to_eol();
        self.finish_node();
    }

    // ── Sub declaration ─────────────────────────────────────

    fn parse_sub_decl(&mut self) {
        self.start_node(SyntaxKind::SubDecl);
        self.eat_trivia();

        // Consume modifiers: Public/Private/Friend/Static
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwFriend,
            SyntaxKind::KwStatic,
        ]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwSub);
        self.eat_whitespace();

        // Name
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump();
        }
        self.eat_whitespace();

        // Parameter list
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
        }

        // Consume to end of line
        self.eat_to_eol();

        // Body
        self.parse_block(&[SyntaxKind::KwEnd]);

        // End Sub
        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwSub) {
                self.bump();
            }
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Function declaration ────────────────────────────────

    fn parse_function_decl(&mut self) {
        self.start_node(SyntaxKind::FunctionDecl);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwFriend,
            SyntaxKind::KwStatic,
        ]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwFunction);
        self.eat_whitespace();

        // Name
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump();
        }
        self.eat_whitespace();

        // Parameter list
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
        }
        self.eat_whitespace();

        // Return type: As Type
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_type_ref();
        }

        self.eat_to_eol();

        // Body
        self.parse_block(&[SyntaxKind::KwEnd]);

        // End Function
        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwFunction) {
                self.bump();
            }
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Property declaration ────────────────────────────────

    fn parse_property_decl(&mut self) {
        self.start_node(SyntaxKind::PropertyDecl);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwFriend,
            SyntaxKind::KwStatic,
        ]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwProperty);
        self.eat_whitespace();

        // Get/Let/Set
        if self.at_any(&[SyntaxKind::KwGet, SyntaxKind::KwLet, SyntaxKind::KwSet]) {
            self.bump();
        }
        self.eat_whitespace();

        // Name
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump();
        }
        self.eat_whitespace();

        // Parameter list
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
        }
        self.eat_whitespace();

        // Return type
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_type_ref();
        }

        self.eat_to_eol();

        // Body
        self.parse_block(&[SyntaxKind::KwEnd]);

        // End Property
        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwProperty) {
                self.bump();
            }
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Declare statement (external function) ───────────────

    fn parse_declare_stmt(&mut self) {
        self.start_node(SyntaxKind::DeclareStmt);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[SyntaxKind::KwPublic, SyntaxKind::KwPrivate]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwDeclare);
        // Consume rest of line (Lib "..." Alias "..." param list etc.)
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Dim statement ───────────────────────────────────────

    fn parse_dim_stmt(&mut self) {
        self.start_node(SyntaxKind::DimStmt);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwStatic,
            SyntaxKind::KwDim,
        ]) {
            self.bump();
            self.eat_whitespace();
        }

        // Variable declarations until end of line
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Const statement ─────────────────────────────────────

    fn parse_const_stmt(&mut self) {
        self.start_node(SyntaxKind::ConstStmt);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwConst,
        ]) {
            self.bump();
            self.eat_whitespace();
        }

        self.eat_to_eol();
        self.finish_node();
    }

    // ── Type block ──────────────────────────────────────────

    fn parse_type_block(&mut self) {
        self.start_node(SyntaxKind::TypeBlock);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[SyntaxKind::KwPublic, SyntaxKind::KwPrivate]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwType);
        self.eat_to_eol();

        // Members until End Type
        self.parse_block(&[SyntaxKind::KwEnd]);

        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwType) {
                self.bump();
            }
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Enum block ──────────────────────────────────────────

    fn parse_enum_block(&mut self) {
        self.start_node(SyntaxKind::EnumBlock);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[SyntaxKind::KwPublic, SyntaxKind::KwPrivate]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwEnum);
        self.eat_to_eol();

        // Members until End Enum
        self.parse_block(&[SyntaxKind::KwEnd]);

        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwEnum) {
                self.bump();
            }
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Event declaration ───────────────────────────────────

    fn parse_event_decl(&mut self) {
        self.start_node(SyntaxKind::EventDecl);
        self.eat_trivia();

        // Modifiers
        while self.at_any(&[SyntaxKind::KwPublic, SyntaxKind::KwPrivate]) {
            self.bump();
            self.eat_whitespace();
        }

        self.expect(SyntaxKind::KwEvent);
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Parameter list ──────────────────────────────────────

    fn parse_param_list(&mut self) {
        self.start_node(SyntaxKind::ParamList);
        self.expect(SyntaxKind::LParen);

        loop {
            self.eat_trivia();
            if self.at(SyntaxKind::RParen) || self.at_eof() {
                break;
            }

            self.start_node(SyntaxKind::Param);
            // Optional/ByRef/ByVal/ParamArray
            while self.at_any(&[
                SyntaxKind::KwOptional,
                SyntaxKind::KwByRef,
                SyntaxKind::KwByVal,
                SyntaxKind::KwParamArray,
            ]) {
                self.bump();
                self.eat_whitespace();
            }

            // Parameter name
            if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                self.bump();
            }
            self.eat_whitespace();

            // Type suffix
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }

            // As Type
            if self.at(SyntaxKind::KwAs) {
                self.bump();
                self.eat_whitespace();
                self.parse_type_ref();
            }

            // Default value: = expr
            if self.at(SyntaxKind::Eq) {
                self.bump();
                self.eat_whitespace();
                // Consume until comma or RParen
                while !self.at_eof()
                    && !self.at(SyntaxKind::Comma)
                    && !self.at(SyntaxKind::RParen)
                    && !self.at(SyntaxKind::Newline)
                {
                    self.bump();
                }
            }

            self.finish_node(); // Param

            self.eat_whitespace();
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
        }

        self.eat_trivia();
        self.eat(SyntaxKind::RParen);
        self.finish_node(); // ParamList
    }

    // ── Type reference ──────────────────────────────────────

    fn parse_type_ref(&mut self) {
        self.start_node(SyntaxKind::TypeRef);
        // KwNew before type name
        if self.at(SyntaxKind::KwNew) {
            self.bump();
            self.eat_whitespace();
        }
        // Type name (possibly qualified: Module.Type)
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump();
            while self.at(SyntaxKind::Dot) {
                self.bump();
                if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                    self.bump();
                }
            }
        }
        self.finish_node();
    }

    // ── Block (procedure body or struct members) ────────────

    fn parse_block(&mut self, terminators: &[SyntaxKind]) {
        self.start_node(SyntaxKind::Block);

        loop {
            self.eat_trivia();
            if self.at_eof() {
                break;
            }

            // Check for terminators
            let kind = self.current_non_trivia();
            if terminators.contains(&kind) {
                break;
            }

            let before = self.pos;
            self.parse_statement();

            if self.pos == before {
                // No progress — consume one token to avoid infinite loop
                self.bump();
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::Colon) {
                self.bump();
            }
        }

        self.finish_node();
    }

    // ── Statement parsing ───────────────────────────────────

    fn parse_statement(&mut self) {
        self.eat_trivia();
        if self.at_eof() {
            return;
        }

        let kind = self.current();
        match kind {
            SyntaxKind::KwIf => self.parse_if_stmt(),
            SyntaxKind::KwFor => self.parse_for_stmt(),
            SyntaxKind::KwDo => self.parse_do_stmt(),
            SyntaxKind::KwWhile => self.parse_while_stmt(),
            SyntaxKind::KwSelect => self.parse_select_stmt(),
            SyntaxKind::KwWith => self.parse_with_stmt(),
            SyntaxKind::KwDim | SyntaxKind::KwStatic => self.parse_dim_stmt(),
            SyntaxKind::KwConst => self.parse_const_stmt(),
            SyntaxKind::KwReDim => self.parse_redim_stmt(),
            SyntaxKind::KwSet => self.parse_set_stmt(),
            SyntaxKind::KwLet => self.parse_let_stmt(),
            SyntaxKind::KwCall => self.parse_call_stmt(),
            SyntaxKind::KwOn => self.parse_on_error_stmt(),
            SyntaxKind::KwResume => self.parse_resume_stmt(),
            SyntaxKind::KwErase => self.parse_erase_stmt(),
            SyntaxKind::KwExit => self.parse_exit_stmt(),
            SyntaxKind::KwGoTo => self.parse_goto_stmt(),
            SyntaxKind::KwGoSub => self.parse_gosub_stmt(),
            SyntaxKind::KwReturn => self.parse_return_stmt(),
            SyntaxKind::KwRaiseEvent => self.parse_raise_event_stmt(),
            SyntaxKind::KwDebug => self.parse_call_stmt_generic(),
            SyntaxKind::KwStop => self.parse_call_stmt_generic(),
            SyntaxKind::Ident | SyntaxKind::BracketedIdent | SyntaxKind::KwMe | SyntaxKind::Dot => {
                self.parse_assign_or_call()
            }
            k if Self::is_contextual_name_keyword(k) => self.parse_assign_or_call(),
            _ => {
                // Unrecognized — wrap in error node
                self.error_recover("unexpected statement");
            }
        }
    }

    // ── If statement ────────────────────────────────────────

    fn parse_if_stmt(&mut self) {
        self.start_node(SyntaxKind::IfStmt);
        self.eat_trivia();
        self.bump(); // If
        self.eat_whitespace();

        // Condition expression (stops at Then)
        if self.is_expr_start() {
            self.parse_expr();
        }
        self.eat_whitespace();

        if self.at(SyntaxKind::KwThen) {
            self.bump(); // Then
        }

        // Check if this is a single-line If
        self.eat_whitespace();
        if !self.at(SyntaxKind::Newline) && !self.at(SyntaxKind::Comment) && !self.at_eof() {
            // Single-line If: consume rest of line
            self.eat_to_eol();
            self.finish_node();
            return;
        }

        // Multi-line If: parse block until ElseIf/Else/End If
        self.eat_to_eol();
        self.parse_block(&[SyntaxKind::KwElseIf, SyntaxKind::KwElse, SyntaxKind::KwEnd]);

        // ElseIf clauses
        while self.current_non_trivia() == SyntaxKind::KwElseIf {
            self.start_node(SyntaxKind::ElseIfClause);
            self.eat_trivia();
            self.bump(); // ElseIf
            self.eat_whitespace();

            // Condition expression (stops at Then)
            if self.is_expr_start() {
                self.parse_expr();
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::KwThen) {
                self.bump();
            }
            self.eat_to_eol();

            self.parse_block(&[SyntaxKind::KwElseIf, SyntaxKind::KwElse, SyntaxKind::KwEnd]);
            self.finish_node();
        }

        // Else clause
        if self.current_non_trivia() == SyntaxKind::KwElse {
            self.start_node(SyntaxKind::ElseClause);
            self.eat_trivia();
            self.bump(); // Else
            self.eat_to_eol();

            self.parse_block(&[SyntaxKind::KwEnd]);
            self.finish_node();
        }

        // End If
        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwIf) {
                self.bump();
            } else {
                self.error("expected `End If`".into());
            }
        } else {
            self.error("expected `End If`".into());
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── For statement ───────────────────────────────────────

    fn parse_for_stmt(&mut self) {
        self.start_node(SyntaxKind::ForStmt);
        self.eat_trivia();
        self.bump(); // For
        self.eat_whitespace();

        // Check For Each
        if self.at(SyntaxKind::KwEach) {
            self.bump(); // Each
            self.eat_whitespace();
            // variable In collection
            if self.is_expr_start() {
                self.parse_expr();
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::KwIn) {
                self.bump(); // In
                self.eat_whitespace();
                if self.is_expr_start() {
                    self.parse_expr();
                }
            }
        } else {
            // For i = start To end [Step step]
            if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                self.bump(); // counter variable
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::Eq) {
                self.bump(); // =
                self.eat_whitespace();
                if self.is_expr_start() {
                    self.parse_expr(); // start (stops at To)
                }
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::KwTo) {
                self.bump(); // To
                self.eat_whitespace();
                if self.is_expr_start() {
                    self.parse_expr(); // end (stops at Step)
                }
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::KwStep) {
                self.bump(); // Step
                self.eat_whitespace();
                if self.is_expr_start() {
                    self.parse_expr(); // step
                }
            }
        }
        self.eat_to_eol();

        // Body
        self.parse_block(&[SyntaxKind::KwNext]);

        // Next
        self.eat_trivia();
        if self.at(SyntaxKind::KwNext) {
            self.bump();
            self.eat_to_eol();
        } else {
            self.error("expected `Next`".into());
        }

        self.finish_node();
    }

    // ── Do statement ────────────────────────────────────────

    fn parse_do_stmt(&mut self) {
        self.start_node(SyntaxKind::DoStmt);
        self.eat_trivia();
        self.bump(); // Do
        self.eat_whitespace();

        // Do [While|Until condition]
        if self.at(SyntaxKind::KwWhile) || self.at(SyntaxKind::KwUntil) {
            self.bump(); // While/Until
            self.eat_whitespace();
            if self.is_expr_start() {
                self.parse_expr();
            }
        }
        self.eat_to_eol();

        // Body
        self.parse_block(&[SyntaxKind::KwLoop]);

        // Loop [While|Until condition]
        self.eat_trivia();
        if self.at(SyntaxKind::KwLoop) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwWhile) || self.at(SyntaxKind::KwUntil) {
                self.bump();
                self.eat_whitespace();
                if self.is_expr_start() {
                    self.parse_expr();
                }
            }
            self.eat_to_eol();
        } else {
            self.error("expected `Loop`".into());
        }

        self.finish_node();
    }

    // ── While statement ─────────────────────────────────────

    fn parse_while_stmt(&mut self) {
        self.start_node(SyntaxKind::WhileStmt);
        self.eat_trivia();
        self.bump(); // While
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr();
        }
        self.eat_to_eol();

        // Body
        self.parse_block(&[SyntaxKind::KwWend]);

        // Wend
        self.eat_trivia();
        if self.at(SyntaxKind::KwWend) {
            self.bump();
            self.eat_to_eol();
        }

        self.finish_node();
    }

    // ── Select Case ─────────────────────────────────────────

    fn parse_select_stmt(&mut self) {
        self.start_node(SyntaxKind::SelectStmt);
        self.eat_trivia();
        self.bump(); // Select
        self.eat_whitespace();
        if self.at(SyntaxKind::KwCase) {
            self.bump(); // Case
            self.eat_whitespace();
            if self.is_expr_start() {
                self.parse_expr();
            }
        }
        self.eat_to_eol();

        // Case clauses
        loop {
            self.eat_trivia();
            if self.at_eof() || self.at(SyntaxKind::KwEnd) {
                break;
            }

            if self.at(SyntaxKind::KwCase) {
                self.start_node(SyntaxKind::CaseClause);
                self.bump(); // Case
                self.eat_whitespace();
                if self.at(SyntaxKind::KwElse) {
                    self.bump();
                } else if self.is_expr_start() {
                    self.parse_expr();
                    self.eat_whitespace();
                    if self.at(SyntaxKind::KwTo) {
                        self.bump();
                        self.eat_whitespace();
                        if self.is_expr_start() {
                            self.parse_expr();
                        }
                    }
                }
                self.eat_to_eol();
                self.parse_block(&[SyntaxKind::KwCase, SyntaxKind::KwEnd]);
                self.finish_node();
            } else {
                break;
            }
        }

        // End Select
        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwSelect) {
                self.bump();
            }
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── With statement ──────────────────────────────────────

    fn parse_with_stmt(&mut self) {
        self.start_node(SyntaxKind::WithStmt);
        self.eat_trivia();
        self.bump(); // With
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr();
        }
        self.eat_to_eol();

        self.parse_block(&[SyntaxKind::KwEnd]);

        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwWith) {
                self.bump();
            } else {
                self.error("expected `End With`".into());
            }
        } else {
            self.error("expected `End With`".into());
        }
        self.eat_to_eol();

        self.finish_node();
    }

    // ── Simple statement parsers ────────────────────────────

    fn parse_set_stmt(&mut self) {
        self.start_node(SyntaxKind::SetStmt);
        self.eat_trivia();
        self.bump(); // Set
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_lvalue_expr(); // target
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::Eq) {
            self.bump(); // =
            self.eat_whitespace();
            self.parse_required_expr("expected expression after `=`");
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_let_stmt(&mut self) {
        self.start_node(SyntaxKind::LetStmt);
        self.eat_trivia();
        self.bump(); // Let
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_lvalue_expr(); // target
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::Eq) {
            self.bump(); // =
            self.eat_whitespace();
            self.parse_required_expr("expected expression after `=`");
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_call_stmt(&mut self) {
        self.start_node(SyntaxKind::CallStmt);
        self.eat_trivia();
        self.bump(); // Call
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr();
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_call_stmt_generic(&mut self) {
        self.start_node(SyntaxKind::CallStmt);
        self.eat_trivia();
        // Debug.Print, Stop, etc.
        if self.is_expr_start() {
            self.parse_expr();
        }
        // Bare argument list (e.g. Debug.Print x, y)
        self.eat_whitespace();
        if self.is_expr_start() || self.at(SyntaxKind::Comma) {
            self.parse_bare_arg_list();
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_on_error_stmt(&mut self) {
        self.start_node(SyntaxKind::OnErrorStmt);
        self.eat_trivia();
        self.bump(); // On
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_resume_stmt(&mut self) {
        self.start_node(SyntaxKind::ResumeStmt);
        self.eat_trivia();
        self.bump(); // Resume
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_redim_stmt(&mut self) {
        self.start_node(SyntaxKind::ReDimStmt);
        self.eat_trivia();
        self.bump(); // ReDim
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_erase_stmt(&mut self) {
        self.start_node(SyntaxKind::EraseStmt);
        self.eat_trivia();
        self.bump(); // Erase
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_exit_stmt(&mut self) {
        self.start_node(SyntaxKind::ExitStmt);
        self.eat_trivia();
        self.bump(); // Exit
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_goto_stmt(&mut self) {
        self.start_node(SyntaxKind::GoToStmt);
        self.eat_trivia();
        self.bump(); // GoTo
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_gosub_stmt(&mut self) {
        self.start_node(SyntaxKind::GoSubStmt);
        self.eat_trivia();
        self.bump(); // GoSub
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_return_stmt(&mut self) {
        self.start_node(SyntaxKind::ReturnStmt);
        self.eat_trivia();
        self.bump(); // Return
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_raise_event_stmt(&mut self) {
        self.start_node(SyntaxKind::RaiseEventStmt);
        self.eat_trivia();
        self.bump(); // RaiseEvent
        self.eat_to_statement_end();
        self.finish_node();
    }

    // ── Assignment or implicit call ─────────────────────────

    fn parse_assign_or_call(&mut self) {
        // Look ahead: if there's an `=` after the identifier chain, it's assignment.
        // Otherwise it's an implicit call.
        let is_assign = self.lookahead_is_assignment();

        if is_assign {
            self.start_node(SyntaxKind::AssignStmt);
            self.eat_trivia();
            self.parse_lvalue_expr(); // target
            self.eat_whitespace();
            if self.at(SyntaxKind::Eq) {
                self.bump(); // =
                self.eat_whitespace();
                self.parse_required_expr("expected expression after `=`");
            }
            self.eat_to_statement_end();
        } else {
            self.start_node(SyntaxKind::CallStmt);
            self.eat_trivia();
            if self.is_expr_start() {
                self.parse_expr(); // callee (possibly with member/index postfix)
            }
            // Bare argument list
            self.eat_whitespace();
            if self.is_expr_start() || self.at(SyntaxKind::Comma) {
                self.parse_bare_arg_list();
            }
            self.eat_to_statement_end();
        }

        self.finish_node();
    }

    /// Peek ahead to determine if the current line is an assignment (`x = ...`)
    /// or an implicit call (`DoSomething arg1, arg2`).
    fn lookahead_is_assignment(&self) -> bool {
        let mut j = self.pos;
        let mut paren_depth: i32 = 0;

        while j < self.tokens.len() {
            let k = self.tokens[j].0;
            match k {
                SyntaxKind::Newline | SyntaxKind::Eof | SyntaxKind::Comment => return false,
                SyntaxKind::LParen => paren_depth += 1,
                SyntaxKind::RParen => paren_depth -= 1,
                SyntaxKind::Eq if paren_depth == 0 => return true,
                _ => {}
            }
            j += 1;
        }
        false
    }

    // ── Builder finish ──────────────────────────────────────

    // (finish is handled by self.finish() above)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn parse_empty_source() {
        let p = parse("");
        assert_eq!(p.syntax().kind(), SyntaxKind::SourceFile);
        assert_eq!(p.syntax().text(), "");
        assert!(p.errors().is_empty());
    }

    #[test]
    fn round_trip_simple_sub() {
        let src = "Sub Main()\r\n    Dim x As Long\r\nEnd Sub\r\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_with_comments() {
        let src = "' Module comment\nSub Foo() ' inline\n    x = 1\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_function_with_return_type() {
        let src =
            "Public Function GetValue(x As Long) As Double\n    GetValue = x * 1.5\nEnd Function\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_if_block() {
        let src = "Sub Test()\nIf x > 0 Then\n    y = 1\nElseIf x < 0 Then\n    y = -1\nElse\n    y = 0\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_for_loop() {
        let src = "Sub Test()\nFor i = 1 To 10\n    x = x + i\nNext i\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_select_case() {
        let src = "Sub Test()\nSelect Case x\nCase 1\n    y = 1\nCase 2\n    y = 2\nCase Else\n    y = 0\nEnd Select\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_do_loop() {
        let src = "Sub Test()\nDo While x > 0\n    x = x - 1\nLoop\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_option_explicit() {
        let src = "Option Explicit\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_property() {
        let src = "Public Property Get Value() As Long\n    Value = mValue\nEnd Property\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_exported_attribute_lines() {
        let src = "Attribute VB_Name = \"Module1\"\nAttribute VB_Description = \"demo\"\nSub T()\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let attrs = collect_nodes(&p.syntax(), SyntaxKind::AttributeStmt);
        assert_eq!(
            attrs.len(),
            2,
            "expected exported attributes, got {:?}",
            attrs
        );
    }

    #[test]
    fn error_recovery_produces_tree() {
        let src = ")\n";
        let p = parse(src);
        // Tree is still lossless even with errors
        assert_eq!(p.syntax().text(), src);
        assert!(!p.errors().is_empty(), "expected parse errors");
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::ErrorNode),
            "expected explicit ErrorNode in recovered tree"
        );
    }

    #[test]
    fn incomplete_assignment_reports_error_node_without_losing_text() {
        let src = "Sub T()\n    x = \nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert_eq!(p.errors().len(), 1);
        assert_eq!(p.errors()[0].message, "expected expression after `=`");
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::ErrorNode),
            "expected zero-width ErrorNode for missing RHS"
        );
    }

    #[test]
    fn incomplete_set_assignment_reports_error_node_without_losing_text() {
        let src = "Sub T()\n    Set x = \nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert_eq!(p.errors().len(), 1);
        assert_eq!(p.errors()[0].message, "expected expression after `=`");
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::ErrorNode),
            "expected zero-width ErrorNode for missing Set RHS"
        );
    }

    #[test]
    fn incomplete_block_edits_report_stable_recovery_diagnostics() {
        let cases = [
            (
                "if",
                "Sub T()\nIf x Then\n    y = 1\nEnd Sub\n",
                "expected `End If`",
            ),
            (
                "for",
                "Sub T()\nFor i = 1 To 3\n    y = i\nEnd Sub\n",
                "expected `Next`",
            ),
            (
                "do",
                "Sub T()\nDo While x\n    x = x - 1\nEnd Sub\n",
                "expected `Loop`",
            ),
            (
                "with",
                "Sub T()\nWith obj\n    .Value = 1\nEnd Sub\n",
                "expected `End With`",
            ),
        ];

        for (label, src, expected) in cases {
            let p = parse(src);
            assert_eq!(
                p.syntax().text(),
                src,
                "{label}: source must remain lossless"
            );
            assert!(
                p.errors().iter().any(|error| error.message == expected),
                "{label}: expected diagnostic {expected:?}, got {:?}",
                p.errors()
            );
        }
    }

    #[test]
    fn node_kinds_correct() {
        let src = "Sub Foo()\nEnd Sub\n";
        let p = parse(src);
        let root = p.syntax();
        assert_eq!(root.kind(), SyntaxKind::SourceFile);

        let nodes = root.child_nodes();
        assert!(
            nodes.iter().any(|n| n.kind() == SyntaxKind::SubDecl),
            "expected SubDecl node, got: {:?}",
            nodes.iter().map(|n| n.kind()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn function_node_has_param_list() {
        let src = "Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
        let p = parse(src);
        let root = p.syntax();
        let func = root
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::FunctionDecl)
            .expect("expected FunctionDecl");

        let has_params = func
            .child_nodes()
            .iter()
            .any(|n| n.kind() == SyntaxKind::ParamList);
        assert!(has_params, "expected ParamList in FunctionDecl");
    }

    #[test]
    fn green_root_clone_preserves_shared_identity() {
        let src = "Sub T()\n    x = 1\nEnd Sub\n";
        let p = parse(src);
        let cloned = Arc::clone(p.green());
        assert!(Arc::ptr_eq(p.green(), &cloned));
        assert_eq!(cloned.width(), src.len() as u32);
    }

    #[test]
    fn red_tree_nested_token_offsets_cover_source_ranges() {
        let src = "Sub T()\n    x = a + b\nEnd Sub\n";
        let p = parse(src);
        let mut tokens = Vec::new();
        collect_tokens_recursive(&p.syntax(), &mut tokens);

        assert_eq!(p.syntax().text_range(), (0, src.len() as u32));
        assert_eq!(p.syntax().width(), src.len() as u32);
        assert!(
            tokens
                .iter()
                .any(|t| t.text == "a" && t.offset == src.find('a').unwrap() as u32),
            "expected token `a` at its source byte offset: {:?}",
            tokens
        );
        assert!(
            tokens.iter().any(|t| t.kind == SyntaxKind::Newline),
            "expected newline trivia token to be traversable"
        );
    }

    #[test]
    fn round_trip_all_statement_kinds() {
        let src = concat!(
            "Option Explicit\n",
            "Dim x As Long\n",
            "Const C = 42\n",
            "Sub Test()\n",
            "    Set obj = Nothing\n",
            "    Let y = 1\n",
            "    Call DoStuff\n",
            "    On Error GoTo ErrHandler\n",
            "    Resume Next\n",
            "    ReDim arr(10)\n",
            "    Erase arr\n",
            "    Exit Sub\n",
            "    GoTo Label1\n",
            "    GoSub Label2\n",
            "    Return\n",
            "    RaiseEvent MyEvent\n",
            "End Sub\n",
        );
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn round_trip_inline_statement_separators() {
        let src = "Sub T()\n    x = 1: y = 2: RaiseEvent Tick\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let assigns = collect_nodes(&p.syntax(), SyntaxKind::AssignStmt);
        assert_eq!(
            assigns.len(),
            2,
            "expected two assignments, got {:?}",
            assigns
        );
        assert!(has_node_kind(&p.syntax(), SyntaxKind::RaiseEventStmt));
    }

    #[test]
    fn round_trip_inline_on_error_and_resume() {
        let src = "Sub T()\n    On Error Resume Next: Resume Next\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::OnErrorStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ResumeStmt));
    }

    #[test]
    fn round_trip_with_stmt() {
        let src = "Sub Test()\nWith obj\n    .Value = 1\nEnd With\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    // ── Expression parser tests ─────────────────────────────

    /// Collect all descendant nodes of the given kind.
    fn collect_nodes(node: &crate::red::SyntaxNode<'_>, kind: SyntaxKind) -> Vec<String> {
        let mut result = Vec::new();
        collect_nodes_recursive(node, kind, &mut result);
        result
    }

    fn collect_nodes_recursive(
        node: &crate::red::SyntaxNode<'_>,
        kind: SyntaxKind,
        result: &mut Vec<String>,
    ) {
        if node.kind() == kind {
            result.push(node.text());
        }
        for child in node.child_nodes() {
            collect_nodes_recursive(&child, kind, result);
        }
    }

    fn collect_tokens_recursive<'a>(
        node: &crate::red::SyntaxNode<'a>,
        result: &mut Vec<crate::red::SyntaxToken<'a>>,
    ) {
        result.extend(node.child_tokens());
        for child in node.child_nodes() {
            collect_tokens_recursive(&child, result);
        }
    }

    /// Check that a specific node kind exists in the tree.
    fn has_node_kind(node: &crate::red::SyntaxNode<'_>, kind: SyntaxKind) -> bool {
        if node.kind() == kind {
            return true;
        }
        node.child_nodes().iter().any(|c| has_node_kind(c, kind))
    }

    #[test]
    fn expr_operator_precedence() {
        // a + b * c → BinaryExpr(a, +, BinaryExpr(b, *, c))
        let src = "Sub T()\n    x = a + b * c\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);

        let binaries = collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr);
        assert!(
            binaries.len() >= 2,
            "expected nested BinaryExpr, got {:?}",
            binaries
        );
        // The outer binary should be addition
        assert!(
            binaries.iter().any(|s| s.contains('+')),
            "expected + binary: {:?}",
            binaries
        );
        assert!(
            binaries.iter().any(|s| s.contains('*')),
            "expected * binary: {:?}",
            binaries
        );
    }

    #[test]
    fn expr_right_assoc_power() {
        // 2 ^ 3 ^ 4 → BinaryExpr(2, ^, BinaryExpr(3, ^, 4))
        let src = "Sub T()\n    x = 2 ^ 3 ^ 4\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);

        let binaries = collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr);
        assert!(
            binaries.len() >= 2,
            "expected nested ^ exprs, got {:?}",
            binaries
        );
        assert!(
            binaries.iter().any(|s| s == "3 ^ 4"),
            "expected right-associative power RHS, got {:?}",
            binaries
        );
    }

    #[test]
    fn expr_unary_not() {
        let src = "Sub T()\nIf Not x And y Then\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::UnaryExpr),
            "expected UnaryExpr for Not"
        );
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::BinaryExpr),
            "expected BinaryExpr for And"
        );
    }

    #[test]
    fn expr_member_chaining() {
        let src = "Sub T()\n    x = a.b.c\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        let members = collect_nodes(&p.syntax(), SyntaxKind::MemberExpr);
        assert!(
            members.len() >= 2,
            "expected chained MemberExpr, got {:?}",
            members
        );
    }

    #[test]
    fn expr_call_index() {
        let src = "Sub T()\n    x = f(a, b)\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::IndexExpr),
            "expected IndexExpr for f(a,b)"
        );
        assert!(
            has_node_kind(&p.syntax(), SyntaxKind::ArgList),
            "expected ArgList"
        );
    }

    #[test]
    fn expr_mixed_postfix() {
        let src = "Sub T()\n    x = obj.Method(a).Prop\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::MemberExpr));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IndexExpr));
    }

    #[test]
    fn expr_bang_index_and_member_postfix_chain() {
        let src = "Sub T()\n    x = obj!Field(0).Value\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let members = collect_nodes(&p.syntax(), SyntaxKind::MemberExpr);
        assert!(
            members.iter().any(|s| s.contains("obj!Field")),
            "expected bang member, got {:?}",
            members
        );
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IndexExpr));
    }

    #[test]
    fn expr_keyword_colliding_callee_with_type_suffix() {
        let src = "Sub T()\n    Name$ 1\n    x = Name$(1).Value\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::CallStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IndexExpr));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::MemberExpr));
    }

    #[test]
    fn expr_explicit_and_implicit_statement_call_forms() {
        let src = "Sub T()\n    Call obj.Method(1, 2)\n    obj.Method 1, 2\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let calls = collect_nodes(&p.syntax(), SyntaxKind::CallStmt);
        assert!(
            calls.len() >= 2,
            "expected both call forms, got {:?}",
            calls
        );
        assert!(has_node_kind(&p.syntax(), SyntaxKind::MemberExpr));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArgList));
    }

    #[test]
    fn expr_parens() {
        let src = "Sub T()\n    x = (a + b) * c\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ParenExpr));
    }

    #[test]
    fn expr_new() {
        let src = "Sub T()\n    Set x = New Collection\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::NewExpr));
    }

    #[test]
    fn expr_with_dot_assign() {
        let src = "Sub T()\nWith obj\n    .Value = 1\nEnd With\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::MemberExpr));
    }

    #[test]
    fn expr_stop_at_then() {
        let src = "Sub T()\nIf x + 1 Then\n    y = 1\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::BinaryExpr));
    }

    #[test]
    fn expr_stop_at_to() {
        let src = "Sub T()\nFor i = 1 To 10\n    x = i\nNext i\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn expr_line_continuation() {
        let src = "Sub T()\n    x = a + _\n        b\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::BinaryExpr));
    }

    #[test]
    fn expr_keyword_operators() {
        // Test And, Or, Xor, Mod, Like, Is, Imp, Eqv
        let src = "Sub T()\n    x = a And b\n    y = a Or b\n    z = a Xor b\n    w = a Mod b\n    p = a Like b\n    q = a Is b\n    r = a Imp b\n    s = a Eqv b\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        let binaries = collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr);
        assert!(
            binaries.len() >= 8,
            "expected 8 BinaryExpr nodes, got {}",
            binaries.len()
        );
    }

    #[test]
    fn expr_typeof_is() {
        let src = "Sub T()\nIf TypeOf obj Is Class1 Then\n    x = 1\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let binaries = collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr);
        assert!(
            binaries.iter().any(|s| s.contains("TypeOf obj Is Class1")),
            "expected TypeOf ... Is binary expression, got {:?}",
            binaries
        );
    }

    #[test]
    fn expr_error_recovery_missing_operand() {
        let src = "Sub T()\n    x = a +\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        // Should still have a BinaryExpr even with missing RHS
        assert!(has_node_kind(&p.syntax(), SyntaxKind::BinaryExpr));
    }

    #[test]
    fn expr_error_recovery_unclosed_paren() {
        let src = "Sub T()\n    x = (a + b\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(
            !p.errors().is_empty(),
            "expected parse errors for unclosed paren"
        );
    }

    #[test]
    fn expr_string_concat() {
        let src = "Sub T()\n    x = a & b & c\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        let binaries = collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr);
        assert!(binaries.len() >= 2, "expected chained & exprs");
    }

    #[test]
    fn expr_comparison_operators() {
        let src = "Sub T()\nIf x >= 0 And x <= 10 Then\n    y = 1\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn expr_unary_minus() {
        let src = "Sub T()\n    x = -a + b\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::UnaryExpr));
    }

    #[test]
    fn expr_implicit_call_with_args() {
        let src = "Sub T()\n    DoSomething x, y\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::CallStmt));
    }

    #[test]
    fn expr_for_step() {
        let src = "Sub T()\nFor i = 1 To 10 Step 2\n    x = i\nNext i\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn expr_literals() {
        let src = "Sub T()\n    x = 42\n    y = 3.14\n    z = \"hello\"\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        let lits = collect_nodes(&p.syntax(), SyntaxKind::LiteralExpr);
        assert!(lits.len() >= 3, "expected 3 literals, got {:?}", lits);
    }

    #[test]
    fn expr_boolean_literals() {
        let src = "Sub T()\n    x = True\n    y = False\n    z = Empty\n    w = Null\n    Set o = Nothing\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        let lits = collect_nodes(&p.syntax(), SyntaxKind::LiteralExpr);
        assert!(lits.len() >= 5, "expected special literals, got {:?}", lits);
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn parse_non_empty_source_roundtrips_input() {
        let len: usize = kani::any();
        kani::assume(len <= 24);

        let mut source = String::new();
        for _ in 0..len {
            let b: u8 = kani::any();
            let ascii = 32 + (b % 95);
            source.push(char::from(ascii));
        }

        let p = parse(&source);
        assert_eq!(p.syntax().text(), source);
    }
}
