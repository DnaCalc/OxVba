use std::collections::HashMap;

use crate::{
    bytecode::{Bytecode, Instruction},
    resolve::{BoundModule, BoundOp},
};

pub fn emit_bytecode(module: &BoundModule) -> Bytecode {
    let mut slot_map: HashMap<&str, usize> = HashMap::new();
    for (slot, name) in module.declarations.iter().enumerate() {
        slot_map.insert(name.as_str(), slot);
    }

    let mut instructions = Vec::new();
    for op in &module.ops {
        match op {
            BoundOp::AssignConst { name, value } => {
                if let Some(slot) = slot_map.get(name.as_str()) {
                    instructions.push(Instruction::LoadConstI32 {
                        slot: *slot,
                        value: *value,
                    });
                }
            }
            BoundOp::AddConst { name, value } => {
                if let Some(slot) = slot_map.get(name.as_str()) {
                    instructions.push(Instruction::AddConstI32 {
                        slot: *slot,
                        value: *value,
                    });
                }
            }
            BoundOp::SubConst { name, value } => {
                if let Some(slot) = slot_map.get(name.as_str()) {
                    instructions.push(Instruction::SubConstI32 {
                        slot: *slot,
                        value: *value,
                    });
                }
            }
            BoundOp::Unsupported { .. } => {}
        }
    }

    instructions.push(Instruction::Halt);

    Bytecode {
        instructions,
        slot_count: module.declarations.len(),
    }
}
