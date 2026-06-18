//! `oxvba-bind` — the binder.
//!
//! A single typed pass that walks the resolved `oxvba-syntax` CST, asks the
//! `oxvba-symbol` resolution environment what each name means, infers types,
//! inserts coercions, and emits the symbol-free `oxvba_bundle::coreir` Core IR
//! that `linearize` turns into a runnable `Bundle`. This ties the clean path
//! together: source → CST → symbol resolution → coreir → linearize → vm2/JIT.

mod call;
mod error;
mod expr;
mod ids;
mod place;
mod proc;
mod stmt;
mod types;

pub use error::BindError;

use std::cell::RefCell;
use std::collections::HashMap;

use oxvba_bundle::coreir::{CorePlace, CoreProc, CoreProgram, CoreValue, LabelId, LocalId, ProcId};
use oxvba_bundle::{
    BundleExport, BundleImport, ComClassExport, EventRoute, ExportTarget, ExportToken,
    ExternalCallDescriptor, ProcedureKind,
};
use oxvba_runtime::DynLinkSymbol;
use oxvba_symbol::binding::Binding;
use oxvba_symbol::manifest::{ModuleKind, SymbolProjectManifest};
use oxvba_symbol::model::{
    ScopeId, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace, fold_identifier,
};
use oxvba_symbol::provider::{ResolutionContext, ResolutionEnvironment};
use oxvba_symbol::signature::VarTypeRef;
use oxvba_symbol::surface::{MemberOrigin, SurfaceType, SurfaceTypeKind};
use oxvba_symbol::{TypeLibResolver, build_resolution_environment};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::ids::{IdAllocator, ProcInfo};

/// Collects the cross-bundle imports a project's binding makes (deduped by
/// `(unit, token)`, case-insensitively). Lives on [`Lower`] behind a `RefCell` so
/// any proc body can `intern` an import during lowering; drained into
/// `CoreProgram.imports` afterward, with each `ExternProc`/`NewExtern` carrying its
/// returned index.
#[derive(Default)]
struct ImportCollector {
    imports: Vec<BundleImport>,
}

impl ImportCollector {
    /// Return the index of `import`, adding it if new (VBA-case-insensitive match).
    fn intern(&mut self, import: BundleImport) -> usize {
        if let Some(i) = self.imports.iter().position(|e| {
            e.unit.eq_ignore_ascii_case(&import.unit) && e.token.matches(&import.token)
        }) {
            return i;
        }
        self.imports.push(import);
        self.imports.len() - 1
    }
}

/// Bind a whole project: parse (once) + resolve + lower every active module's
/// CST to symbol-free Core IR.
pub fn bind_program(
    manifest: &SymbolProjectManifest,
    typelibs: &dyn TypeLibResolver,
) -> Result<CoreProgram, BindError> {
    let env = build_resolution_environment(manifest, typelibs)?;
    bind_one(&env, manifest)
}

/// Bind every referenced project plus the active project — the **whole transitive
/// closure**, leaf-first — into one `CoreProgram` per project (each a bundle). Each
/// project is bound from its own full [`SymbolProjectManifest`] (carrying its
/// flattened transitive reference source). The caller links them with
/// `oxvba_vm2::Vm::link` (entry last); `Vm::link` resolves each import by unit name
/// against any loaded bundle and rejects a duplicate unit, so a shared/diamond
/// dependency (`A→B→D`, `A→C→D`) appears — and links — exactly once.
///
/// `closure_leaf_first` lists every project in the closure in leaf-first order,
/// **deduped by unit name** (a project that several others reference is bound once).
/// The active (entry) project is last.
pub fn bind_projects(
    closure_leaf_first: &[SymbolProjectManifest],
    typelibs: &dyn TypeLibResolver,
) -> Result<Vec<CoreProgram>, BindError> {
    // The closure is already deduped by project FILE (path); two entries sharing a
    // unit NAME are therefore two *different* projects colliding on a name — bind
    // none silently (it would drop a project's bundle entirely, and `Vm::link`
    // rejects duplicate unit names anyway). Error loudly instead.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut programs = Vec::with_capacity(closure_leaf_first.len());
    for manifest in closure_leaf_first {
        if !seen.insert(fold_identifier(&manifest.project_name)) {
            return Err(BindError::Malformed(format!(
                "duplicate project unit name `{}` in the reference closure",
                manifest.project_name
            )));
        }
        let env = build_resolution_environment(manifest, typelibs)?;
        programs.push(bind_one(&env, manifest)?);
    }
    Ok(programs)
}

/// Bind one project (already-built environment) to its `CoreProgram` bundle.
fn bind_one(
    env: &ResolutionEnvironment,
    manifest: &SymbolProjectManifest,
) -> Result<CoreProgram, BindError> {
    let ids = IdAllocator::build(env, manifest)?;
    let lower = Lower {
        env,
        ids: &ids,
        imports: RefCell::new(ImportCollector::default()),
    };

    // Proc decl nodes in the same order `ids.procs` was built (ProcId order). The
    // two are produced by the identical filter; guard against any future drift so
    // a body can never be bound against the wrong frame.
    let decls = collect_proc_decls(env);
    if ids.procs.len() != decls.len() {
        return Err(BindError::Malformed(format!(
            "proc-decl/frame count mismatch: {} frames vs {} decls",
            ids.procs.len(),
            decls.len()
        )));
    }
    let mut procs: Vec<CoreProc> = Vec::with_capacity(ids.procs.len());
    for (info, decl) in ids.procs.iter().zip(decls.iter()) {
        let body = lower.bind_proc_body(info, *decl)?;
        procs.push(CoreProc {
            name: info.name.clone(),
            kind: info.kind,
            params: info.params.clone(),
            locals: info.locals.clone(),
            return_local: info.return_local,
            body,
        });
    }

    // Module-level fixed-size array globals AND every proc's `Static` array/record
    // locals are allocated once per VM session. Keep them in a hidden initializer
    // proc rather than splicing into `Main`, because COM activation-style hosts can
    // enter directly through class members without running the top-level entry proc.
    let init_context = ids
        .entry()
        .or_else(|| (!ids.procs.is_empty()).then_some(ProcId(0)));
    let global_initializer = if let Some(entry_id) = init_context {
        let mut inits = lower.module_global_array_inits(&ids.procs[entry_id.0])?;
        inits.extend(lower.static_local_inits(&decls)?);
        if inits.is_empty() {
            None
        } else {
            let proc = ProcId(procs.len());
            procs.push(CoreProc {
                name: "__vba_global_initialize".to_string(),
                kind: ProcedureKind::Sub,
                params: Vec::new(),
                locals: Vec::new(),
                return_local: None,
                body: inits,
            });
            Some(proc)
        }
    } else {
        None
    };

    // The active project's published surface → this bundle's exports (the contract
    // a referrer's imports resolve against). Imports were accumulated while lowering.
    let exports = build_exports(env, &ids);
    let imports = lower.imports.into_inner().imports;

    Ok(CoreProgram {
        globals: ids.globals.clone(),
        procs,
        classes: ids.classes.clone(),
        event_routes: build_event_routes(env, &ids),
        external_calls: build_external_calls(env),
        com_class_exports: build_com_class_exports(manifest),
        entry: ids.entry(),
        global_initializer,
        unit_name: manifest.project_name.clone(),
        exports,
        imports,
    })
}

/// This bundle's export table: each public coclass → a `Class` token resolving to
/// its `ClassId`; each public hidden-module member → a `ModuleFunc` token resolving
/// to its proc/accessor `ProcId`. Built from the active export surface (the single
/// source of truth for what is published) so source- and compiled-form contracts
/// agree.
fn build_exports(env: &ResolutionEnvironment, ids: &IdAllocator) -> Vec<BundleExport> {
    let Some(active) = env.export_surfaces().first() else {
        return Vec::new();
    };
    let mut exports = Vec::new();
    for ty in &active.types {
        match &ty.kind {
            SurfaceTypeKind::Coclass { .. } => {
                if let Some(&class_id) = ids.class_of.get(&fold_identifier(&ty.name)) {
                    exports.push(BundleExport {
                        token: ExportToken::Class {
                            name: ty.name.clone(),
                        },
                        target: ExportTarget::Class(class_id.0),
                    });
                }
            }
            SurfaceTypeKind::Module => export_module_members(ids, ty, &mut exports),
        }
    }
    exports
}

/// A hidden module's public members → `ModuleFunc` export tokens. A member's
/// `ProcId` comes from `proc_of` (method) or `prop_accessor_of` (a property/field
/// accessor), keyed by its originating symbol + kind.
fn export_module_members(ids: &IdAllocator, ty: &SurfaceType, out: &mut Vec<BundleExport>) {
    for m in &ty.members {
        let proc = match m.origin {
            MemberOrigin::Proc => ids.proc_of.get(&m.symbol).copied(),
            MemberOrigin::PropertyAccessor | MemberOrigin::Field => ids
                .prop_accessor_of
                .get(&(m.symbol, m.member_kind))
                .copied(),
        };
        if let Some(proc) = proc {
            out.push(BundleExport {
                token: ExportToken::ModuleFunc {
                    module: ty.name.clone(),
                    member: m.name.clone(),
                    kind: m.member_kind,
                },
                target: ExportTarget::Proc(proc.0),
            });
        }
    }
}

/// Proc decl nodes across all active modules, in `ProcId` order (mirrors the
/// loop in [`IdAllocator::build`]).
fn collect_proc_decls<'a>(env: &'a ResolutionEnvironment) -> Vec<SyntaxNode<'a>> {
    let mut decls = Vec::new();
    for module in env.modules() {
        for node in module.syntax.child_nodes() {
            if matches!(
                node.kind(),
                SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl
            ) && node.proc_name_token().is_some()
            {
                decls.push(node);
            }
        }
    }
    decls
}

/// Resolve `WithEvents` sinks to event routes: for each `WithEvents` field
/// (binding token `T`, source class `C`) in a sink class, and each event `E` of
/// `C`, a handler proc named `<field>_<event>` in the sink class produces an
/// `EventRoute{ binding: T, event: index(E), handler }`. This is the table vm2
/// consults when a `RaiseEvent` fires (`event_routes[(binding, event)]`).
fn build_event_routes(env: &ResolutionEnvironment, ids: &IdAllocator) -> Vec<EventRoute> {
    let symbols = &env.symbols;
    let module_scope_by_name: HashMap<String, ScopeId> = env
        .modules()
        .map(|m| (fold_identifier(m.module_name), m.module_scope))
        .collect();
    let mut routes = Vec::new();
    for (&field_sym, &binding) in &ids.withevents_binding_of {
        let Some(field) = symbols.symbol(field_sym) else {
            continue;
        };
        let sink_scope = field.scope;
        let Some(field_name) = symbols.name(field.name).map(|n| n.folded.clone()) else {
            continue;
        };
        // The source class is the WithEvents field's declared object type — an
        // active class OR a referenced one (resolved through its export surface).
        let SymbolImpl::DeclaredType(VarTypeRef::Object(source_name)) = &field.imp else {
            continue;
        };
        for (ev_name_folded, event) in source_events(env, ids, &module_scope_by_name, source_name) {
            let handler_name = format!("{field_name}_{ev_name_folded}");
            if let Ok(Some(handler_sym)) =
                symbols.find_in_scope(sink_scope, SymbolNamespace::Procedure, &handler_name)
                && let Some(&proc) = ids.proc_of.get(&handler_sym)
            {
                routes.push(EventRoute {
                    binding,
                    event,
                    handler: proc.0,
                });
            }
        }
    }
    // A COM coclass source reads LIBRARY-WIDE typelib events, so an event name shared
    // across source interfaces (e.g. `SheetChange` on both `AppEvents` and
    // `WorkbookEvents`, which Excel assigns the same dispid) yields one route per
    // occurrence — all with an identical `(binding, event, handler)`. Collapse those
    // duplicates so a `WithEvents` field is advised once per (binding, event).
    //
    // NOTE: `EventRoute` carries no connection-point IID, so this dedup cannot choose
    // BETWEEN distinct source interfaces that share a dispid — it only removes exact
    // duplicates. The runtime selects the actual connection point from the bound object's
    // own (coclass-scoped) `event_specs`. That is correct for the common single-source case
    // and for same-named shared-dispid events (identical handler => genuinely one route);
    // a coclass exposing two DIFFERENTLY-named events on one dispid would need the IID
    // carried here to disambiguate — see `event_specs_from_typelib_metadata`.
    let mut seen = std::collections::HashSet::new();
    routes.retain(|route| seen.insert((route.binding, route.event, route.handler)));
    routes
}

/// The `(folded event name, event id)` list of a WithEvents source class. An
/// **active** source reads its `Event` symbols + `ids.event_index_of`; a
/// **referenced** source reads its export surface's `SurfaceEvent`s (whose
/// `event_id` equals the source bundle's own `event_index_of`, so a sink built here
/// routes the same id the source's `RaiseEvent` fires). A **COM coclass** source
/// (a referenced typelib coclass, e.g. `OxVba.TestEventServer`) reads the typelib's
/// source events via the `ComTypeLibProvider`, whose `event id` is the event's
/// dispid/token — exactly the value `subscribe_com_events` passes to
/// `subscribe_event`, so a delivered callback for that dispid routes to the handler.
/// `source_name` may be project-qualified (`Lib.Clock`).
fn source_events(
    env: &ResolutionEnvironment,
    ids: &IdAllocator,
    module_scope_by_name: &HashMap<String, ScopeId>,
    source_name: &str,
) -> Vec<(String, i32)> {
    // Active project class.
    if let Some(&source_scope) = module_scope_by_name.get(&fold_identifier(source_name)) {
        let symbols = &env.symbols;
        let mut out = Vec::new();
        for ev_sym in symbols.symbols_in_scope(source_scope).unwrap_or_default() {
            let Some(ev) = symbols.symbol(ev_sym) else {
                continue;
            };
            if ev.kind != SymbolKind::Event {
                continue;
            }
            let Some(&event) = ids.event_index_of.get(&ev_sym) else {
                continue;
            };
            let Some(ev_name) = symbols.name(ev.name).map(|n| n.folded.clone()) else {
                continue;
            };
            out.push((ev_name, event));
        }
        return out;
    }
    // Referenced project class (possibly `Project.Class`). Search the referenced
    // surfaces (index 0 is the active project's own surface, skipped).
    let (proj, class) = match source_name.split_once('.') {
        Some((p, c)) => (Some(fold_identifier(p)), fold_identifier(c)),
        None => (None, fold_identifier(source_name)),
    };
    for surface in env.export_surfaces().iter().skip(1) {
        if proj
            .as_deref()
            .is_some_and(|p| fold_identifier(&surface.project_name) != p)
        {
            continue;
        }
        if let Some(ty) = surface
            .types
            .iter()
            .find(|t| fold_identifier(&t.name) == class)
        {
            return ty
                .events
                .iter()
                .map(|e| (fold_identifier(&e.name), e.event_id))
                .collect();
        }
    }
    // Referenced COM coclass (a typelib source). The `event id` is the event's
    // dispid/token — the runtime subscribe/poll key.
    if let Some(events) = env.com_source_events(source_name) {
        return events;
    }
    Vec::new()
}

/// Build the `Declare Lib` external-call descriptors from the scanned `Declare`
/// symbols. The source-declared fields come straight from the symbol model; the
/// runtime-resolution fields (`symbol`/`marshal_lane`/`selection_policy`) get the
/// loader's canonical defaults. ByRef write-back targets are the call-site
/// `CallArg::ByRef` slots, so the descriptor carries no write-back list.
///
/// `marshal_lane` is a **static** property of the target: a real library binds to
/// the native FFI lane (`m1-native-ffi`); the synthetic `host` library (the HAL's
/// `hostping`/`hostdouble` self-test symbols) stays on the deterministic lane. The
/// HAL decides at run time whether to actually call native code (it requires
/// `allow_dynamic_link` + a non-deterministic policy on a matching host profile),
/// so sandboxed/deterministic runs stay safe without any binder-side policy logic.
fn build_external_calls(env: &ResolutionEnvironment) -> Vec<ExternalCallDescriptor> {
    let mut out: Vec<ExternalCallDescriptor> = Vec::new();
    for sym in env.symbols.symbols() {
        let SymbolImpl::Declare(d) = &sym.imp else {
            continue;
        };
        let marshal_lane = if d.library.eq_ignore_ascii_case("host") {
            "m0-deterministic"
        } else {
            "m1-native-ffi"
        };
        out.push(ExternalCallDescriptor {
            descriptor_id: d.descriptor_id,
            declared_name: d.declared_name.clone(),
            library: d.library.clone(),
            alias: d.alias.clone(),
            ordinal_alias: d.ordinal_alias,
            symbol: DynLinkSymbol::new(d.descriptor_id as i32),
            marshal_lane: marshal_lane.to_string(),
            calling_convention: "platform-default".to_string(),
            selection_policy: if d.ordinal_alias {
                "ordinal-literal-canonical".to_string()
            } else {
                "case-insensitive-canonical".to_string()
            },
            param_count: d.param_types.len(),
            param_types: d.param_types.clone(),
            param_by_ref: d.param_by_ref.clone(),
            return_type: d.return_type,
        });
    }
    out.sort_by_key(|d| d.descriptor_id);
    out.dedup_by_key(|d| d.descriptor_id);
    out
}

/// COM-server export metadata for each class module (hosting only; not used by
/// VM execution). `creatable`/`prog_id` come from the module's VB attributes.
fn build_com_class_exports(manifest: &SymbolProjectManifest) -> Vec<ComClassExport> {
    manifest
        .modules
        .iter()
        .filter(|m| m.module_kind == ModuleKind::Class)
        .map(|m| {
            let class_name = if m.attributes.vb_name.is_empty() {
                m.module_name.clone()
            } else {
                m.attributes.vb_name.clone()
            };
            ComClassExport {
                class_name,
                prog_id: m.attributes.prog_id.clone(),
                creatable: m.attributes.vb_creatable,
            }
        })
        .collect()
}

/// Project-wide immutable lowering context (resolution + id maps). The import
/// collector is interior-mutable so proc bodies can register cross-bundle imports.
struct Lower<'a> {
    env: &'a ResolutionEnvironment,
    ids: &'a IdAllocator,
    imports: RefCell<ImportCollector>,
}

impl Lower<'_> {
    /// Register a cross-bundle import and return its index in `CoreProgram.imports`.
    fn intern_import(&self, import: BundleImport) -> usize {
        self.imports.borrow_mut().intern(import)
    }

    /// A declared type names a class *or* a `Type` (UDT) — the parser/scanner can't
    /// tell them apart, so an `Object(name)` whose name is a declared UDT is a record
    /// value type. (Applied wherever the binder reads a variable's declared type.)
    fn resolve_udt_type(&self, ty: VarTypeRef) -> VarTypeRef {
        match ty {
            VarTypeRef::Object(name) if self.env.is_udt(&name) => VarTypeRef::Udt(name),
            other => other,
        }
    }

    /// The runtime array-element layout for a `ReDim`/`Erase` element type. Builtin
    /// elements map to their scalar tag; a **UDT element** becomes a recursive
    /// [`ArrayElementType::Record`] carrying each field's layout (so the VM seeds a
    /// default record per element, recursing into nested scalar-UDT subfields). A
    /// field that is itself an *array* (of UDT or otherwise) stays `Variant` —
    /// unallocated until its own `ReDim`. The recursion terminates because VBA
    /// forbids by-value self-referential UDTs.
    fn array_element_layout(&self, elem: &VarTypeRef) -> oxvba_bundle::ArrayElementType {
        match self.resolve_udt_type(elem.clone()) {
            VarTypeRef::Udt(udt) => {
                let fields = self
                    .env
                    .udt_field_list(&udt)
                    .map(|fs| {
                        fs.iter()
                            .map(|(_, ty)| self.array_field_layout(ty))
                            .collect()
                    })
                    .unwrap_or_default();
                oxvba_bundle::ArrayElementType::Record(fields)
            }
            other => crate::types::array_element_of(&other),
        }
    }

    /// The layout of one *record field* for [`Self::array_element_layout`]: a scalar
    /// UDT field is a nested `Record`; an array field (`Words() As OcrWord`) stays
    /// `Variant` (unallocated until `ReDim`); a builtin maps to its scalar tag.
    fn array_field_layout(&self, ty: &VarTypeRef) -> oxvba_bundle::ArrayElementType {
        match self.resolve_udt_type(ty.clone()) {
            VarTypeRef::Udt(_) => self.array_element_layout(ty),
            VarTypeRef::Array(_) => oxvba_bundle::ArrayElementType::Variant,
            other => crate::types::array_element_of(&other),
        }
    }
}

/// Per-procedure mutable lowering state.
struct ProcLower<'a> {
    g: &'a Lower<'a>,
    info: &'a ProcInfo,
    /// Active `With` receivers (for leading-dot member access).
    with_stack: Vec<Bound>,
    /// Label name → its allocated id (allocated on first reference).
    labels: HashMap<String, LabelId>,
    label_order: Vec<String>,
}

/// The result of binding an expression: its value, inferred type, and (when it
/// denotes an l-value) the place it reads from / writes to.
#[derive(Debug, Clone)]
struct Bound {
    value: CoreValue,
    ty: VarTypeRef,
    /// The l-value this expression denotes, when any. Consumed by the
    /// objects/COM phase (default-member application, `With`/member receivers)
    /// and by `udt_receiver_place` to reach a UDT `With`-receiver's record.
    place: Option<CorePlace>,
}

impl<'a> ProcLower<'a> {
    // ── Resolution helpers ──────────────────────────────────

    fn resolve(&self, name: &str) -> Option<Binding> {
        self.g
            .env
            .resolve(&ResolutionContext::at(self.info.proc_scope), name)
    }

    /// Member resolution against a typed receiver (`recv.name`).
    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<oxvba_bundle::ProjectMemberKind>,
    ) -> Option<Binding> {
        self.g.env.resolve_member(recv, name, want)
    }

    /// The value of `Me` inside a class member — a `Load` of the implicit `Me`
    /// slot (synthetic parameter 0). `None` outside a class member.
    pub(crate) fn me_value(&self) -> Option<CoreValue> {
        self.info
            .me_local
            .map(|l| CoreValue::Load(CorePlace::Local(l)))
    }

    /// The declared type of a resolved symbol (Variant if it has no declared type).
    fn symbol_type(&self, sym: SymbolId) -> VarTypeRef {
        let ty = match self.g.env.symbols.symbol(sym).map(|s| &s.imp) {
            Some(SymbolImpl::DeclaredType(t)) => t.clone(),
            _ => VarTypeRef::Variant,
        };
        self.g.resolve_udt_type(ty)
    }

    /// The place + type for a resolved variable symbol: a local/param slot, a
    /// global slot, or — inside a class member — a per-instance field / WithEvents
    /// binding reached through `Me`.
    fn place_for_symbol(&self, sym: SymbolId) -> Option<(CorePlace, VarTypeRef)> {
        if let Some(&lid) = self.info.local_of.get(&sym) {
            return Some((CorePlace::Local(lid), self.symbol_type(sym)));
        }
        if let Some(&gid) = self.g.ids.global_of.get(&sym) {
            return Some((CorePlace::Global(gid), self.symbol_type(sym)));
        }
        if let Some(&field) = self.g.ids.field_token_of.get(&sym) {
            let me = self.me_value()?;
            return Some((
                CorePlace::Field {
                    object: Box::new(me),
                    field,
                },
                self.symbol_type(sym),
            ));
        }
        if let Some(&binding) = self.g.ids.withevents_binding_of.get(&sym) {
            let me = self.me_value()?;
            return Some((
                CorePlace::WithEvents {
                    owner: Box::new(me),
                    binding,
                },
                self.symbol_type(sym),
            ));
        }
        None
    }

    /// `Some(return_local)` if `name` is the enclosing function's name (the
    /// function-result pseudo-variable).
    fn return_target(&self, name: &str) -> Option<LocalId> {
        if self.info.return_local.is_some()
            && fold_identifier(name) == fold_identifier(&self.info.name)
        {
            self.info.return_local
        } else {
            None
        }
    }

    /// Allocate (or reuse) the id for a label name.
    fn label_id(&mut self, name: &str) -> LabelId {
        let key = fold_identifier(name);
        if let Some(id) = self.labels.get(&key) {
            return *id;
        }
        let id = LabelId(self.label_order.len());
        self.labels.insert(key.clone(), id);
        self.label_order.push(key);
        id
    }

    fn unresolved(&self, name: &str, context: &str) -> BindError {
        BindError::Unresolved {
            name: name.to_string(),
            context: context.to_string(),
        }
    }
}
