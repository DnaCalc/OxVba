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
    ClassId, CoreClass, CoreClassMethod, CoreGlobal, CoreLocal, CoreParam, GlobalId, LocalId,
    ProcId,
};
use oxvba_bundle::{ProcedureKind, ProjectMemberKind, StringCompareMode};
use oxvba_symbol::binding::DispatchRoute;
use oxvba_symbol::manifest::{ModuleKind, SymbolProjectManifest};
use oxvba_symbol::model::{
    ScopeId, ScopeKind, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace, fold_identifier,
};
use oxvba_symbol::provider::ResolutionEnvironment;
use oxvba_symbol::signature::{BuiltinType, PassingMode, Signature, VarTypeRef};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
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
    /// True when the member carries `Attribute <member>.VB_UserMemId = -4`.
    pub is_enumerator_member: bool,
    /// `Some(LocalId(0))` for a class member — the implicit `Me` slot.
    pub me_local: Option<LocalId>,
    /// The enclosing module's `Option Compare` mode (for string comparisons).
    pub compare_mode: StringCompareMode,
    /// The enclosing module's `Option Base` (0 or 1): the default lower bound of
    /// an array dimension declared with only an upper bound (`Dim a(10)`) and of
    /// the `Array()` function's result.
    pub option_base: i32,
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
    /// Folded display name → `ClassId` for the active project's `VB_PredeclaredId`
    /// classes — a bare reference to one of these names is its global singleton
    /// (`CoreValue::Predeclared`), not a `New`. Active-project only; a referenced
    /// project's predeclared classes resolve through its export surface.
    pub predeclared_class_of: HashMap<String, ClassId>,
    /// A class instance field symbol → its stable field token (`CorePlace::Field`).
    pub field_token_of: HashMap<SymbolId, i32>,
    /// A `WithEvents` field symbol → its binding token (`CorePlace::WithEvents`).
    pub withevents_binding_of: HashMap<SymbolId, i32>,
    /// A class `Event` symbol → its event index (within its declaring class).
    pub event_index_of: HashMap<SymbolId, i32>,
    /// Folded names of class modules that appear in some `Implements` clause —
    /// i.e. project interfaces. A member dispatch on an interface-typed receiver
    /// is mangled to `Interface_Member`.
    pub interfaces: HashSet<String>,
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
            predeclared_class_of: HashMap::new(),
            field_token_of: HashMap::new(),
            withevents_binding_of: HashMap::new(),
            event_index_of: HashMap::new(),
            interfaces: HashSet::new(),
        };

        // 1) Module-level members. Standard modules contribute globals; class
        //    modules contribute per-instance field / binding / event tokens.
        //
        // Field tokens are per class (they index that instance's own sparse field
        // map). WithEvents *binding* tokens are bundle-GLOBAL, because the binding
        // token is half of the bundle-wide `event_routes[(binding, event)]` dispatch
        // key: if two sink classes each declared a WithEvents handler for the same
        // source event, a per-class binding token would collide on `(binding, event)`
        // and one handler would shadow the other (the dispatch dedup invariant in
        // `LoadedBundle::load` would fire). A bundle-global counter keeps every
        // WithEvents field's routes distinct. Binding tokens never index per-instance
        // storage (a WithEvents field's bound source lives in the VM's `withevents`
        // map, keyed by owner+binding), so they need no per-class numbering.
        let mut next_withevents_binding = 0i32;
        for module in env.modules() {
            let is_class = class_name_for(manifest, module.module_name).is_some();
            let mut member_token = 0i32; // per-class field tokens (instance storage index)
            let mut event_index = 0i32;
            for sym_id in symbols.symbols_in_scope(module.module_scope)? {
                let sym = symbols.symbol(sym_id).expect("symbol in scope");
                match sym.kind {
                    SymbolKind::Field if is_class => {
                        alloc.field_token_of.insert(sym_id, member_token);
                        member_token += 1;
                    }
                    SymbolKind::WithEventsField if is_class => {
                        alloc
                            .withevents_binding_of
                            .insert(sym_id, next_withevents_binding);
                        next_withevents_binding += 1;
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
                        alloc.globals.push(CoreGlobal {
                            name: alloc_name(env, sym.name),
                            ty: declared_var_type(&sym.imp),
                            array_element,
                        });
                        alloc.global_of.insert(sym_id, gid);
                    }
                    _ => {}
                }
            }
        }

        // 2) Procs — per active module, zip top-level proc decls with the module's
        //    `Procedure` scopes (both in source order). `Const`/`Enum` values are
        //    folded once in the symbol layer (`env.const_value`), the published
        //    type system's single source of truth — the binder no longer folds.
        for module in env.modules() {
            let class_name = class_name_for(manifest, module.module_name);
            let compare_mode = module_compare_mode(module.syntax);
            let option_base = module_option_base(module.syntax);
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
                alloc.alloc_proc(
                    env,
                    module.module_scope,
                    module.syntax,
                    *decl,
                    proc_scope,
                    class_name.clone(),
                    compare_mode,
                    option_base,
                )?;
            }
        }

        // 3) Classes — one `CoreClass` per class module, with its method dispatch
        //    table + Class_Initialize/Terminate. Built after procs so ProcIds exist.
        let mut classes = Vec::new();
        let mut class_of = HashMap::new();
        let mut predeclared_class_of = HashMap::new();
        // Folded module name → module scope, for resolving `Implements` targets.
        let module_scope_by_name: HashMap<String, ScopeId> = env
            .modules()
            .map(|m| (fold_identifier(m.module_name), m.module_scope))
            .collect();
        for module in env.modules() {
            let Some(display) = class_name_for(manifest, module.module_name) else {
                continue;
            };
            let class_id = ClassId(classes.len());
            class_of.insert(fold_identifier(&display), class_id);
            // A `VB_PredeclaredId = True` class has a global singleton reachable by
            // its name (the `ThisWorkbook`/`Sheet1` document-module shape).
            if predeclared_class(manifest, module.module_name) {
                predeclared_class_of.insert(fold_identifier(&display), class_id);
            }
            let folded = fold_identifier(&display);
            let default_member = env
                .resolve_default_member(&VarTypeRef::Object(display.clone()))
                .and_then(|binding| {
                    let symbol = binding.symbol?;
                    let member_name = symbols
                        .symbol(symbol)
                        .and_then(|sym| symbols.name(sym.name))
                        .map(|name| name.first_spelling.clone())?;
                    match binding.route {
                        DispatchRoute::ProjectMember { kind } => Some((member_name, kind)),
                        _ => None,
                    }
                });
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
                        is_default_member: default_member.as_ref().is_some_and(|(name, _)| {
                            fold_identifier(name) == fold_identifier(&info.name)
                        }),
                        is_enumerator_member: info.is_enumerator_member,
                    }),
                }
            }
            // `Implements I` clauses: record the interface names, mark them as
            // project interfaces, and verify every interface member has a matching
            // `Interface_Member` implementation in this class.
            let implements = collect_module_implements(module.syntax);
            for iface in &implements {
                alloc.interfaces.insert(fold_identifier(iface));
                validate_interface_members(
                    env,
                    &module_scope_by_name,
                    module.module_scope,
                    &display,
                    iface,
                )?;
            }
            classes.push(CoreClass {
                name: display,
                initialize,
                terminate,
                methods,
                as_new_fields: Vec::new(),
                implements,
            });
        }
        alloc.classes = classes;
        alloc.class_of = class_of;
        alloc.predeclared_class_of = predeclared_class_of;

        Ok(alloc)
    }

    fn alloc_proc(
        &mut self,
        env: &ResolutionEnvironment,
        module_scope: ScopeId,
        module_syntax: SyntaxNode<'_>,
        decl: SyntaxNode<'_>,
        proc_scope: ScopeId,
        class_name: Option<String>,
        compare_mode: StringCompareMode,
        option_base: i32,
    ) -> Result<(), BindError> {
        let symbols = &env.symbols;
        let proc_id = ProcId(self.procs.len());
        let logical = match decl.proc_name_token() {
            // Strip brackets to match the scanner's `normalize_identifier_token`
            // (find_in_scope folds case internally); preserve original case.
            Some(t) => t
                .text
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_string(),
            None => return Ok(()),
        };
        let member_kind = match decl.kind() {
            SyntaxKind::PropertyDecl => Some(property_accessor_kind(decl)),
            _ => None,
        };
        let kind = match (decl.kind(), member_kind) {
            (SyntaxKind::FunctionDecl, _) => ProcedureKind::Function,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertyGet)) => {
                ProcedureKind::PropertyGet
            }
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertyLet)) => {
                ProcedureKind::PropertyLet
            }
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertySet)) => {
                ProcedureKind::PropertySet
            }
            _ => ProcedureKind::Sub,
        };

        // Map proc symbol → proc id (project-member resolution targets this).
        let proc_sym = symbols.find_in_scope(module_scope, SymbolNamespace::Procedure, &logical)?;
        let is_enumerator_member = has_user_mem_id_decl(decl, -4)
            || has_exported_member_user_mem_id(module_syntax, &logical, -4);
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
            .map(|ty| normalize_declared_type(env, ty))
            .unwrap_or(VarTypeRef::Variant);
        let is_class_member = class_name.is_some();
        let Frame {
            params,
            locals,
            return_local,
            local_of,
            me_local,
        } = build_frame(
            env,
            signature.as_ref(),
            proc_scope,
            kind,
            &logical,
            is_class_member,
        )?;

        // `Static` locals persist across calls: allocate one zero-initialized
        // bundle global per static local (mangled by proc so display names stay
        // readable). Place resolution maps the symbol to this global; the frame
        // skipped it above. Display-name collisions are harmless — `global_of`
        // keys on the unique `SymbolId`.
        for &sym_id in &symbols.symbols_in_scope(proc_scope).unwrap_or_default() {
            let sym = symbols.symbol(sym_id).expect("symbol in scope");
            if sym.kind != SymbolKind::StaticLocal {
                continue;
            }
            let gid = GlobalId(self.globals.len());
            let array_element = match &sym.imp {
                SymbolImpl::DeclaredType(t) => types::array_element(t),
                _ => None,
            };
            self.globals.push(CoreGlobal {
                name: format!("{logical}#{}", alloc_name(env, sym.name)),
                ty: declared_var_type(&sym.imp),
                array_element,
            });
            self.global_of.insert(sym_id, gid);
        }

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
            is_enumerator_member,
            me_local,
            compare_mode,
            option_base,
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

fn normalize_declared_type(env: &ResolutionEnvironment, ty: VarTypeRef) -> VarTypeRef {
    match ty {
        VarTypeRef::Object(name) if env.is_udt(&name) => VarTypeRef::Udt(name),
        VarTypeRef::Object(name) if env.is_enum_type(&name) => {
            VarTypeRef::Builtin(BuiltinType::Long)
        }
        VarTypeRef::Array(element) => {
            VarTypeRef::Array(Box::new(normalize_declared_type(env, *element)))
        }
        VarTypeRef::FixedArray { element, bounds } => VarTypeRef::FixedArray {
            element: Box::new(normalize_declared_type(env, *element)),
            bounds,
        },
        other => other,
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
/// The declared static type carried by a symbol's implementation — recorded on the
/// Core IR binding so the OxIR elaboration pass can recover it. `Variant` when the
/// symbol has no declared type (an implicit / untyped variable).
fn declared_var_type(imp: &SymbolImpl) -> VarTypeRef {
    match imp {
        SymbolImpl::DeclaredType(t) => t.clone(),
        _ => VarTypeRef::Variant,
    }
}

/// The frame layout [`build_frame`] computes for one procedure.
struct Frame {
    params: Vec<CoreParam>,
    locals: Vec<CoreLocal>,
    /// The synthetic function/property-get return local, if any.
    return_local: Option<LocalId>,
    /// Maps each parameter/local symbol to its frame slot.
    local_of: HashMap<SymbolId, LocalId>,
    /// The `Me` receiver slot (slot 0) for a class member.
    me_local: Option<LocalId>,
}

fn build_frame(
    env: &ResolutionEnvironment,
    signature: Option<&Signature>,
    proc_scope: ScopeId,
    kind: ProcedureKind,
    logical: &str,
    is_class_member: bool,
) -> Result<Frame, BindError> {
    let symbols = &env.symbols;
    let scope_syms = symbols.symbols_in_scope(proc_scope).unwrap_or_default();
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut local_of = HashMap::new();
    let mut next = 0usize;

    // A class member receives `Me` as a synthetic first parameter, so it lands at
    // frame slot 0 — exactly where vm2's `run_proc_with_me` binds the receiver.
    let me_local = if is_class_member {
        params.push(CoreParam {
            name: "Me".into(),
            // The receiver is the enclosing class instance; precise `Object(class)`
            // typing of `Me` is deferred to the object-elaboration sub-section.
            ty: VarTypeRef::Variant,
            by_ref: false,
            variadic: false,
        });
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
            let ty = sig_param
                .map(|p| normalize_declared_type(env, p.ty.clone()))
                .unwrap_or(VarTypeRef::Variant);
            if !variadic
                && sig_param.is_some_and(|p| p.mode == PassingMode::ByVal)
                && matches!(ty, VarTypeRef::Array(_) | VarTypeRef::FixedArray { .. })
            {
                return Err(BindError::ArrayArgumentMustBeByRef);
            }
            // A ParamArray is a fresh local array, never an alias — force ByVal.
            let by_ref = !variadic
                && sig_param
                    .map(|p| p.mode == PassingMode::ByRef)
                    .unwrap_or(true);
            params.push(CoreParam {
                name: alloc_name(env, sym.name),
                ty,
                by_ref,
                variadic,
            });
            local_of.insert(sym_id, LocalId(next));
            next += 1;
            param_index += 1;
        }
    }

    // Then locals (declaration order, block scoping already flattened). A proc-level
    // `Const` is namespace `Local` but folded to a value — it gets no frame slot;
    // a `StaticLocal` persists across calls and is lowered to a bundle global
    // (allocated in `alloc_proc`), so it gets no frame slot either.
    for &sym_id in &scope_syms {
        let sym = symbols.symbol(sym_id).expect("symbol in scope");
        if sym.namespace == SymbolNamespace::Local
            && sym.kind != SymbolKind::Const
            && sym.kind != SymbolKind::StaticLocal
        {
            let array_element = match &sym.imp {
                SymbolImpl::DeclaredType(t) => types::array_element(t),
                _ => None,
            };
            locals.push(CoreLocal {
                name: alloc_name(env, sym.name),
                ty: normalize_declared_type(env, declared_var_type(&sym.imp)),
                array_element,
            });
            local_of.insert(sym_id, LocalId(next));
            next += 1;
        }
    }

    // The synthetic return local for a Function / Property Get.
    let return_local = if matches!(kind, ProcedureKind::Function | ProcedureKind::PropertyGet) {
        let id = LocalId(next);
        locals.push(CoreLocal {
            name: logical.to_string(),
            ty: signature
                .and_then(|s| s.return_type.clone())
                .map(|ty| normalize_declared_type(env, ty))
                .unwrap_or(VarTypeRef::Variant),
            array_element: None,
        });
        Some(id)
    } else {
        None
    };

    Ok(Frame {
        params,
        locals,
        return_local,
        local_of,
        me_local,
    })
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

fn has_user_mem_id_decl(node: SyntaxNode<'_>, id: i32) -> bool {
    node.text()
        .lines()
        .any(|line| line_has_user_mem_id(line, id))
}

fn has_exported_member_user_mem_id(root: SyntaxNode<'_>, member: &str, id: i32) -> bool {
    if root.kind() == SyntaxKind::AttributeStmt
        && let Some(attr_member) = attribute_user_mem_id_member(&root.text(), id)
        && fold_identifier(&attr_member) == fold_identifier(member)
    {
        return true;
    }
    root.child_nodes()
        .into_iter()
        .any(|child| has_exported_member_user_mem_id(child, member, id))
}

fn attribute_user_mem_id_member(text: &str, id: i32) -> Option<String> {
    let compact = text.to_ascii_lowercase().replace([' ', '\t'], "");
    if !compact.starts_with("attribute") || !compact.contains(".vb_usermemid=") {
        return None;
    }
    let after_keyword = text.trim().split_once(char::is_whitespace)?.1.trim();
    let (lhs, value) = after_keyword.split_once('=')?;
    if parse_user_mem_id_value(value)? != id {
        return None;
    }
    let (member, attr) = lhs.trim().rsplit_once('.')?;
    if !attr.trim().eq_ignore_ascii_case("VB_UserMemId") {
        return None;
    }
    let member = member.trim();
    (!member.is_empty()).then(|| member.to_string())
}

fn line_has_user_mem_id(line: &str, id: i32) -> bool {
    let Some((_, value)) = line.split_once('=') else {
        return false;
    };
    line.to_ascii_lowercase().contains("vb_usermemid") && parse_user_mem_id_value(value) == Some(id)
}

fn parse_user_mem_id_value(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok()
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
        let is_text = toks
            .iter()
            .any(|t| t.kind == SyntaxKind::Ident && t.text.eq_ignore_ascii_case("Text"));
        if is_compare && is_text {
            return StringCompareMode::Text;
        }
    }
    StringCompareMode::Binary
}

/// The module's `Option Base` (0 or 1): the default lower bound of an array
/// dimension declared with only an upper bound, and of the `Array()` result.
/// `Option Base 1` sets 1; absent (or `Option Base 0`) it is 0. VBA permits
/// only 0 or 1 — any other literal is read as 1 only when it is exactly `1`.
fn module_option_base(module: SyntaxNode<'_>) -> i32 {
    for node in module.child_nodes() {
        if node.kind() != SyntaxKind::OptionStmt {
            continue;
        }
        let toks = node.child_tokens();
        if !toks.iter().any(|t| t.kind == SyntaxKind::KwBase) {
            continue;
        }
        let is_one = toks
            .iter()
            .any(|t| t.kind == SyntaxKind::IntLiteral && t.text.trim() == "1");
        return i32::from(is_one);
    }
    0
}

/// Interface display names from a class module's `Implements` clauses (each is a
/// `TypeRef` child of an `ImplementsStmt`).
fn collect_module_implements(module: SyntaxNode<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for node in module.child_nodes() {
        if node.kind() == SyntaxKind::ImplementsStmt
            && let Some(type_ref) = node.child_node(SyntaxKind::TypeRef)
        {
            let name = type_ref.text().trim().to_string();
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// Verify each member of `iface` has a matching `Interface_Member` implementation
/// in the implementing class. A typelib / non-project interface (no module scope)
/// is skipped — its coverage is the host's concern.
fn validate_interface_members(
    env: &ResolutionEnvironment,
    module_scope_by_name: &HashMap<String, ScopeId>,
    class_scope: ScopeId,
    class_name: &str,
    iface: &str,
) -> Result<(), BindError> {
    let symbols = &env.symbols;
    let Some(&iface_scope) = module_scope_by_name.get(&fold_identifier(iface)) else {
        return Ok(());
    };
    for sym_id in symbols.symbols_in_scope(iface_scope).unwrap_or_default() {
        let Some(sym) = symbols.symbol(sym_id) else {
            continue;
        };
        if sym.namespace != SymbolNamespace::Procedure {
            continue;
        }
        let Some(member) = symbols.name(sym.name).map(|n| n.folded.clone()) else {
            continue;
        };
        if member == "class_initialize" || member == "class_terminate" {
            continue;
        }
        let mangled = format!("{iface}_{member}");
        if !matches!(
            symbols.find_in_scope(class_scope, SymbolNamespace::Procedure, &mangled),
            Ok(Some(_))
        ) {
            return Err(BindError::Unsupported(format!(
                "class `{class_name}` implements `{iface}` but does not implement `{mangled}`"
            )));
        }
    }
    Ok(())
}

pub(crate) fn class_name_for(
    manifest: &SymbolProjectManifest,
    module_name: &str,
) -> Option<String> {
    let folded = fold_identifier(module_name);
    manifest.modules.iter().find_map(|m| {
        let name = if m.attributes.vb_name.is_empty() {
            &m.module_name
        } else {
            &m.attributes.vb_name
        };
        (fold_identifier(name) == folded && m.module_kind == ModuleKind::Class)
            .then(|| name.clone())
    })
}

/// True if the active-project class module `module_name` is a `VB_PredeclaredId`
/// class (its name denotes a global singleton instance).
fn predeclared_class(manifest: &SymbolProjectManifest, module_name: &str) -> bool {
    let folded = fold_identifier(module_name);
    manifest.modules.iter().any(|m| {
        let name = if m.attributes.vb_name.is_empty() {
            &m.module_name
        } else {
            &m.attributes.vb_name
        };
        fold_identifier(name) == folded
            && m.module_kind == ModuleKind::Class
            && m.attributes.vb_predeclared_id
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
            conditional_compilation_target: Default::default(),
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
            conditional_compilation_target: Default::default(),
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
        assert!(
            alloc.globals.is_empty(),
            "class fields must not become globals"
        );
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

    /// The published `SurfaceEvent.event_id` MUST equal the binder's
    /// `event_index_of` for the same event symbol — a cross-bundle `WithEvents` sink
    /// builds routes from the surface id, and the source's `RaiseEvent` fires the
    /// binder index, so any divergence silently misroutes events.
    #[test]
    fn surface_event_id_matches_binder_event_index() {
        let mut clock_attrs = ModuleAttributes::named("Clock");
        clock_attrs.vb_exposed = true;
        let manifest = SymbolProjectManifest {
            project_name: "Proj".into(),
            project_kind: ProjectKind::Source,
            modules: vec![
                ModuleUnit {
                    module_name: "Main".into(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes::named("Main"),
                    source: "Sub Main()\nEnd Sub\n".into(),
                },
                ModuleUnit {
                    module_name: "Clock".into(),
                    module_kind: ModuleKind::Class,
                    attributes: clock_attrs,
                    // A `Private Event` between two Public ones: the binder's
                    // `event_index_of` counts ALL events (Tick=0, Internal=1, Done=2),
                    // so the surface must too (exposing only Public Tick=0, Done=2).
                    source: "Public Event Tick(ByVal n As Long)\nPrivate Event Internal()\nPublic Event Done()\n".into(),
                },
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
            conditional_compilation_target: Default::default(),
        };
        let env = build_resolution_environment(&manifest, &NullTypeLibs).unwrap();
        let alloc = IdAllocator::build(&env, &manifest).unwrap();
        let surface = &env.export_surfaces()[0];
        let clock = surface
            .types
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case("Clock"))
            .expect("Clock in surface");
        assert_eq!(clock.events.len(), 2);
        for ev in &clock.events {
            assert_eq!(
                alloc.event_index_of.get(&ev.symbol).copied(),
                Some(ev.event_id),
                "event `{}`: surface id {} must equal the binder's event index",
                ev.name,
                ev.event_id
            );
        }
    }
}
