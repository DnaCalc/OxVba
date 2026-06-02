use thiserror::Error;

use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use crate::frontend_symbols::{
    FrontendSourceSpan, ScopeId, ScopeKind, SourceProvenance, SymbolId, SymbolModel,
    SymbolModelError, SymbolNamespace, build_symbol_model_from_source,
};
use crate::resolve::normalize_ident;

pub(crate) fn is_builtin_intrinsic_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "lbound"
            | "ubound"
            | "isarray"
            | "vartype"
            | "typename"
            | "isnumeric"
            | "isdate"
            | "isobject"
            | "isempty"
            | "isnull"
            | "iserror"
            | "len"
            | "left"
            | "right"
            | "mid"
            | "instr"
            | "instrrev"
            | "replace"
            | "strcomp"
            | "abs"
            | "int"
            | "fix"
            | "sgn"
            | "round"
            | "sqr"
            | "sin"
            | "cos"
            | "log"
            | "exp"
            | "atn"
            | "tan"
            | "year"
            | "month"
            | "day"
            | "weekday"
            | "monthname"
            | "datevalue"
            | "timevalue"
            | "dateserial"
            | "timeserial"
            | "dateadd"
            | "datediff"
            | "cstr"
            | "str"
            | "val"
            | "cdate"
            | "hex"
            | "oct"
            | "lcase"
            | "ucase"
            | "trim"
            | "ltrim"
            | "rtrim"
            | "space"
            | "string"
            | "chr"
            | "asc"
            | "strreverse"
            | "strconv"
            | "format"
            | "split"
            | "join"
            | "collectionadd"
            | "collectionitem"
            | "collectionremove"
            | "collectioncount"
            | "array"
            | "fv"
            | "pv"
            | "pmt"
            | "npv"
            | "irr"
            | "mirr"
            | "rate"
            | "nper"
            | "rnd"
            | "randomize"
    )
}

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
    New {
        type_name: String,
    },
    Unary {
        op: HirUnaryOp,
        expr: HirExprId,
    },
    Binary {
        op: HirBinaryOp,
        lhs: HirExprId,
        rhs: HirExprId,
    },
    TypeOfIs {
        expr: HirExprId,
        type_name: String,
    },
    Call(HirCallId),
    Member(HirMemberId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirLiteral {
    Empty,
    Null,
    Nothing,
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
    Is,
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
    DoWhile {
        condition: HirExprId,
        body: Vec<HirStmtId>,
        post_check: bool,
        until: bool,
    },
    SelectCase {
        expr: HirExprId,
        arms: Vec<(Vec<HirCaseClause>, Vec<HirStmtId>)>,
        else_body: Vec<HirStmtId>,
    },
    ForRange {
        var: SymbolId,
        start: HirExprId,
        end: HirExprId,
        step: Option<HirExprId>,
        body: Vec<HirStmtId>,
    },
    ForEach {
        var: SymbolId,
        iterable: HirExprId,
        body: Vec<HirStmtId>,
    },
    ExitDo,
    ExitFor,
    ExitProcedure,
    OnErrorResumeNext,
    OnErrorGoto0,
    OnErrorGotoLabel {
        label: String,
    },
    ResumeNext,
    Resume,
    ResumeLabel {
        label: String,
    },
    ReDim {
        name: String,
        bounds: Vec<HirArrayBound>,
        preserve: bool,
    },
    Erase {
        name: String,
    },
    RaiseEvent {
        name: String,
        args: Vec<HirExprId>,
    },
    Label {
        name: String,
    },
    GoTo {
        label: String,
    },
    GoSub {
        label: String,
    },
    Return,
    Block(Vec<HirStmtId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirCaseClause {
    Value(HirExprId),
    Range { start: HirExprId, end: HirExprId },
    Is { op: HirBinaryOp, value: HirExprId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HirArrayBound {
    pub lower: Option<HirExprId>,
    pub upper: HirExprId,
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
    pub args: Vec<HirCallArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirCallArg {
    pub name: Option<String>,
    pub expr: HirExprId,
    pub force_byval: bool,
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

    pub fn decl_mut(&mut self, id: HirDeclId) -> Option<&mut HirDecl> {
        self.decls.get_mut(id.0)
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

    pub fn properties(&self) -> &[HirProperty] {
        &self.properties
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
        with_receivers: Vec::new(),
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
    with_receivers: Vec<HirExprId>,
}

impl HirBuilder {
    fn collect_decl(&mut self, scope: ScopeId, node: SyntaxNode<'_>) -> Result<(), HirBuildError> {
        match node.kind() {
            SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl => {
                let Some(name) = first_identifier_text(node) else {
                    return Ok(());
                };
                let symbol_name = procedure_symbol_name(node, &name);
                let symbol = self
                    .symbols
                    .find_in_scope(scope, SymbolNamespace::Procedure, &symbol_name)?
                    .ok_or_else(|| HirBuildError::UnresolvedName {
                        name: symbol_name.clone(),
                        scope,
                    })?;
                let procedure_scope = self
                    .scope_by_kind_and_name(ScopeKind::Procedure, &symbol_name)
                    .ok_or_else(|| HirBuildError::UnresolvedName {
                        name: symbol_name.clone(),
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
                if node.kind() == SyntaxKind::PropertyDecl {
                    self.arenas.alloc_property(HirProperty {
                        cst: cst(node),
                        symbol,
                        kind: property_kind(node),
                        value_type: None,
                    });
                }
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
            SyntaxKind::DimStmt | SyntaxKind::ConstStmt => {
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
            SyntaxKind::CallStmt => {
                let has_call_keyword = node
                    .child_tokens()
                    .into_iter()
                    .any(|token| token.kind == SyntaxKind::KwCall);
                let call_stmt_cst = if has_call_keyword {
                    cst(node)
                } else {
                    cst_with_kind(node, "CallStmtNoCallKeyword")
                };
                let expr_node = expression_children(node)
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "call statement without expression: `{}`",
                            node.text().trim()
                        ))
                    })?;
                let direct_arg_list = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::ArgList);
                let mut expr = if !has_call_keyword
                    && expr_node.kind() == SyntaxKind::IndexExpr
                    && direct_arg_list.is_none()
                {
                    let exprs = expression_children(expr_node);
                    let Some(target_node) = exprs.first().copied() else {
                        return Err(HirBuildError::Unsupported(format!(
                            "statement call without target: `{}`",
                            node.text().trim()
                        )));
                    };
                    let target = self.lower_expr(scope, target_node)?;
                    let args = expr_node
                        .child_nodes()
                        .into_iter()
                        .find(|child| child.kind() == SyntaxKind::ArgList)
                        .map(|arg_list| self.lower_call_args(scope, arg_list, true))
                        .transpose()?
                        .unwrap_or_default();
                    let call = self.arenas.alloc_call(HirCall {
                        cst: call_stmt_cst.clone(),
                        target,
                        args,
                    });
                    self.arenas.alloc_expr(HirExpr {
                        cst: call_stmt_cst.clone(),
                        kind: HirExprKind::Call(call),
                    })
                } else {
                    self.lower_expr(scope, expr_node)?
                };
                if let Some(arg_list) = direct_arg_list {
                    let args = self.lower_call_args(scope, arg_list, false)?;
                    let call = self.arenas.alloc_call(HirCall {
                        cst: call_stmt_cst.clone(),
                        target: expr,
                        args,
                    });
                    expr = self.arenas.alloc_expr(HirExpr {
                        cst: call_stmt_cst.clone(),
                        kind: HirExprKind::Call(call),
                    });
                }
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: call_stmt_cst,
                    kind: HirStmtKind::Expr(expr),
                })))
            }
            SyntaxKind::WithStmt => {
                let receiver_node =
                    expression_children(node)
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            HirBuildError::Unsupported(format!(
                                "With statement without receiver: `{}`",
                                node.text().trim()
                            ))
                        })?;
                let receiver = self.lower_expr(scope, receiver_node)?;
                let block = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::Block)
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "With statement without body block: `{}`",
                            node.text().trim()
                        ))
                    })?;
                self.with_receivers.push(receiver);
                let body = self.collect_stmt_block(scope, block);
                self.with_receivers.pop();
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::Block(body?),
                })))
            }
            SyntaxKind::IfStmt => {
                let condition_node =
                    expression_children(node)
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            HirBuildError::Unsupported(format!(
                                "if statement without condition: `{}`",
                                node.text().trim()
                            ))
                        })?;
                let condition = self.lower_expr(scope, condition_node)?;
                let then_body = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::Block)
                    .map(|block| self.collect_stmt_block(scope, block))
                    .transpose()?
                    .unwrap_or_default();
                let else_body = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::ElseClause)
                    .and_then(|else_clause| {
                        else_clause
                            .child_nodes()
                            .into_iter()
                            .find(|child| child.kind() == SyntaxKind::Block)
                    })
                    .map(|block| self.collect_stmt_block(scope, block))
                    .transpose()?
                    .unwrap_or_default();
                let mut else_body = else_body;
                let elseif_clauses = node
                    .child_nodes()
                    .into_iter()
                    .filter(|child| child.kind() == SyntaxKind::ElseIfClause)
                    .collect::<Vec<_>>();
                for clause in elseif_clauses.into_iter().rev() {
                    let nested_if = self.lower_elseif_clause(scope, clause, else_body)?;
                    else_body = vec![nested_if];
                }
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::If {
                        condition,
                        then_body,
                        else_body,
                    },
                })))
            }
            SyntaxKind::DoStmt => {
                let block = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::Block)
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "do statement without body block: `{}`",
                            node.text().trim()
                        ))
                    })?;
                let block_start = block.text_range().0;
                let condition_token = node
                    .child_tokens()
                    .into_iter()
                    .find(|token| matches!(token.kind, SyntaxKind::KwWhile | SyntaxKind::KwUntil))
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "Do without While/Until is not yet supported by HIR lowering: `{}`",
                            node.text().trim()
                        ))
                    })?;
                let post_check = condition_token.offset > block_start;
                let until = condition_token.kind == SyntaxKind::KwUntil;
                let condition_node =
                    expression_children(node)
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            HirBuildError::Unsupported(format!(
                                "Do While statement without condition: `{}`",
                                node.text().trim()
                            ))
                        })?;
                let condition = self.lower_expr(scope, condition_node)?;
                let body = self.collect_stmt_block(scope, block)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::DoWhile {
                        condition,
                        body,
                        post_check,
                        until,
                    },
                })))
            }
            SyntaxKind::WhileStmt => {
                let condition_node =
                    expression_children(node)
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            HirBuildError::Unsupported(format!(
                                "While statement without condition: `{}`",
                                node.text().trim()
                            ))
                        })?;
                let condition = self.lower_expr(scope, condition_node)?;
                let block = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::Block)
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "While statement without body block: `{}`",
                            node.text().trim()
                        ))
                    })?;
                let body = self.collect_stmt_block(scope, block)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::DoWhile {
                        condition,
                        body,
                        post_check: false,
                        until: false,
                    },
                })))
            }
            SyntaxKind::ForStmt => {
                if node
                    .child_tokens()
                    .iter()
                    .any(|token| token.kind == SyntaxKind::KwEach)
                {
                    let var_name = first_identifier_text(node).ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "For Each statement without counter variable: `{}`",
                            node.text().trim()
                        ))
                    })?;
                    let var = self.resolve_name(scope, &var_name)?;
                    let exprs = expression_children(node);
                    let iterable_node = exprs.get(1).copied().ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "For Each statement without iterable expression: `{}`",
                            node.text().trim()
                        ))
                    })?;
                    let iterable = self.lower_expr(scope, iterable_node)?;
                    let block = node
                        .child_nodes()
                        .into_iter()
                        .find(|child| child.kind() == SyntaxKind::Block)
                        .ok_or_else(|| {
                            HirBuildError::Unsupported(format!(
                                "For Each statement without body block: `{}`",
                                node.text().trim()
                            ))
                        })?;
                    let body = self.collect_stmt_block(scope, block)?;
                    return Ok(Some(self.arenas.alloc_stmt(HirStmt {
                        cst: cst(node),
                        kind: HirStmtKind::ForEach {
                            var,
                            iterable,
                            body,
                        },
                    })));
                }
                let var_name = first_identifier_text(node).ok_or_else(|| {
                    HirBuildError::Unsupported(format!(
                        "For statement without counter variable: `{}`",
                        node.text().trim()
                    ))
                })?;
                let var = self.resolve_name(scope, &var_name)?;
                let exprs = expression_children(node);
                if exprs.len() < 2 {
                    return Err(HirBuildError::Unsupported(format!(
                        "For statement without start/end expressions: `{}`",
                        node.text().trim()
                    )));
                }
                let start = self.lower_expr(scope, exprs[0])?;
                let end = self.lower_expr(scope, exprs[1])?;
                let step = exprs
                    .get(2)
                    .map(|expr| self.lower_expr(scope, *expr))
                    .transpose()?;
                let block = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::Block)
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "For statement without body block: `{}`",
                            node.text().trim()
                        ))
                    })?;
                let body = self.collect_stmt_block(scope, block)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::ForRange {
                        var,
                        start,
                        end,
                        step,
                        body,
                    },
                })))
            }
            SyntaxKind::ExitStmt => {
                let kind = exit_stmt_kind(node)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind,
                })))
            }
            SyntaxKind::OnErrorStmt => {
                let kind = on_error_stmt_kind(node)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind,
                })))
            }
            SyntaxKind::ResumeStmt => {
                let kind = resume_stmt_kind(node)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind,
                })))
            }
            SyntaxKind::LabelStmt => {
                let name = label_name_from_stmt(node)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::Label { name },
                })))
            }
            SyntaxKind::GoToStmt => {
                let label = jump_label_from_stmt(node, SyntaxKind::KwGoTo)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::GoTo { label },
                })))
            }
            SyntaxKind::GoSubStmt => {
                let label = jump_label_from_stmt(node, SyntaxKind::KwGoSub)?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::GoSub { label },
                })))
            }
            SyntaxKind::ReturnStmt => Ok(Some(self.arenas.alloc_stmt(HirStmt {
                cst: cst(node),
                kind: HirStmtKind::Return,
            }))),
            SyntaxKind::EraseStmt => {
                let name = name_after_keyword(node, SyntaxKind::KwErase, "Erase")?;
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::Erase { name },
                })))
            }
            SyntaxKind::ReDimStmt => {
                let name = redim_name(node)?;
                let preserve = node
                    .child_tokens()
                    .into_iter()
                    .any(|token| token.kind == SyntaxKind::KwPreserve);
                let bounds = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::ArgList)
                    .map(|arg_list| self.lower_redim_bounds(scope, arg_list))
                    .transpose()?
                    .unwrap_or_default();
                if bounds.is_empty() {
                    return Err(HirBuildError::Unsupported(format!(
                        "ReDim statement without supported bounds: `{}`",
                        node.text().trim()
                    )));
                }
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::ReDim {
                        name,
                        bounds,
                        preserve,
                    },
                })))
            }
            SyntaxKind::RaiseEventStmt => {
                let name = name_after_keyword(node, SyntaxKind::KwRaiseEvent, "RaiseEvent")?;
                let args = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::ArgList)
                    .map(|arg_list| {
                        expression_children(arg_list)
                            .into_iter()
                            .map(|expr| self.lower_expr(scope, expr))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::RaiseEvent { name, args },
                })))
            }
            SyntaxKind::SelectStmt => {
                let expr = expression_children(node)
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "Select Case statement without selector: `{}`",
                            node.text().trim()
                        ))
                    })
                    .and_then(|selector| self.lower_expr(scope, selector))?;
                let mut arms = Vec::new();
                let mut else_body = Vec::new();
                for clause in node
                    .child_nodes()
                    .into_iter()
                    .filter(|child| child.kind() == SyntaxKind::CaseClause)
                {
                    let block = clause
                        .child_nodes()
                        .into_iter()
                        .find(|child| child.kind() == SyntaxKind::Block);
                    let body = block
                        .map(|block| self.collect_stmt_block(scope, block))
                        .transpose()?
                        .unwrap_or_default();
                    if clause
                        .child_tokens()
                        .iter()
                        .any(|token| token.kind == SyntaxKind::KwElse)
                    {
                        else_body = body;
                        continue;
                    }
                    let clause_header = clause
                        .text()
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if clause_header.contains(',') && clause_header.contains(" to ") {
                        return Err(HirBuildError::Unsupported(format!(
                            "mixed Select Case range lists are not yet supported by HIR lowering: `{}`",
                            clause.text().trim()
                        )));
                    }
                    let values: Vec<HirExprId> = expression_children(clause)
                        .into_iter()
                        .map(|expr| self.lower_expr(scope, expr))
                        .collect::<Result<_, _>>()?;
                    let clauses = if clause_header.trim_start().starts_with("case is ") {
                        if values.len() != 1 {
                            return Err(HirBuildError::Unsupported(format!(
                                "Case Is clause without comparison value: `{}`",
                                clause.text().trim()
                            )));
                        }
                        vec![HirCaseClause::Is {
                            op: case_is_operator(clause)?,
                            value: values[0],
                        }]
                    } else if clause_header.contains(" to ") {
                        if values.len() != 2 {
                            return Err(HirBuildError::Unsupported(format!(
                                "Case range without start/end values: `{}`",
                                clause.text().trim()
                            )));
                        }
                        vec![HirCaseClause::Range {
                            start: values[0],
                            end: values[1],
                        }]
                    } else if values.len() == 1 {
                        vec![HirCaseClause::Value(values[0])]
                    } else if clause_header.contains(',') {
                        values.into_iter().map(HirCaseClause::Value).collect()
                    } else {
                        return Err(HirBuildError::Unsupported(format!(
                            "Case clause without a single value: `{}`",
                            clause.text().trim()
                        )));
                    };
                    arms.push((clauses, body));
                }
                Ok(Some(self.arenas.alloc_stmt(HirStmt {
                    cst: cst(node),
                    kind: HirStmtKind::SelectCase {
                        expr,
                        arms,
                        else_body,
                    },
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

    fn lower_elseif_clause(
        &mut self,
        scope: ScopeId,
        clause: SyntaxNode<'_>,
        else_body: Vec<HirStmtId>,
    ) -> Result<HirStmtId, HirBuildError> {
        let condition_node = expression_children(clause)
            .into_iter()
            .next()
            .ok_or_else(|| {
                HirBuildError::Unsupported(format!(
                    "ElseIf clause without condition: `{}`",
                    clause.text().trim()
                ))
            })?;
        let condition = self.lower_expr(scope, condition_node)?;
        let then_body = clause
            .child_nodes()
            .into_iter()
            .find(|child| child.kind() == SyntaxKind::Block)
            .map(|block| self.collect_stmt_block(scope, block))
            .transpose()?
            .unwrap_or_default();
        Ok(self.arenas.alloc_stmt(HirStmt {
            cst: cst(clause),
            kind: HirStmtKind::If {
                condition,
                then_body,
                else_body,
            },
        }))
    }

    fn collect_stmt_block(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
    ) -> Result<Vec<HirStmtId>, HirBuildError> {
        let mut stmts = Vec::new();
        for child in node.child_nodes() {
            if let Some(stmt) = self.collect_stmt(scope, child)? {
                stmts.push(stmt);
            }
        }
        Ok(stmts)
    }

    fn lower_call_args(
        &mut self,
        scope: ScopeId,
        arg_list: SyntaxNode<'_>,
        force_byval_all: bool,
    ) -> Result<Vec<HirCallArg>, HirBuildError> {
        let mut out = Vec::new();
        let mut candidate_name: Option<String> = None;
        let mut pending_name: Option<String> = None;
        for element in arg_list.children() {
            match element {
                SyntaxElement::Token(token) if token.kind.is_trivia() => {}
                SyntaxElement::Token(token)
                    if token.kind == SyntaxKind::Ident || token.kind.is_keyword() =>
                {
                    candidate_name = Some(
                        normalize_ident(
                            token
                                .text
                                .strip_prefix('[')
                                .and_then(|value| value.strip_suffix(']'))
                                .unwrap_or(token.text),
                        )
                        .unwrap_or_else(|| token.text.to_string()),
                    );
                }
                SyntaxElement::Token(token) if token.kind == SyntaxKind::ColonEq => {
                    pending_name = candidate_name.take();
                }
                SyntaxElement::Token(token) if token.kind == SyntaxKind::Comma => {
                    candidate_name = None;
                    pending_name = None;
                }
                SyntaxElement::Node(arg) if is_expression_node(arg.kind()) => {
                    let force_byval = force_byval_all || arg.kind() == SyntaxKind::ParenExpr;
                    out.push(HirCallArg {
                        name: pending_name.take(),
                        expr: self.lower_expr(scope, arg)?,
                        force_byval,
                    });
                    candidate_name = None;
                }
                _ => {}
            }
        }
        Ok(out)
    }

    fn lower_redim_bounds(
        &mut self,
        scope: ScopeId,
        arg_list: SyntaxNode<'_>,
    ) -> Result<Vec<HirArrayBound>, HirBuildError> {
        let mut bounds = Vec::new();
        let mut pending_lower: Option<HirExprId> = None;
        let mut pending_upper: Option<HirExprId> = None;
        let mut saw_to = false;

        for element in arg_list.children() {
            match element {
                SyntaxElement::Node(child) if is_expression_node(child.kind()) => {
                    let expr = self.lower_expr(scope, child)?;
                    if saw_to {
                        bounds.push(HirArrayBound {
                            lower: pending_lower.take(),
                            upper: expr,
                        });
                        saw_to = false;
                    } else if pending_upper.is_none() {
                        pending_upper = Some(expr);
                    } else {
                        return Err(HirBuildError::Unsupported(format!(
                            "ReDim bound contains adjacent expressions: `{}`",
                            arg_list.text().trim()
                        )));
                    }
                }
                SyntaxElement::Token(token) if token.kind == SyntaxKind::KwTo => {
                    if saw_to || pending_upper.is_none() {
                        return Err(HirBuildError::Unsupported(format!(
                            "ReDim bound contains unsupported `To`: `{}`",
                            arg_list.text().trim()
                        )));
                    }
                    pending_lower = pending_upper.take();
                    saw_to = true;
                }
                SyntaxElement::Token(token) if token.kind == SyntaxKind::Comma => {
                    if saw_to {
                        return Err(HirBuildError::Unsupported(format!(
                            "ReDim bound missing upper expression: `{}`",
                            arg_list.text().trim()
                        )));
                    }
                    if let Some(upper) = pending_upper.take() {
                        bounds.push(HirArrayBound { lower: None, upper });
                    }
                    pending_lower = None;
                }
                _ => {}
            }
        }

        if saw_to {
            return Err(HirBuildError::Unsupported(format!(
                "ReDim bound missing upper expression: `{}`",
                arg_list.text().trim()
            )));
        }
        if let Some(upper) = pending_upper {
            bounds.push(HirArrayBound { lower: None, upper });
        }

        Ok(bounds)
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
                HirExprKind::Name(self.resolve_name(scope, &name)?)
            }
            SyntaxKind::LiteralExpr => HirExprKind::Literal(lower_literal(node)?),
            SyntaxKind::NewExpr => HirExprKind::New {
                type_name: new_expr_type_name(node).ok_or_else(|| {
                    HirBuildError::Unsupported(format!(
                        "New expression without type name: `{}`",
                        node.text().trim()
                    ))
                })?,
            },
            SyntaxKind::ParenExpr => {
                let inner = expression_children(node)
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        HirBuildError::Unsupported("empty parenthesized expression".to_string())
                    })?;
                return self.lower_expr(scope, inner);
            }
            SyntaxKind::UnaryExpr => {
                let exprs = expression_children(node);
                if exprs.len() != 1 {
                    return Err(HirBuildError::Unsupported(format!(
                        "unary expression without one operand: `{}`",
                        node.text().trim()
                    )));
                }
                let Some(token) = node.child_tokens().into_iter().find(|token| {
                    matches!(
                        token.kind,
                        SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::KwNot
                    )
                }) else {
                    return Err(HirBuildError::Unsupported(format!(
                        "unary expression without direct operator: `{}`",
                        node.text().trim()
                    )));
                };
                if token.kind == SyntaxKind::Plus {
                    return self.lower_expr(scope, exprs[0]);
                }
                HirExprKind::Unary {
                    op: lower_unary_op(token.kind),
                    expr: self.lower_expr(scope, exprs[0])?,
                }
            }
            SyntaxKind::BinaryExpr => {
                let exprs = expression_children(node);
                if exprs.len() < 2 {
                    return Err(HirBuildError::Unsupported(format!(
                        "binary expression without operands: `{}`",
                        node.text().trim()
                    )));
                }
                if is_typeof_is_expr(node) {
                    let expr = self.lower_expr(scope, exprs[0])?;
                    let type_name = exprs[1].text().trim().to_string();
                    return Ok(self.arenas.alloc_expr(HirExpr {
                        cst: cst(node),
                        kind: HirExprKind::TypeOfIs { expr, type_name },
                    }));
                }
                let lhs = self.lower_expr(scope, exprs[0])?;
                let rhs = self.lower_expr(scope, exprs[1])?;
                HirExprKind::Binary {
                    op: lower_binary_op(node)?,
                    lhs,
                    rhs,
                }
            }
            SyntaxKind::CallExpr | SyntaxKind::IndexExpr => {
                let exprs = expression_children(node);
                let Some(target_node) = exprs.first().copied() else {
                    return Err(HirBuildError::Unsupported(format!(
                        "call expression without target: `{}`",
                        node.text().trim()
                    )));
                };
                let target = self.lower_expr(scope, target_node)?;
                let args = node
                    .child_nodes()
                    .into_iter()
                    .find(|child| child.kind() == SyntaxKind::ArgList)
                    .map(|arg_list| self.lower_call_args(scope, arg_list, false))
                    .transpose()?
                    .unwrap_or_default();
                let call = self.arenas.alloc_call(HirCall {
                    cst: cst(node),
                    target,
                    args,
                });
                HirExprKind::Call(call)
            }
            SyntaxKind::MemberExpr => {
                let receiver = if let Some(receiver) = expression_children(node).into_iter().next()
                {
                    self.lower_expr(scope, receiver)?
                } else {
                    *self.with_receivers.last().ok_or_else(|| {
                        HirBuildError::Unsupported(format!(
                            "member expression without receiver: `{}`",
                            node.text().trim()
                        ))
                    })?
                };
                let member_name = direct_member_name(node).ok_or_else(|| {
                    HirBuildError::Unsupported(format!(
                        "member expression without supported member name: `{}`",
                        node.text().trim()
                    ))
                })?;
                let member_symbol = self.member_symbol(scope, &member_name, node)?;
                let member = self.arenas.alloc_member(HirMember {
                    cst: cst(node),
                    receiver: Some(receiver),
                    symbol: member_symbol,
                });
                HirExprKind::Member(member)
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

    fn member_symbol(
        &mut self,
        scope: ScopeId,
        name: &str,
        node: SyntaxNode<'_>,
    ) -> Result<SymbolId, HirBuildError> {
        if let Some(symbol) =
            self.symbols
                .resolve_in_scope_chain(scope, SymbolNamespace::Member, name)?
        {
            return Ok(symbol);
        }
        let (start, end) = node.text_range();
        Ok(self.symbols.declare_symbol(
            scope,
            SymbolNamespace::Member,
            name,
            SourceProvenance {
                module_name: None,
                span: Some(FrontendSourceSpan {
                    start: start as usize,
                    end: end as usize,
                }),
            },
        )?)
    }

    fn local_decl(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
    ) -> Result<Option<HirDeclId>, HirBuildError> {
        let Some(symbol) = self.first_symbol_in_node(scope, SymbolNamespace::Local, node) else {
            return Ok(None);
        };
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

    fn resolve_name(&mut self, scope: ScopeId, name: &str) -> Result<SymbolId, HirBuildError> {
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
        if let Some(symbol) = self.resolve_property_procedure_self_name(scope, name)? {
            return Ok(symbol);
        }
        if let Some(symbol) = self.resolve_property_get_name(scope, name)? {
            return Ok(symbol);
        }
        if let Some(symbol) = self.resolve_property_write_name(scope, name)? {
            return Ok(symbol);
        }
        if is_builtin_intrinsic_name(name) {
            return Ok(self.symbols.declare_symbol(
                scope,
                SymbolNamespace::Procedure,
                name,
                SourceProvenance {
                    module_name: None,
                    span: None,
                },
            )?);
        }
        Err(HirBuildError::UnresolvedName {
            name: name.to_string(),
            scope,
        })
    }

    fn resolve_property_procedure_self_name(
        &self,
        scope: ScopeId,
        name: &str,
    ) -> Result<Option<SymbolId>, HirBuildError> {
        let scope_data = self.symbols.scope(scope)?;
        if scope_data.kind != ScopeKind::Procedure {
            return Ok(None);
        }
        let Some(scope_name) = scope_data
            .name
            .and_then(|name_id| self.symbols.name(name_id))
            .map(|name| name.folded.as_str())
        else {
            return Ok(None);
        };
        let Some(property_group) = scope_name
            .strip_prefix("property_get_")
            .or_else(|| scope_name.strip_prefix("property_let_"))
            .or_else(|| scope_name.strip_prefix("property_set_"))
        else {
            return Ok(None);
        };
        if !property_group.eq_ignore_ascii_case(name) {
            return Ok(None);
        }
        Ok(self
            .symbols
            .resolve_in_scope_chain(scope, SymbolNamespace::Procedure, scope_name)?)
    }

    fn resolve_property_get_name(
        &self,
        scope: ScopeId,
        name: &str,
    ) -> Result<Option<SymbolId>, HirBuildError> {
        let property_get_name = format!("property_get_{name}");
        Ok(self.symbols.resolve_in_scope_chain(
            scope,
            SymbolNamespace::Procedure,
            &property_get_name,
        )?)
    }

    fn resolve_property_write_name(
        &self,
        scope: ScopeId,
        name: &str,
    ) -> Result<Option<SymbolId>, HirBuildError> {
        for prefix in ["property_let_", "property_set_"] {
            let property_write_name = format!("{prefix}{name}");
            if let Some(symbol) = self.symbols.resolve_in_scope_chain(
                scope,
                SymbolNamespace::Procedure,
                &property_write_name,
            )? {
                return Ok(Some(symbol));
            }
        }
        Ok(None)
    }

    fn scope_by_kind_and_name(&self, kind: ScopeKind, name: &str) -> Option<ScopeId> {
        let wanted = self.symbols.lookup_name(name)?;
        self.symbols
            .scopes()
            .iter()
            .find(|scope| scope.kind == kind && scope.name == Some(wanted))
            .map(|scope| scope.id)
    }

    fn first_symbol_in_node(
        &self,
        scope: ScopeId,
        namespace: SymbolNamespace,
        node: SyntaxNode<'_>,
    ) -> Option<SymbolId> {
        let (start, end) = node.text_range();
        self.symbols
            .symbols()
            .iter()
            .filter(|symbol| symbol.scope == scope && symbol.namespace == namespace)
            .find(|symbol| {
                symbol
                    .provenance
                    .span
                    .is_some_and(|span| span.start >= start as usize && span.end <= end as usize)
            })
            .map(|symbol| symbol.id)
    }
}

fn cst(node: SyntaxNode<'_>) -> CstBackpointer {
    cst_with_kind(node, &format!("{:?}", node.kind()))
}

fn cst_with_kind(node: SyntaxNode<'_>, syntax_kind: &str) -> CstBackpointer {
    let (start, end) = node.text_range();
    CstBackpointer {
        syntax_kind: syntax_kind.to_string(),
        span: FrontendSourceSpan {
            start: start as usize,
            end: end as usize,
        },
    }
}

fn expression_children(node: SyntaxNode<'_>) -> Vec<SyntaxNode<'_>> {
    node.child_nodes()
        .into_iter()
        .filter(|child| is_expression_node(child.kind()))
        .collect()
}

fn is_expression_node(kind: SyntaxKind) -> bool {
    matches!(
        kind,
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
}

fn first_identifier_text(node: SyntaxNode<'_>) -> Option<String> {
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if token.kind == SyntaxKind::Ident
                    || token.kind == SyntaxKind::BracketedIdent
                    || (node.kind() == SyntaxKind::IdentExpr && token.kind.is_keyword()) =>
            {
                return Some(
                    normalize_ident(
                        token
                            .text
                            .strip_prefix('[')
                            .and_then(|value| value.strip_suffix(']'))
                            .unwrap_or(token.text),
                    )
                    .unwrap_or_else(|| token.text.to_string()),
                );
            }
            SyntaxElement::Node(child) => {
                if let Some(text) = first_identifier_text(child) {
                    return Some(text);
                }
            }
            _ => {}
        }
    }
    if node.kind() != SyntaxKind::IdentExpr {
        return None;
    }
    let text = node.text();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let name = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(trimmed);
    normalize_ident(name)
}

fn new_expr_type_name(node: SyntaxNode<'_>) -> Option<String> {
    if node.kind() != SyntaxKind::NewExpr {
        return None;
    }
    let mut parts = Vec::new();
    let mut expect_name = false;
    for token in node.child_tokens() {
        if token.kind.is_trivia() {
            continue;
        }
        if token.kind == SyntaxKind::KwNew {
            expect_name = true;
            continue;
        }
        if token.kind == SyntaxKind::Dot {
            expect_name = true;
            continue;
        }
        if expect_name
            && (token.kind == SyntaxKind::Ident
                || token.kind == SyntaxKind::BracketedIdent
                || token.kind.is_keyword())
        {
            let text = token
                .text
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .unwrap_or(token.text);
            parts.push(normalize_ident(text)?);
            expect_name = false;
            continue;
        }
        return None;
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn direct_member_name(node: SyntaxNode<'_>) -> Option<String> {
    let mut after_member_operator = false;
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if token.kind == SyntaxKind::Dot || token.kind == SyntaxKind::Bang =>
            {
                after_member_operator = true;
            }
            SyntaxElement::Token(token)
                if after_member_operator
                    && !token.kind.is_trivia()
                    && (token.kind == SyntaxKind::Ident
                        || token.kind == SyntaxKind::BracketedIdent
                        || token.kind.is_keyword()) =>
            {
                let text = token
                    .text
                    .strip_prefix('[')
                    .and_then(|value| value.strip_suffix(']'))
                    .unwrap_or(token.text);
                return normalize_ident(text);
            }
            SyntaxElement::Token(token) if !token.kind.is_trivia() => {
                after_member_operator = false;
            }
            _ => {}
        }
    }
    None
}

fn property_kind(node: SyntaxNode<'_>) -> HirPropertyKind {
    if node
        .child_tokens()
        .iter()
        .any(|token| token.kind == SyntaxKind::KwLet)
    {
        HirPropertyKind::Let
    } else if node
        .child_tokens()
        .iter()
        .any(|token| token.kind == SyntaxKind::KwSet)
    {
        HirPropertyKind::Set
    } else {
        HirPropertyKind::Get
    }
}

fn procedure_symbol_name(node: SyntaxNode<'_>, name: &str) -> String {
    if node.kind() != SyntaxKind::PropertyDecl {
        return name.to_string();
    }
    let prefix = match property_kind(node) {
        HirPropertyKind::Get => "property_get",
        HirPropertyKind::Let => "property_let",
        HirPropertyKind::Set => "property_set",
    };
    format!("{prefix}_{name}")
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
        SyntaxKind::KwNothing => Ok(HirLiteral::Nothing),
        _ => Err(HirBuildError::Unsupported(format!(
            "unsupported literal token {:?}",
            token.kind
        ))),
    }
}

fn lower_unary_op(kind: SyntaxKind) -> HirUnaryOp {
    match kind {
        SyntaxKind::Minus => HirUnaryOp::Negate,
        SyntaxKind::KwNot => HirUnaryOp::Not,
        _ => unreachable!("filtered unary operator token"),
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
                | SyntaxKind::KwIs
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
        SyntaxKind::KwIs => Ok(HirBinaryOp::Is),
        SyntaxKind::KwAnd => Ok(HirBinaryOp::And),
        SyntaxKind::KwOr => Ok(HirBinaryOp::Or),
        _ => unreachable!("filtered operator token"),
    }
}

fn is_typeof_is_expr(node: SyntaxNode<'_>) -> bool {
    node.child_tokens()
        .into_iter()
        .any(|token| token.kind == SyntaxKind::KwTypeOf)
        && node
            .child_tokens()
            .into_iter()
            .any(|token| token.kind == SyntaxKind::KwIs)
}

fn case_is_operator(node: SyntaxNode<'_>) -> Result<HirBinaryOp, HirBuildError> {
    for token in node.child_tokens() {
        let op = match token.kind {
            SyntaxKind::Eq => Some(HirBinaryOp::Eq),
            SyntaxKind::LtGt => Some(HirBinaryOp::Ne),
            SyntaxKind::Lt => Some(HirBinaryOp::Lt),
            SyntaxKind::LtEq => Some(HirBinaryOp::Le),
            SyntaxKind::Gt => Some(HirBinaryOp::Gt),
            SyntaxKind::GtEq => Some(HirBinaryOp::Ge),
            _ => None,
        };
        if let Some(op) = op {
            return Ok(op);
        }
    }
    Err(HirBuildError::Unsupported(format!(
        "Case Is clause without comparison operator: `{}`",
        node.text().trim()
    )))
}

fn exit_stmt_kind(node: SyntaxNode<'_>) -> Result<HirStmtKind, HirBuildError> {
    for token in node.child_tokens() {
        match token.kind {
            SyntaxKind::KwDo => return Ok(HirStmtKind::ExitDo),
            SyntaxKind::KwFor => return Ok(HirStmtKind::ExitFor),
            SyntaxKind::KwSub | SyntaxKind::KwFunction | SyntaxKind::KwProperty => {
                return Ok(HirStmtKind::ExitProcedure);
            }
            _ => {}
        }
    }
    Err(HirBuildError::Unsupported(format!(
        "Exit statement without supported target: `{}`",
        node.text().trim()
    )))
}

fn on_error_stmt_kind(node: SyntaxNode<'_>) -> Result<HirStmtKind, HirBuildError> {
    let tokens = node
        .child_tokens()
        .into_iter()
        .filter(|token| !token.kind.is_trivia())
        .collect::<Vec<_>>();
    if tokens.windows(2).any(|window| {
        window[0].kind == SyntaxKind::KwResume && window[1].kind == SyntaxKind::KwNext
    }) {
        return Ok(HirStmtKind::OnErrorResumeNext);
    }
    if tokens.windows(2).any(|window| {
        window[0].kind == SyntaxKind::KwGoTo
            && window[1].kind == SyntaxKind::IntLiteral
            && window[1].text.trim() == "0"
    }) {
        return Ok(HirStmtKind::OnErrorGoto0);
    }
    if let Some(label) = jump_label_from_stmt(node, SyntaxKind::KwGoTo).ok() {
        return Ok(HirStmtKind::OnErrorGotoLabel { label });
    }
    Err(HirBuildError::Unsupported(format!(
        "On Error statement without supported target: `{}`",
        node.text().trim()
    )))
}

fn resume_stmt_kind(node: SyntaxNode<'_>) -> Result<HirStmtKind, HirBuildError> {
    let tokens = node
        .child_tokens()
        .into_iter()
        .filter(|token| !token.kind.is_trivia())
        .collect::<Vec<_>>();
    if tokens.iter().any(|token| token.kind == SyntaxKind::KwNext) {
        return Ok(HirStmtKind::ResumeNext);
    }
    if tokens.len() == 1 && tokens[0].kind == SyntaxKind::KwResume {
        return Ok(HirStmtKind::Resume);
    }
    if let Some(label) = tokens
        .iter()
        .skip(1)
        .find_map(|token| label_name_from_token(*token))
    {
        return Ok(HirStmtKind::ResumeLabel { label });
    }
    Err(HirBuildError::Unsupported(format!(
        "Resume statement without supported target: `{}`",
        node.text().trim()
    )))
}

fn label_name_from_stmt(node: SyntaxNode<'_>) -> Result<String, HirBuildError> {
    node.child_tokens()
        .into_iter()
        .find_map(label_name_from_token)
        .ok_or_else(|| {
            HirBuildError::Unsupported(format!(
                "label statement without supported label: `{}`",
                node.text().trim()
            ))
        })
}

fn jump_label_from_stmt(
    node: SyntaxNode<'_>,
    jump_kind: SyntaxKind,
) -> Result<String, HirBuildError> {
    let mut after_jump = false;
    for token in node.child_tokens() {
        if token.kind == jump_kind {
            after_jump = true;
            continue;
        }
        if !after_jump || token.kind.is_trivia() {
            continue;
        }
        if let Some(label) = label_name_from_token(token) {
            return Ok(label);
        }
        break;
    }
    Err(HirBuildError::Unsupported(format!(
        "jump statement without supported label: `{}`",
        node.text().trim()
    )))
}

fn label_name_from_token(token: oxvba_syntax::SyntaxToken<'_>) -> Option<String> {
    match token.kind {
        SyntaxKind::IntLiteral => token
            .text
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|line| *line >= 0)
            .map(|line| format!("__line_{line}")),
        SyntaxKind::Ident | SyntaxKind::BracketedIdent => normalize_ident(token.text),
        _ => None,
    }
}

fn name_after_keyword(
    node: SyntaxNode<'_>,
    keyword: SyntaxKind,
    label: &str,
) -> Result<String, HirBuildError> {
    let mut after_keyword = false;
    for token in node.child_tokens() {
        if token.kind == keyword {
            after_keyword = true;
            continue;
        }
        if !after_keyword || token.kind.is_trivia() {
            continue;
        }
        if let Some(name) = normalize_ident(token.text) {
            return Ok(name);
        }
        break;
    }
    Err(HirBuildError::Unsupported(format!(
        "{label} statement without supported name: `{}`",
        node.text().trim()
    )))
}

fn redim_name(node: SyntaxNode<'_>) -> Result<String, HirBuildError> {
    let mut after_redim = false;
    for token in node.child_tokens() {
        if token.kind == SyntaxKind::KwReDim {
            after_redim = true;
            continue;
        }
        if !after_redim || token.kind.is_trivia() || token.kind == SyntaxKind::KwPreserve {
            continue;
        }
        if let Some(name) = normalize_ident(token.text) {
            return Ok(name);
        }
        break;
    }
    Err(HirBuildError::Unsupported(format!(
        "ReDim statement without supported name: `{}`",
        node.text().trim()
    )))
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
    fn hir_builder_lowers_unary_expressions_from_cst() {
        let source = "Sub Main(ByVal ready As Boolean)\nDim x As Long\nDim y As Long\nDim flag As Boolean\nx = -7\ny = +7\nflag = Not ready\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let procedure = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("procedure declaration");
        let unary_exprs = procedure
            .iter()
            .filter_map(|stmt| find_let_stmt(&module.arenas, *stmt))
            .filter_map(|(_, value)| module.arenas.expr(value))
            .filter_map(|expr| match expr.kind {
                HirExprKind::Unary { op, .. } => Some(op),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            unary_exprs,
            vec![HirUnaryOp::Negate, HirUnaryOp::Not],
            "expected unary minus and Not expressions: {module:#?}"
        );
        assert!(
            procedure
                .iter()
                .filter_map(|stmt| find_let_stmt(&module.arenas, *stmt))
                .filter_map(|(_, value)| module.arenas.expr(value))
                .any(|expr| matches!(expr.kind, HirExprKind::Literal(HirLiteral::Int(7)))),
            "expected unary plus to lower transparently to the inner literal: {module:#?}"
        );
    }

    #[test]
    fn hir_builder_lowers_object_is_and_nothing_literal_from_cst() {
        let source = "Sub Main()\n    Dim obj As Object\n    Dim same As Boolean\n    same = obj Is Nothing\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let procedure = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("procedure declaration");
        let let_stmt = procedure
            .iter()
            .find_map(|stmt| find_let_stmt(&module.arenas, *stmt))
            .expect("let statement");
        let value = module.arenas.expr(let_stmt.1).expect("assignment value");
        let HirExprKind::Binary {
            op: HirBinaryOp::Is,
            rhs,
            ..
        } = value.kind
        else {
            panic!("expected object identity binary expression, got {value:?}");
        };
        assert!(matches!(
            module.arenas.expr(rhs).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Nothing))
        ));
    }

    #[test]
    fn hir_builder_preserves_new_expression_type_name() {
        let source = "Sub Main()\nDim obj As Object\nSet obj = New Foo.Bar\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let procedure = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("procedure declaration");
        let set_stmt = procedure
            .iter()
            .find_map(|stmt| find_set_stmt(&module.arenas, *stmt))
            .expect("set statement");
        assert!(matches!(
            module.arenas.expr(set_stmt.1).map(|expr| &expr.kind),
            Some(HirExprKind::New { type_name }) if type_name == "foo.bar"
        ));
    }

    #[test]
    fn hir_builder_lowers_call_statement_to_call_expression() {
        let source = "Sub Main()\nCall Use(5)\nEnd Sub\nSub Use(ByVal x As Long)\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. }
                    if module
                        .symbols
                        .symbol(decl.symbol)
                        .and_then(|symbol| module.symbols.name(symbol.name))
                        .is_some_and(|name| name.folded == "main") =>
                {
                    Some(body)
                }
                _ => None,
            })
            .expect("main body");
        let call_expr = main_body
            .iter()
            .find_map(
                |stmt| match module.arenas.stmt(*stmt).map(|stmt| &stmt.kind) {
                    Some(HirStmtKind::Expr(expr)) => Some(*expr),
                    Some(HirStmtKind::Block(children)) => children.iter().find_map(|child| {
                        match module.arenas.stmt(*child).map(|stmt| &stmt.kind) {
                            Some(HirStmtKind::Expr(expr)) => Some(*expr),
                            _ => None,
                        }
                    }),
                    _ => None,
                },
            )
            .expect("call expression statement");
        let HirExprKind::Call(call) = module.arenas.expr(call_expr).expect("call expr").kind else {
            panic!("expected call expression");
        };
        let call = module.arenas.call(call).expect("call data");
        assert_eq!(call.args.len(), 1);
        assert!(matches!(
            module.arenas.expr(call.target).map(|expr| &expr.kind),
            Some(HirExprKind::Name(_))
        ));
    }

    #[test]
    fn hir_builder_preserves_named_call_arguments() {
        let source =
            "Sub Main()\nCall Use(value := 5)\nEnd Sub\nSub Use(ByVal value As Long)\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. }
                    if module
                        .symbols
                        .symbol(decl.symbol)
                        .and_then(|symbol| module.symbols.name(symbol.name))
                        .is_some_and(|name| name.folded == "main") =>
                {
                    Some(body)
                }
                _ => None,
            })
            .expect("main body");
        let call_expr = main_body
            .iter()
            .find_map(
                |stmt| match module.arenas.stmt(*stmt).map(|stmt| &stmt.kind) {
                    Some(HirStmtKind::Expr(expr)) => Some(*expr),
                    Some(HirStmtKind::Block(children)) => children.iter().find_map(|child| {
                        match module.arenas.stmt(*child).map(|stmt| &stmt.kind) {
                            Some(HirStmtKind::Expr(expr)) => Some(*expr),
                            _ => None,
                        }
                    }),
                    _ => None,
                },
            )
            .expect("call expression statement");
        let HirExprKind::Call(call) = module.arenas.expr(call_expr).expect("call expr").kind else {
            panic!("expected call expression");
        };
        let call = module.arenas.call(call).expect("call data");
        assert_eq!(call.args.len(), 1);
        assert_eq!(call.args[0].name.as_deref(), Some("value"));
    }

    #[test]
    fn hir_builder_lowers_multiline_if_statement() {
        let source = "Sub Main()\nDim x As Long\nIf x = 0 Then\nx = 1\nEnd If\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let if_stmt = main_body
            .iter()
            .find_map(|stmt| find_if_stmt(&module.arenas, *stmt))
            .expect("if statement");
        let Some(HirStmt {
            kind:
                HirStmtKind::If {
                    condition,
                    then_body,
                    else_body,
                },
            ..
        }) = module.arenas.stmt(if_stmt)
        else {
            panic!("expected if statement");
        };

        assert!(matches!(
            module.arenas.expr(*condition).map(|expr| &expr.kind),
            Some(HirExprKind::Binary {
                op: HirBinaryOp::Eq,
                ..
            })
        ));
        assert!(
            then_body
                .iter()
                .any(|stmt| find_let_stmt(&module.arenas, *stmt).is_some()),
            "expected assignment in then body: {module:#?}"
        );
        assert!(else_body.is_empty());
    }

    #[test]
    fn hir_builder_lowers_front_checked_do_while_statement() {
        let source = "Sub Main()\nDim x As Long\nDo While x < 3\nx = x + 1\nLoop\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let do_stmt = main_body
            .iter()
            .find_map(|stmt| find_do_while_stmt(&module.arenas, *stmt))
            .expect("do while statement");
        let Some(HirStmt {
            kind:
                HirStmtKind::DoWhile {
                    condition,
                    body,
                    post_check,
                    until,
                },
            ..
        }) = module.arenas.stmt(do_stmt)
        else {
            panic!("expected do while statement");
        };

        assert!(!post_check);
        assert!(!until);
        assert!(matches!(
            module.arenas.expr(*condition).map(|expr| &expr.kind),
            Some(HirExprKind::Binary {
                op: HirBinaryOp::Lt,
                ..
            })
        ));
        assert!(
            body.iter()
                .any(|stmt| find_let_stmt(&module.arenas, *stmt).is_some()),
            "expected assignment in loop body: {module:#?}"
        );
    }

    #[test]
    fn hir_builder_lowers_until_and_post_checked_do_loops() {
        let source = "Sub Main()\nDim x As Long\nDo Until x = 3\nx = x + 1\nLoop\nDo\nx = x + 1\nLoop Until x = 7\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let loops: Vec<_> = main_body
            .iter()
            .filter_map(|stmt| find_do_while_stmt(&module.arenas, *stmt))
            .collect();
        assert_eq!(loops.len(), 2, "{module:#?}");
        let first = module.arenas.stmt(loops[0]).expect("first loop");
        let second = module.arenas.stmt(loops[1]).expect("second loop");
        assert!(matches!(
            first.kind,
            HirStmtKind::DoWhile {
                post_check: false,
                until: true,
                ..
            }
        ));
        assert!(matches!(
            second.kind,
            HirStmtKind::DoWhile {
                post_check: true,
                until: true,
                ..
            }
        ));
    }

    #[test]
    fn hir_builder_lowers_while_wend_statement_as_front_checked_loop() {
        let source = "Sub Main()\nDim x As Long\nWhile x < 3\nx = x + 1\nWend\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let loop_stmt = main_body
            .iter()
            .find_map(|stmt| find_do_while_stmt(&module.arenas, *stmt))
            .expect("while wend loop");
        assert!(matches!(
            module.arenas.stmt(loop_stmt).map(|stmt| &stmt.kind),
            Some(HirStmtKind::DoWhile {
                post_check: false,
                until: false,
                ..
            })
        ));
    }

    #[test]
    fn hir_builder_lowers_simple_for_range_statement() {
        let source = "Sub Main()\nDim i As Long\nFor i = 1 To 3\ni = i + 1\nNext\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let for_stmt = main_body
            .iter()
            .find_map(|stmt| find_for_range_stmt(&module.arenas, *stmt))
            .expect("for range statement");
        let Some(HirStmt {
            kind:
                HirStmtKind::ForRange {
                    start,
                    end,
                    step,
                    body,
                    ..
                },
            ..
        }) = module.arenas.stmt(for_stmt)
        else {
            panic!("expected for range statement");
        };

        assert!(matches!(
            module.arenas.expr(*start).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Int(1)))
        ));
        assert!(matches!(
            module.arenas.expr(*end).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Int(3)))
        ));
        assert!(step.is_none());
        assert!(
            body.iter()
                .any(|stmt| find_let_stmt(&module.arenas, *stmt).is_some()),
            "expected assignment in for body: {module:#?}"
        );
    }

    #[test]
    fn hir_builder_lowers_for_each_statement() {
        let source =
            "Sub Main()\nDim item As Variant\nFor Each item In item\nitem = item\nNext\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let for_each_stmt = main_body
            .iter()
            .find_map(|stmt| find_for_each_stmt(&module.arenas, *stmt))
            .expect("for each statement");
        let Some(HirStmt {
            kind: HirStmtKind::ForEach { iterable, body, .. },
            ..
        }) = module.arenas.stmt(for_each_stmt)
        else {
            panic!("expected for each statement");
        };
        assert!(matches!(
            module.arenas.expr(*iterable).map(|expr| &expr.kind),
            Some(HirExprKind::Name(_))
        ));
        assert!(
            body.iter()
                .any(|stmt| find_let_stmt(&module.arenas, *stmt).is_some()),
            "expected assignment in for each body: {module:#?}"
        );
    }

    #[test]
    fn hir_builder_lowers_simple_select_case_statement() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase 1\nx = 2\nEnd Select\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let select_stmt = main_body
            .iter()
            .find_map(|stmt| find_select_case_stmt(&module.arenas, *stmt))
            .expect("select case statement");
        let Some(HirStmt {
            kind:
                HirStmtKind::SelectCase {
                    expr,
                    arms,
                    else_body,
                },
            ..
        }) = module.arenas.stmt(select_stmt)
        else {
            panic!("expected select case statement");
        };

        assert!(matches!(
            module.arenas.expr(*expr).map(|expr| &expr.kind),
            Some(HirExprKind::Name(_))
        ));
        assert_eq!(arms.len(), 1);
        assert_eq!(arms[0].0.len(), 1);
        let HirCaseClause::Value(value) = &arms[0].0[0] else {
            panic!("expected value case clause");
        };
        assert!(matches!(
            module.arenas.expr(*value).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Int(1)))
        ));
        assert!(
            arms[0]
                .1
                .iter()
                .any(|stmt| find_let_stmt(&module.arenas, *stmt).is_some()),
            "expected assignment in case body: {module:#?}"
        );
        assert!(else_body.is_empty());
    }

    #[test]
    fn hir_builder_lowers_select_case_range_clause() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase 1 To 3\nx = 2\nEnd Select\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let select_stmt = main_body
            .iter()
            .find_map(|stmt| find_select_case_stmt(&module.arenas, *stmt))
            .expect("select case statement");
        let Some(HirStmt {
            kind: HirStmtKind::SelectCase { arms, .. },
            ..
        }) = module.arenas.stmt(select_stmt)
        else {
            panic!("expected select case statement");
        };
        let HirCaseClause::Range { start, end } = &arms[0].0[0] else {
            panic!("expected range case clause");
        };
        assert!(matches!(
            module.arenas.expr(*start).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Int(1)))
        ));
        assert!(matches!(
            module.arenas.expr(*end).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Int(3)))
        ));
    }

    #[test]
    fn hir_builder_lowers_select_case_multi_value_clause() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase 1, 2\nx = 2\nEnd Select\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let select_stmt = main_body
            .iter()
            .find_map(|stmt| find_select_case_stmt(&module.arenas, *stmt))
            .expect("select case statement");
        let Some(HirStmt {
            kind: HirStmtKind::SelectCase { arms, .. },
            ..
        }) = module.arenas.stmt(select_stmt)
        else {
            panic!("expected select case statement");
        };
        assert_eq!(arms[0].0.len(), 2);
        assert!(matches!(arms[0].0[0], HirCaseClause::Value(_)));
        assert!(matches!(arms[0].0[1], HirCaseClause::Value(_)));
    }

    #[test]
    fn hir_builder_lowers_select_case_is_clause() {
        let source =
            "Sub Main()\nDim x As Long\nSelect Case x\nCase Is < 0\nx = 2\nEnd Select\nEnd Sub\n";
        let module = build_hir_from_source("Module1", source).expect("HIR module");
        let main_body = module
            .declarations
            .iter()
            .filter_map(|decl| module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                HirDeclKind::Procedure { body, .. } => Some(body),
                _ => None,
            })
            .expect("main body");
        let select_stmt = main_body
            .iter()
            .find_map(|stmt| find_select_case_stmt(&module.arenas, *stmt))
            .expect("select case statement");
        let Some(HirStmt {
            kind: HirStmtKind::SelectCase { arms, .. },
            ..
        }) = module.arenas.stmt(select_stmt)
        else {
            panic!("expected select case statement");
        };
        let HirCaseClause::Is { op, value } = &arms[0].0[0] else {
            panic!("expected Case Is clause");
        };
        assert_eq!(*op, HirBinaryOp::Lt);
        assert!(matches!(
            module.arenas.expr(*value).map(|expr| &expr.kind),
            Some(HirExprKind::Literal(HirLiteral::Int(0)))
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

    fn find_set_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<(HirExprId, HirExprId)> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::Set { target, value }) => Some((*target, *value)),
            Some(HirStmtKind::Block(children)) => {
                children.iter().find_map(|child| find_set_stmt(hir, *child))
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

    fn find_if_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<HirStmtId> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::If { .. }) => Some(stmt),
            Some(HirStmtKind::Block(children)) => {
                children.iter().find_map(|child| find_if_stmt(hir, *child))
            }
            _ => None,
        }
    }

    fn find_do_while_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<HirStmtId> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::DoWhile { .. }) => Some(stmt),
            Some(HirStmtKind::Block(children)) => children
                .iter()
                .find_map(|child| find_do_while_stmt(hir, *child)),
            _ => None,
        }
    }

    fn find_select_case_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<HirStmtId> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::SelectCase { .. }) => Some(stmt),
            Some(HirStmtKind::Block(children)) => children
                .iter()
                .find_map(|child| find_select_case_stmt(hir, *child)),
            _ => None,
        }
    }

    fn find_for_range_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<HirStmtId> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::ForRange { .. }) => Some(stmt),
            Some(HirStmtKind::Block(children)) => children
                .iter()
                .find_map(|child| find_for_range_stmt(hir, *child)),
            _ => None,
        }
    }

    fn find_for_each_stmt(hir: &HirArenas, stmt: HirStmtId) -> Option<HirStmtId> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::ForEach { .. }) => Some(stmt),
            Some(HirStmtKind::Block(children)) => children
                .iter()
                .find_map(|child| find_for_each_stmt(hir, *child)),
            _ => None,
        }
    }
}
