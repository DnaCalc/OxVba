use std::collections::BTreeMap;

use thiserror::Error;

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
}
