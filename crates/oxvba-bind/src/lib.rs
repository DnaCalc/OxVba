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

use std::collections::HashMap;

use oxvba_bundle::coreir::{
    CoreProc, CoreProgram, CorePlace, CoreValue, LabelId, LocalId,
};
use oxvba_bundle::EventRoute;
use oxvba_symbol::binding::Binding;
use oxvba_symbol::manifest::SymbolProjectManifest;
use oxvba_symbol::model::{
    fold_identifier, ScopeId, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace,
};
use oxvba_symbol::provider::{ResolutionContext, ResolutionEnvironment};
use oxvba_symbol::signature::VarTypeRef;
use oxvba_symbol::{build_resolution_environment, TypeLibResolver};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::ids::{IdAllocator, ProcInfo};

/// Bind a whole project: parse (once) + resolve + lower every active module's
/// CST to symbol-free Core IR.
pub fn bind_program(
    manifest: &SymbolProjectManifest,
    typelibs: &dyn TypeLibResolver,
) -> Result<CoreProgram, BindError> {
    let env = build_resolution_environment(manifest, typelibs)?;
    let ids = IdAllocator::build(&env, manifest)?;
    let lower = Lower { env: &env, ids: &ids };

    // Proc decl nodes in the same order `ids.procs` was built (ProcId order). The
    // two are produced by the identical filter; guard against any future drift so
    // a body can never be bound against the wrong frame.
    let decls = collect_proc_decls(&env);
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

    Ok(CoreProgram {
        globals: ids.globals.clone(),
        procs,
        classes: ids.classes.clone(),
        event_routes: build_event_routes(&env, &ids),
        external_calls: Vec::new(),  // Declare descriptors: COM/Declare phase
        com_class_exports: Vec::new(),
        entry: ids.entry(),
    })
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
        let Some(field) = symbols.symbol(field_sym) else { continue };
        let sink_scope = field.scope;
        let Some(field_name) = symbols.name(field.name).map(|n| n.folded.clone()) else { continue };
        // The source class is the WithEvents field's declared object type.
        let SymbolImpl::DeclaredType(VarTypeRef::Object(source_name)) = &field.imp else { continue };
        let Some(&source_scope) = module_scope_by_name.get(&fold_identifier(source_name)) else {
            continue;
        };
        for ev_sym in symbols.symbols_in_scope(source_scope).unwrap_or_default() {
            let Some(ev) = symbols.symbol(ev_sym) else { continue };
            if ev.kind != SymbolKind::Event {
                continue;
            }
            let Some(&event) = ids.event_index_of.get(&ev_sym) else { continue };
            let Some(ev_name) = symbols.name(ev.name).map(|n| n.folded.clone()) else { continue };
            let handler_name = format!("{field_name}_{ev_name}");
            if let Ok(Some(handler_sym)) =
                symbols.find_in_scope(sink_scope, SymbolNamespace::Procedure, &handler_name)
            {
                if let Some(&proc) = ids.proc_of.get(&handler_sym) {
                    routes.push(EventRoute { binding, event, handler: proc.0 });
                }
            }
        }
    }
    routes
}

/// Project-wide immutable lowering context (resolution + id maps).
struct Lower<'a> {
    env: &'a ResolutionEnvironment,
    ids: &'a IdAllocator,
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
    /// objects/COM phase (default-member application, `With`/member receivers).
    #[allow(dead_code)]
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
        self.info.me_local.map(|l| CoreValue::Load(CorePlace::Local(l)))
    }

    /// The declared type of a resolved symbol (Variant if it has no declared type).
    fn symbol_type(&self, sym: SymbolId) -> VarTypeRef {
        match self.g.env.symbols.symbol(sym).map(|s| &s.imp) {
            Some(SymbolImpl::DeclaredType(t)) => t.clone(),
            _ => VarTypeRef::Variant,
        }
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
            return Some((CorePlace::Field { object: Box::new(me), field }, self.symbol_type(sym)));
        }
        if let Some(&binding) = self.g.ids.withevents_binding_of.get(&sym) {
            let me = self.me_value()?;
            return Some((CorePlace::WithEvents { owner: Box::new(me), binding }, self.symbol_type(sym)));
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
        BindError::Unresolved { name: name.to_string(), context: context.to_string() }
    }
}
