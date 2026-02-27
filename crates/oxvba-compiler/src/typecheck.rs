use std::collections::HashSet;

use crate::resolve::{BoundModule, BoundOp};

pub fn check_types(module: BoundModule) -> Result<BoundModule, String> {
    let mut module = module;
    let mut declared: HashSet<String> = module.declarations.iter().cloned().collect();

    for op in &module.ops {
        match op {
            BoundOp::AssignConst { name, .. }
            | BoundOp::AddConst { name, .. }
            | BoundOp::SubConst { name, .. } => {
                if !declared.contains(name) {
                    if module.option_explicit {
                        return Err(format!("use of undeclared variable: {name}"));
                    }

                    declared.insert(name.clone());
                    module.declarations.push(name.clone());
                }
            }
            BoundOp::Unsupported { line } => {
                return Err(format!("unsupported statement: {line}"));
            }
        }
    }

    Ok(module)
}
