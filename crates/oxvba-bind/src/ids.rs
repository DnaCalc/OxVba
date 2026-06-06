//! ID allocation: map symbol-model `SymbolId`s to symbol-free coreir ids
//! (`GlobalId`/`ProcId`/`LocalId`/`ClassId` + field/binding/event tokens) and
//! build each `CoreProc` frame skeleton.
//!
//! This replays the scanner's deterministic scope/symbol order. The key link:
//! the scanner creates one `Procedure` scope per top-level proc decl, in source
//! order, so the i-th top-level proc decl in a module's CST corresponds to the
//! i-th `Procedure` scope under that module's scope. We zip them to attach a
//! `ProcId` + frame to each decl, then the binder fills the body afterward.
//!
//! Module kind matters here. In a **standard** module a module-level variable is
//! a `Global`; in a **class** module it is a per-instance *field* (a stable `i32`
//! token), a `WithEvents` *binding* token, or an *event* index — and every proc
//! is a class member, so its frame reserves `LocalId(0)` for the implicit `Me`
//! (a synthetic first parameter, matching vm2's `run_proc_with_me`, which binds
//! the receiver at frame slot 0 and the i-th call arg at slot `1+i`).

use std::collections::{HashMap, HashSet};

use oxvba_bundle::coreir::{
    ClassId, CoreClass, CoreClassMethod, CoreConst, CoreGlobal, CoreLocal, CoreParam, GlobalId,
    LocalId, ProcId,
};
use oxvba_bundle::{ProcedureKind, ProjectMemberKind, StringCompareMode};
use oxvba_symbol::manifest::{ModuleKind, SymbolProjectManifest};
use oxvba_symbol::model::{
    fold_identifier, ScopeId, ScopeKind, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace,
};
use oxvba_symbol::provider::ResolutionEnvironment;
use oxvba_symbol::signature::{PassingMode, Signature, VarTypeRef};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::expr::{eval_const_expr, ConstEval};
use crate::types;

/// One procedure's frame skeleton + the `SymbolId`→`LocalId` map for its scope.
pub struct ProcInfo {
    pub proc_id: ProcId,
    pub name: String,
    pub kind: ProcedureKind,
    /// The symbol-model `Procedure` scope holding this proc's params + locals.
    pub proc_scope: ScopeId,
    pub params: Vec<CoreParam>,
    pub locals: Vec<CoreLocal>,
    pub return_local: Option<LocalId>,
    /// The function/property-get return type (for coercing the return assignment).
    pub return_type: VarTypeRef,
    /// Parameter/Local `SymbolId` → its frame slot.
    pub local_of: HashMap<SymbolId, LocalId>,
    /// The class module this proc belongs to (its display name), if any.
    pub class_name: Option<String>,
    /// `Some(LocalId(0))` for a class member — the implicit `Me` slot.
    pub me_local: Option<LocalId>,
    /// The enclosing module's `Option Compare` mode (for string comparisons).
    pub compare_mode: StringCompareMode,
}

/// The whole project's id allocation: globals, procs, classes, and the symbol→id
/// maps the expression/call binders consult.
pub struct IdAllocator {
    pub globals: Vec<CoreGlobal>,
    pub procs: Vec<ProcInfo>,
    pub classes: Vec<CoreClass>,
    pub global_of: HashMap<SymbolId, GlobalId>,
    pub proc_of: HashMap<SymbolId, ProcId>,
    pub prop_accessor_of: HashMap<(SymbolId, ProjectMemberKind), ProcId>,
    /// Folded class display name → `ClassId` (index into `classes`).
    pub class_of: HashMap<String, ClassId>,
    /// A class instance field symbol → its stable field token (`CorePlace::Field`).
    pub field_token_of: HashMap<SymbolId, i32>,
    /// A `WithEvents` field symbol → its binding token (`CorePlace::WithEvents`).
    pub withevents_binding_of: HashMap<SymbolId, i32>,
    /// A class `Event` symbol → its event index (within its declaring class).
    pub event_index_of: HashMap<SymbolId, i32>,
    /// A `Const` symbol → its folded literal value (substituted at use sites).
    pub const_of: HashMap<SymbolId, CoreConst>,
}

impl IdAllocator {
    pub fn build(
        env: &ResolutionEnvironment,
        manifest: &SymbolProjectManifest,
    ) -> Result<Self, BindError> {
        let symbols = &env.symbols;
        let mut alloc = IdAllocator {
            globals: Vec::new(),
            procs: Vec::new(),
            classes: Vec::new(),
            global_of: HashMap::new(),
            proc_of: HashMap::new(),
            prop_accessor_of: HashMap::new(),
            class_of: HashMap::new(),
            field_token_of: HashMap::new(),
            withevents_binding_of: HashMap::new(),
            event_index_of: HashMap::new(),
            const_of: HashMap::new(),
        };

        // 1) Module-level members. Standard modules contribute globals; class
        //    modules contribute per-instance field / binding / event tokens.
        for module in env.modules() {
            let is_class = class_name_for(manifest, module.module_name).is_some();
            let mut member_token = 0i32; // field + WithEvents binding tokens (per class)
            let mut event_index = 0i32;
            for sym_id in symbols.symbols_in_scope(module.module_scope)? {
                let sym = symbols.symbol(sym_id).expect("symbol in scope");
                match sym.kind {
                    SymbolKind::Field if is_class => {
                        alloc.field_token_of.insert(sym_id, member_token);
                        member_token += 1;
                    }
                    SymbolKind::WithEventsField if is_class => {
                        alloc.withevents_binding_of.insert(sym_id, member_token);
                        member_token += 1;
                    }
                    SymbolKind::Event => {
                        alloc.event_index_of.insert(sym_id, event_index);
                        event_index += 1;
                    }
                    SymbolKind::Field | SymbolKind::WithEventsField => {
                        // Standard-module module-level variable → a global.
                        let gid = GlobalId(alloc.globals.len());
                        let array_element = match &sym.imp {
                            SymbolImpl::DeclaredType(t) => types::array_element(t),
                            _ => None,
                        };
                        alloc
                            .globals
                            .push(CoreGlobal { name: alloc_name(env, sym.name), array_element });
                        alloc.global_of.insert(sym_id, gid);
                    }
                    _ => {}
                }
            }
        }

        // 2) Procs — per active module, zip top-level proc decls with the module's
        //    `Procedure` scopes (both in source order). `Const`s are *collected*
        //    here and folded in a fixed-point pass afterward, so a const may refer
        //    to another const declared later or in another module.
        let mut pending_consts: Vec<(ScopeId, SymbolId, SyntaxNode<'_>)> = Vec::new();
        let mut const_syms: HashSet<SymbolId> = HashSet::new();
        for module in env.modules() {
            let class_name = class_name_for(manifest, module.module_name);
            let compare_mode = module_compare_mode(module.syntax);
            // Module-level `Const`s (top-level declarations).
            for node in module.syntax.child_nodes() {
                if node.kind() == SyntaxKind::ConstStmt {
                    collect_const_declarators(
                        env,
                        module.module_scope,
                        node,
                        &mut pending_consts,
                        &mut const_syms,
                    );
                }
            }
            let proc_scopes: Vec<ScopeId> = symbols
                .scopes()
                .iter()
                .filter(|s| s.kind == ScopeKind::Procedure && s.parent == Some(module.module_scope))
                .map(|s| s.id)
                .collect();
            let decls: Vec<SyntaxNode> = top_level_proc_decls(module.syntax);
            // The scanner creates one Procedure scope per named proc decl, in source
            // order (both gated on `proc_name_token`). If these ever desync, a body
            // would be bound against the wrong frame — fail loudly instead.
            if decls.len() != proc_scopes.len() {
                return Err(BindError::Malformed(format!(
                    "module `{}`: {} proc decls but {} procedure scopes",
                    module.module_name,
                    decls.len(),
                    proc_scopes.len()
                )));
            }
            for (decl, &proc_scope) in decls.iter().zip(proc_scopes.iter()) {
                // Proc-level `Const`s (possibly nested in blocks; one proc scope).
                collect_proc_consts(env, proc_scope, *decl, &mut pending_consts, &mut const_syms);
                alloc.alloc_proc(
                    env,
                    module.module_scope,
                    *decl,
                    proc_scope,
                    class_name.clone(),
                    compare_mode,
                )?;
            }
        }
        // Fold every collected `Const` to a value, resolving cross-const
        // references by fixed point (cycles / non-constant refs are hard errors).
        resolve_const_worklist(env, pending_consts, &const_syms, &mut alloc.const_of)?;

        // 3) Classes — one `CoreClass` per class module, with its method dispatch
        //    table + Class_Initialize/Terminate. Built after procs so ProcIds exist.
        let mut classes = Vec::new();
        let mut class_of = HashMap::new();
        for module in env.modules() {
            let Some(display) = class_name_for(manifest, module.module_name) else {
                continue;
            };
            let class_id = ClassId(classes.len());
            class_of.insert(fold_identifier(&display), class_id);
            let folded = fold_identifier(&display);
            let mut initialize = None;
            let mut terminate = None;
            let mut methods = Vec::new();
            for info in alloc.procs.iter() {
                if info.class_name.as_deref().map(fold_identifier) != Some(folded.clone()) {
                    continue;
                }
                match fold_identifier(&info.name).as_str() {
                    "class_initialize" => initialize = Some(info.proc_id),
                    "class_terminate" => terminate = Some(info.proc_id),
                    _ => methods.push(CoreClassMethod {
                        name: info.name.clone(),
                        kind: member_kind_of(info.kind),
                        proc: info.proc_id,
                    }),
                }
            }
            classes.push(CoreClass { name: display, initialize, terminate, methods });
        }
        alloc.classes = classes;
        alloc.class_of = class_of;

        Ok(alloc)
    }

    fn alloc_proc(
        &mut self,
        env: &ResolutionEnvironment,
        module_scope: ScopeId,
        decl: SyntaxNode<'_>,
        proc_scope: ScopeId,
        class_name: Option<String>,
        compare_mode: StringCompareMode,
    ) -> Result<(), BindError> {
        let symbols = &env.symbols;
        let proc_id = ProcId(self.procs.len());
        let logical = match decl.proc_name_token() {
            // Strip brackets to match the scanner's `normalize_identifier_token`
            // (find_in_scope folds case internally); preserve original case.
            Some(t) => t.text.trim_start_matches('[').trim_end_matches(']').to_string(),
            None => return Ok(()),
        };
        let member_kind = match decl.kind() {
            SyntaxKind::PropertyDecl => Some(property_accessor_kind(decl)),
            _ => None,
        };
        let kind = match (decl.kind(), member_kind) {
            (SyntaxKind::FunctionDecl, _) => ProcedureKind::Function,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertyGet)) => ProcedureKind::PropertyGet,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertyLet)) => ProcedureKind::PropertyLet,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertySet)) => ProcedureKind::PropertySet,
            _ => ProcedureKind::Sub,
        };

        // Map proc symbol → proc id (project-member resolution targets this).
        let proc_sym = symbols.find_in_scope(module_scope, SymbolNamespace::Procedure, &logical)?;
        if let Some(sym) = proc_sym {
            match member_kind {
                Some(mk) => {
                    self.prop_accessor_of.insert((sym, mk), proc_id);
                }
                None => {
                    self.proc_of.insert(sym, proc_id);
                }
            }
        }

        // The signature carries the parameter passing modes for the frame.
        let signature = proc_signature(env, proc_sym, member_kind);
        let return_type = signature
            .as_ref()
            .and_then(|s| s.return_type.clone())
            .unwrap_or(VarTypeRef::Variant);
        let is_class_member = class_name.is_some();
        let (params, locals, return_local, local_of, me_local) =
            build_frame(env, signature.as_ref(), proc_scope, kind, &logical, is_class_member);

        self.procs.push(ProcInfo {
            proc_id,
            name: logical,
            kind,
            proc_scope,
            params,
            locals,
            return_local,
            return_type,
            local_of,
            class_name,
            me_local,
            compare_mode,
        });
        Ok(())
    }

    /// The `ProcId` of the entry point: case-insensitive `Main`, if present.
    pub fn entry(&self) -> Option<ProcId> {
        let main = fold_identifier("Main");
        self.procs
            .iter()
            .find(|p| fold_identifier(&p.name) == main)
            .map(|p| p.proc_id)
    }
}

/// Map a procedure kind to its class-member dispatch kind.
fn member_kind_of(kind: ProcedureKind) -> ProjectMemberKind {
    match kind {
        ProcedureKind::Sub | ProcedureKind::Function => ProjectMemberKind::Method,
        ProcedureKind::PropertyGet => ProjectMemberKind::PropertyGet,
        ProcedureKind::PropertyLet => ProjectMemberKind::PropertyLet,
        ProcedureKind::PropertySet => ProjectMemberKind::PropertySet,
    }
}

#[allow(clippy::type_complexity)]
fn build_frame(
    env: &ResolutionEnvironment,
    signature: Option<&Signature>,
    proc_scope: ScopeId,
    kind: ProcedureKind,
    logical: &str,
    is_class_member: bool,
) -> (Vec<CoreParam>, Vec<CoreLocal>, Option<LocalId>, HashMap<SymbolId, LocalId>, Option<LocalId>) {
    let symbols = &env.symbols;
    let scope_syms = symbols.symbols_in_scope(proc_scope).unwrap_or_default();
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut local_of = HashMap::new();
    let mut next = 0usize;

    // A class member receives `Me` as a synthetic first parameter, so it lands at
    // frame slot 0 — exactly where vm2's `run_proc_with_me` binds the receiver.
    let me_local = if is_class_member {
        params.push(CoreParam { name: "Me".into(), by_ref: false, variadic: false });
        next += 1;
        Some(LocalId(0))
    } else {
        None
    };

    // Parameters (declaration order), pairing with the signature for `by_ref` and
    // the `ParamArray` (variadic) marker.
    let mut param_index = 0usize;
    for &sym_id in &scope_syms {
        let sym = symbols.symbol(sym_id).expect("symbol in scope");
        if sym.namespace == SymbolNamespace::Parameter {
            let sig_param = signature.and_then(|s| s.params.get(param_index));
            let variadic = sig_param.map(|p| p.param_array).unwrap_or(false);
            // A ParamArray is a fresh local array, never an alias — force ByVal.
            let by_ref = !variadic
                && sig_param.map(|p| p.mode == PassingMode::ByRef).unwrap_or(true);
            params.push(CoreParam { name: alloc_name(env, sym.name), by_ref, variadic });
            local_of.insert(sym_id, LocalId(next));
            next += 1;
            param_index += 1;
        }
    }

    // Then locals (declaration order, block scoping already flattened).
    for &sym_id in &scope_syms {
        let sym = symbols.symbol(sym_id).expect("symbol in scope");
        if sym.namespace == SymbolNamespace::Local {
            let array_element = match &sym.imp {
                SymbolImpl::DeclaredType(t) => types::array_element(t),
                _ => None,
            };
            locals.push(CoreLocal { name: alloc_name(env, sym.name), array_element });
            local_of.insert(sym_id, LocalId(next));
            next += 1;
        }
    }

    // The synthetic return local for a Function / Property Get.
    let return_local = if matches!(kind, ProcedureKind::Function | ProcedureKind::PropertyGet) {
        let id = LocalId(next);
        locals.push(CoreLocal { name: logical.to_string(), array_element: None });
        Some(id)
    } else {
        None
    };

    (params, locals, return_local, local_of, me_local)
}

fn proc_signature(
    env: &ResolutionEnvironment,
    proc_sym: Option<SymbolId>,
    member_kind: Option<ProjectMemberKind>,
) -> Option<Signature> {
    let sym = proc_sym?;
    let imp = &env.symbols.symbol(sym)?.imp;
    let sig_id = match (imp, member_kind) {
        (SymbolImpl::Signature(id), None) => Some(*id),
        (SymbolImpl::Property(group), Some(mk)) => match mk {
            ProjectMemberKind::PropertyGet => group.get,
            ProjectMemberKind::PropertyLet => group.let_,
            ProjectMemberKind::PropertySet => group.set,
            ProjectMemberKind::Method => None,
        },
        _ => None,
    }?;
    env.signatures.get(sig_id).cloned()
}

fn alloc_name(env: &ResolutionEnvironment, name: oxvba_symbol::model::InternedNameId) -> String {
    env.symbols
        .name(name)
        .map(|n| n.first_spelling.clone())
        .unwrap_or_default()
}

fn top_level_proc_decls(root: SyntaxNode<'_>) -> Vec<SyntaxNode<'_>> {
    root.child_nodes()
        .into_iter()
        .filter(|n| {
            matches!(
                n.kind(),
                SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl
            ) && n.proc_name_token().is_some()
        })
        .collect()
}

fn property_accessor_kind(decl: SyntaxNode<'_>) -> ProjectMemberKind {
    for t in decl.child_tokens() {
        match t.kind {
            SyntaxKind::KwGet => return ProjectMemberKind::PropertyGet,
            SyntaxKind::KwLet => return ProjectMemberKind::PropertyLet,
            SyntaxKind::KwSet => return ProjectMemberKind::PropertySet,
            _ => {}
        }
    }
    ProjectMemberKind::PropertyGet
}

/// Collect each `Const` declarator in a `ConstStmt` into the fold worklist,
/// keyed by the declared symbol (namespace `Local` for both module- and
/// proc-level consts). The initializer is folded later by
/// [`resolve_const_worklist`] so cross-const references resolve regardless of
/// declaration order.
fn collect_const_declarators<'a>(
    env: &ResolutionEnvironment,
    scope: ScopeId,
    const_stmt: SyntaxNode<'a>,
    pending: &mut Vec<(ScopeId, SymbolId, SyntaxNode<'a>)>,
    const_syms: &mut HashSet<SymbolId>,
) {
    for declarator in const_stmt.declarators() {
        let Some(name) = declarator.declarator_name() else { continue };
        let Some(init) = declarator.first_expr_child() else { continue };
        if let Ok(Some(sym)) =
            env.symbols.find_in_scope(scope, SymbolNamespace::Local, name.text)
        {
            const_syms.insert(sym);
            pending.push((scope, sym, init));
        }
    }
}

/// Collect proc-level `Const`s anywhere in a procedure body (block scoping is
/// flattened to the one procedure scope).
fn collect_proc_consts<'a>(
    env: &ResolutionEnvironment,
    proc_scope: ScopeId,
    node: SyntaxNode<'a>,
    pending: &mut Vec<(ScopeId, SymbolId, SyntaxNode<'a>)>,
    const_syms: &mut HashSet<SymbolId>,
) {
    if node.kind() == SyntaxKind::ConstStmt {
        collect_const_declarators(env, proc_scope, node, pending, const_syms);
    }
    for child in node.child_nodes() {
        collect_proc_consts(env, proc_scope, child, pending, const_syms);
    }
}

/// Fold every collected `Const` initializer to a value by fixed point: each pass
/// resolves the consts whose dependencies are already known; iterate until none
/// remain (success) or a pass makes no progress (a cycle or unresolvable
/// reference — a hard error). A folding `Error` aborts immediately.
fn resolve_const_worklist(
    env: &ResolutionEnvironment,
    pending: Vec<(ScopeId, SymbolId, SyntaxNode<'_>)>,
    const_syms: &HashSet<SymbolId>,
    const_of: &mut HashMap<SymbolId, CoreConst>,
) -> Result<(), BindError> {
    let mut remaining = pending;
    loop {
        let mut progress = false;
        let mut still = Vec::new();
        for (scope, sym, init) in remaining {
            if const_of.contains_key(&sym) {
                continue; // a duplicate declarator already resolved
            }
            match eval_const_expr(env, scope, init, const_of, const_syms) {
                ConstEval::Value(value) => {
                    const_of.insert(sym, value);
                    progress = true;
                }
                ConstEval::Pending => still.push((scope, sym, init)),
                ConstEval::Error(e) => return Err(e),
            }
        }
        if still.is_empty() {
            return Ok(());
        }
        if !progress {
            return Err(BindError::Unsupported(
                "cyclic or unresolvable Const initializer".into(),
            ));
        }
        remaining = still;
    }
}

/// The module's `Option Compare` mode (`Text` makes string comparisons
/// case-insensitive); `Binary` is the default. `Text` lexes as an `Ident` after
/// the `Compare` keyword.
fn module_compare_mode(module: SyntaxNode<'_>) -> StringCompareMode {
    for node in module.child_nodes() {
        if node.kind() != SyntaxKind::OptionStmt {
            continue;
        }
        let toks = node.child_tokens();
        let is_compare = toks.iter().any(|t| t.kind == SyntaxKind::KwCompare);
        let is_text = toks.iter().any(|t| t.kind == SyntaxKind::Ident && t.text.eq_ignore_ascii_case("Text"));
        if is_compare && is_text {
            return StringCompareMode::Text;
        }
    }
    StringCompareMode::Binary
}

fn class_name_for(manifest: &SymbolProjectManifest, module_name: &str) -> Option<String> {
    let folded = fold_identifier(module_name);
    manifest.modules.iter().find_map(|m| {
        let name = if m.attributes.vb_name.is_empty() { &m.module_name } else { &m.attributes.vb_name };
        (fold_identifier(name) == folded && m.module_kind == ModuleKind::Class).then(|| name.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_symbol::build_resolution_environment;
    use oxvba_symbol::manifest::{ModuleAttributes, ModuleUnit, ProjectKind};
    use oxvba_symbol::provider::TypeLibResolver;
    use std::collections::BTreeMap;

    struct NullTypeLibs;
    impl TypeLibResolver for NullTypeLibs {
        fn resolve(
            &self,
            _request: &oxvba_com::TypeLibResolveRequest,
        ) -> Option<oxvba_com::TypeLibMetadataBlob> {
            None
        }
    }

    fn procedural(source: &str) -> SymbolProjectManifest {
        SymbolProjectManifest {
            project_name: "Proj".into(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Mod1".into(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes::named("Mod1"),
                source: source.into(),
            }],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        }
    }

    /// A standard module + a class module (sorted after "Main" — see the
    /// integration-fixture ordering rule).
    fn with_class(main: &str, class_name: &str, class_src: &str) -> SymbolProjectManifest {
        SymbolProjectManifest {
            project_name: "Proj".into(),
            project_kind: ProjectKind::Source,
            modules: vec![
                ModuleUnit {
                    module_name: "Main".into(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes::named("Main"),
                    source: main.into(),
                },
                ModuleUnit {
                    module_name: class_name.into(),
                    module_kind: ModuleKind::Class,
                    attributes: ModuleAttributes::named(class_name),
                    source: class_src.into(),
                },
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        }
    }

    #[test]
    fn allocates_globals_procs_and_frames() {
        let src = "Public total As Long\n\n\
                   Sub Main()\n    Dim x As Long\n    x = Add(2, 3)\nEnd Sub\n\n\
                   Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
        let env = build_resolution_environment(&procedural(src), &NullTypeLibs).unwrap();
        let alloc = IdAllocator::build(&env, &procedural(src)).unwrap();

        assert_eq!(alloc.globals.len(), 1);
        assert_eq!(alloc.globals[0].name, "total");
        assert_eq!(alloc.global_of.len(), 1);

        assert_eq!(alloc.procs.len(), 2);
        let main = &alloc.procs[0];
        assert_eq!(main.name, "Main");
        assert_eq!(main.kind, ProcedureKind::Sub);
        assert!(main.params.is_empty());
        assert!(main.me_local.is_none());
        assert_eq!(main.locals.len(), 1); // x
        assert_eq!(main.locals[0].name, "x");
        assert!(main.return_local.is_none());

        let add = &alloc.procs[1];
        assert_eq!(add.name, "Add");
        assert_eq!(add.kind, ProcedureKind::Function);
        assert_eq!(add.params.len(), 2);
        assert!(add.params[0].by_ref); // VBA params default to ByRef
        // params occupy slots 0,1; the synthetic return local is slot 2.
        assert_eq!(add.return_local, Some(LocalId(2)));
        assert_eq!(add.locals.len(), 1); // just the return local
        assert_eq!(add.locals[0].name, "Add");

        assert_eq!(alloc.entry(), Some(ProcId(0)));
        assert_eq!(alloc.proc_of.len(), 2);
        assert!(alloc.classes.is_empty());
    }

    #[test]
    fn class_member_frame_reserves_me_and_fields_are_not_globals() {
        // Class `Widget` with a field, a Function method, and Class_Initialize.
        let class_src = "Private mValue As Long\n\n\
                         Public Function GetValue(extra As Long) As Long\n\
                         GetValue = mValue + extra\nEnd Function\n\n\
                         Private Sub Class_Initialize()\nmValue = 7\nEnd Sub\n";
        let manifest = with_class("Sub Main()\nEnd Sub\n", "Widget", class_src);
        let env = build_resolution_environment(&manifest, &NullTypeLibs).unwrap();
        let alloc = IdAllocator::build(&env, &manifest).unwrap();

        // The class field is NOT a global; it gets a field token instead.
        assert!(alloc.globals.is_empty(), "class fields must not become globals");
        assert_eq!(alloc.field_token_of.len(), 1);
        assert_eq!(*alloc.field_token_of.values().next().unwrap(), 0);

        // One class, with the method in its table and Class_Initialize wired.
        assert_eq!(alloc.classes.len(), 1);
        let class = &alloc.classes[0];
        assert_eq!(class.name, "Widget");
        assert!(class.initialize.is_some());
        assert!(class.terminate.is_none());
        assert_eq!(class.methods.len(), 1);
        assert_eq!(class.methods[0].name, "GetValue");
        assert_eq!(class.methods[0].kind, ProjectMemberKind::Method);

        // GetValue's frame: Me at slot 0, the real param `extra` at 1, return at 2.
        let get_value = alloc.procs.iter().find(|p| p.name == "GetValue").unwrap();
        assert_eq!(get_value.me_local, Some(LocalId(0)));
        assert_eq!(get_value.params.len(), 2); // Me + extra
        assert_eq!(get_value.params[0].name, "Me");
        assert_eq!(get_value.params[1].name, "extra");
        assert_eq!(get_value.return_local, Some(LocalId(2)));
    }
}
