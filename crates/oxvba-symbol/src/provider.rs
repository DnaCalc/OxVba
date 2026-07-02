//! The `Provider` trait, the `ResolutionEnvironment`, and the wiring that stands
//! up the full scope chain. `resolve` is the single source-agnostic lookup site:
//! it walks the source scope chain, then the ordered providers — it never
//! branches on source kind.

use oxvba_bundle::ProjectMemberKind;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use oxvba_syntax::{Parse, SyntaxKind, SyntaxNode};

use crate::binding::{Binding, DispatchRoute};
use crate::cond_comp;
use crate::manifest::{ModuleKind, ProjectReference, SymbolProjectManifest};
use crate::model::{
    ScopeId, ScopeKind, SymbolId, SymbolImpl, SymbolKind, SymbolModelError, SymbolNamespace,
    SymbolTable, Visibility, fold_identifier,
};
use crate::providers::com::ComTypeLibProvider;
use crate::providers::host::HostProvider;
use crate::providers::project::ProjectProvider;
use crate::providers::surface_provider::SurfaceProvider;
use crate::providers::vba_library::VbaLibraryProvider;
use crate::scanner::{self, ModuleScan};
use crate::signature::{SignatureTable, VarTypeRef};
use crate::surface::{ProjectExportSurface, synthesize_export_surface};

/// The context a resolution happens in: the innermost source scope and, for
/// member access, the receiver's static type.
#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub scope: ScopeId,
    pub receiver: Option<VarTypeRef>,
}

impl ResolutionContext {
    pub fn at(scope: ScopeId) -> Self {
        Self {
            scope,
            receiver: None,
        }
    }
}

/// A non-source scope filler. Each provider owns one link of the chain and is
/// queried by the environment; adding a source is adding one of these.
pub trait Provider {
    /// Resolve a bare name in this provider's scope.
    fn resolve(&self, name: &str) -> Option<Binding> {
        let _ = name;
        None
    }
    /// True when this provider owns more than one equally viable bare-name
    /// candidate and lookup must stop with a VBA-style ambiguous-name diagnostic.
    fn has_ambiguous_unqualified_name(&self, name: &str) -> bool {
        let _ = name;
        false
    }
    /// Resolve `recv.name` against this provider (member access).
    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let _ = (recv, name, want);
        None
    }
    /// Resolve a 1/2/3-part qualified name (`Module.Member`, `Project.Module.Member`).
    fn resolve_qualified(&self, parts: &[&str]) -> Option<Binding> {
        let _ = parts;
        None
    }
    /// Resolve the default member of `recv`'s type, if this provider owns it.
    fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        let _ = recv;
        None
    }
    /// Resolve the default member of `recv`'s type for a specific call-site
    /// accessor. `None` is a read-context lookup and should not pick a write-only
    /// descriptor when a provider can distinguish accessors.
    fn resolve_default_member_kind(
        &self,
        recv: &VarTypeRef,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let binding = self.resolve_default_member(recv)?;
        match (want, &binding.route) {
            (None, _) => Some(binding),
            (Some(kind), crate::binding::DispatchRoute::ProjectMember { kind: have })
            | (Some(kind), crate::binding::DispatchRoute::ExternMember { kind: have, .. })
            | (
                Some(kind),
                crate::binding::DispatchRoute::ComMember {
                    member_kind: have, ..
                },
            ) if kind == *have => Some(binding),
            _ => None,
        }
    }
    /// Enumerate the source events of `recv`'s type, if this provider owns it, as
    /// `(folded event name, event token/dispid)`. The token is the value the
    /// runtime keys subscriptions on (it is what `subscribe_event` is called with),
    /// so a `WithEvents` sink built from this list routes a delivered callback for
    /// that dispid to its `<field>_<EventName>` handler. Returns `None` when this
    /// provider does not own the type (so the chain can keep searching).
    fn source_events(&self, recv: &VarTypeRef) -> Option<Vec<(String, i32)>> {
        let _ = recv;
        None
    }
    /// If `name` is a creatable COM coclass this provider owns, return its
    /// activation ProgID (so `New <coclass>` can lower to `CreateObject`).
    fn resolve_coclass(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
    /// True when `type_name` is a statically known object type owned by this
    /// provider. A missing member on such a type is an error surface, not an
    /// untyped late-bound dispatch fallback.
    fn is_known_object_type(&self, type_name: &str) -> bool {
        let _ = type_name;
        false
    }
    /// If `name` is a creatable coclass published by a *referenced project*, return
    /// its `(unit, class)` names so `New Lib.Widget` (or bare `New Widget`) lowers
    /// to a cross-bundle `NewExtern` against that project's bundle.
    fn resolve_extern_coclass(&self, name: &str) -> Option<(String, String)> {
        let _ = name;
        None
    }
    /// If `name` is a `VB_PredeclaredId` class published by a *referenced project*,
    /// return its `(unit, class)` names so a bare reference to the class name lowers
    /// to a cross-bundle `PredeclaredExtern` (the `ThisWorkbook` document-instance
    /// shape) against that project's bundle.
    fn resolve_extern_predeclared(&self, name: &str) -> Option<(String, String)> {
        let _ = name;
        None
    }
}

/// Resolves a typelib reference to its metadata blob. The default impl uses the
/// `oxvba-com` catalog; tests pass a fixture impl for determinism.
pub trait TypeLibResolver {
    fn resolve(
        &self,
        request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob>;
}

/// The default resolver: drive the real `oxvba-com` typelib catalog.
pub struct CatalogTypeLibResolver;

impl TypeLibResolver for CatalogTypeLibResolver {
    fn resolve(
        &self,
        request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        let identity = oxvba_com::resolve_known_typelib_identity(request)?;
        Some(oxvba_com::build_typelib_metadata(&identity))
    }
}

const NAMESPACE_PRIORITY: &[SymbolNamespace] = &[
    SymbolNamespace::Local,
    SymbolNamespace::Parameter,
    SymbolNamespace::Procedure,
    SymbolNamespace::Member,
    SymbolNamespace::Type,
    SymbolNamespace::Module,
    SymbolNamespace::Project,
    SymbolNamespace::Library,
];

/// One module's parsed CST, retained on the environment so the binder lowers the
/// **same** tree the scanner resolved against — parsed once. Both active and
/// referenced-project modules are retained: the binder lowers every project's
/// bodies into the single bundle (cross-project references are devirtualized into
/// one image), so it needs every CST, tagged with its origin.
pub struct ModuleCst {
    pub project_name: String,
    pub module_name: String,
    pub module_scope: ScopeId,
    pub parse: Parse,
    /// `true` for the active project; `false` for a referenced project.
    pub is_active: bool,
    pub module_kind: ModuleKind,
}

/// A borrowed view of a retained module CST (the `parse.syntax()` red root).
pub struct ModuleCstRef<'a> {
    pub module_name: &'a str,
    pub module_scope: ScopeId,
    pub syntax: SyntaxNode<'a>,
    pub is_active: bool,
    pub module_kind: ModuleKind,
}

pub struct ResolutionEnvironment {
    pub symbols: SymbolTable,
    pub signatures: SignatureTable,
    providers: Vec<Box<dyn Provider>>,
    /// Module CSTs (parsed once during build): active project first (manifest
    /// order), then each referenced project (reference order). `modules()` yields
    /// the active ones; `all_modules()` yields every module.
    module_csts: Vec<ModuleCst>,
    /// Synthesized export surfaces — the active project's, then each referenced
    /// project's (same order as `manifest.reference_projects`). The binder reads
    /// these to build per-class dispid→method dispatch maps (WS-H).
    surfaces: Vec<ProjectExportSurface>,
    /// Folded compile-time `Const`/`Enum`-member values for the whole closure
    /// (active + referenced), keyed by `SymbolId`. Const-folding is a published
    /// type-system property, computed once here and read uniformly by the binder.
    const_values: std::collections::HashMap<SymbolId, oxvba_bundle::coreir::CoreConst>,
    /// Folded `Optional` parameter defaults, keyed by `(procedure symbol, parameter
    /// index)`. The binder substitutes these for omitted optional arguments.
    optional_defaults:
        std::collections::HashMap<(SymbolId, usize), oxvba_bundle::coreir::CoreConst>,
    /// User-defined `Type` (UDT) field tables: folded type name → ordered
    /// `(folded field name, field type)`. The binder reads field index + type for
    /// `p.X` access and the field count for record allocation.
    udt_fields: HashMap<String, Vec<(String, crate::signature::VarTypeRef)>>,
    /// Folded declared enum type names, including module/project-qualified aliases.
    enum_types: HashSet<String>,
    /// Folded unqualified type names that have multiple public type-space owners,
    /// keyed by folded project name.
    ambiguous_type_names: HashMap<String, HashSet<String>>,
}

impl ResolutionEnvironment {
    /// The single source-agnostic lookup site: source scope chain, then providers
    /// in chain order (first hit wins → positional shadowing).
    pub fn resolve(&self, ctx: &ResolutionContext, name: &str) -> Option<Binding> {
        for ns in NAMESPACE_PRIORITY {
            if let Ok(Some(symbol)) = self.symbols.resolve_in_scope_chain(ctx.scope, *ns, name) {
                return Some(self.binding_for_symbol(symbol));
            }
        }
        for provider in &self.providers {
            if provider.has_ambiguous_unqualified_name(name) {
                return None;
            }
            if let Some(binding) = provider.resolve(name) {
                return Some(binding);
            }
        }
        None
    }

    /// True when an ordered provider would reject `name` as ambiguous after the
    /// source scope chain fails to resolve it.
    pub fn has_ambiguous_unqualified_name(&self, name: &str) -> bool {
        self.providers
            .iter()
            .any(|provider| provider.has_ambiguous_unqualified_name(name))
    }

    /// The activation ProgID for a creatable COM coclass name (for `New <coclass>`).
    pub fn resolve_coclass(&self, name: &str) -> Option<String> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_coclass(name))
    }

    pub fn is_known_object_type(&self, type_name: &str) -> bool {
        self.providers
            .iter()
            .any(|provider| provider.is_known_object_type(type_name))
    }

    /// The `(unit, class)` of a creatable coclass published by a referenced project
    /// (for `New Lib.Widget` / bare `New Widget` → a cross-bundle `NewExtern`).
    pub fn resolve_extern_coclass(&self, name: &str) -> Option<(String, String)> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_extern_coclass(name))
    }

    /// The `(unit, class)` of a `VB_PredeclaredId` class published by a referenced
    /// project (for a bare class-name reference → a cross-bundle `PredeclaredExtern`).
    pub fn resolve_extern_predeclared(&self, name: &str) -> Option<(String, String)> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_extern_predeclared(name))
    }

    /// Resolve `recv.name` (member access) against the providers.
    pub fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_member(recv, name, want))
    }

    /// Resolve a qualified name (`Module.Member` / `Project.Module.Member`).
    pub fn resolve_qualified(&self, parts: &[&str]) -> Option<Binding> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_qualified(parts))
    }

    /// Resolve the default member of `recv`'s type (for `obj` used in value
    /// context). COM: the `[id(0)]` member; project: the `VB_UserMemId = 0` member.
    pub fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_default_member(recv))
    }

    /// Resolve the default member for a specific accessor. `None` is read context.
    pub fn resolve_default_member_kind(
        &self,
        recv: &VarTypeRef,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        self.providers
            .iter()
            .find_map(|provider| provider.resolve_default_member_kind(recv, want))
    }

    /// Enumerate the source events of a COM coclass named `source_name` (the
    /// declared type of a `WithEvents` field), as `(folded event name, event
    /// token/dispid)`. Used by the binder to build `EventRoute`s for a COM-source
    /// `WithEvents` sink. Returns `None` when no provider owns the type (e.g. it is
    /// an active/referenced project class, handled by the binder's own branches).
    pub fn com_source_events(&self, source_name: &str) -> Option<Vec<(String, i32)>> {
        let recv = VarTypeRef::Object(source_name.to_string());
        self.providers
            .iter()
            .find_map(|provider| provider.source_events(&recv))
    }

    pub fn push_provider(&mut self, provider: Box<dyn Provider>) {
        self.providers.push(provider);
    }

    /// The active-project module CSTs, parsed once during build. The binder walks
    /// these (no re-parse) to lower each module against the resolution it shares.
    pub fn modules(&self) -> impl Iterator<Item = ModuleCstRef<'_>> {
        self.all_modules().filter(|m| m.is_active)
    }

    /// Every retained module CST — active project first, then each referenced
    /// project (reference order). The binder lowers all of them into the single
    /// bundle (referenced bodies are devirtualization targets).
    pub fn all_modules(&self) -> impl Iterator<Item = ModuleCstRef<'_>> {
        self.module_csts.iter().map(|m| ModuleCstRef {
            module_name: &m.module_name,
            module_scope: m.module_scope,
            syntax: m.parse.syntax(),
            is_active: m.is_active,
            module_kind: m.module_kind,
        })
    }

    /// The synthesized export surfaces (active first, then referenced projects).
    pub fn export_surfaces(&self) -> &[ProjectExportSurface] {
        &self.surfaces
    }

    /// The folded compile-time value of a `Const` / `Enum` member symbol (anywhere
    /// in the closure), if it folded to a constant.
    pub fn const_value(&self, sym: SymbolId) -> Option<&oxvba_bundle::coreir::CoreConst> {
        self.const_values.get(&sym)
    }

    /// The folded default of optional parameter `index` of procedure `proc`, if it
    /// has an explicit (foldable) default expression.
    pub fn optional_default(
        &self,
        proc: SymbolId,
        index: usize,
    ) -> Option<&oxvba_bundle::coreir::CoreConst> {
        self.optional_defaults.get(&(proc, index))
    }

    /// Is `name` (any case) a user-defined `Type`?
    pub fn is_udt(&self, name: &str) -> bool {
        self.udt_fields
            .contains_key(&crate::model::fold_identifier(name))
    }

    /// Is `name` (any case) a declared enum type?
    pub fn is_enum_type(&self, name: &str) -> bool {
        self.enum_types
            .contains(&crate::model::fold_identifier(name))
    }

    /// Is an unqualified type reference ambiguous across public type-space owners?
    pub fn has_ambiguous_type_name(&self, name: &str) -> bool {
        let folded = crate::model::fold_identifier(name);
        self.ambiguous_type_names
            .values()
            .any(|names| names.contains(&folded))
    }

    /// Is `name` (any case) an active or referenced VBA project name?
    pub fn is_project_name(&self, name: &str) -> bool {
        let target = crate::model::fold_identifier(name);
        self.symbols.scopes().iter().any(|scope| {
            matches!(
                scope.kind,
                ScopeKind::Project | ScopeKind::ReferencedProject
            ) && scope
                .name
                .and_then(|id| self.symbols.name(id))
                .is_some_and(|scope_name| scope_name.folded == target)
        })
    }

    /// The field count of UDT `name` (for record allocation).
    pub fn udt_field_count(&self, name: &str) -> Option<usize> {
        self.udt_fields
            .get(&crate::model::fold_identifier(name))
            .map(Vec::len)
    }

    /// All fields of UDT `name` in declaration order, as (folded name, type) —
    /// for recursively default-initializing nested-UDT fields as records.
    pub fn udt_field_list(&self, name: &str) -> Option<&[(String, crate::signature::VarTypeRef)]> {
        self.udt_fields
            .get(&crate::model::fold_identifier(name))
            .map(Vec::as_slice)
    }

    /// The (index, type) of field `field` in UDT `name`, for `p.<field>` access.
    pub fn udt_field(
        &self,
        name: &str,
        field: &str,
    ) -> Option<(usize, &crate::signature::VarTypeRef)> {
        let fields = self.udt_fields.get(&crate::model::fold_identifier(name))?;
        let folded = crate::model::fold_identifier(field);
        fields
            .iter()
            .enumerate()
            .find_map(|(i, (f, ty))| (*f == folded).then_some((i, ty)))
    }

    /// Find a module scope by (case-insensitive) name — convenience for callers
    /// that resolve names as seen from inside a given module.
    pub fn module_scope(&self, name: &str) -> Option<ScopeId> {
        let target = crate::model::fold_identifier(name);
        self.symbols.scopes().iter().find_map(|scope| {
            if scope.kind != ScopeKind::Module {
                return None;
            }
            let scope_name = scope.name.and_then(|id| self.symbols.name(id))?;
            (scope_name.folded == target).then_some(scope.id)
        })
    }

    fn binding_for_symbol(&self, id: SymbolId) -> Binding {
        let Some(symbol) = self.symbols.symbol(id) else {
            return Binding::new(Some(id), DispatchRoute::Unresolved);
        };
        let route = match &symbol.imp {
            SymbolImpl::Native(native) => DispatchRoute::Native(*native),
            SymbolImpl::Structural(structural) => DispatchRoute::Structural(*structural),
            SymbolImpl::Declare(declare) => DispatchRoute::Declare {
                descriptor_id: declare.descriptor_id,
            },
            // A property resolves (by default, read context) to its Get accessor;
            // the binder selects Let/Set for assignment from the symbol's group.
            SymbolImpl::Property(_) => DispatchRoute::ProjectMember {
                kind: ProjectMemberKind::PropertyGet,
            },
            SymbolImpl::Signature(_)
            | SymbolImpl::DeclaredType(_)
            | SymbolImpl::None
            | SymbolImpl::ComClass(_)
            | SymbolImpl::Predeclared(_) => match symbol.kind {
                SymbolKind::Procedure | SymbolKind::Function | SymbolKind::Event => {
                    DispatchRoute::ProjectMember {
                        kind: ProjectMemberKind::Method,
                    }
                }
                _ => DispatchRoute::Value,
            },
        };
        Binding::new(Some(id), route)
    }
}

fn request_from(reference: &ProjectReference) -> Option<oxvba_com::TypeLibResolveRequest> {
    match reference {
        ProjectReference::TypeLibrary {
            name,
            guid,
            version_major,
            version_minor,
            lcid,
            import_lib,
        } => Some(oxvba_com::TypeLibResolveRequest {
            reference_name: name.clone(),
            requested_coclass: None,
            importlib_hint: import_lib.clone(),
            libid_hint: guid.clone(),
            major_version_hint: *version_major,
            minor_version_hint: *version_minor,
            lcid_hint: *lcid,
        }),
        _ => None,
    }
}

fn resolve_typelib_provider_closure(
    typelibs: &dyn TypeLibResolver,
    request: oxvba_com::TypeLibResolveRequest,
    used_type_names: &HashSet<String>,
) -> Vec<oxvba_com::TypeLibMetadataBlob> {
    // VBA sees a referenced typelib's returned interfaces as part of the same
    // referenced library. Follow named object-return descriptors so a chain like
    // `Application.Workbooks.Count` does not need a second synthetic reference.
    let mut blobs = Vec::new();
    let mut queue = VecDeque::from([request]);
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();

    while let Some(request) = queue.pop_front() {
        let key = (
            fold_identifier(&request.reference_name),
            request
                .requested_coclass
                .as_ref()
                .map(|name| fold_identifier(name)),
        );
        if !seen.insert(key) {
            continue;
        }

        let Some(blob) = typelibs.resolve(&request) else {
            continue;
        };

        let reference_name = blob.identity.reference_name.clone();
        let importlib_hint = request.importlib_hint.clone();
        let libid_hint = request.libid_hint.clone();
        let major_version_hint = request.major_version_hint;
        let minor_version_hint = request.minor_version_hint;
        let lcid_hint = request.lcid_hint;

        for interface_name in returned_interface_names(&blob)
            .into_iter()
            .chain(used_coclass_names(&blob, used_type_names))
        {
            queue.push_back(oxvba_com::TypeLibResolveRequest {
                reference_name: reference_name.clone(),
                requested_coclass: Some(interface_name),
                importlib_hint: importlib_hint.clone(),
                libid_hint: libid_hint.clone(),
                major_version_hint,
                minor_version_hint,
                lcid_hint,
            });
        }

        blobs.push(blob);
    }

    blobs
}

fn returned_interface_names(blob: &oxvba_com::TypeLibMetadataBlob) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for member in &blob.members {
        if let Some(oxvba_com::TypeLibWireType::InterfacePointer { name }) =
            &member.return_wire_type
        {
            let trimmed = name.trim();
            if !trimmed.is_empty() && seen.insert(fold_identifier(trimmed)) {
                names.push(trimmed.to_string());
            }
        }
    }
    names
}

fn used_coclass_names(
    blob: &oxvba_com::TypeLibMetadataBlob,
    used_type_names: &HashSet<String>,
) -> Vec<String> {
    let reference = fold_identifier(&blob.identity.reference_name);
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for coclass in &blob.coclass_names {
        let folded = fold_identifier(coclass);
        if (used_type_names.contains(&folded)
            || used_type_names.contains(&format!("{reference}.{folded}")))
            && seen.insert(folded)
        {
            names.push(coclass.clone());
        }
    }
    names
}

#[derive(Default)]
struct TypeNameIndex {
    udt_fields: HashMap<String, Vec<(String, VarTypeRef)>>,
    enum_types: HashSet<String>,
    public_candidates: BTreeMap<String, BTreeMap<String, Vec<TypeNameCandidate>>>,
}

#[derive(Clone, Copy)]
struct TypeNameCandidate {
    owner: SymbolId,
}

impl TypeNameIndex {
    fn add_module(
        &mut self,
        project_name: &str,
        module_name: &str,
        scan: &ModuleScan,
        syntax: SyntaxNode<'_>,
    ) {
        let module_udts = scanner::collect_udt_fields(&[syntax]);
        let module_folded = crate::model::fold_identifier(module_name);
        let project_folded = crate::model::fold_identifier(project_name);
        for member in &scan.members {
            if !matches!(member.kind, SymbolKind::Type | SymbolKind::Enum) {
                continue;
            }
            let name = member.name_folded.clone();
            let aliases = [
                name.clone(),
                format!("{module_folded}.{name}"),
                format!("{project_folded}.{module_folded}.{name}"),
            ];
            match member.kind {
                SymbolKind::Type => {
                    if let Some(fields) = module_udts.get(&name) {
                        for alias in aliases {
                            self.udt_fields
                                .entry(alias)
                                .or_insert_with(|| fields.clone());
                        }
                    }
                }
                SymbolKind::Enum => {
                    for alias in aliases {
                        self.enum_types.insert(alias);
                    }
                }
                _ => {}
            }
            if member.visibility == Visibility::Public {
                self.public_candidates
                    .entry(project_folded.clone())
                    .or_default()
                    .entry(name)
                    .or_default()
                    .push(TypeNameCandidate {
                        owner: scan.module_symbol,
                    });
            }
        }
    }

    fn ambiguous_type_names(&self) -> HashMap<String, HashSet<String>> {
        self.public_candidates
            .iter()
            .filter_map(|(project_name, candidates_by_name)| {
                let names: HashSet<String> = candidates_by_name
                    .iter()
                    .filter(|(_, candidates)| has_competing_type_owners(candidates))
                    .map(|(name, _)| name.clone())
                    .collect();
                (!names.is_empty()).then(|| (project_name.clone(), names))
            })
            .collect()
    }
}

fn has_competing_type_owners(candidates: &[TypeNameCandidate]) -> bool {
    let Some(first) = candidates.first() else {
        return false;
    };
    candidates
        .iter()
        .skip(1)
        .any(|candidate| candidate.owner != first.owner)
}

fn validate_type_refs(
    modules: &[ModuleCst],
    ambiguous_type_names: &HashMap<String, HashSet<String>>,
) -> Result<(), SymbolModelError> {
    for module in modules {
        let project_name = crate::model::fold_identifier(&module.project_name);
        if let Some(names) = ambiguous_type_names.get(&project_name) {
            validate_type_refs_in(module.parse.syntax(), names)?;
        }
    }
    Ok(())
}

fn collect_type_ref_names(modules: &[ModuleCst]) -> HashSet<String> {
    let mut names = HashSet::new();
    for module in modules {
        collect_type_ref_names_in(module.parse.syntax(), &mut names);
    }
    names
}

fn collect_type_ref_names_in(node: SyntaxNode<'_>, names: &mut HashSet<String>) {
    if node.kind() == SyntaxKind::TypeRef {
        let name = type_ref_name(node);
        if !name.is_empty() {
            names.insert(fold_identifier(&name));
        }
    }
    for child in node.child_nodes() {
        collect_type_ref_names_in(child, names);
    }
}

fn validate_type_refs_in(
    node: SyntaxNode<'_>,
    ambiguous_type_names: &HashSet<String>,
) -> Result<(), SymbolModelError> {
    if node.kind() == SyntaxKind::TypeRef {
        let name = type_ref_name(node);
        if !name.contains('.')
            && ambiguous_type_names.contains(&crate::model::fold_identifier(&name))
        {
            return Err(SymbolModelError::AmbiguousName { name });
        }
    }
    for child in node.child_nodes() {
        validate_type_refs_in(child, ambiguous_type_names)?;
    }
    Ok(())
}

fn type_ref_name(node: SyntaxNode<'_>) -> String {
    let text = node.text();
    let s = text.trim_start();
    let name = if s
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("new"))
        && s.get(3..)
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
    {
        s.get(3..).unwrap_or("").trim_start()
    } else {
        s
    };
    name.split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c| c == '[' || c == ']')
        .to_string()
}

/// Stand up the full resolution environment: scan the active project's modules
/// and referenced projects into the table, build the project / VBA-library /
/// host / COM providers in chain order, resolving each COM reference's typelib
/// via `typelibs` (so COM resolution is owned here, driven by the manifest's own
/// reference detail).
pub fn build_resolution_environment(
    manifest: &SymbolProjectManifest,
    typelibs: &dyn TypeLibResolver,
) -> Result<ResolutionEnvironment, SymbolModelError> {
    let mut symbols = SymbolTable::new();
    let mut signatures = SignatureTable::new();

    let project_scope = symbols.add_scope(
        ScopeKind::Project,
        symbols.global_scope(),
        Some(&manifest.project_name),
    )?;

    let mut next_descriptor_id: u32 = 0;
    let mut active_scans: Vec<ModuleScan> = Vec::new();
    // Conditional-compilation environment for the active project: predefined target
    // constants + the project's `DefineConstants`. Each module's `#If`/`#Const`
    // directives are evaluated here and inactive branches stripped before parse.
    let active_target = manifest.conditional_compilation_target;
    let active_cc =
        cond_comp::base_cc_constants_for_target(&manifest.conditional_constants, active_target);
    // Active-project modules: parse once, scan against the parsed CST, and retain
    // the `Parse` so the binder lowers the same tree (no second parse).
    let mut module_csts: Vec<ModuleCst> = Vec::new();
    let mut type_index = TypeNameIndex::default();
    for module in &manifest.modules {
        let source =
            cond_comp::preprocess(&module.source, &active_cc).map_err(SymbolModelError::Syntax)?;
        let parse = oxvba_syntax::parse(&source);
        if !parse.errors().is_empty() {
            return Err(SymbolModelError::Parse {
                module: module.module_name.clone(),
                source_text: source,
                errors: parse.errors().to_vec(),
            });
        }
        let scan = scanner::scan_module(
            &mut symbols,
            &mut signatures,
            &mut next_descriptor_id,
            module,
            parse.syntax(),
            project_scope,
        )?;
        type_index.add_module(
            &manifest.project_name,
            &scan.module_name,
            &scan,
            parse.syntax(),
        );
        module_csts.push(ModuleCst {
            project_name: manifest.project_name.clone(),
            module_name: scan.module_name.clone(),
            module_scope: scan.module_scope,
            parse,
            is_active: true,
            module_kind: module.module_kind,
        });
        active_scans.push(scan);
    }

    // Referenced projects: scan their modules into a ReferencedProject scope AND
    // retain the CSTs — the binder lowers their bodies into the same bundle
    // (cross-project references are devirtualized in-image). The scans are kept so
    // each project's public surface is synthesized *after* the whole closure's
    // consts are folded (the surface publishes folded literal values).
    let mut referenced_scans: Vec<Vec<ModuleScan>> = Vec::new();
    for referenced in &manifest.reference_projects {
        let scope = symbols.add_scope(
            ScopeKind::ReferencedProject,
            symbols.global_scope(),
            Some(&referenced.project_name),
        )?;
        // A referenced project's manifest does not carry its own `DefineConstants`,
        // so its `#If`s evaluate against the same target's predefined constants only.
        let referenced_cc = cond_comp::predefined_cc_constants_for_target(active_target);
        let mut scans: Vec<ModuleScan> = Vec::new();
        for module in &referenced.modules {
            let source = cond_comp::preprocess(&module.source, &referenced_cc)
                .map_err(SymbolModelError::Syntax)?;
            let parse = oxvba_syntax::parse(&source);
            if !parse.errors().is_empty() {
                return Err(SymbolModelError::Parse {
                    module: module.module_name.clone(),
                    source_text: source,
                    errors: parse.errors().to_vec(),
                });
            }
            let scan = scanner::scan_module(
                &mut symbols,
                &mut signatures,
                &mut next_descriptor_id,
                module,
                parse.syntax(),
                scope,
            )?;
            type_index.add_module(
                &referenced.project_name,
                &scan.module_name,
                &scan,
                parse.syntax(),
            );
            module_csts.push(ModuleCst {
                project_name: referenced.project_name.clone(),
                module_name: scan.module_name.clone(),
                module_scope: scan.module_scope,
                parse,
                is_active: false,
                module_kind: module.module_kind,
            });
            scans.push(scan);
        }
        referenced_scans.push(scans);
    }

    // Fold the whole closure's Const/Enum values once (a published type-system
    // property), so each surface publishes its folded literal values and the binder
    // reads constant values uniformly via `const_value`. Folded before any surface
    // is synthesized so a cross-project `Const X = Other.Y` resolves.
    let roots: Vec<(ScopeId, SyntaxNode<'_>)> = module_csts
        .iter()
        .map(|m| (m.module_scope, m.parse.syntax()))
        .collect();
    let const_values = crate::const_eval::fold_const_values(&symbols, &roots);
    // Optional-parameter defaults fold against the closure's const values.
    let optional_defaults =
        crate::const_eval::fold_optional_defaults(&symbols, &roots, &const_values);
    drop(roots);
    let ambiguous_type_names = type_index.ambiguous_type_names();
    validate_type_refs(&module_csts, &ambiguous_type_names)?;
    let used_type_names = collect_type_ref_names(&module_csts);

    // Each referenced project's public surface — a referencing call binds through
    // the same COM contract a compiled component would present.
    let referenced_surfaces: Vec<ProjectExportSurface> = manifest
        .reference_projects
        .iter()
        .zip(referenced_scans.iter())
        .map(|(referenced, scans)| {
            synthesize_export_surface(
                &referenced.project_name,
                &referenced.modules,
                scans,
                &symbols,
                &signatures,
                &const_values,
            )
        })
        .collect();

    // The active project's own surface (consumed by the binder for per-class
    // dispid→method dispatch maps, and by a future COM-server emit).
    let active_surface = synthesize_export_surface(
        &manifest.project_name,
        &manifest.modules,
        &active_scans,
        &symbols,
        &signatures,
        &const_values,
    );
    let mut surfaces = vec![active_surface];
    surfaces.extend(referenced_surfaces.iter().cloned());

    // The cross-module index is the ACTIVE project only — a referenced project is
    // reached through its export-surface provider, not by flattening its modules
    // into the active name index.
    let project_provider =
        ProjectProvider::build(&manifest.project_name, &symbols, &active_scans, &[]);

    let mut com_providers: Vec<ComTypeLibProvider> = Vec::new();
    let mut host_blobs: Vec<oxvba_com::TypeLibMetadataBlob> = Vec::new();
    for reference in &manifest.references {
        if let Some(request) = request_from(reference) {
            com_providers.extend(
                resolve_typelib_provider_closure(typelibs, request, &used_type_names)
                    .into_iter()
                    .map(ComTypeLibProvider::new),
            );
        }
        if let ProjectReference::HostInjected {
            referenced_project_name,
        } = reference
        {
            // A host-injected reference that names a registered typelib contributes
            // the host object model (Application/ThisWorkbook) via the same path.
            let (reference_name, requested_coclass) = referenced_project_name
                .rsplit_once('.')
                .map(|(library, coclass)| (library.to_string(), Some(coclass.to_string())))
                .unwrap_or_else(|| (referenced_project_name.clone(), None));
            let request = oxvba_com::TypeLibResolveRequest {
                reference_name,
                requested_coclass,
                importlib_hint: None,
                libid_hint: None,
                major_version_hint: None,
                minor_version_hint: None,
                lcid_hint: None,
            };
            host_blobs.extend(resolve_typelib_provider_closure(
                typelibs,
                request,
                &used_type_names,
            ));
        }
    }

    // Chain order: active project → referenced-project surfaces → VBA library →
    // host → COM typelibs. Active names shadow referenced ones (active-wins on a
    // collision); a referenced project's surface is consulted before the library.
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    providers.push(Box::new(project_provider));
    for surface in &referenced_surfaces {
        providers.push(Box::new(SurfaceProvider::new(surface.clone())));
    }
    providers.push(Box::new(VbaLibraryProvider));
    providers.push(Box::new(HostProvider::new(host_blobs)));
    for com in com_providers {
        providers.push(Box::new(com));
    }

    Ok(ResolutionEnvironment {
        symbols,
        signatures,
        providers,
        module_csts,
        surfaces,
        const_values,
        optional_defaults,
        udt_fields: type_index.udt_fields,
        enum_types: type_index.enum_types,
        ambiguous_type_names,
    })
}
