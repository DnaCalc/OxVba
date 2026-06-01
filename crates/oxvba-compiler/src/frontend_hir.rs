use thiserror::Error;

use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::frontend_symbols::{
    FrontendSourceSpan, ScopeId, ScopeKind, SymbolId, SymbolModel, SymbolModelError,
    SymbolNamespace, build_symbol_model_from_source,
};

#[derive(Debug, Clone)]
pub struct BoundHirModule {
    pub symbols: SymbolModel,
    pub arenas: HirArenas,
    pub declarations: Vec<HirDeclId>,
}

#[derive(Debug, Error)]
pub enum HirBuildError {
    #[error(transparent)]
    Symbols(#[from] SymbolModelError),
    #[error("syntax parse failed: {0}")]
    Syntax(String),
    #[error("unsupported HIR source shape: {0}")]
    Unsupported(String),
    #[error("unresolved HIR symbol `{name}` in scope {scope:?}")]
    UnresolvedName { name: String, scope: ScopeId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirExprId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirStmtId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirDeclId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirCallId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirMemberId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirPropertyId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HirTypeId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CstBackpointer {
    pub syntax_kind: String,
    pub span: FrontendSourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirExpr {
    pub cst: CstBackpointer,
    pub kind: HirExprKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirExprKind {
    Missing,
    Literal(HirLiteral),
    Name(SymbolId),
    Unary {
        op: HirUnaryOp,
        expr: HirExprId,
    },
    Binary {
        op: HirBinaryOp,
        lhs: HirExprId,
        rhs: HirExprId,
    },
    Call(HirCallId),
    Member(HirMemberId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirLiteral {
    Empty,
    Null,
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirStmt {
    pub cst: CstBackpointer,
    pub kind: HirStmtKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirStmtKind {
    Empty,
    Let {
        target: HirExprId,
        value: HirExprId,
    },
    Set {
        target: HirExprId,
        value: HirExprId,
    },
    Expr(HirExprId),
    If {
        condition: HirExprId,
        then_body: Vec<HirStmtId>,
        else_body: Vec<HirStmtId>,
    },
    Block(Vec<HirStmtId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirDecl {
    pub cst: CstBackpointer,
    pub symbol: SymbolId,
    pub kind: HirDeclKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirDeclKind {
    Module,
    Procedure {
        params: Vec<SymbolId>,
        return_type: Option<HirTypeId>,
        body: Vec<HirStmtId>,
    },
    Local {
        ty: Option<HirTypeId>,
    },
    Field {
        ty: Option<HirTypeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirCall {
    pub cst: CstBackpointer,
    pub target: HirExprId,
    pub args: Vec<HirExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirMember {
    pub cst: CstBackpointer,
    pub receiver: Option<HirExprId>,
    pub symbol: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirProperty {
    pub cst: CstBackpointer,
    pub symbol: SymbolId,
    pub kind: HirPropertyKind,
    pub value_type: Option<HirTypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirPropertyKind {
    Get,
    Let,
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirType {
    pub cst: CstBackpointer,
    pub kind: HirTypeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirTypeKind {
    UnresolvedName(SymbolId),
    Builtin(HirBuiltinType),
    Object(SymbolId),
    Array { element: HirTypeId, rank: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBuiltinType {
    Boolean,
    Integer,
    Long,
    Double,
    String,
    Variant,
    Object,
}

#[derive(Debug, Clone, Default)]
pub struct HirArenas {
    exprs: Vec<HirExpr>,
    stmts: Vec<HirStmt>,
    decls: Vec<HirDecl>,
    calls: Vec<HirCall>,
    members: Vec<HirMember>,
    properties: Vec<HirProperty>,
    types: Vec<HirType>,
}

impl HirArenas {
    pub fn alloc_expr(&mut self, expr: HirExpr) -> HirExprId {
        let id = HirExprId(self.exprs.len());
        self.exprs.push(expr);
        id
    }

    pub fn expr(&self, id: HirExprId) -> Option<&HirExpr> {
        self.exprs.get(id.0)
    }

    pub fn alloc_stmt(&mut self, stmt: HirStmt) -> HirStmtId {
        let id = HirStmtId(self.stmts.len());
        self.stmts.push(stmt);
        id
    }

    pub fn stmt(&self, id: HirStmtId) -> Option<&HirStmt> {
        self.stmts.get(id.0)
    }

    pub fn alloc_decl(&mut self, decl: HirDecl) -> HirDeclId {
        let id = HirDeclId(self.decls.len());
        self.decls.push(decl);
        id
    }

    pub fn decl(&self, id: HirDeclId) -> Option<&HirDecl> {
        self.decls.get(id.0)
    }

    pub fn alloc_call(&mut self, call: HirCall) -> HirCallId {
        let id = HirCallId(self.calls.len());
        self.calls.push(call);
        id
    }

    pub fn call(&self, id: HirCallId) -> Option<&HirCall> {
        self.calls.get(id.0)
    }

    pub fn alloc_member(&mut self, member: HirMember) -> HirMemberId {
        let id = HirMemberId(self.members.len());
        self.members.push(member);
        id
    }

    pub fn member(&self, id: HirMemberId) -> Option<&HirMember> {
        self.members.get(id.0)
    }

    pub fn alloc_property(&mut self, property: HirProperty) -> HirPropertyId {
        let id = HirPropertyId(self.properties.len());
        self.properties.push(property);
        id
    }

    pub fn property(&self, id: HirPropertyId) -> Option<&HirProperty> {
        self.properties.get(id.0)
    }

    pub fn alloc_type(&mut self, ty: HirType) -> HirTypeId {
        let id = HirTypeId(self.types.len());
        self.types.push(ty);
        id
    }

    pub fn ty(&self, id: HirTypeId) -> Option<&HirType> {
        self.types.get(id.0)
    }
}

pub fn build_hir_from_source(
    module_name: &str,
    source: &str,
) -> Result<BoundHirModule, HirBuildError> {
    let symbols = build_symbol_model_from_source(module_name, source)?;
    let parsed = oxvba_syntax::parse(source);
    if !parsed.errors().is_empty() {
        return Err(HirBuildError::Syntax(format!("{:?}", parsed.errors())));
    }

    let mut builder = HirBuilder {
        symbols,
        arenas: HirArenas::default(),
        declarations: Vec::new(),
    };
    let module_scope = builder
        .scope_by_kind_and_name(ScopeKind::Module, module_name)
        .unwrap_or_else(|| builder.symbols.global_scope());
    for child in parsed.syntax().child_nodes() {
        builder.collect_decl(module_scope, child)?;
    }
    Ok(BoundHirModule {
        symbols: builder.symbols,
        arenas: builder.arenas,
        declarations: builder.declarations,
    })
}

struct HirBuilder {
    symbols: SymbolModel,
    arenas: HirArenas,
    declarations: Vec<HirDeclId>,
}

impl HirBuilder {
    fn collect_decl(&mut self, scope: ScopeId, node: SyntaxNode<'_>) -> Result<(), HirBuildError> {
        match node.kind() {
            SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl => {
                let Some(name) = first_identifier_text(node) else {
                    return Ok(());
                };
                let symbol = self
                    .symbols
                    .find_in_scope(scope, SymbolNamespace::Procedure, name)?
                    .ok_or_else(|| HirBuildError::UnresolvedName {
                        name: name.to_string(),
                        scope,
                    })?;
                let procedure_scope = self
                    .scope_by_kind_and_name(ScopeKind::Procedure, name)
                    .ok_or_else(|| HirBuildError::UnresolvedName {
                        name: name.to_string(),
                        scope,
                    })?;
                let params = self.parameter_symbols(procedure_scope)?;
                let mut body = Vec::new();
                if let Some(block) = node.body_block() {
                    for stmt in block.child_nodes() {
                        if let Some(stmt) = self.collect_stmt(procedure_scope, stmt)? {
                            body.push(stmt);
                        }
                    }
                }
                let decl = self.arenas.alloc_decl(HirDecl {
                    cst: cst(node),
                    symbol,
                    kind: HirDeclKind::Procedure {
                        params,
                        return_type: None,
                        body,
                    },
                });
                self.declarations.push(decl);
            }
            SyntaxKind::DimStmt => {
                if let Some(local) = self.local_decl(scope, node)? {
                    self.declarations.push(local);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn collect_stmt(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
    ) -> Result<Option<HirStmtId>, HirBuildError> {
        match node.kind() {
            SyntaxKind::DimStmt => {
                if let Some(decl) = self.local_decl(scope, node)? {
                    self.declarations.push(decl);
                }
                Ok(None)
            }
            SyntaxKind::AssignStmt | SyntaxKind::LetStmt => {
                let exprs = expression_children(node);
                if exprs.len() < 2 {
                    return Err(HirBuildError::Unsupported(format!(
                        "assignment without target and value: `{}`",
                        node.text().trim()
                    )));
                }
                let target = self.lower_expr(scope, exprs[0])?;
                let value = self.lower_expr(scope, exprs[1])?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::Let { target, value },
                })))
            }
            SyntaxKind::SetStmt => {
                let exprs = expression_children(node);
                if exprs.len() < 2 {
                    return Err(HirBuildError::Unsupported(format!(
                        "set statement without target and value: `{}`",
                        node.text().trim()
                    )));
                }
                let target = self.lower_expr(scope, exprs[0])?;
                let value = self.lower_expr(scope, exprs[1])?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::Set { target, value },
                })))
            }
            _ => {
                let mut stmts = Vec::new();
                for child in node.child_nodes() {
                    if let Some(stmt) = self.collect_stmt(scope, child)? {
                        stmts.push(stmt);
                    }
                }
                if stmts.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(self.arenas.alloc_stmt(HirStmt {
                        cst: cst(node),
                        kind: HirStmtKind::Block(stmts),
                    })))
                }
            }
        }
    }

    fn lower_expr(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
    ) -> Result<HirExprId, HirBuildError> {
        let kind = match node.kind() {
            SyntaxKind::IdentExpr => {
                let Some(name) = first_identifier_text(node) else {
                    return Err(HirBuildError::Unsupported(format!(
                        "identifier expression without name: `{}`",
                        node.text().trim()
                    )));
                };
                HirExprKind::Name(self.resolve_name(scope, name)?)
            }
            SyntaxKind::LiteralExpr => HirExprKind::Literal(lower_literal(node)?),
            SyntaxKind::ParenExpr => {
                let inner = expression_children(node)
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        HirBuildError::Unsupported("empty parenthesized expression".to_string())
                    })?;
                return self.lower_expr(scope, inner);
            }
            SyntaxKind::BinaryExpr => {
                let exprs = expression_children(node);
                if exprs.len() < 2 {
                    return Err(HirBuildError::Unsupported(format!(
                        "binary expression without operands: `{}`",
                        node.text().trim()
                    )));
                }
                let lhs = self.lower_expr(scope, exprs[0])?;
                let rhs = self.lower_expr(scope, exprs[1])?;
                HirExprKind::Binary {
                    op: lower_binary_op(node)?,
                    lhs,
                    rhs,
                }
            }
            other => {
                return Err(HirBuildError::Unsupported(format!(
                    "unsupported expression node {other:?}: `{}`",
                    node.text().trim()
                )));
            }
        };
        Ok(self.arenas.alloc_expr(HirExpr {
            cst: cst(node),
            kind,
        }))
    }

    fn local_decl(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
    ) -> Result<Option<HirDeclId>, HirBuildError> {
        let Some(name) = first_identifier_text(node) else {
            return Ok(None);
        };
        let symbol = self
            .symbols
            .find_in_scope(scope, SymbolNamespace::Local, name)?
            .ok_or_else(|| HirBuildError::UnresolvedName {
                name: name.to_string(),
                scope,
            })?;
        Ok(Some(self.arenas.alloc_decl(HirDecl {
            cst: cst(node),
            symbol,
            kind: HirDeclKind::Local { ty: None },
        })))
    }

    fn parameter_symbols(&self, scope: ScopeId) -> Result<Vec<SymbolId>, HirBuildError> {
        Ok(self
            .symbols
            .symbols_in_scope(scope)?
            .into_iter()
            .filter(|symbol| {
                self.symbols
                    .symbol(*symbol)
                    .is_some_and(|symbol| symbol.namespace == SymbolNamespace::Parameter)
            })
            .collect())
    }

    fn resolve_name(&self, scope: ScopeId, name: &str) -> Result<SymbolId, HirBuildError> {
        for namespace in [
            SymbolNamespace::Local,
            SymbolNamespace::Parameter,
            SymbolNamespace::Procedure,
        ] {
            if let Some(symbol) = self
                .symbols
                .resolve_in_scope_chain(scope, namespace, name)?
            {
                return Ok(symbol);
            }
        }
        Err(HirBuildError::UnresolvedName {
            name: name.to_string(),
            scope,
        })
    }

    fn scope_by_kind_and_name(&self, kind: ScopeKind, name: &str) -> Option<ScopeId> {
        let wanted = self.symbols.lookup_name(name)?;
        self.symbols
            .scopes()
            .iter()
            .find(|scope| scope.kind == kind && scope.name == Some(wanted))
            .map(|scope| scope.id)
    }
}

fn cst(node: SyntaxNode<'_>) -> CstBackpointer {
    let (start, end) = node.text_range();
    CstBackpointer {
        syntax_kind: format!("{:?}", node.kind()),
        span: FrontendSourceSpan {
            start: start as usize,
            end: end as usize,
        },
    }
}

fn expression_children(node: SyntaxNode<'_>) -> Vec<SyntaxNode<'_>> {
    node.child_nodes()
        .into_iter()
        .filter(|child| {
            matches!(
                child.kind(),
                SyntaxKind::BinaryExpr
                    | SyntaxKind::UnaryExpr
                    | SyntaxKind::IdentExpr
                    | SyntaxKind::LiteralExpr
                    | SyntaxKind::ParenExpr
                    | SyntaxKind::NewExpr
                    | SyntaxKind::MemberExpr
                    | SyntaxKind::IndexExpr
                    | SyntaxKind::CallExpr
            )
        })
        .collect()
}

fn first_identifier_text(node: SyntaxNode<'_>) -> Option<&str> {
    node.child_tokens()
        .into_iter()
        .find(|token| token.kind == SyntaxKind::Ident || token.kind == SyntaxKind::BracketedIdent)
        .map(|token| {
            token
                .text
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .unwrap_or(token.text)
        })
}

fn lower_literal(node: SyntaxNode<'_>) -> Result<HirLiteral, HirBuildError> {
    let Some(token) = node
        .child_tokens()
        .into_iter()
        .find(|token| !token.kind.is_trivia())
    else {
        return Err(HirBuildError::Unsupported(
            "literal without token".to_string(),
        ));
    };
    match token.kind {
        SyntaxKind::IntLiteral => token
            .text
            .trim_end_matches(['%', '&', '^'])
            .parse::<i64>()
            .map(HirLiteral::Int)
            .map_err(|_| HirBuildError::Unsupported(format!("unsupported int `{}`", token.text))),
        SyntaxKind::StringLiteral => Ok(HirLiteral::String(
            token.text[1..token.text.len() - 1].replace("\"\"", "\""),
        )),
        SyntaxKind::KwTrue => Ok(HirLiteral::Bool(true)),
        SyntaxKind::KwFalse => Ok(HirLiteral::Bool(false)),
        SyntaxKind::KwEmpty => Ok(HirLiteral::Empty),
        SyntaxKind::KwNull => Ok(HirLiteral::Null),
        _ => Err(HirBuildError::Unsupported(format!(
            "unsupported literal token {:?}",
            token.kind
        ))),
    }
}

fn lower_binary_op(node: SyntaxNode<'_>) -> Result<HirBinaryOp, HirBuildError> {
    let Some(token) = node.child_tokens().into_iter().find(|token| {
        matches!(
            token.kind,
            SyntaxKind::Plus
                | SyntaxKind::Minus
                | SyntaxKind::Star
                | SyntaxKind::Slash
                | SyntaxKind::Caret
                | SyntaxKind::Ampersand
                | SyntaxKind::Eq
                | SyntaxKind::LtGt
                | SyntaxKind::Lt
                | SyntaxKind::LtEq
                | SyntaxKind::Gt
                | SyntaxKind::GtEq
                | SyntaxKind::KwAnd
                | SyntaxKind::KwOr
        )
    }) else {
        return Err(HirBuildError::Unsupported(format!(
            "binary expression without direct operator: `{}`",
            node.text().trim()
        )));
    };
    match token.kind {
        SyntaxKind::Plus => Ok(HirBinaryOp::Add),
        SyntaxKind::Minus => Ok(HirBinaryOp::Sub),
        SyntaxKind::Star => Ok(HirBinaryOp::Mul),
        SyntaxKind::Slash => Ok(HirBinaryOp::Div),
        SyntaxKind::Caret => Ok(HirBinaryOp::Pow),
        SyntaxKind::Ampersand => Ok(HirBinaryOp::Concat),
        SyntaxKind::Eq => Ok(HirBinaryOp::Eq),
        SyntaxKind::LtGt => Ok(HirBinaryOp::Ne),
        SyntaxKind::Lt => Ok(HirBinaryOp::Lt),
        SyntaxKind::LtEq => Ok(HirBinaryOp::Le),
        SyntaxKind::Gt => Ok(HirBinaryOp::Gt),
        SyntaxKind::GtEq => Ok(HirBinaryOp::Ge),
        SyntaxKind::KwAnd => Ok(HirBinaryOp::And),
        SyntaxKind::KwOr => Ok(HirBinaryOp::Or),
        _ => unreachable!("filtered operator token"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_symbols::{ScopeKind, SourceProvenance, SymbolModel, SymbolNamespace};

    fn cst(kind: &str, start: usize, end: usize) -> CstBackpointer {
        CstBackpointer {
            syntax_kind: kind.to_string(),
            span: FrontendSourceSpan { start, end },
        }
    }

    fn provenance(start: usize, end: usize) -> SourceProvenance {
        SourceProvenance {
            module_name: Some("Module1".to_string()),
            span: Some(FrontendSourceSpan { start, end }),
        }
    }

    #[test]
    fn hir_arenas_represent_assignment_with_cst_backpointers() {
        let mut symbols = SymbolModel::default();
        let scope = symbols
            .add_scope(ScopeKind::Procedure, symbols.global_scope(), Some("Main"))
            .expect("scope");
        let value_symbol = symbols
            .declare_symbol(scope, SymbolNamespace::Local, "x", provenance(20, 21))
            .expect("local symbol");

        let mut hir = HirArenas::default();
        let target = hir.alloc_expr(HirExpr {
            cst: cst("NameExpr", 30, 31),
            kind: HirExprKind::Name(value_symbol),
        });
        let value = hir.alloc_expr(HirExpr {
            cst: cst("IntLiteral", 34, 35),
            kind: HirExprKind::Literal(HirLiteral::Int(1)),
        });
        let stmt = hir.alloc_stmt(HirStmt {
            cst: cst("AssignStmt", 30, 35),
            kind: HirStmtKind::Let { target, value },
        });

        assert_eq!(
            hir.stmt(stmt).map(|stmt| &stmt.cst),
            Some(&cst("AssignStmt", 30, 35))
        );
        assert!(matches!(
            hir.expr(target).map(|expr| &expr.kind),
            Some(HirExprKind::Name(symbol)) if *symbol == value_symbol
        ));
    }

    #[test]
    fn hir_arenas_represent_call_member_property_and_type_nodes() {
        let mut symbols = SymbolModel::default();
        let member_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Member,
                "Count",
                provenance(5, 10),
            )
            .expect("member symbol");
        let type_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Type,
                "Collection",
                provenance(11, 21),
            )
            .expect("type symbol");

        let mut hir = HirArenas::default();
        let receiver = hir.alloc_expr(HirExpr {
            cst: cst("NameExpr", 0, 4),
            kind: HirExprKind::Missing,
        });
        let member = hir.alloc_member(HirMember {
            cst: cst("MemberExpr", 0, 10),
            receiver: Some(receiver),
            symbol: member_symbol,
        });
        let member_expr = hir.alloc_expr(HirExpr {
            cst: cst("MemberExpr", 0, 10),
            kind: HirExprKind::Member(member),
        });
        let call = hir.alloc_call(HirCall {
            cst: cst("CallExpr", 0, 12),
            target: member_expr,
            args: Vec::new(),
        });
        let object_type = hir.alloc_type(HirType {
            cst: cst("TypeExpr", 11, 21),
            kind: HirTypeKind::Object(type_symbol),
        });
        let property = hir.alloc_property(HirProperty {
            cst: cst("PropertyGetDecl", 30, 50),
            symbol: member_symbol,
            kind: HirPropertyKind::Get,
            value_type: Some(object_type),
        });

        assert_eq!(hir.call(call).map(|call| call.target), Some(member_expr));
        assert_eq!(
            hir.member(member).and_then(|member| member.receiver),
            Some(receiver)
        );
        assert_eq!(
            hir.property(property)
                .and_then(|property| property.value_type),
            Some(object_type)
        );
    }

    #[test]
    fn hir_arenas_represent_procedure_declaration_body() {
        let mut symbols = SymbolModel::default();
        let proc_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Procedure,
                "Main",
                provenance(0, 8),
            )
            .expect("procedure symbol");

        let mut hir = HirArenas::default();
        let body_stmt = hir.alloc_stmt(HirStmt {
            cst: cst("EmptyStmt", 12, 12),
            kind: HirStmtKind::Empty,
        });
        let decl = hir.alloc_decl(HirDecl {
            cst: cst("SubDecl", 0, 20),
            symbol: proc_symbol,
            kind: HirDeclKind::Procedure {
                params: Vec::new(),
                return_type: None,
                body: vec![body_stmt],
            },
        });

        assert!(matches!(
            hir.decl(decl).map(|decl| &decl.kind),
            Some(HirDeclKind::Procedure { body, .. }) if body == &vec![body_stmt]
        ));
    }

    #[test]
    fn hir_builder_lowers_cst_procedure_assignment_to_symbol_backed_hir() {
        let source = "Sub Main(ByVal seed As Long)\n    Dim x As Long\n    x = seed + 1\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        assert_eq!(module.declarations.len(), 2, "{module:#?}");

        let procedure = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { params, body, .. } => Some((params, body)),
                _ => None,
            })
            .expect("procedure declaration");
        assert_eq!(procedure.0.len(), 1, "expected one parameter");
        assert!(
            procedure
                .1
                .iter()
                .any(|stmt| stmt_tree_contains_let(&module.arenas, *stmt)),
            "expected assignment statement in HIR body: {module:#?}"
        );

        let let_stmt = procedure
            .1
            .iter()
            .find_map(|stmt| find_let_stmt(&module.arenas, *stmt))
            .expect("let statement");
        assert!(matches!(
            module.arenas.expr(let_stmt.0).map(|expr| &expr.kind),
            Some(HirExprKind::Name(_))
        ));
        assert!(matches!(
            module.arenas.expr(let_stmt.1).map(|expr| &expr.kind),
            Some(HirExprKind::Binary {
                op: HirBinaryOp::Add,
                ..
            })
        ));
    }

    #[test]
    fn hir_builder_preserves_cst_backpointer_spans_from_parser() {
        let source = "Sub Main()\n    Dim total As Long\n    total = 1\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let procedure = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find(|decl| matches!(decl.kind, HirDeclKind::Procedure { .. }))
            .expect("procedure declaration");

        assert_eq!(
            procedure.cst,
            CstBackpointer {
                syntax_kind: "SubDecl".to_string(),
                span: FrontendSourceSpan {
                    start: 0,
                    end: source.trim_end_matches('\n').len()
                },
            }
        );
        let HirDeclKind::Procedure { body, .. } = &procedure.kind else {
            panic!("expected procedure");
        };
        let assignment = body
            .iter()
            .find_map(|stmt| find_let_stmt_id(&module.arenas, *stmt))
            .and_then(|stmt| module.arenas.stmt(stmt))
            .expect("assignment statement");
        assert_eq!(assignment.cst.syntax_kind, "AssignStmt");
        assert_eq!(
            source[assignment.cst.span.start..assignment.cst.span.end].trim(),
            "total = 1"
        );
    }

    fn stmt_tree_contains_let(hir: &HirArenas, stmt: HirStmtId) -> bool {
        find_let_stmt(hir, stmt).is_some()
    }

    fn find_let_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<(HirExprId, HirExprId)> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::Let { target, value }) => Some((*target, *value)),
            Some(HirStmtKind::Block(children)) => {
                children.iter().find_map(|child| find_let_stmt(hir, *child))
            }
            _ => None,
        }
    }

    fn find_let_stmt_id(hir: &HirArenas, stmt: HirStmtId) -> Option<HirStmtId> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::Let { .. }) => Some(stmt),
            Some(HirStmtKind::Block(children)) => children
                .iter()
                .find_map(|child| find_let_stmt_id(hir, *child)),
            _ => None,
        }
    }
}
