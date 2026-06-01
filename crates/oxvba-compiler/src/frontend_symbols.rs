use std::collections::BTreeMap;

use thiserror::Error;

use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InternedNameId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolNamespace {
    Project,
    Module,
    Library,
    Type,
    Procedure,
    Member,
    Parameter,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Project,
    Library,
    Module,
    Procedure,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrontendSourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProvenance {
    pub module_name: Option<String>,
    pub span: Option<FrontendSourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternedName {
    pub first_spelling: String,
    pub folded: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub name: Option<InternedNameId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: InternedNameId,
    pub namespace: SymbolNamespace,
    pub scope: ScopeId,
    pub provenance: SourceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SymbolModelError {
    #[error("syntax parse failed: {0}")]
    Syntax(String),
    #[error("duplicate symbol `{name}` in {namespace:?} namespace")]
    DuplicateSymbol {
        name: String,
        namespace: SymbolNamespace,
    },
    #[error("unknown scope {0:?}")]
    UnknownScope(ScopeId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SymbolKey {
    scope: ScopeId,
    name: InternedNameId,
    namespace: SymbolNamespace,
}

#[derive(Debug, Clone)]
pub struct SymbolModel {
    names: Vec<InternedName>,
    names_by_folded: BTreeMap<String, InternedNameId>,
    scopes: Vec<Scope>,
    symbols: Vec<Symbol>,
    symbols_by_key: BTreeMap<SymbolKey, SymbolId>,
}

impl Default for SymbolModel {
    fn default() -> Self {
        let mut model = Self {
            names: Vec::new(),
            names_by_folded: BTreeMap::new(),
            scopes: Vec::new(),
            symbols: Vec::new(),
            symbols_by_key: BTreeMap::new(),
        };
        model.scopes.push(Scope {
            id: ScopeId(0),
            kind: ScopeKind::Global,
            parent: None,
            name: None,
        });
        model
    }
}

impl SymbolModel {
    pub fn global_scope(&self) -> ScopeId {
        ScopeId(0)
    }

    pub fn intern_name(&mut self, name: &str) -> InternedNameId {
        let folded = fold_identifier(name);
        if let Some(id) = self.names_by_folded.get(&folded) {
            return *id;
        }
        let id = InternedNameId(self.names.len());
        self.names.push(InternedName {
            first_spelling: name.to_string(),
            folded: folded.clone(),
        });
        self.names_by_folded.insert(folded, id);
        id
    }

    pub fn name(&self, id: InternedNameId) -> Option<&InternedName> {
        self.names.get(id.0)
    }

    pub fn lookup_name(&self, name: &str) -> Option<InternedNameId> {
        self.names_by_folded.get(&fold_identifier(name)).copied()
    }

    pub fn add_scope(
        &mut self,
        kind: ScopeKind,
        parent: ScopeId,
        name: Option<&str>,
    ) -> Result<ScopeId, SymbolModelError> {
        self.scope(parent)?;
        let name = name.map(|name| self.intern_name(name));
        let id = ScopeId(self.scopes.len());
        self.scopes.push(Scope {
            id,
            kind,
            parent: Some(parent),
            name,
        });
        Ok(id)
    }

    pub fn scope(&self, id: ScopeId) -> Result<&Scope, SymbolModelError> {
        self.scopes
            .get(id.0)
            .ok_or(SymbolModelError::UnknownScope(id))
    }

    pub fn declare_symbol(
        &mut self,
        scope: ScopeId,
        namespace: SymbolNamespace,
        name: &str,
        provenance: SourceProvenance,
    ) -> Result<SymbolId, SymbolModelError> {
        self.scope(scope)?;
        let name_id = self.intern_name(name);
        let key = SymbolKey {
            scope,
            name: name_id,
            namespace,
        };
        if self.symbols_by_key.contains_key(&key) {
            let interned = self.name(name_id).expect("name was just interned");
            return Err(SymbolModelError::DuplicateSymbol {
                name: interned.first_spelling.clone(),
                namespace,
            });
        }

        let id = SymbolId(self.symbols.len());
        self.symbols.push(Symbol {
            id,
            name: name_id,
            namespace,
            scope,
            provenance,
        });
        self.symbols_by_key.insert(key, id);
        Ok(id)
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0)
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    pub fn symbols_in_scope(&self, scope: ScopeId) -> Result<Vec<SymbolId>, SymbolModelError> {
        self.scope(scope)?;
        Ok(self
            .symbols
            .iter()
            .filter(|symbol| symbol.scope == scope)
            .map(|symbol| symbol.id)
            .collect())
    }

    pub fn find_in_scope(
        &self,
        scope: ScopeId,
        namespace: SymbolNamespace,
        name: &str,
    ) -> Result<Option<SymbolId>, SymbolModelError> {
        self.scope(scope)?;
        let Some(name) = self.lookup_name(name) else {
            return Ok(None);
        };
        Ok(self
            .symbols_by_key
            .get(&SymbolKey {
                scope,
                name,
                namespace,
            })
            .copied())
    }

    pub fn resolve_in_scope_chain(
        &self,
        scope: ScopeId,
        namespace: SymbolNamespace,
        name: &str,
    ) -> Result<Option<SymbolId>, SymbolModelError> {
        let Some(name) = self.lookup_name(name) else {
            return Ok(None);
        };
        let mut current = Some(scope);
        while let Some(scope_id) = current {
            let scope = self.scope(scope_id)?;
            if let Some(symbol) = self
                .symbols_by_key
                .get(&SymbolKey {
                    scope: scope_id,
                    name,
                    namespace,
                })
                .copied()
            {
                return Ok(Some(symbol));
            }
            current = scope.parent;
        }
        Ok(None)
    }
}

pub fn fold_identifier(name: &str) -> String {
    name.to_ascii_lowercase()
}

pub fn build_symbol_model_from_source(
    module_name: &str,
    source: &str,
) -> Result<SymbolModel, SymbolModelError> {
    let mut model = SymbolModel::default();
    model.declare_symbol(
        model.global_scope(),
        SymbolNamespace::Module,
        module_name,
        SourceProvenance {
            module_name: Some(module_name.to_string()),
            span: None,
        },
    )?;
    let module_scope =
        model.add_scope(ScopeKind::Module, model.global_scope(), Some(module_name))?;
    collect_symbols_from_source_into_model(&mut model, module_name, module_scope, source)?;
    Ok(model)
}

pub fn collect_symbols_from_source_into_model(
    model: &mut SymbolModel,
    module_name: &str,
    scope: ScopeId,
    source: &str,
) -> Result<(), SymbolModelError> {
    model.scope(scope)?;
    let parsed = oxvba_syntax::parse(source);
    if !parsed.errors().is_empty() {
        return Err(SymbolModelError::Syntax(format!("{:?}", parsed.errors())));
    }
    collect_symbols_from_node(model, module_name, scope, parsed.syntax())
}

fn collect_symbols_from_node(
    model: &mut SymbolModel,
    module_name: &str,
    scope: ScopeId,
    node: SyntaxNode<'_>,
) -> Result<(), SymbolModelError> {
    match node.kind() {
        SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl => {
            collect_procedure_symbols(model, module_name, scope, node)?;
            return Ok(());
        }
        SyntaxKind::DeclareStmt => {
            declare_named_node(model, module_name, scope, SymbolNamespace::Procedure, node)?;
        }
        SyntaxKind::EventDecl => {
            declare_named_node(model, module_name, scope, SymbolNamespace::Member, node)?;
        }
        SyntaxKind::TypeBlock | SyntaxKind::EnumBlock => {
            declare_named_node(model, module_name, scope, SymbolNamespace::Type, node)?;
        }
        SyntaxKind::DimStmt | SyntaxKind::ConstStmt => {
            for token in declaration_name_tokens(node) {
                model.declare_symbol(
                    scope,
                    SymbolNamespace::Local,
                    normalize_identifier_token(token.text),
                    provenance_for_token(module_name, token),
                )?;
            }
        }
        _ => {}
    }

    for child in node.child_nodes() {
        collect_symbols_from_node(model, module_name, scope, child)?;
    }
    Ok(())
}

fn collect_procedure_symbols(
    model: &mut SymbolModel,
    module_name: &str,
    parent_scope: ScopeId,
    node: SyntaxNode<'_>,
) -> Result<(), SymbolModelError> {
    let Some(name_token) = first_identifier_token(node) else {
        return Ok(());
    };
    let procedure_name = procedure_symbol_name(node, normalize_identifier_token(name_token.text));
    model.declare_symbol(
        parent_scope,
        SymbolNamespace::Procedure,
        &procedure_name,
        provenance_for_token(module_name, name_token),
    )?;
    let procedure_scope =
        model.add_scope(ScopeKind::Procedure, parent_scope, Some(&procedure_name))?;

    if let Some(param_list) = node.param_list() {
        for param in param_list.params() {
            if let Some(param_name) = first_identifier_token(param) {
                model.declare_symbol(
                    procedure_scope,
                    SymbolNamespace::Parameter,
                    normalize_identifier_token(param_name.text),
                    provenance_for_token(module_name, param_name),
                )?;
            }
        }
    }

    if let Some(body) = node.body_block() {
        collect_symbols_from_node(model, module_name, procedure_scope, body)?;
    }
    Ok(())
}

fn procedure_symbol_name(node: SyntaxNode<'_>, name: &str) -> String {
    if node.kind() != SyntaxKind::PropertyDecl {
        return name.to_string();
    }
    let prefix = if node
        .child_tokens()
        .iter()
        .any(|token| token.kind == SyntaxKind::KwLet)
    {
        "property_let"
    } else if node
        .child_tokens()
        .iter()
        .any(|token| token.kind == SyntaxKind::KwSet)
    {
        "property_set"
    } else {
        "property_get"
    };
    format!("{prefix}_{name}")
}

fn declare_named_node(
    model: &mut SymbolModel,
    module_name: &str,
    scope: ScopeId,
    namespace: SymbolNamespace,
    node: SyntaxNode<'_>,
) -> Result<Option<SymbolId>, SymbolModelError> {
    let Some(name_token) = first_identifier_token(node) else {
        return Ok(None);
    };
    model
        .declare_symbol(
            scope,
            namespace,
            normalize_identifier_token(name_token.text),
            provenance_for_token(module_name, name_token),
        )
        .map(Some)
}

fn declaration_name_tokens(node: SyntaxNode<'_>) -> Vec<SyntaxToken<'_>> {
    let mut names = Vec::new();
    let mut expect_name = false;
    let mut in_type_ref = false;
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if token.kind == SyntaxKind::KwDim || token.kind == SyntaxKind::KwConst =>
            {
                expect_name = true;
                in_type_ref = false;
            }
            SyntaxElement::Token(token)
                if matches!(
                    token.kind,
                    SyntaxKind::KwPublic
                        | SyntaxKind::KwPrivate
                        | SyntaxKind::KwFriend
                        | SyntaxKind::KwStatic
                ) =>
            {
                expect_name = true;
                in_type_ref = false;
            }
            SyntaxElement::Token(token) if token.kind == SyntaxKind::Comma => {
                expect_name = true;
                in_type_ref = false;
            }
            SyntaxElement::Token(token) if token.kind == SyntaxKind::KwAs => {
                expect_name = false;
                in_type_ref = true;
            }
            SyntaxElement::Token(token)
                if expect_name
                    && !in_type_ref
                    && (is_identifier_like(token.kind) || token.kind.is_keyword()) =>
            {
                names.push(token);
                expect_name = false;
            }
            SyntaxElement::Node(child) if expect_name && child.kind() == SyntaxKind::IdentExpr => {
                if let Some(token) = first_identifier_token_deep(child) {
                    names.push(token);
                    expect_name = false;
                }
            }
            SyntaxElement::Token(token) if !token.kind.is_trivia() => {
                if token.kind != SyntaxKind::TypeSuffix {
                    expect_name = false;
                }
            }
            SyntaxElement::Node(child) if child.kind() == SyntaxKind::TypeRef => {
                in_type_ref = false;
            }
            _ => {}
        }
    }
    names
}

fn first_identifier_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    node.child_tokens()
        .into_iter()
        .find(|token| is_identifier_like(token.kind))
        .or_else(|| first_identifier_token_deep(node))
}

fn first_identifier_token_deep(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if is_identifier_like(token.kind)
                    || (node.kind() == SyntaxKind::IdentExpr && token.kind.is_keyword()) =>
            {
                return Some(token);
            }
            SyntaxElement::Node(child) => {
                if let Some(token) = first_identifier_token_deep(child) {
                    return Some(token);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_identifier_like(kind: SyntaxKind) -> bool {
    kind == SyntaxKind::Ident || kind == SyntaxKind::BracketedIdent
}

fn normalize_identifier_token(text: &str) -> &str {
    text.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(text)
}

fn provenance_for_token(module_name: &str, token: SyntaxToken<'_>) -> SourceProvenance {
    SourceProvenance {
        module_name: Some(module_name.to_string()),
        span: Some(FrontendSourceSpan {
            start: token.offset as usize,
            end: token.offset as usize + token.text.len(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance(start: usize, end: usize) -> SourceProvenance {
        SourceProvenance {
            module_name: Some("Module1".to_string()),
            span: Some(FrontendSourceSpan { start, end }),
        }
    }

    #[test]
    fn symbol_model_interns_names_case_insensitively() {
        let mut model = SymbolModel::default();
        let first = model.intern_name("CustomerID");
        let second = model.intern_name("customerid");
        assert_eq!(first, second);
        assert_eq!(
            model.name(first),
            Some(&InternedName {
                first_spelling: "CustomerID".to_string(),
                folded: "customerid".to_string(),
            })
        );
    }

    #[test]
    fn symbol_model_rejects_duplicate_same_namespace_in_scope() {
        let mut model = SymbolModel::default();
        let module = model
            .add_scope(ScopeKind::Module, model.global_scope(), Some("Module1"))
            .expect("module scope");
        model
            .declare_symbol(module, SymbolNamespace::Local, "Value", provenance(10, 15))
            .expect("first declaration");
        let err = model
            .declare_symbol(module, SymbolNamespace::Local, "value", provenance(20, 25))
            .expect_err("case-insensitive duplicate should fail");
        assert!(matches!(
            err,
            SymbolModelError::DuplicateSymbol {
                namespace: SymbolNamespace::Local,
                ..
            }
        ));
    }

    #[test]
    fn symbol_model_keeps_namespaces_distinct() {
        let mut model = SymbolModel::default();
        let project = model
            .add_scope(ScopeKind::Project, model.global_scope(), Some("Book1"))
            .expect("project scope");
        let module_symbol = model
            .declare_symbol(project, SymbolNamespace::Module, "Sheet1", provenance(0, 6))
            .expect("module symbol");
        let library_symbol = model
            .declare_symbol(
                project,
                SymbolNamespace::Library,
                "sheet1",
                provenance(7, 13),
            )
            .expect("library symbol");
        assert_ne!(module_symbol, library_symbol);
    }

    #[test]
    fn symbol_model_resolves_nearest_scope_first() {
        let mut model = SymbolModel::default();
        let module = model
            .add_scope(ScopeKind::Module, model.global_scope(), Some("Module1"))
            .expect("module scope");
        let proc_scope = model
            .add_scope(ScopeKind::Procedure, module, Some("Main"))
            .expect("procedure scope");
        let module_value = model
            .declare_symbol(module, SymbolNamespace::Local, "Value", provenance(1, 6))
            .expect("module declaration");
        let local_value = model
            .declare_symbol(
                proc_scope,
                SymbolNamespace::Local,
                "value",
                provenance(20, 25),
            )
            .expect("local declaration");
        assert_eq!(
            model
                .resolve_in_scope_chain(proc_scope, SymbolNamespace::Local, "VALUE")
                .expect("resolve"),
            Some(local_value)
        );
        assert_eq!(
            model
                .resolve_in_scope_chain(module, SymbolNamespace::Local, "value")
                .expect("resolve"),
            Some(module_value)
        );
    }

    #[test]
    fn symbol_model_preserves_source_provenance() {
        let mut model = SymbolModel::default();
        let symbol = model
            .declare_symbol(
                model.global_scope(),
                SymbolNamespace::Project,
                "Book1",
                provenance(3, 8),
            )
            .expect("project declaration");
        assert_eq!(
            model.symbol(symbol).map(|symbol| &symbol.provenance),
            Some(&SourceProvenance {
                module_name: Some("Module1".to_string()),
                span: Some(FrontendSourceSpan { start: 3, end: 8 }),
            })
        );
    }

    #[test]
    fn symbol_model_collects_procedure_params_and_locals_from_cst() {
        let source = "Sub Main(ByVal Value As Long)\n    Dim Result As Long\nEnd Sub\n";
        let model = build_symbol_model_from_source("Module1", source).expect("symbol model");
        let module_scope = model
            .scopes()
            .iter()
            .find(|scope| scope.kind == ScopeKind::Module)
            .map(|scope| scope.id)
            .expect("module scope");
        let procedure_scope = model
            .scopes()
            .iter()
            .find(|scope| scope.kind == ScopeKind::Procedure)
            .map(|scope| scope.id)
            .expect("procedure scope");

        assert!(
            model
                .find_in_scope(module_scope, SymbolNamespace::Procedure, "main")
                .expect("lookup")
                .is_some(),
            "expected procedure symbol from CST"
        );
        assert!(
            model
                .find_in_scope(procedure_scope, SymbolNamespace::Parameter, "value")
                .expect("lookup")
                .is_some(),
            "expected parameter symbol from CST"
        );
        assert!(
            model
                .find_in_scope(procedure_scope, SymbolNamespace::Local, "result")
                .expect("lookup")
                .is_some(),
            "expected local symbol from CST"
        );
    }

    #[test]
    fn symbol_model_cst_collection_preserves_spans_and_case_folding() {
        let source = "Function AddItem(ByVal ITEM As Long) As Long\n    Dim [Total Value] As Long\nEnd Function\n";
        let model = build_symbol_model_from_source("MathModule", source).expect("symbol model");
        let proc_scope = model
            .scopes()
            .iter()
            .find(|scope| scope.kind == ScopeKind::Procedure)
            .map(|scope| scope.id)
            .expect("procedure scope");

        let param = model
            .resolve_in_scope_chain(proc_scope, SymbolNamespace::Parameter, "item")
            .expect("resolve")
            .expect("parameter");
        let local = model
            .resolve_in_scope_chain(proc_scope, SymbolNamespace::Local, "TOTAL VALUE")
            .expect("resolve")
            .expect("bracketed local");

        assert_eq!(
            model
                .symbol(param)
                .and_then(|symbol| symbol.provenance.span),
            Some(FrontendSourceSpan { start: 23, end: 27 })
        );
        assert_eq!(
            model
                .symbol(local)
                .and_then(|symbol| symbol.provenance.span),
            Some(FrontendSourceSpan { start: 53, end: 66 })
        );
    }

    #[test]
    fn symbol_model_cst_collection_rejects_duplicate_locals() {
        let source = "Sub Main()\n    Dim Value As Long\n    Dim value As Long\nEnd Sub\n";
        let err = build_symbol_model_from_source("Module1", source)
            .expect_err("duplicate local should be reported");
        assert!(matches!(
            err,
            SymbolModelError::DuplicateSymbol {
                namespace: SymbolNamespace::Local,
                ..
            }
        ));
    }
}
