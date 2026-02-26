use oxvba_ir::VbaHir;

use crate::bytecode::Bytecode;

pub fn emit_bytecode(hir: &VbaHir) -> Bytecode {
    Bytecode {
        instructions: hir.procedures.clone(),
    }
}
