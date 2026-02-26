use crate::{CfgIr, VbaMir};

pub fn lower(input: &VbaMir) -> CfgIr {
    CfgIr {
        nodes: input.blocks.clone(),
    }
}
