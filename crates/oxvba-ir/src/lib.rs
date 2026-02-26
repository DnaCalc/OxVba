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
