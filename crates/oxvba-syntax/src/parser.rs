use std::sync::Arc;

use crate::green::{Checkpoint, GreenNode, GreenNodeBuilder};
use crate::lexer;
use crate::syntax_kind::SyntaxKind;
use oxvba_diagnostics::{Diagnostic, DiagnosticPhase, DiagnosticSource, SourceSpan};

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

impl ParseError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        self.to_diagnostic_with_context(None, None)
    }

    pub fn to_diagnostic_with_context(
        &self,
        module: Option<&str>,
        source_text: Option<&str>,
    ) -> Diagnostic {
        let mut source = DiagnosticSource::new().with_span(SourceSpan::point(self.offset));
        if let Some(module) = module {
            source = source.with_module(module);
        }
        if let Some(source_text) = source_text {
            source = source.with_source_text(source_text);
        }
        Diagnostic::error("SYN-E-PARSE", DiagnosticPhase::Syntax, self.message.clone())
            .with_source(source)
    }
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

/// Parse a single VBA expression (e.g. a `#If` conditional-compilation condition).
/// The returned `Parse`'s root (`SourceFile`) wraps the expression as its sole
/// non-trivia child; callers walk to the first expression child to evaluate it.
pub fn parse_expression(source: &str) -> Parse {
    let tokens = lexer::tokenize(source);
    let mut p = Parser::new(&tokens);
    p.start_node(SyntaxKind::SourceFile);
    p.eat_trivia();
    if !p.at_eof() {
        p.parse_expr();
    }
    p.eat_trivia();
    if p.at(SyntaxKind::Eof) {
        p.bump();
    }
    p.finish_node();
    let (green, errors) = p.finish();
    Parse { green, errors }
}

/// File-I/O statements whose leading word lexes as an identifier (not a keyword).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileIoIdent {
    Put,
    Seek,
    Width,
    Unlock,
    Reset,
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
    /// Extra `Next` closures already consumed by a multi-variable `Next i, j`.
    pending_multinext_closures: usize,
    /// Active source-level `For` parser nesting.
    for_depth: usize,
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
            pending_multinext_closures: 0,
            for_depth: 0,
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

    /// Enter a procedure body after its header (modifiers, keyword, name,
    /// params, return type). A single-line procedure puts its whole body and
    /// `End <kw>` after a `:` on the same physical line
    /// (`Property Get X() As Long: X = m_X: End Property`); consume that
    /// separator so the body block parses the inline statements (which stop at
    /// `End`). Otherwise consume the rest of the header line as usual.
    fn enter_proc_body(&mut self) {
        self.eat_whitespace();
        if self.at(SyntaxKind::Colon) {
            self.bump();
        } else {
            self.eat_to_eol();
        }
    }

    fn eat_to_statement_end(&mut self) {
        while !self.at_eof()
            && !self.at(SyntaxKind::Newline)
            && !self.at(SyntaxKind::Comment)
            && !self.at(SyntaxKind::Colon)
            && !self.at(SyntaxKind::KwElse)
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
                | SyntaxKind::KwError
                // `Lib` is a keyword only inside `Declare … Lib "…"`; everywhere else
                // (e.g. a project named `Lib` in `Lib.Member` / `TypeOf x Is Lib.IFoo`)
                // it is an ordinary identifier.
                | SyntaxKind::KwLib
        )
    }

    fn is_expr_name_token(kind: SyntaxKind) -> bool {
        matches!(kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent)
            || Self::is_contextual_name_keyword(kind)
    }

    /// True when the current parenthesized postfix indexes a receiver that
    /// immediately continues with member access (`obj(1).Foo arg`), so it still
    /// belongs to an implicit statement-call callee.
    fn paren_postfix_continues_member(&self) -> bool {
        debug_assert!(self.at(SyntaxKind::LParen));

        let mut j = self.pos;
        let mut depth: i32 = 0;
        while j < self.tokens.len() {
            match self.tokens[j].0 {
                SyntaxKind::LParen => depth += 1,
                SyntaxKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        while j < self.tokens.len()
                            && matches!(
                                self.tokens[j].0,
                                SyntaxKind::Whitespace | SyntaxKind::LineContinuation
                            )
                        {
                            j += 1;
                        }
                        return matches!(
                            self.tokens.get(j).map(|(kind, _)| *kind),
                            Some(SyntaxKind::Dot | SyntaxKind::Bang)
                        );
                    }
                }
                SyntaxKind::Eof | SyntaxKind::Newline | SyntaxKind::Comment | SyntaxKind::Colon
                    if depth == 0 =>
                {
                    return false;
                }
                _ => {}
            }
            j += 1;
        }

        false
    }

    fn paren_postfix_is_attached(&self) -> bool {
        debug_assert!(self.at(SyntaxKind::LParen));
        self.pos > 0
            && !matches!(
                self.tokens[self.pos - 1].0,
                SyntaxKind::Whitespace | SyntaxKind::LineContinuation
            )
    }

    /// True if this token stops expression parsing (at zero paren depth).
    fn is_expr_stop(&self, kind: SyntaxKind) -> bool {
        match kind {
            SyntaxKind::Eof => true,
            SyntaxKind::Newline | SyntaxKind::Comment if self.paren_depth == 0 => true,
            SyntaxKind::KwThen
            | SyntaxKind::KwElse
            | SyntaxKind::KwTo
            | SyntaxKind::KwStep
            | SyntaxKind::Colon => true,
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
            | SyntaxKind::KwAddressOf
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

    /// Parse the callee of an implicit statement call. A following parenthesized
    /// expression belongs to the argument list (`Foo (x)`), not the callee.
    fn parse_statement_call_callee_expr(&mut self) {
        self.eat_expr_whitespace();
        self.parse_expr_bp_with_options(29, false);
    }

    /// Pratt parser core.
    fn parse_expr_bp(&mut self, min_bp: u8) {
        self.parse_expr_bp_with_options(min_bp, true);
    }

    fn parse_expr_bp_with_options(&mut self, min_bp: u8, allow_index_postfix: bool) {
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
                    let allow_index_here = allow_index_postfix
                        || self.paren_postfix_is_attached()
                        || self.paren_postfix_continues_member();
                    if !allow_index_here || min_bp > 30 {
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
            // AddressOf proc  (a procedure reference)
            SyntaxKind::KwAddressOf => {
                self.start_node(SyntaxKind::AddressOfExpr);
                self.bump(); // AddressOf
                self.eat_expr_whitespace();
                // The operand is a procedure name (possibly `Module.Proc`); parse it
                // at a high binding power so no trailing operator is consumed.
                self.parse_expr_bp(15);
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
                self.parse_bare_arg();
            }

            while self.at(SyntaxKind::Comma) || {
                self.eat_expr_whitespace();
                self.at(SyntaxKind::Comma)
            } {
                self.bump(); // ,
                self.eat_expr_whitespace();
                // Support empty argument slots: f(a,,b)
                if !self.at(SyntaxKind::Comma) && !self.at(SyntaxKind::RParen) && !self.at_eof() {
                    self.parse_bare_arg();
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

        // Named arg: name:=expr — check for ColonEq. A LEADING omitted positional
        // (`Add , , 2` — the first one or two args omitted) starts the list with a
        // comma, so the first slot has no expression: only parse it when there is one,
        // mirroring the empty-slot guard in the loop below. (Without this, a leading
        // `,` fell into `parse_expr_bp` and raised "expected expression".)
        if !self.at(SyntaxKind::Comma) {
            self.parse_bare_arg();
        }

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
        self.parse_bare_arg_with_optional_to(false);
    }

    fn parse_bare_arg_with_optional_to(&mut self, allow_to: bool) {
        // Check for named argument: Ident := expr
        if (self.at(SyntaxKind::Ident) || self.current().is_keyword()) && self.peek_is_colon_eq() {
            self.bump(); // name
            self.eat_expr_whitespace();
            self.bump(); // :=
            self.eat_expr_whitespace();
        }
        // Call-site passing-mode override: `ByVal expr` / `ByRef expr` (the
        // CopyMemory/API idiom forcing a value or reference at the call). The
        // keyword is kept as a leading token of the argument segment; the
        // binder honoring it is tracked separately.
        if self.at(SyntaxKind::KwByVal) || self.at(SyntaxKind::KwByRef) {
            self.bump();
            self.eat_expr_whitespace();
        }
        self.parse_expr_bp(0);
        self.eat_expr_whitespace();
        if allow_to && self.at(SyntaxKind::KwTo) {
            self.bump(); // To
            self.eat_expr_whitespace();
            if !self.is_expr_stop(self.current()) && !self.at(SyntaxKind::Comma) {
                self.parse_expr_bp(0);
            }
        }
    }

    /// Check if the next non-trivia token after current is ColonEq.
    fn peek_is_colon_eq(&self) -> bool {
        let mut j = self.pos + 1;
        while j < self.tokens.len() && self.tokens[j].0.is_trivia() {
            j += 1;
        }
        j < self.tokens.len() && self.tokens[j].0 == SyntaxKind::ColonEq
    }

    fn peek_is_label_colon(&self) -> bool {
        let mut j = self.pos + 1;
        while j < self.tokens.len() && self.tokens[j].0 == SyntaxKind::Whitespace {
            j += 1;
        }
        j < self.tokens.len() && self.tokens[j].0 == SyntaxKind::Colon
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
        if self.at(SyntaxKind::KwExplicit) {
            self.bump();
        } else if self.at(SyntaxKind::KwBase) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::IntLiteral) {
                self.bump();
            }
        } else if self.at(SyntaxKind::KwCompare) {
            self.bump();
            self.eat_whitespace();
            // Binary (keyword) | Text (lexes as Ident) | Database (Ident)
            if self.at(SyntaxKind::KwBinary) || self.at(SyntaxKind::Ident) {
                self.bump();
            }
        } else if self.at(SyntaxKind::KwPrivate) {
            self.bump(); // Option Private Module
            self.eat_whitespace();
            if self.at(SyntaxKind::Ident) {
                self.bump();
            }
        }
        self.finish_node();
    }

    // ── Attribute statement (exported module metadata) ──────

    fn parse_attribute_stmt(&mut self) {
        self.start_node(SyntaxKind::AttributeStmt);
        self.eat_trivia();
        self.bump(); // Attribute
        self.eat_whitespace();
        self.parse_name_path(); // VB_Name | Foo.VB_VarUserMemId
        self.eat_whitespace();
        if self.at(SyntaxKind::Eq) {
            self.bump(); // =
            self.eat_whitespace();
            self.parse_required_expr("expected attribute value");
            loop {
                self.eat_whitespace();
                if !self.eat(SyntaxKind::Comma) {
                    break;
                }
                self.eat_whitespace();
                self.parse_expr_bp(0);
            }
        }
        self.finish_node();
    }

    // ── Implements statement ────────────────────────────────

    fn parse_implements_stmt(&mut self) {
        self.start_node(SyntaxKind::ImplementsStmt);
        self.eat_trivia();
        self.bump(); // Implements
        self.eat_whitespace();
        self.parse_type_ref();
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
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
        }
        self.eat_whitespace();

        // Parameter list
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
        }

        // Header → body (inline `: … : End Sub`, or the rest of the line)
        self.enter_proc_body();

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
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
        }
        self.eat_whitespace();

        // Parameter list
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
        }
        self.eat_whitespace();

        // Return type: As Type, optionally an array return (`As Byte()`).
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_type_ref();
            self.eat_whitespace();
            if self.at(SyntaxKind::LParen) {
                self.parse_array_bounds();
            }
        }

        // Header → body (inline `: … : End Function`, or the rest of the line)
        self.enter_proc_body();

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

        // Header → body (inline `: … : End Property`, or the rest of the line)
        self.enter_proc_body();

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
        self.eat_whitespace();
        // Optional `PtrSafe` (lexes as an identifier; the only ident here).
        if self.at(SyntaxKind::Ident) {
            self.bump();
            self.eat_whitespace();
        }
        // Sub | Function
        if self.at_any(&[SyntaxKind::KwSub, SyntaxKind::KwFunction]) {
            self.bump();
            self.eat_whitespace();
        }
        // Name
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump();
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::KwLib) {
            self.parse_lib_clause();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::KwAlias) {
            self.parse_alias_clause();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_type_ref();
        }
        self.finish_node();
    }

    fn parse_lib_clause(&mut self) {
        self.start_node(SyntaxKind::LibClause);
        self.bump(); // Lib
        self.eat_whitespace();
        if self.at(SyntaxKind::StringLiteral) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_alias_clause(&mut self) {
        self.start_node(SyntaxKind::AliasClause);
        self.bump(); // Alias
        self.eat_whitespace();
        if self.at(SyntaxKind::StringLiteral) {
            self.bump();
        }
        self.finish_node();
    }

    // ── Dim statement ───────────────────────────────────────

    fn parse_dim_stmt(&mut self) {
        self.start_node(SyntaxKind::DimStmt);
        self.eat_trivia();
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwStatic,
            SyntaxKind::KwDim,
        ]) {
            self.bump();
            self.eat_whitespace();
        }
        self.parse_declarator_list();
        self.finish_node();
    }

    // ── Const statement ─────────────────────────────────────

    fn parse_const_stmt(&mut self) {
        self.start_node(SyntaxKind::ConstStmt);
        self.eat_trivia();
        while self.at_any(&[
            SyntaxKind::KwPublic,
            SyntaxKind::KwPrivate,
            SyntaxKind::KwConst,
        ]) {
            self.bump();
            self.eat_whitespace();
        }
        self.parse_declarator_list();
        self.finish_node();
    }

    /// Comma-separated `VarDeclarator`s for Dim/Const.
    fn parse_declarator_list(&mut self) {
        loop {
            let before = self.pos;
            self.parse_var_declarator();
            self.eat_trivia();
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
            self.eat_whitespace();
            if self.pos == before {
                break; // safety
            }
        }
    }

    /// One declarator: `[WithEvents] name [(bounds)] [As [New] Type [* N]] [= expr]`.
    fn parse_var_declarator(&mut self) {
        self.start_node(SyntaxKind::VarDeclarator);
        self.eat_whitespace();
        if self.at(SyntaxKind::KwWithEvents) {
            self.bump();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::Ident)
            || self.at(SyntaxKind::BracketedIdent)
            || self.current().is_keyword()
        {
            self.bump(); // name
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::LParen) {
            self.parse_array_bounds();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_type_ref(); // handles `New` + qualified name
            self.eat_whitespace();
            if self.at(SyntaxKind::Star) {
                self.bump(); // fixed-length string: As String * N
                self.eat_whitespace();
                self.parse_expr_bp(0);
            }
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::Eq) {
            self.bump(); // Const initializer
            self.eat_whitespace();
            self.parse_required_expr("expected constant value");
        }
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
        self.eat_whitespace();
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump(); // type name
        }

        // UDT fields until `End Type`.
        loop {
            self.eat_trivia();
            if self.at_eof() || self.current_non_trivia() == SyntaxKind::KwEnd {
                break;
            }
            let before = self.pos;
            self.parse_type_field();
            if self.pos == before {
                self.bump(); // safety
            }
        }

        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwType) {
                self.bump();
            }
        }

        self.finish_node();
    }

    /// One UDT field: `Name [(bounds)] As Type [* N]`.
    fn parse_type_field(&mut self) {
        self.start_node(SyntaxKind::TypeField);
        self.eat_trivia();
        if self.at(SyntaxKind::Ident)
            || self.at(SyntaxKind::BracketedIdent)
            || self.current().is_keyword()
        {
            self.bump();
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::LParen) {
            self.parse_array_bounds();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_type_ref();
            self.eat_whitespace();
            if self.at(SyntaxKind::Star) {
                self.bump(); // fixed-length string
                self.eat_whitespace();
                self.parse_expr_bp(0);
            }
        }
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
        self.eat_whitespace();
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump(); // enum name
        }

        // Members until `End Enum`.
        loop {
            self.eat_trivia();
            if self.at_eof() || self.current_non_trivia() == SyntaxKind::KwEnd {
                break;
            }
            let before = self.pos;
            self.parse_enum_member();
            if self.pos == before {
                self.bump(); // safety
            }
        }

        self.eat_trivia();
        if self.at(SyntaxKind::KwEnd) {
            self.bump();
            self.eat_whitespace();
            if self.at(SyntaxKind::KwEnum) {
                self.bump();
            }
        }

        self.finish_node();
    }

    /// One enum member: `Name [= expr]`.
    fn parse_enum_member(&mut self) {
        self.start_node(SyntaxKind::EnumMember);
        self.eat_trivia();
        if self.at(SyntaxKind::Ident)
            || self.at(SyntaxKind::BracketedIdent)
            || self.current().is_keyword()
        {
            self.bump();
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::Eq) {
            self.bump();
            self.eat_whitespace();
            self.parse_expr_bp(0);
        }
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
        self.eat_whitespace();
        if self.at(SyntaxKind::Ident)
            || self.at(SyntaxKind::BracketedIdent)
            || self.current().is_keyword()
        {
            self.bump(); // event name
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::LParen) {
            self.parse_param_list();
        }
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

            // Type suffix
            self.eat_whitespace();
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }

            // As Type
            self.eat_whitespace();
            if self.at(SyntaxKind::KwAs) {
                self.bump();
                self.eat_whitespace();
                self.parse_type_ref();
            }

            // Default value: = expr
            self.eat_whitespace();
            if self.at(SyntaxKind::Eq) {
                self.start_node(SyntaxKind::ParamDefault);
                self.bump(); // =
                self.eat_whitespace();
                self.parse_expr_bp(0);
                self.finish_node();
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

    // ── Shared structured sub-nodes ─────────────────────────

    /// A label *reference* (`GoTo X` / `On Error GoTo 0` / `On Error GoTo -1` /
    /// `Resume Label`). `-1` is the sole valid negative target (it clears the active
    /// error for `On Error GoTo -1`); it lexes as a `Minus` followed by an `IntLiteral`.
    fn parse_label_ref(&mut self) {
        self.eat_whitespace();
        if self.at(SyntaxKind::Minus) {
            self.start_node(SyntaxKind::LabelRef);
            self.bump(); // -
            self.eat_whitespace();
            self.eat(SyntaxKind::IntLiteral);
            self.finish_node();
        } else if self.at(SyntaxKind::Ident)
            || self.at(SyntaxKind::BracketedIdent)
            || self.at(SyntaxKind::IntLiteral)
        {
            self.start_node(SyntaxKind::LabelRef);
            self.bump();
            self.finish_node();
        }
    }

    /// `( bound, bound, … )` — array dimensions for Dim/Const/Type/ReDim.
    fn parse_array_bounds(&mut self) {
        self.start_node(SyntaxKind::ArrayBounds);
        if self.at(SyntaxKind::LParen) {
            self.bump_lparen();
        }
        self.eat_expr_whitespace();
        if !self.at(SyntaxKind::RParen) && !self.at_eof() {
            if !self.at(SyntaxKind::Comma) {
                self.parse_bound();
            }
            loop {
                self.eat_expr_whitespace();
                if !self.at(SyntaxKind::Comma) {
                    break;
                }
                self.bump(); // ,
                self.eat_expr_whitespace();
                if !self.at(SyntaxKind::Comma) && !self.at(SyntaxKind::RParen) && !self.at_eof() {
                    self.parse_bound();
                }
            }
        }
        self.eat_expr_whitespace();
        if self.at(SyntaxKind::RParen) {
            self.bump_rparen();
        } else {
            self.error("expected `)`".into());
        }
        self.finish_node();
    }

    /// One dimension: `[lower To] upper`.
    fn parse_bound(&mut self) {
        self.start_node(SyntaxKind::Bound);
        self.eat_expr_whitespace();
        self.parse_expr_bp(0);
        self.eat_expr_whitespace();
        if self.at(SyntaxKind::KwTo) {
            self.bump(); // To
            self.eat_expr_whitespace();
            self.parse_expr_bp(0);
        }
        self.finish_node();
    }

    // ── Block (procedure body or struct members) ────────────

    fn parse_block(&mut self, terminators: &[SyntaxKind]) {
        self.start_node(SyntaxKind::Block);

        loop {
            self.eat_trivia();
            while self.at(SyntaxKind::Colon) {
                self.bump();
                self.eat_whitespace();
            }
            if self.at_eof() {
                break;
            }

            // Check for terminators
            let kind = self.current_non_trivia();
            if self.pending_multinext_closures > 0 && terminators.contains(&SyntaxKind::KwNext) {
                break;
            }
            if self.is_block_terminator(kind, terminators) {
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

    fn is_block_terminator(&self, kind: SyntaxKind, terminators: &[SyntaxKind]) -> bool {
        if !terminators.contains(&kind) {
            return false;
        }
        if kind != SyntaxKind::KwEnd {
            return true;
        }
        matches!(
            self.peek_next_non_trivia_before_statement_end(),
            SyntaxKind::KwSub
                | SyntaxKind::KwFunction
                | SyntaxKind::KwProperty
                | SyntaxKind::KwIf
                | SyntaxKind::KwSelect
                | SyntaxKind::KwWith
                | SyntaxKind::KwType
                | SyntaxKind::KwEnum
        )
    }

    fn peek_next_non_trivia_before_statement_end(&self) -> SyntaxKind {
        let mut j = self.pos + 1;
        while j < self.tokens.len() {
            match self.tokens[j].0 {
                SyntaxKind::Whitespace | SyntaxKind::LineContinuation => j += 1,
                SyntaxKind::Newline | SyntaxKind::Comment | SyntaxKind::Colon => {
                    return SyntaxKind::Eof;
                }
                kind => return kind,
            }
        }
        SyntaxKind::Eof
    }

    // ── Statement parsing ───────────────────────────────────

    fn parse_statement(&mut self) {
        self.eat_trivia();
        if self.at_eof() {
            return;
        }

        // File-I/O statements whose leading word lexes as an identifier
        // (Put/Seek/Width/Unlock/Reset), routed by statement-position + shape.
        if self.current() == SyntaxKind::Ident
            && !self.peek_is_label_colon()
            && let Some(which) = self.ident_file_stmt_kind()
        {
            self.parse_ident_file_stmt(which);
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
            SyntaxKind::KwLSet => self.parse_lset_rset_stmt(SyntaxKind::LSetStmt),
            SyntaxKind::KwRSet => self.parse_lset_rset_stmt(SyntaxKind::RSetStmt),
            SyntaxKind::KwCall => self.parse_call_stmt(),
            SyntaxKind::KwOn if self.peek_next_non_trivia_is(SyntaxKind::KwError) => {
                self.parse_on_error_stmt()
            }
            SyntaxKind::KwOn => self.parse_on_computed_stmt(),
            SyntaxKind::KwError => self.parse_error_stmt(),
            SyntaxKind::KwResume => self.parse_resume_stmt(),
            SyntaxKind::KwErase => self.parse_erase_stmt(),
            SyntaxKind::KwEnd => self.parse_end_stmt(),
            SyntaxKind::KwExit => self.parse_exit_stmt(),
            SyntaxKind::KwGoTo => self.parse_goto_stmt(),
            SyntaxKind::KwGoSub => self.parse_gosub_stmt(),
            SyntaxKind::KwReturn => self.parse_return_stmt(),
            SyntaxKind::KwRaiseEvent => self.parse_raise_event_stmt(),
            SyntaxKind::KwDebug => self.parse_call_stmt_generic(),
            SyntaxKind::KwStop => self.parse_call_stmt_generic(),
            // ── File-I/O statements led by a keyword ──
            SyntaxKind::KwOpen if !self.peek_keyword_used_as_name() => self.parse_open_stmt(),
            SyntaxKind::KwClose if !self.peek_keyword_used_as_name() => self.parse_close_stmt(),
            SyntaxKind::KwWrite if !self.peek_keyword_used_as_name() => {
                self.parse_print_like_stmt(SyntaxKind::WriteStmt)
            }
            SyntaxKind::KwLock if !self.peek_keyword_used_as_name() => self.parse_lock_stmt(),
            SyntaxKind::KwGet if !self.peek_keyword_used_as_name() => {
                self.parse_get_put_stmt(SyntaxKind::GetStmt)
            }
            SyntaxKind::KwName
                if !self.peek_keyword_used_as_name()
                    && self.peek_top_level_before_stmt_end(SyntaxKind::KwAs) =>
            {
                self.parse_name_stmt()
            }
            SyntaxKind::KwLine if self.peek_next_non_trivia_is(SyntaxKind::KwInput) => {
                self.parse_line_input_stmt()
            }
            SyntaxKind::KwPrint if self.peek_next_non_trivia_is(SyntaxKind::Hash) => {
                self.parse_print_like_stmt(SyntaxKind::PrintStmt)
            }
            SyntaxKind::KwInput if self.peek_next_non_trivia_is(SyntaxKind::Hash) => {
                self.parse_input_stmt()
            }
            k if Self::is_contextual_name_keyword(k) && self.peek_is_label_colon() => {
                self.parse_label_stmt()
            }
            SyntaxKind::Ident | SyntaxKind::BracketedIdent if self.peek_is_label_colon() => {
                self.parse_label_stmt()
            }
            SyntaxKind::IntLiteral => self.parse_line_number_label_stmt(),
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
            self.parse_inline_statement_block(&[SyntaxKind::KwElse]);
            self.eat_whitespace();
            if self.at(SyntaxKind::KwElse) {
                self.start_node(SyntaxKind::ElseClause);
                self.bump(); // Else
                self.parse_inline_statement_block(&[]);
                self.finish_node();
            }
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
            self.eat_whitespace();
            if self.at(SyntaxKind::Colon) {
                self.parse_inline_statement_block(&[]);
                self.eat_to_eol();
            } else {
                self.eat_to_eol();
            }

            self.parse_block(&[SyntaxKind::KwElseIf, SyntaxKind::KwElse, SyntaxKind::KwEnd]);
            self.finish_node();
        }

        // Else clause
        if self.current_non_trivia() == SyntaxKind::KwElse {
            self.start_node(SyntaxKind::ElseClause);
            self.eat_trivia();
            self.bump(); // Else
            self.eat_whitespace();
            if self.at(SyntaxKind::Colon) {
                self.parse_inline_statement_block(&[]);
                self.eat_to_eol();
            } else {
                self.eat_to_eol();
            }

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

    fn parse_inline_statement_block(&mut self, terminators: &[SyntaxKind]) {
        self.start_node(SyntaxKind::Block);
        loop {
            self.eat_whitespace();
            if self.at(SyntaxKind::Colon) {
                self.bump();
                continue;
            }
            let kind = self.current_non_trivia();
            if self.at_eof()
                || self.at(SyntaxKind::Newline)
                || self.at(SyntaxKind::Comment)
                || terminators.contains(&kind)
            {
                break;
            }

            let before = self.pos;
            self.parse_statement();
            if self.pos == before {
                self.bump();
            }
            self.eat_whitespace();
            if self.at(SyntaxKind::Colon) {
                self.bump();
            }
        }
        self.finish_node();
    }

    // ── For statement ───────────────────────────────────────

    fn parse_for_stmt(&mut self) {
        self.start_node(SyntaxKind::ForStmt);
        self.for_depth += 1;
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
        if self.pending_multinext_closures > 0 {
            self.pending_multinext_closures -= 1;
        } else if self.at(SyntaxKind::KwNext) {
            self.parse_next_clause();
        } else {
            self.error("expected `Next`".into());
        }

        self.for_depth -= 1;
        self.finish_node();
    }

    fn parse_next_clause(&mut self) {
        self.bump(); // Next
        self.eat_whitespace();

        let mut names = 0usize;
        loop {
            if self.at(SyntaxKind::Ident)
                || self.at(SyntaxKind::BracketedIdent)
                || self.current().is_keyword()
            {
                names += 1;
                self.bump();
                if self.at(SyntaxKind::TypeSuffix) {
                    self.bump();
                }
                self.eat_whitespace();
            }
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
            self.eat_whitespace();
        }
        if names > 1 {
            let extra = names - 1;
            let available_enclosing = self.for_depth.saturating_sub(1);
            self.pending_multinext_closures += extra.min(available_enclosing);
            if extra > available_enclosing {
                self.error("`Next` without matching `For`".into());
            }
        }
        self.eat_to_eol();
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
                } else if self.at(SyntaxKind::KwIs) {
                    self.bump();
                    self.eat_whitespace();
                    if matches!(
                        self.current(),
                        SyntaxKind::Eq
                            | SyntaxKind::LtGt
                            | SyntaxKind::Lt
                            | SyntaxKind::LtEq
                            | SyntaxKind::Gt
                            | SyntaxKind::GtEq
                    ) {
                        self.bump();
                        self.eat_whitespace();
                    }
                    if self.is_expr_start() {
                        self.parse_expr();
                    }
                } else if self.is_expr_start() {
                    loop {
                        self.parse_expr();
                        self.eat_whitespace();
                        if self.at(SyntaxKind::KwTo) {
                            self.bump();
                            self.eat_whitespace();
                            if self.is_expr_start() {
                                self.parse_expr();
                            }
                            self.eat_whitespace();
                        }
                        if self.at(SyntaxKind::Comma) {
                            self.bump();
                            self.eat_whitespace();
                            if !self.is_expr_start() {
                                break;
                            }
                        } else {
                            break;
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

    fn parse_lset_rset_stmt(&mut self, node: SyntaxKind) {
        self.start_node(node);
        self.eat_trivia();
        self.bump(); // LSet/RSet
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
        self.eat_whitespace();
        if self.is_expr_start() || self.at(SyntaxKind::Comma) {
            self.parse_bare_arg_list();
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
        self.eat_whitespace();
        self.eat(SyntaxKind::KwError);
        self.eat_whitespace();
        if self.at(SyntaxKind::KwResume) {
            self.bump();
            self.eat_whitespace();
            self.eat(SyntaxKind::KwNext);
        } else if self.at(SyntaxKind::KwGoTo) {
            self.bump();
            self.parse_label_ref(); // a label, or `0`
        }
        self.finish_node();
    }

    /// The legacy `Error <number>` statement (MS-VBAL §5.4.4.3) — raises the given
    /// run-time error. The number is a full expression.
    fn parse_error_stmt(&mut self) {
        self.start_node(SyntaxKind::ErrorStmt);
        self.eat_trivia();
        self.bump(); // Error
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr();
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    /// `On <expr> GoTo L1, L2, …` / `On <expr> GoSub S1, S2, …` — the computed branch
    /// (1-based; `0`/out-of-range falls through). Distinguished from `On Error …` by the
    /// caller (which checks for a following `Error` keyword).
    fn parse_on_computed_stmt(&mut self) {
        self.start_node(SyntaxKind::OnComputedStmt);
        self.eat_trivia();
        self.bump(); // On
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr(); // the selector (stops at GoTo / GoSub)
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::KwGoTo) || self.at(SyntaxKind::KwGoSub) {
            self.bump();
        }
        self.eat_whitespace();
        loop {
            self.parse_label_ref();
            self.eat_whitespace();
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
            self.eat_whitespace();
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    fn parse_resume_stmt(&mut self) {
        self.start_node(SyntaxKind::ResumeStmt);
        self.eat_trivia();
        self.bump(); // Resume
        self.eat_whitespace();
        if self.at(SyntaxKind::KwNext) {
            self.bump();
        } else {
            self.parse_label_ref();
        }
        self.finish_node();
    }

    fn parse_end_stmt(&mut self) {
        self.start_node(SyntaxKind::EndStmt);
        self.eat_trivia();
        self.bump(); // End
        self.finish_node();
    }

    fn parse_redim_stmt(&mut self) {
        self.start_node(SyntaxKind::ReDimStmt);
        self.eat_trivia();
        self.bump(); // ReDim
        self.eat_whitespace();
        if self.at(SyntaxKind::KwPreserve) {
            self.bump();
            self.eat_whitespace();
        }
        loop {
            // Target name path (`a` / `obj.arr`); the bounds follow, so we stop at `(`.
            self.parse_name_path();
            self.eat_whitespace();
            if self.at(SyntaxKind::LParen) {
                self.parse_array_bounds();
            }
            self.eat_whitespace();
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
            self.eat_whitespace();
        }
        self.finish_node();
    }

    /// A dotted/banged name path with no trailing index — used by ReDim targets.
    fn parse_name_path(&mut self) {
        if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
            self.bump();
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
            while self.at(SyntaxKind::Dot) || self.at(SyntaxKind::Bang) {
                self.bump();
                self.eat_whitespace();
                if self.at(SyntaxKind::Ident) || self.current().is_keyword() {
                    self.bump();
                    if self.at(SyntaxKind::TypeSuffix) {
                        self.bump();
                    }
                } else {
                    break;
                }
            }
        }
    }

    fn parse_erase_stmt(&mut self) {
        self.start_node(SyntaxKind::EraseStmt);
        self.eat_trivia();
        self.bump(); // Erase
        self.eat_whitespace();
        loop {
            if !self.is_expr_start() {
                break;
            }
            self.parse_lvalue_expr();
            self.eat_whitespace();
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
            self.eat_whitespace();
        }
        self.finish_node();
    }

    fn parse_exit_stmt(&mut self) {
        self.start_node(SyntaxKind::ExitStmt);
        self.eat_trivia();
        self.bump(); // Exit
        self.eat_whitespace();
        if self.at_any(&[
            SyntaxKind::KwSub,
            SyntaxKind::KwFunction,
            SyntaxKind::KwProperty,
            SyntaxKind::KwFor,
            SyntaxKind::KwDo,
        ]) {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_goto_stmt(&mut self) {
        self.start_node(SyntaxKind::GoToStmt);
        self.eat_trivia();
        self.bump(); // GoTo
        self.parse_label_ref();
        self.finish_node();
    }

    fn parse_label_stmt(&mut self) {
        self.start_node(SyntaxKind::LabelStmt);
        self.eat_trivia();
        self.bump(); // label
        self.eat_whitespace();
        self.expect(SyntaxKind::Colon);
        self.finish_node();
    }

    fn parse_line_number_label_stmt(&mut self) {
        self.start_node(SyntaxKind::LabelStmt);
        self.eat_trivia();
        self.bump(); // line number
        self.eat_whitespace();
        self.eat(SyntaxKind::Colon);
        self.finish_node();
    }

    fn parse_gosub_stmt(&mut self) {
        self.start_node(SyntaxKind::GoSubStmt);
        self.eat_trivia();
        self.bump(); // GoSub
        self.parse_label_ref();
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
        self.eat_whitespace();
        if self.at(SyntaxKind::Ident)
            || self.at(SyntaxKind::BracketedIdent)
            || self.current().is_keyword()
        {
            self.bump();
            if self.at(SyntaxKind::TypeSuffix) {
                self.bump();
            }
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::LParen) {
            self.parse_arg_list();
        } else if self.is_expr_start() || self.at(SyntaxKind::Comma) {
            self.parse_bare_arg_list();
        }
        self.eat_to_statement_end();
        self.finish_node();
    }

    // ── File I/O statements ─────────────────────────────────

    /// `[#] channel` — the file number shared by every file statement.
    fn parse_file_number(&mut self) {
        self.start_node(SyntaxKind::FileNumber);
        self.eat_whitespace();
        if self.at(SyntaxKind::Hash) {
            self.bump();
        }
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr_bp(0);
        }
        self.finish_node();
    }

    fn parse_open_stmt(&mut self) {
        self.start_node(SyntaxKind::OpenStmt);
        self.eat_trivia();
        self.bump(); // Open
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr_bp(0); // path (stops at `For`)
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::KwFor) {
            self.bump();
            self.eat_whitespace();
            if self.at_any(&[
                SyntaxKind::KwInput,
                SyntaxKind::KwOutput,
                SyntaxKind::KwAppend,
                SyntaxKind::KwBinary,
                SyntaxKind::KwRandom,
            ]) {
                self.bump();
            }
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::KwAccess) {
            self.bump();
            self.eat_whitespace();
            while self.at_any(&[SyntaxKind::KwRead, SyntaxKind::KwWrite]) {
                self.bump();
                self.eat_whitespace();
            }
        }
        if self.at(SyntaxKind::KwShared) {
            self.bump();
            self.eat_whitespace();
        } else if self.at(SyntaxKind::KwLock) {
            self.bump();
            self.eat_whitespace();
            while self.at_any(&[SyntaxKind::KwRead, SyntaxKind::KwWrite]) {
                self.bump();
                self.eat_whitespace();
            }
        }
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            self.parse_file_number();
            self.eat_whitespace();
        }
        if self.at(SyntaxKind::Ident) {
            self.bump(); // Len
            self.eat_whitespace();
            if self.at(SyntaxKind::Eq) {
                self.bump();
                self.eat_whitespace();
                self.parse_expr_bp(0); // record length
            }
        }
        self.finish_node();
    }

    /// `Close [#n, …]` — also handles `Reset` (close all).
    fn parse_close_stmt(&mut self) {
        self.start_node(SyntaxKind::CloseStmt);
        self.eat_trivia();
        self.bump(); // Close | Reset
        self.eat_whitespace();
        if self.at(SyntaxKind::Hash) || self.is_expr_start() {
            loop {
                self.parse_file_number();
                self.eat_whitespace();
                if !self.eat(SyntaxKind::Comma) {
                    break;
                }
                self.eat_whitespace();
            }
        }
        self.finish_node();
    }

    fn parse_print_like_stmt(&mut self, node: SyntaxKind) {
        self.start_node(node);
        self.eat_trivia();
        self.bump(); // Print | Write
        self.eat_whitespace();
        self.parse_file_number();
        self.eat_whitespace();
        self.eat(SyntaxKind::Comma);
        self.parse_print_item_list();
        self.finish_node();
    }

    fn parse_print_item_list(&mut self) {
        self.start_node(SyntaxKind::PrintItemList);
        loop {
            self.eat_whitespace();
            let kind = self.current();
            if self.is_expr_stop(kind) {
                break;
            }
            if kind == SyntaxKind::Comma || kind == SyntaxKind::Semicolon {
                self.bump();
            } else if self.is_expr_start() {
                self.start_node(SyntaxKind::PrintItem);
                self.parse_expr_bp(0); // expr, or Spc(n)/Tab(n) as a call
                self.finish_node();
            } else {
                break;
            }
        }
        self.finish_node();
    }

    fn parse_input_stmt(&mut self) {
        self.start_node(SyntaxKind::InputStmt);
        self.eat_trivia();
        self.bump(); // Input
        self.eat_whitespace();
        self.parse_file_number();
        self.eat_whitespace();
        self.eat(SyntaxKind::Comma);
        loop {
            self.eat_whitespace();
            if !self.is_expr_start() {
                break;
            }
            self.parse_lvalue_expr();
            self.eat_whitespace();
            if !self.eat(SyntaxKind::Comma) {
                break;
            }
        }
        self.finish_node();
    }

    fn parse_line_input_stmt(&mut self) {
        self.start_node(SyntaxKind::LineInputStmt);
        self.eat_trivia();
        self.bump(); // Line
        self.eat_whitespace();
        self.eat(SyntaxKind::KwInput);
        self.eat_whitespace();
        self.parse_file_number();
        self.eat_whitespace();
        self.eat(SyntaxKind::Comma);
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_lvalue_expr();
        }
        self.finish_node();
    }

    /// `Get/Put [#]n, [rec], var`.
    fn parse_get_put_stmt(&mut self, node: SyntaxKind) {
        self.start_node(node);
        self.eat_trivia();
        self.bump(); // Get | Put
        self.eat_whitespace();
        self.parse_file_number();
        self.eat_whitespace();
        self.eat(SyntaxKind::Comma);
        self.eat_whitespace();
        if !self.at(SyntaxKind::Comma) && self.is_expr_start() {
            self.parse_expr_bp(0); // record number
            self.eat_whitespace();
        }
        self.eat(SyntaxKind::Comma);
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_lvalue_expr(); // variable
        }
        self.finish_node();
    }

    /// `Seek/Width [#]n, value`.
    fn parse_seek_width_stmt(&mut self, node: SyntaxKind) {
        self.start_node(node);
        self.eat_trivia();
        self.bump(); // Seek | Width
        self.eat_whitespace();
        self.parse_file_number();
        self.eat_whitespace();
        self.eat(SyntaxKind::Comma);
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr_bp(0);
        }
        self.finish_node();
    }

    fn parse_name_stmt(&mut self) {
        self.start_node(SyntaxKind::NameStmt);
        self.eat_trivia();
        self.bump(); // Name
        self.eat_whitespace();
        if self.is_expr_start() {
            self.parse_expr_bp(0); // old (stops at `As`)
        }
        self.eat_whitespace();
        if self.at(SyntaxKind::KwAs) {
            self.bump();
            self.eat_whitespace();
            if self.is_expr_start() {
                self.parse_expr_bp(0); // new
            }
        }
        self.finish_node();
    }

    /// `Lock/Unlock #n [, range [To range]]`.
    fn parse_lock_stmt(&mut self) {
        self.start_node(SyntaxKind::LockStmt);
        self.eat_trivia();
        self.bump(); // Lock | Unlock
        self.eat_whitespace();
        self.parse_file_number();
        self.eat_whitespace();
        if self.eat(SyntaxKind::Comma) {
            self.eat_whitespace();
            self.parse_expr_bp(0);
            self.eat_whitespace();
            if self.at(SyntaxKind::KwTo) {
                self.bump();
                self.eat_whitespace();
                self.parse_expr_bp(0);
            }
        }
        self.finish_node();
    }

    fn parse_ident_file_stmt(&mut self, which: FileIoIdent) {
        match which {
            FileIoIdent::Put => self.parse_get_put_stmt(SyntaxKind::PutStmt),
            FileIoIdent::Seek => self.parse_seek_width_stmt(SyntaxKind::SeekStmt),
            FileIoIdent::Width => self.parse_seek_width_stmt(SyntaxKind::WidthStmt),
            FileIoIdent::Unlock => self.parse_lock_stmt(),
            FileIoIdent::Reset => self.parse_close_stmt(),
        }
    }

    // ── File-I/O dispatch peek helpers ──────────────────────

    /// True if the current statement-leading contextual keyword is being used as
    /// an identifier (assignment / member / call), not a statement keyword.
    fn peek_keyword_used_as_name(&self) -> bool {
        matches!(
            self.peek_next_non_trivia(),
            SyntaxKind::Eq
                | SyntaxKind::Dot
                | SyntaxKind::Bang
                | SyntaxKind::LParen
                | SyntaxKind::Colon
                | SyntaxKind::TypeSuffix
        )
    }

    /// The next non-trivia token kind after the current one.
    fn peek_next_non_trivia(&self) -> SyntaxKind {
        let mut j = self.pos + 1;
        while j < self.tokens.len() && self.tokens[j].0.is_trivia() {
            j += 1;
        }
        self.tokens
            .get(j)
            .map(|(k, _)| *k)
            .unwrap_or(SyntaxKind::Eof)
    }

    fn peek_next_non_trivia_is(&self, target: SyntaxKind) -> bool {
        self.peek_next_non_trivia() == target
    }

    /// Scan the rest of the statement (top level), returning true if `target`
    /// appears at paren depth 0 before the statement ends.
    fn peek_top_level_before_stmt_end(&self, target: SyntaxKind) -> bool {
        let mut j = self.pos + 1;
        let mut depth: i32 = 0;
        while j < self.tokens.len() {
            let k = self.tokens[j].0;
            match k {
                SyntaxKind::Newline | SyntaxKind::Comment | SyntaxKind::Colon | SyntaxKind::Eof => {
                    return false;
                }
                SyntaxKind::LParen => depth += 1,
                SyntaxKind::RParen => depth -= 1,
                _ if k == target && depth == 0 => return true,
                _ => {}
            }
            j += 1;
        }
        false
    }

    /// Classify a leading identifier (`Put`/`Seek`/`Width`/`Unlock`/`Reset`) as a
    /// file statement when the shape matches (statement-position + shape guard).
    fn ident_file_stmt_kind(&self) -> Option<FileIoIdent> {
        let text = self.tokens.get(self.pos).map(|(_, t)| *t)?;
        let which = match text.to_ascii_lowercase().as_str() {
            "put" => FileIoIdent::Put,
            "seek" => FileIoIdent::Seek,
            "width" => FileIoIdent::Width,
            "unlock" => FileIoIdent::Unlock,
            "reset" => FileIoIdent::Reset,
            _ => return None,
        };
        if self.peek_keyword_used_as_name() {
            return None;
        }
        match which {
            // `Reset` takes no operands.
            FileIoIdent::Reset => Some(which),
            _ => {
                if self.peek_top_level_before_stmt_end(SyntaxKind::Hash)
                    || self.peek_top_level_before_stmt_end(SyntaxKind::Comma)
                {
                    Some(which)
                } else {
                    None
                }
            }
        }
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
                // Statement-position calls split the callable expression from the
                // following bare argument list. Parse only the attached
                // name/member/index callee here so `Proc -1, x` is a call with a
                // negative first argument, not a binary-expression callee
                // `Proc - 1`, and whitespace-separated `Proc (x)` keeps `(x)` as
                // a ByVal-forcing argument expression.
                self.parse_statement_call_callee_expr();
            }
            // Bare argument list. A leading call-site `ByVal`/`ByRef` modifier
            // (`Inc ByVal r`) also starts the argument list even though the
            // keyword itself is not an expression start.
            self.eat_whitespace();
            if self.at_bare_arg_list_start() {
                self.parse_bare_arg_list();
            }
            self.eat_to_statement_end();
        }

        self.finish_node();
    }

    /// True when a bare (no-paren) argument list begins here: an expression, an
    /// empty leading slot (`,`), or a call-site `ByVal`/`ByRef` passing modifier.
    fn at_bare_arg_list_start(&self) -> bool {
        self.is_expr_start()
            || self.at(SyntaxKind::Comma)
            || self.at(SyntaxKind::KwByVal)
            || self.at(SyntaxKind::KwByRef)
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
    fn parse_error_diagnostic_carries_source_location() {
        let error = ParseError {
            offset: 13,
            message: "expected Identifier".to_string(),
        };
        let diagnostic =
            error.to_diagnostic_with_context(Some("Main"), Some("Sub Main()\n  Dim\nEnd Sub\n"));
        assert_eq!(diagnostic.code.as_str(), "SYN-E-PARSE");
        let source = diagnostic
            .source
            .as_ref()
            .expect("parse diagnostic should carry source");
        assert_eq!(source.module.as_deref(), Some("Main"));
        assert_eq!(source.line, Some(2));
        assert_eq!(source.column, Some(3));
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
    fn round_trip_function_type_suffix_with_return_type() {
        let src = "Function alpha%() As Object\nalpha = Nothing\nEnd Function\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "{:?}", p.errors());
        let func = p
            .syntax()
            .child_nodes()
            .into_iter()
            .find(|n| n.kind() == SyntaxKind::FunctionDecl)
            .expect("expected FunctionDecl");
        assert!(func.return_type().is_some(), "expected return type");
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

    // ── Fully-structured CST: declarations & statements ─────

    /// Parse, assert lossless round-trip + no errors, return the tree's root text.
    fn parse_ok(src: &str) -> Parse {
        let p = parse(src);
        assert_eq!(p.syntax().text(), src, "round-trip");
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        p
    }

    fn in_sub(body: &str) -> String {
        format!("Sub T()\n{body}\nEnd Sub\n")
    }

    #[test]
    fn structures_dim_declarators() {
        let p = parse_ok(&in_sub(
            "Dim a As Long, b(1 To 10) As String, c As New Widget",
        ));
        assert_eq!(
            collect_nodes(&p.syntax(), SyntaxKind::VarDeclarator).len(),
            3
        );
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArrayBounds));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::Bound));
    }

    #[test]
    fn structures_withevents_and_fixed_string_dim() {
        let p = parse_ok(&in_sub(
            "Dim WithEvents src As Widget\nDim buf As String * 20",
        ));
        assert_eq!(
            collect_nodes(&p.syntax(), SyntaxKind::VarDeclarator).len(),
            2
        );
    }

    #[test]
    fn structures_const_declarators() {
        let p = parse_ok("Public Const PI As Double = 3.14, N = 5\n");
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ConstStmt));
        assert_eq!(
            collect_nodes(&p.syntax(), SyntaxKind::VarDeclarator).len(),
            2
        );
    }

    #[test]
    fn structures_declare() {
        let src = "Private Declare PtrSafe Function MB Lib \"user32\" Alias \"MessageBoxA\" (ByVal h As LongPtr) As Long\n";
        let p = parse_ok(src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::DeclareStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::LibClause));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::AliasClause));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ParamList));
    }

    #[test]
    fn structures_declare_function_type_suffix() {
        let src = "Private Declare PtrSafe Function Tick& Lib \"kernel32\" ()\n";
        let p = parse_ok(src);
        assert_eq!(p.syntax().text(), src);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::DeclareStmt));
    }

    #[test]
    fn structures_declare_continued_parameter_list_after_alias() {
        let src = "Private Declare PtrSafe Sub CopyMemory Lib \"kernel32\" Alias \"RtlMoveMemory\" _\n    (ByVal dest As LongPtr, ByVal src As LongPtr, ByVal size As Long)\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::DeclareStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ParamList));
    }

    #[test]
    fn structures_sub_continued_parameter_list_after_open_paren() {
        let src = "Public Sub RegisterConverter( _\n    Name As String, MediaType As String, Optional Instance As Object)\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::Param).len(), 3);
    }

    #[test]
    fn structures_keyword_identifiers_in_declaration_name_contexts() {
        let src = concat!(
            "Public Property Get Name() As String\n",
            "    Name = \"ok\"\n",
            "End Property\n",
            "Public Sub UseKeywords(Name As String, Print As Long)\n",
            "End Sub\n",
            "Type Rec\n",
            "    Name As String * 32\n",
            "    Print As Long\n",
            "End Type\n",
            "Public Enum Options\n",
            "    Name = 1\n",
            "    Print = 2\n",
            "End Enum\n",
        );
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::PropertyDecl));
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::Param).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::TypeField).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::EnumMember).len(), 2);
    }

    #[test]
    fn structures_type_block_fields() {
        let src = "Type Rec\n    Id As Long\n    Name As String * 32\n    Tags(1 To 3) As Integer\nEnd Type\n";
        let p = parse_ok(src);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::TypeField).len(), 3);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArrayBounds));
    }

    #[test]
    fn structures_enum_members() {
        let src = "Public Enum Color\n    Red\n    Green = 5\n    Blue\nEnd Enum\n";
        let p = parse_ok(src);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::EnumMember).len(), 3);
    }

    #[test]
    fn structures_event_decl() {
        let p = parse_ok("Public Event Changed(ByVal n As Long)\n");
        assert!(has_node_kind(&p.syntax(), SyntaxKind::EventDecl));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ParamList));

        let p = parse_ok("Public Event [Exit]()\n");
        assert!(has_node_kind(&p.syntax(), SyntaxKind::EventDecl));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ParamList));
    }

    #[test]
    fn structures_option_and_attribute_and_implements() {
        assert!(has_node_kind(
            &parse_ok("Option Base 1\n").syntax(),
            SyntaxKind::OptionStmt
        ));
        assert!(has_node_kind(
            &parse_ok("Option Compare Text\n").syntax(),
            SyntaxKind::OptionStmt
        ));
        let attr = parse_ok("Attribute VB_Name = \"Mod1\"\n");
        assert!(has_node_kind(&attr.syntax(), SyntaxKind::AttributeStmt));
        assert!(has_node_kind(&attr.syntax(), SyntaxKind::LiteralExpr));
        let impl_ = parse_ok("Implements IShape\n");
        assert!(has_node_kind(&impl_.syntax(), SyntaxKind::ImplementsStmt));
        assert!(has_node_kind(&impl_.syntax(), SyntaxKind::TypeRef));
    }

    #[test]
    fn structures_error_control_and_jumps() {
        let p = parse_ok(&in_sub(
            "On Error GoTo Handler\nResume Next\nExit Sub\nHandler:\nResume\nGoTo Done\nDone:",
        ));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::OnErrorStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ResumeStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ExitStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::GoToStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::LabelRef));
        // On Error GoTo 0 keeps the `0` as a LabelRef too.
        let z = parse_ok(&in_sub("On Error GoTo 0"));
        assert!(has_node_kind(&z.syntax(), SyntaxKind::LabelRef));
    }

    #[test]
    fn error_keyword_can_be_function_expression() {
        let p = parse_ok(&in_sub(
            "s = Error(11)\nt = \"[\" & Error$() & \"]\"\nError 5",
        ));
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::AssignStmt).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::IndexExpr).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::ErrorStmt).len(), 1);
    }

    #[test]
    fn structures_erase_and_param_default() {
        let e = parse_ok(&in_sub("Erase a, b"));
        assert!(has_node_kind(&e.syntax(), SyntaxKind::EraseStmt));
        let d = parse_ok("Sub S(Optional x As Long = 5)\nEnd Sub\n");
        assert!(has_node_kind(&d.syntax(), SyntaxKind::ParamDefault));
        assert!(has_node_kind(&d.syntax(), SyntaxKind::LiteralExpr));
    }

    #[test]
    fn structures_file_open_close() {
        let p = parse_ok(&in_sub(
            "Open \"data.txt\" For Input As #1\nClose #1\nReset",
        ));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::OpenStmt));
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::CloseStmt).len(), 2); // Close + Reset
        assert!(has_node_kind(&p.syntax(), SyntaxKind::FileNumber));
    }

    #[test]
    fn structures_print_and_write() {
        let p = parse_ok(&in_sub("Print #1, x; y\nWrite #2, a, b"));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::PrintStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::WriteStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::PrintItemList));
        assert!(collect_nodes(&p.syntax(), SyntaxKind::PrintItem).len() >= 4);
        // `Print x` (no #) stays a console call statement.
        let console = parse_ok(&in_sub("Print x"));
        assert!(!has_node_kind(&console.syntax(), SyntaxKind::PrintStmt));
        assert!(has_node_kind(&console.syntax(), SyntaxKind::CallStmt));
    }

    #[test]
    fn structures_input_get_put_seek_width_name_lock() {
        let p = parse_ok(&in_sub(
            "Input #1, x, y\nLine Input #1, s\nGet #1, , rec\nPut #1, 5, rec\nSeek #1, 10\nWidth #1, 80\nName \"a\" As \"b\"\nLock #1, 1 To 5\nUnlock #1",
        ));
        for kind in [
            SyntaxKind::InputStmt,
            SyntaxKind::LineInputStmt,
            SyntaxKind::GetStmt,
            SyntaxKind::PutStmt,
            SyntaxKind::SeekStmt,
            SyntaxKind::WidthStmt,
            SyntaxKind::NameStmt,
        ] {
            assert!(has_node_kind(&p.syntax(), kind), "expected {kind:?}");
        }
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::LockStmt).len(), 2); // Lock + Unlock
    }

    #[test]
    fn file_io_contextual_keywords_disambiguate() {
        // Statement form → file statement.
        assert!(has_node_kind(
            &parse_ok(&in_sub("Put #1, , d")).syntax(),
            SyntaxKind::PutStmt
        ));
        assert!(has_node_kind(
            &parse_ok(&in_sub("Width #1, 80")).syntax(),
            SyntaxKind::WidthStmt
        ));
        // Identifier form → assignment / expression, NOT a file statement.
        let assign = parse_ok(&in_sub("Width = 80"));
        assert!(!has_node_kind(&assign.syntax(), SyntaxKind::WidthStmt));
        assert!(has_node_kind(&assign.syntax(), SyntaxKind::AssignStmt));
        let call = parse_ok(&in_sub("x = Seek(f)"));
        assert!(!has_node_kind(&call.syntax(), SyntaxKind::SeekStmt));
        assert!(has_node_kind(&call.syntax(), SyntaxKind::AssignStmt));
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
    fn lset_rset_statements_are_structured_assignments() {
        let p = parse_ok(&in_sub("LSet s = \"ab\": RSet t = \"cd\""));
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::LSetStmt).len(), 1);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::RSetStmt).len(), 1);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::AssignStmt).len(), 0);
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
    fn round_trip_inline_statement_separators_allow_empty_segments() {
        let src = "Sub T()\n    : x = 1:: Set cache = New Dictionary: Exit Sub:\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::AssignStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::SetStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ExitStmt));
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
    fn parses_identifier_and_numeric_labels() {
        let src = "Sub T()\nGoTo done\ndone:\nGoTo 100\n100:\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::LabelStmt).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::GoToStmt).len(), 2);
    }

    #[test]
    fn parses_colonless_line_number_labels_with_statement_body() {
        let src = "Sub T()\nGoTo 200\n100 x = 1\n200 x = 5\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::LabelStmt).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::AssignStmt).len(), 2);
    }

    #[test]
    fn parses_contextual_keyword_labels() {
        let src = "Sub T()\nGoTo Name\nName:\nGoTo Print\nPrint:\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::LabelStmt).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::GoToStmt).len(), 2);
    }

    #[test]
    fn parses_keyword_named_arguments() {
        let src = "Sub T()\n    Configure Name:=\"api\", Print:=1\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArgList));
    }

    #[test]
    fn parses_raise_event_arguments() {
        let src = "Sub Main()\nRaiseEvent Tick(1)\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::RaiseEventStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArgList));
    }

    #[test]
    fn parses_redim_runtime_bound_expression() {
        let src = "Sub Main()\nReDim Preserve buf(length - 1)\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ReDimStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArrayBounds));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::Bound));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::BinaryExpr));
    }

    #[test]
    fn parses_redim_explicit_lower_bound_expression() {
        let src = "Sub Main()\nReDim buf(1 To length - 1, 0 To cols - 1)\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ReDimStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArrayBounds));
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::Bound).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr).len(), 2);
    }

    #[test]
    fn parses_redim_member_target_bounds() {
        let src = "Sub Main()\nReDim r.Scores(2)\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let redim = collect_nodes(&p.syntax(), SyntaxKind::ReDimStmt);
        assert_eq!(redim.len(), 1);
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ArrayBounds));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::LiteralExpr));
    }

    #[test]
    fn round_trip_with_stmt() {
        let src = "Sub Test()\nWith obj\n    .Value = 1\nEnd With\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
    }

    #[test]
    fn parses_type_block_field_declaration_shapes_losslessly() {
        let src = "Type Inner\nX As Long\nEnd Type\nType Record\nName As String * 5\nScores(1 To 2) As Long\nInner As Inner\nEnd Type\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::TypeBlock).len(), 2);
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
    fn single_line_if_accepts_colon_after_then() {
        let src = "Sub T()\nIf Len(s) > 0 Then: s = s & \"&\"\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IfStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::AssignStmt));
    }

    #[test]
    fn single_line_if_accepts_same_line_else_and_empty_segments() {
        let src = "Sub T()\nIf ready Then:: On Error Resume Next: Set cache = New Dictionary Else: Exit Sub\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IfStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseClause));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::OnErrorStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::SetStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ExitStmt));
    }

    #[test]
    fn nested_single_line_if_with_else_stays_inside_multiline_branch() {
        let src = "Sub T()\nIf outer Then\n    If inner Then: a = 1 Else: a = 2\nElse\n    a = 3\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::IfStmt).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::ElseClause).len(), 2);
    }

    #[test]
    fn multiline_if_accepts_inline_statements_after_elseif_and_else_colons() {
        let src = "Sub T()\nIf a Then\n    x = 1\nElseIf b Then: x = 2: y = 3\nElse: x = 4\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseIfClause));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseClause));
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::AssignStmt).len(), 4);
    }

    #[test]
    fn multiline_if_accepts_call_condition_with_else() {
        let src = "Sub T()\nIf InStr(parts(1), \"Z\") Then\n    value = 1\nElse\n    value = 2\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IfStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseClause));
    }

    #[test]
    fn multiline_if_branch_accepts_nested_call_assignment_before_else() {
        let src = "Sub T()\nIf InStr(parts(1), \"Z\") Then\n    timeParts = Split(Replace(parts(1), \"Z\", \"\"), \":\")\nElse\n    value = 2\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IfStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseClause));
    }

    #[test]
    fn multiline_if_branch_accepts_qualified_nested_call_assignment_before_else() {
        let src = "Sub T()\nIf VBA.InStr(parts(1), \"Z\") Then\n    timeParts = VBA.Split(VBA.Replace(parts(1), \"Z\", \"\"), \":\")\nElse\n    value = 2\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::IfStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseClause));
    }

    #[test]
    fn single_line_if_before_enclosing_else_does_not_steal_else() {
        let src = "Sub T()\nIf offsetIndex > 0 Then\n    Select Case UBound(offsetParts)\n    Case 1\n        offsetValue = TimeSerial(CInt(offsetParts(0)), CInt(offsetParts(1)), 0)\n    Case 2\n        offsetValue = TimeSerial(CInt(offsetParts(0)), CInt(offsetParts(1)), Int(Val(offsetParts(2))))\n    End Select\n\n    If negativeOffset Then: offsetValue = -offsetValue\nElse\n    timeParts = Split(parts(1), \":\")\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert!(has_node_kind(&p.syntax(), SyntaxKind::SelectStmt));
        assert!(has_node_kind(&p.syntax(), SyntaxKind::ElseClause));
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
    fn expr_typeof_is_project_qualified_type() {
        // A project-qualified type after `Is` — `Lib` is a keyword only inside
        // `Declare … Lib "…"`, so it must parse as an ordinary identifier here.
        let src = "Sub T()\nIf TypeOf obj Is Lib.IShape Then\n    x = 1\nEnd If\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        let binaries = collect_nodes(&p.syntax(), SyntaxKind::BinaryExpr);
        assert!(
            binaries
                .iter()
                .any(|s| s.contains("TypeOf obj Is Lib.IShape")),
            "expected a TypeOf … Is with a dotted type, got {:?}",
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
    fn parses_multivariable_next_as_nested_loop_closures() {
        let src = "Sub T()\nFor i = 1 To 2\nFor j = 1 To 3\nx = x + 1\nNext j, i\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(p.errors().is_empty(), "unexpected errors: {:?}", p.errors());
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::ForStmt).len(), 2);
        assert_eq!(collect_nodes(&p.syntax(), SyntaxKind::AssignStmt).len(), 1);
    }

    #[test]
    fn multivariable_next_rejects_more_names_than_open_loops() {
        let src = "Sub T()\nFor i = 1 To 2\nFor j = 1 To 3\nNext j, i, k\nEnd Sub\n";
        let p = parse(src);
        assert_eq!(p.syntax().text(), src);
        assert!(
            p.errors()
                .iter()
                .any(|err| err.message.contains("without matching `For`")),
            "unexpected errors: {:?}",
            p.errors()
        );
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
