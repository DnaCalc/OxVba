use crate::{VbaHir, VbaMir};

pub fn lower(input: &VbaHir) -> VbaMir {
    VbaMir {
        blocks: input.procedures.clone(),
    }
}
