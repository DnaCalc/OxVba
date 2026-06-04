//! The `Provider` trait, the `ResolutionEnvironment`, and the wiring that stands
//! up the full scope chain. `resolve` is the single source-agnostic lookup site:
//! it walks the source scope chain, then the ordered providers — it never
//! branches on source kind.

use oxvba_bundle::ProjectMemberKind;

use crate::binding::{Binding, DispatchRoute};
use crate::manifest::{ProjectReference, SymbolProjectManifest};
use crate::model::{ScopeId, ScopeKind, SymbolId, SymbolImpl, SymbolKind, SymbolModelError, SymbolNamespace, SymbolTable};
use crate::providers::com::ComTypeLibProvider;
use crate::providers::host::HostProvider;
use crate::providers::project::ProjectProvider;
use crate::providers::vba_library::VbaLibraryProvider;
use crate::scanner::{self, ModuleScan};
use crate::signature::{SignatureTable, VarTypeRef};

/// The context a resolution happens in: the innermost source scope and, for
/// member access, the receiver's static type.
#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub scope: ScopeId,
    pub receiver: Option<VarTypeRef>,
}

impl ResolutionContext {
    pub fn at(scope: ScopeId) -> Self {
        Self { scope, receiver: None }
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
}

/// Resolves a typelib reference to its metadata blob. The default impl uses the
/// `oxvba-com` catalog; tests pass a fixture impl for determinism.
pub trait TypeLibResolver {
    fn resolve(&self, request: &oxvba_com::TypeLibResolveRequest) -> Option<oxvba_com::TypeLibMetadataBlob>;
}

/// The default resolver: drive the real `oxvba-com` typelib catalog.
pub struct CatalogTypeLibResolver;

impl TypeLibResolver for CatalogTypeLibResolver {
    fn resolve(&self, request: &oxvba_com::TypeLibResolveRequest) -> Option<oxvba_com::TypeLibMetadataBlob> {
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

pub struct ResolutionEnvironment {
    pub symbols: SymbolTable,
    pub signatures: SignatureTable,
    providers: Vec<Box<dyn Provider>>,
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
        self.providers.iter().find_map(|provider| provider.resolve(name))
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
        self.providers.iter().find_map(|provider| provider.resolve_qualified(parts))
    }

    /// Resolve the default member of `recv`'s type (for `obj` used in value
    /// context). COM: the `[id(0)]` member; project: the `VB_UserMemId = 0` member.
    pub fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        self.providers.iter().find_map(|provider| provider.resolve_default_member(recv))
    }

    pub fn push_provider(&mut self, provider: Box<dyn Provider>) {
        self.providers.push(provider);
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
            SymbolImpl::Declare(declare) => {
                DispatchRoute::Declare { descriptor_id: declare.descriptor_id }
            }
            SymbolImpl::LibraryConst(_) => DispatchRoute::Value,
            // A property resolves (by default, read context) to its Get accessor;
            // the binder selects Let/Set for assignment from the symbol's group.
            SymbolImpl::Property(_) => DispatchRoute::ProjectMember { kind: ProjectMemberKind::PropertyGet },
            SymbolImpl::Signature(_)
            | SymbolImpl::DeclaredType(_)
            | SymbolImpl::None
            | SymbolImpl::ComClass(_)
            | SymbolImpl::Predeclared(_) => match symbol.kind {
                SymbolKind::Procedure | SymbolKind::Function | SymbolKind::Event => {
                    DispatchRoute::ProjectMember { kind: ProjectMemberKind::Method }
                }
                _ => DispatchRoute::Value,
            },
        };
        Binding::new(Some(id), route)
    }
}

fn request_from(reference: &ProjectReference) -> Option<oxvba_com::TypeLibResolveRequest> {
    match reference {
        ProjectReference::TypeLibrary { name, guid, version_major, version_minor, lcid, import_lib } => {
            Some(oxvba_com::TypeLibResolveRequest {
                reference_name: name.clone(),
                requested_coclass: None,
                importlib_hint: import_lib.clone(),
                libid_hint: guid.clone(),
                major_version_hint: *version_major,
                minor_version_hint: *version_minor,
                lcid_hint: *lcid,
            })
        }
        _ => None,
    }
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

    let project_scope =
        symbols.add_scope(ScopeKind::Project, symbols.global_scope(), Some(&manifest.project_name))?;

    let mut next_descriptor_id: u32 = 0;
    let mut active_scans: Vec<ModuleScan> = Vec::new();
    for module in &manifest.modules {
        active_scans.push(scanner::scan_module(
            &mut symbols,
            &mut signatures,
            &mut next_descriptor_id,
            module,
            project_scope,
        )?);
    }

    let mut referenced_scans: Vec<ModuleScan> = Vec::new();
    for referenced in &manifest.reference_projects {
        let scope = symbols.add_scope(
            ScopeKind::ReferencedProject,
            symbols.global_scope(),
            Some(&referenced.project_name),
        )?;
        for module in &referenced.modules {
            referenced_scans.push(scanner::scan_module(
                &mut symbols,
                &mut signatures,
                &mut next_descriptor_id,
                module,
                scope,
            )?);
        }
    }

    let project_provider = ProjectProvider::build(&symbols, &active_scans, &referenced_scans);

    let mut com_providers: Vec<ComTypeLibProvider> = Vec::new();
    let mut host_blobs: Vec<oxvba_com::TypeLibMetadataBlob> = Vec::new();
    for reference in &manifest.references {
        if let Some(request) = request_from(reference) {
            if let Some(blob) = typelibs.resolve(&request) {
                com_providers.push(ComTypeLibProvider::new(blob));
            }
        }
        if let ProjectReference::HostInjected { referenced_project_name } = reference {
            // A host-injected reference that names a registered typelib contributes
            // the host object model (Application/ThisWorkbook) via the same path.
            let request = oxvba_com::TypeLibResolveRequest {
                reference_name: referenced_project_name.clone(),
                requested_coclass: None,
                importlib_hint: None,
                libid_hint: None,
                major_version_hint: None,
                minor_version_hint: None,
                lcid_hint: None,
            };
            if let Some(blob) = typelibs.resolve(&request) {
                host_blobs.push(blob);
            }
        }
    }

    // Chain order: project (sibling + referenced) → VBA library → host → COM typelibs.
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    providers.push(Box::new(project_provider));
    providers.push(Box::new(VbaLibraryProvider));
    providers.push(Box::new(HostProvider::new(host_blobs)));
    for com in com_providers {
        providers.push(Box::new(com));
    }

    Ok(ResolutionEnvironment { symbols, signatures, providers })
}
