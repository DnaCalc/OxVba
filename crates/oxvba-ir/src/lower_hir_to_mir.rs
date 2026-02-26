use crate::{VbaHir, VbaMir};

pub fn lower(input: &VbaHir) -> VbaMir {
    VbaMir {
        blocks: input.procedures.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::lower;
    use crate::VbaHir;

    #[test]
    fn lowers_procedures_to_blocks() {
        let hir = VbaHir {
            procedures: vec!["P1".to_string(), "P2".to_string()],
        };
        let mir = lower(&hir);
        assert_eq!(mir.blocks, hir.procedures);
    }
}
