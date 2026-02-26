use crate::{CfgIr, VbaMir};

pub fn lower(input: &VbaMir) -> CfgIr {
    CfgIr {
        nodes: input.blocks.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::lower;
    use crate::VbaMir;

    #[test]
    fn lowers_blocks_to_cfg_nodes() {
        let mir = VbaMir {
            blocks: vec!["B0".to_string(), "B1".to_string()],
        };
        let cfg = lower(&mir);
        assert_eq!(cfg.nodes, mir.blocks);
    }
}
