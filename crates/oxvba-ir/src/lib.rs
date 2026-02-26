//! oxvba-ir: multi-level IR scaffolding.

pub mod cfg;
pub mod hir;
pub mod lower_hir_to_mir;
pub mod lower_mir_to_cfg;
pub mod mir;
pub mod opt_cfg;
pub mod opt_hir;
pub mod opt_mir;

pub use cfg::CfgIr;
pub use hir::VbaHir;
pub use mir::VbaMir;

#[cfg(test)]
mod tests {
    use crate::{VbaHir, lower_hir_to_mir, lower_mir_to_cfg};

    #[test]
    fn lowering_pipeline_preserves_sequence() {
        let hir = VbaHir {
            procedures: vec!["ProcA".to_string(), "ProcB".to_string()],
        };
        let mir = lower_hir_to_mir::lower(&hir);
        let cfg = lower_mir_to_cfg::lower(&mir);
        assert_eq!(cfg.nodes, hir.procedures);
    }
}
