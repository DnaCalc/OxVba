//! Binding one procedure: stand up the per-proc `ProcLower` over the proc's
//! frame skeleton and walk its body block into `CoreStmt`s.

use std::collections::HashMap;

use oxvba_bundle::coreir::CoreStmt;
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
                // VBA hoists declarations: a fixed-size array `Dim` allocates once at
                // proc entry, before any statement (so a `Dim` in a loop or after its
                // first use still works), then the body runs.
                let mut stmts = pl.collect_fixed_array_inits(block)?;
                stmts.extend(pl.bind_block(block)?);
                Ok(stmts)
            }
            None => Ok(Vec::new()),
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
