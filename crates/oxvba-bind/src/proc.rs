//! Binding one procedure: stand up the per-proc `ProcLower` over the proc's
//! frame skeleton and walk its body block into `CoreStmt`s.

use std::collections::{HashMap, HashSet};

use oxvba_bundle::ProcedureKind;
use oxvba_bundle::coreir::{CoreClass, CoreLocal, CoreParam, CoreProc, CoreStmt};
use oxvba_symbol::manifest::ModuleKind;
use oxvba_symbol::model::{ScopeKind, SymbolImpl, SymbolKind, fold_identifier};
use oxvba_symbol::signature::VarTypeRef;
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::ids::{
    ProcInfo, module_compare_mode, module_default_type_rules, module_option_base,
    module_option_explicit,
};
use crate::{Lower, LowerContext, ProcLower};

impl<'a> Lower<'a> {
    fn lower_with(&'a self, info: LowerContext, locals: Vec<CoreLocal>) -> ProcLower<'a> {
        ProcLower {
            g: self,
            info,
            locals,
            implicit_local_of: HashMap::new(),
            with_stack: Vec::new(),
            loop_stack: Vec::new(),
            next_with_temp: 0,
            labels: HashMap::new(),
            label_order: Vec::new(),
            defined_labels: HashSet::new(),
            label_lines: Vec::new(),
        }
    }

    fn proc_lower(&'a self, info: &ProcInfo) -> ProcLower<'a> {
        self.lower_with(LowerContext::from_proc(info), info.locals.clone())
    }

    /// Allocations for module-level **fixed-size array globals** (`Dim g(1 To 3)` at
    /// module scope), run once per VM session before user code. Each declaration is
    /// resolved in its declaring module's scope, so `Private` globals are visible
    /// exactly where VBA makes them visible. A dynamic `Dim g()` global is skipped.
    /// Only the active project's modules contribute.
    pub(crate) fn module_global_array_inits(&'a self) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        for module in self.env.modules() {
            // Class-module declarations are per-instance *fields*, not bundle
            // globals: a `Private p As SomeUdt` resolves only through `Me` (a
            // `CorePlace::Field`), which a module initializer cannot reach
            // (`me_local = None`). Each class field's default record-init is
            // emitted per-instance into its `Class_Initialize` prologue instead
            // (see `class_field_record_inits`).
            if module.module_kind == ModuleKind::Class {
                continue;
            }
            let module_info = LowerContext::from_module(
                module.module_name,
                module.module_scope,
                module_compare_mode(module.syntax),
                module_option_base(module.syntax),
            );
            let mut pl = self.lower_with(module_info, Vec::new());
            for node in module.syntax.child_nodes() {
                if node.kind() == SyntaxKind::DimStmt {
                    out.extend(pl.bind_dim(node)?);
                }
            }
        }
        Ok(out)
    }

    /// Allocate every procedure's `Static` array/record locals exactly once, at
    /// program entry. A proc's static declarators resolve to bundle globals, so
    /// the `ReDim`/record-init runs from the entry prologue but targets the
    /// persistent global; the per-call prologue skips them
    /// ([`ProcLower::bind_dim`]). Bound in each declaring proc's own lower
    /// context so its static symbols resolve. `decls` is in `ids.procs` order.
    pub(crate) fn static_local_inits(
        &'a self,
        decls: &[SyntaxNode<'a>],
    ) -> Result<Vec<CoreStmt>, BindError> {
        let mut out = Vec::new();
        for (info, decl) in self.ids.procs.iter().zip(decls.iter()) {
            let Some(block) = decl.body_block() else {
                continue;
            };
            let mut pl = self.proc_lower(info);
            walk_static_dims(&mut pl, block, &mut out)?;
        }
        Ok(out)
    }

    pub(crate) fn bind_proc_body(
        &'a self,
        info: &'a ProcInfo,
        decl: SyntaxNode<'a>,
    ) -> Result<BoundProcBody, BindError> {
        let mut pl = self.proc_lower(info);
        let body = match decl.body_block() {
            Some(block) => {
                // A class's `Class_Initialize` runs once per instance, before any
                // user code: prepend the default record-init of each per-instance
                // UDT-typed field (`Private m_Results As OcrResults`), resolved in
                // this member's frame (where `Me` exists), so the scalar UDT field
                // is a default record rather than `Empty`.
                let mut stmts = pl.class_field_record_inits()?;
                // VBA hoists declarations: a fixed-size array `Dim` allocates once at
                // proc entry, before any statement (so a `Dim` in a loop or after its
                // first use still works), then the body runs.
                stmts.extend(pl.function_return_default_init()?);
                stmts.extend(pl.collect_fixed_array_inits(block)?);
                stmts.extend(pl.bind_block(block)?);
                stmts
            }
            None => Vec::new(),
        };
        pl.validate_label_refs()?;
        Ok(BoundProcBody {
            body,
            label_lines: pl.label_lines,
            locals: pl.locals,
        })
    }

    pub(crate) fn attach_synthetic_class_initializers(
        &'a self,
        classes: &mut [CoreClass],
        procs: &mut Vec<CoreProc>,
    ) -> Result<(), BindError> {
        for module in self.env.modules() {
            let Some(display) = crate::ids::class_name_for(self.manifest, module.module_name)
            else {
                continue;
            };
            let Some(&class_id) = self.ids.class_of.get(&fold_identifier(&display)) else {
                continue;
            };
            let class = classes
                .get_mut(class_id.0)
                .ok_or_else(|| BindError::Malformed(format!("unknown class `{display}`")))?;
            if class.initialize.is_some() {
                continue;
            }
            let Some(proc) =
                self.synthetic_class_initialize_proc(module.module_scope, module.syntax, &display)?
            else {
                continue;
            };
            let proc_id = oxvba_bundle::coreir::ProcId(procs.len());
            procs.push(proc);
            class.initialize = Some(proc_id);
        }
        Ok(())
    }

    fn synthetic_class_initialize_proc(
        &'a self,
        module_scope: oxvba_symbol::model::ScopeId,
        module_syntax: SyntaxNode<'a>,
        class_name: &str,
    ) -> Result<Option<CoreProc>, BindError> {
        let params = vec![CoreParam {
            name: "Me".to_string(),
            ty: VarTypeRef::Variant,
            optional: false,
            by_ref: false,
            variadic: false,
        }];
        let mut pl = self.lower_with(
            LowerContext::synthetic_class_initialize(
                module_scope,
                module_syntax,
                class_name,
                params.len(),
            ),
            Vec::new(),
        );
        let body = pl.class_field_record_inits()?;
        if body.is_empty() {
            return Ok(None);
        }
        Ok(Some(CoreProc {
            name: "Class_Initialize".to_string(),
            kind: ProcedureKind::Sub,
            params,
            locals: pl.locals,
            return_local: None,
            label_lines: Vec::new(),
            body,
        }))
    }
}

pub(crate) struct BoundProcBody {
    pub body: Vec<CoreStmt>,
    pub label_lines: Vec<Option<i32>>,
    pub locals: Vec<CoreLocal>,
}

impl<'a> ProcLower<'a> {
    /// The per-instance default record-init statements for this proc, when it is a
    /// class's `Class_Initialize`: one `NewRecord` (recursing into nested scalar-UDT
    /// subfields) per module-level UDT-typed instance field. Empty for any other
    /// proc — including a class proc that is not `Class_Initialize`, and a class with
    /// no UDT fields.
    ///
    /// These are emitted here (not as bundle globals) because a class field is
    /// reached through `Me` (`CorePlace::Field`), which only exists inside a class
    /// member's frame. Fields are visited in symbol order (stable) and resolved by
    /// name in this member's context, so `place_by_name` returns the field place.
    fn class_field_record_inits(&mut self) -> Result<Vec<CoreStmt>, BindError> {
        // Only a class's `Class_Initialize` materializes instance fields.
        if self.info.class_name.is_none()
            || !fold_identifier(&self.info.name).eq("class_initialize")
        {
            return Ok(Vec::new());
        }
        // The class field symbols live in the module scope (the proc scope's parent).
        let Some(module_scope) = self.class_member_module_scope() else {
            return Ok(Vec::new());
        };
        // Collect (folded field name, declared type) for each module-level Field, so
        // the symbol-table borrow is released before binding (which borrows `self`).
        let fields: Vec<String> = self
            .g
            .env
            .symbols
            .symbols_in_scope(module_scope)
            .map_err(|e| BindError::Malformed(format!("{e:?}")))?
            .into_iter()
            .filter_map(|sym_id| {
                let sym = self.g.env.symbols.symbol(sym_id)?;
                if sym.kind != SymbolKind::Field {
                    return None;
                }
                let SymbolImpl::DeclaredType(ty) = &sym.imp else {
                    return None;
                };
                // Only scalar UDT fields need a default record; array fields stay
                // unallocated until `ReDim` (their element materialization is a
                // separate runtime feature).
                if !matches!(self.g.resolve_udt_type(ty.clone()), VarTypeRef::Udt(_)) {
                    return None;
                }
                Some(self.g.env.symbols.name(sym.name)?.first_spelling.clone())
            })
            .collect();
        let mut out = Vec::new();
        for name in fields {
            out.extend(self.udt_record_init(&name)?);
        }
        Ok(out)
    }

    fn class_member_module_scope(&self) -> Option<oxvba_symbol::model::ScopeId> {
        let scope = self
            .g
            .env
            .symbols
            .scopes()
            .iter()
            .find(|s| s.id == self.info.proc_scope)?;
        match scope.kind {
            ScopeKind::Procedure => scope.parent,
            ScopeKind::Module => Some(scope.id),
            _ => None,
        }
    }
}

impl LowerContext {
    fn synthetic_class_initialize(
        module_scope: oxvba_symbol::model::ScopeId,
        module_syntax: SyntaxNode<'_>,
        class_name: &str,
        param_count: usize,
    ) -> Self {
        Self {
            name: "Class_Initialize".to_string(),
            proc_scope: module_scope,
            local_of: HashMap::new(),
            param_count,
            return_local: None,
            return_type: VarTypeRef::Variant,
            class_name: Some(class_name.to_string()),
            me_local: Some(oxvba_bundle::coreir::LocalId(0)),
            compare_mode: module_compare_mode(module_syntax),
            option_base: module_option_base(module_syntax),
            option_explicit: module_option_explicit(module_syntax),
            default_types: module_default_type_rules(module_syntax),
        }
    }
}

/// Walk a proc body emitting the once-per-program allocation of each `Static`
/// array/record declarator (`ProcLower::bind_static_dim` filters to statics).
fn walk_static_dims<'a>(
    pl: &mut ProcLower<'a>,
    node: SyntaxNode<'a>,
    out: &mut Vec<CoreStmt>,
) -> Result<(), BindError> {
    if node.kind() == SyntaxKind::DimStmt {
        out.extend(pl.bind_static_dim(node)?);
    }
    for child in node.child_nodes() {
        walk_static_dims(pl, child, out)?;
    }
    Ok(())
}
