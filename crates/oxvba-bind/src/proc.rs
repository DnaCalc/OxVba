//! Binding one procedure: stand up the per-proc `ProcLower` over the proc's
//! frame skeleton and walk its body block into `CoreStmt`s.

use std::collections::HashMap;

use oxvba_bundle::coreir::CoreStmt;
use oxvba_syntax::SyntaxNode;

use crate::error::BindError;
use crate::ids::ProcInfo;
use crate::{Lower, ProcLower};

impl<'a> Lower<'a> {
    pub(crate) fn bind_proc_body(
        &self,
        info: &'a ProcInfo,
        decl: SyntaxNode<'a>,
    ) -> Result<Vec<CoreStmt>, BindError> {
        let mut pl = ProcLower {
            g: self,
            info,
            with_stack: Vec::new(),
            labels: HashMap::new(),
            label_order: Vec::new(),
        };
        match decl.body_block() {
            Some(block) => pl.bind_block(block),
            None => Ok(Vec::new()),
        }
    }
}
