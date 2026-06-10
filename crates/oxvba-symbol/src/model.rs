//! The uniform symbol core: interned names, the scope graph, and the symbol
//! table with a source-agnostic scope-chain resolver. Ported from the legacy
//! `frontend_symbols`, generalised so every symbol carries a `SymbolKind` and a
//! binder-facing `SymbolImpl` payload.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::providers::declare::DeclareSymbol;
use crate::signature::{SignatureId, VarTypeRef};
use crate::structural::StructuralIntrinsic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InternedNameId(pub u32);

/// The resolution axis. Kept separate from [`SymbolKind`] so a name may exist in
/// several axes at once (e.g. a `Module` named the same as a `Library` const).
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

/// The concrete kind of a symbol (folds the legacy `SymbolNamespace` finer kinds
/// + `ProjectSymbolKind`). What the binder switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Project,
    Module,
    Class,
    Type,
    Enum,
    EnumMember,
    Procedure,
    Function,
    /// A property — one logical member with up to three accessors (see
    /// [`SymbolImpl::Property`]).
    Property,
    Event,
    Field,
    WithEventsField,
    Parameter,
    Local,
    /// A procedure-local declared `Static` (or any local of a `Static`
    /// procedure): default-initialized once and persisting across calls. Lowered
    /// to a bundle global with a mangled identity, not a frame slot.
    StaticLocal,
    Const,
    LibraryConst,
    LibraryProc,
    StructuralIntrinsic,
    PredeclaredObject,
    ComMember,
    ComEvent,
}

/// The declared visibility of a module-level member, governing **cross-project**
/// (COM export-surface) exposure. Within its own project a member is always
/// resolvable; only `Public` crosses the project boundary — `Private`/`Friend`
/// (and any member of an `Option Private Module` module) do not. VBA's default
/// differs by declaration kind, so it is computed at scan time (see
/// `scanner::decl_visibility`), not assumed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Friend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Library,
    Host,
    ComTypeLib,
    ReferencedProject,
    Project,
    Module,
    Procedure,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceProvenance {
    pub module_name: Option<String>,
    pub span: Option<SourceSpan>,
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

/// A library `vb*` constant value (ported from the legacy library provider).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryConstValue {
    Str(String),
    Int(i32),
}

/// The predeclared base-library objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredeclaredObjectId {
    Debug,
    Err,
    Collection,
}

/// A property's accessor set — one logical member with up to three accessors
/// (`Get`/`Let`/`Set`), resolved as one symbol per the contract.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropertyGroup {
    pub get: Option<SignatureId>,
    pub let_: Option<SignatureId>,
    pub set: Option<SignatureId>,
}

/// The coclass-level payload of a COM class type symbol (member-level dispatch
/// data lives on the resolved [`crate::binding::DispatchRoute`], not here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComClassSymbol {
    pub activation_prog_id: Option<String>,
    /// Which resolved typelib this class came from (its `identity.cache_key`),
    /// so member resolution targets the right provider.
    pub typelib_cache_key: String,
}

/// The binder-facing payload of a resolved symbol — "how its body/value is reached".
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolImpl {
    /// A pure namespace placeholder (project/module/type marker).
    None,
    /// A procedure/event with a parsed signature.
    Signature(SignatureId),
    /// A property's accessor set (Get/Let/Set), merged across declarations.
    Property(PropertyGroup),
    /// A field/local/parameter's declared type.
    DeclaredType(VarTypeRef),
    /// A `vb*` library constant.
    LibraryConst(LibraryConstValue),
    /// A base-library intrinsic body.
    Native(oxvba_bundle::NativeImplId),
    /// A compiler-internal structural intrinsic.
    Structural(StructuralIntrinsic),
    /// A `Declare Lib` external procedure.
    Declare(DeclareSymbol),
    /// A COM coclass type.
    ComClass(ComClassSymbol),
    /// A predeclared base-library object (`Debug`/`Err`/`Collection`).
    Predeclared(PredeclaredObjectId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: InternedNameId,
    pub kind: SymbolKind,
    pub namespace: SymbolNamespace,
    pub scope: ScopeId,
    pub provenance: SourceProvenance,
    pub imp: SymbolImpl,
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
pub struct SymbolTable {
    names: Vec<InternedName>,
    names_by_folded: BTreeMap<String, InternedNameId>,
    scopes: Vec<Scope>,
    symbols: Vec<Symbol>,
    symbols_by_key: BTreeMap<SymbolKey, SymbolId>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        let mut table = Self {
            names: Vec::new(),
            names_by_folded: BTreeMap::new(),
            scopes: Vec::new(),
            symbols: Vec::new(),
            symbols_by_key: BTreeMap::new(),
        };
        table.scopes.push(Scope {
            id: ScopeId(0),
            kind: ScopeKind::Global,
            parent: None,
            name: None,
        });
        table
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn global_scope(&self) -> ScopeId {
        ScopeId(0)
    }

    pub fn intern_name(&mut self, name: &str) -> InternedNameId {
        let folded = fold_identifier(name);
        if let Some(id) = self.names_by_folded.get(&folded) {
            return *id;
        }
        let id = InternedNameId(self.names.len() as u32);
        self.names.push(InternedName {
            first_spelling: name.to_string(),
            folded: folded.clone(),
        });
        self.names_by_folded.insert(folded, id);
        id
    }

    pub fn name(&self, id: InternedNameId) -> Option<&InternedName> {
        self.names.get(id.0 as usize)
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
        let id = ScopeId(self.scopes.len() as u32);
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
            .get(id.0 as usize)
            .ok_or(SymbolModelError::UnknownScope(id))
    }

    pub fn declare_symbol(
        &mut self,
        scope: ScopeId,
        namespace: SymbolNamespace,
        kind: SymbolKind,
        name: &str,
        provenance: SourceProvenance,
        imp: SymbolImpl,
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
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            id,
            name: name_id,
            kind,
            namespace,
            scope,
            provenance,
            imp,
        });
        self.symbols_by_key.insert(key, id);
        Ok(id)
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0 as usize)
    }

    /// Replace a symbol's payload — used to merge property accessors into one
    /// logical member as each `Property Get/Let/Set` is scanned.
    pub fn update_impl(&mut self, id: SymbolId, imp: SymbolImpl) {
        if let Some(symbol) = self.symbols.get_mut(id.0 as usize) {
            symbol.imp = imp;
        }
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

    /// Walk the parent chain from `scope` upward, returning the nearest match.
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
