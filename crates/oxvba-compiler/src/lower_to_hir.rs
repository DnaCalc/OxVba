use oxvba_ir::VbaHir;

use crate::resolve::BoundModule;

pub fn lower_to_hir(module: &BoundModule) -> VbaHir {
    VbaHir {
        procedures: vec![module.source.clone()],
    }
}
