//! Binding one procedure: stand up the per-proc `ProcLower` over the proc's
//! frame skeleton and walk its body block into `CoreStmt`s.

use std::collections::HashMap;

use oxvba_bundle::coreir::CoreStmt;
use oxvba_symbol::manifest::ModuleKind;
use oxvba_symbol::model::{SymbolImpl, SymbolKind, fold_identifier};
use oxvba_symbol::signature::VarTypeRef;
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::ids::ProcInfo;
use crate::{Lower, ProcLower};

impl<'a> Lower<'a> {
    fn proc_lower(&'a self, info: &'a ProcInfo) -> ProcLower<'a> {
        ProcLower {
            g: self,
            info,
            with_stack: Vec::new(),
            labels: HashMap::new(),
            label_order: Vec::new(),
        }
    }

    /// Allocations for module-level **fixed-size array globals** (`Dim g(1 To 3)` at
    /// module scope), run once at program entry before the entry body. Resolved in the
    /// entry proc's frame (globals are visible from any proc); a dynamic `Dim g()` global
    /// is skipped. Only the active project's modules contribute.
    pub(crate) fn module_global_array_inits(
        &'a self,
        entry_info: &'a ProcInfo,
    ) -> Result<Vec<CoreStmt>, BindError> {
        let mut pl = self.proc_lower(entry_info);
        let mut out = Vec::new();
        for module in self.env.modules() {
            // Class-module declarations are per-instance *fields*, not bundle
            // globals: a `Private p As SomeUdt` resolves only through `Me` (a
            // `CorePlace::Field`), which the entry proc's frame can't reach
            // (`me_local = None`). Binding them here against the entry context
            // would fault `"… is not a variable"`. Each class field's default
            // record-init is emitted per-instance into its `Class_Initialize`
            // prologue instead (see `class_field_record_inits`).
            if module.module_kind == ModuleKind::Class {
                continue;
            }
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
    ) -> Result<Vec<CoreStmt>, BindError> {
        let mut pl = self.proc_lower(info);
        match decl.body_block() {
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
                stmts.extend(pl.collect_fixed_array_inits(block)?);
                stmts.extend(pl.bind_block(block)?);
                Ok(stmts)
            }
            None => Ok(Vec::new()),
        }
    }
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
        let Some(module_scope) = self
            .g
            .env
            .symbols
            .scopes()
            .iter()
            .find(|s| s.id == self.info.proc_scope)
            .and_then(|s| s.parent)
        else {
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
