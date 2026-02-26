use std::collections::HashSet;

use crate::resolve::{BoundModule, BoundOp};

pub fn check_types(module: BoundModule) -> Result<BoundModule, String> {
    let declared: HashSet<String> = module.declarations.iter().cloned().collect();

    for op in &module.ops {
        match op {
            BoundOp::AssignConst { name, .. } | BoundOp::AddConst { name, .. } => {
                if !declared.contains(name) {
                    return Err(format!("use of undeclared variable: {name}"));
                }
            }
            BoundOp::Unsupported { line } => {
                return Err(format!("unsupported statement: {line}"));
            }
        }
    }

    Ok(module)
}
