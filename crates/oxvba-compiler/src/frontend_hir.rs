use crate::frontend_symbols::{FrontendSourceSpan, SymbolId};

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
}
