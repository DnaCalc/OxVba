use std::collections::HashMap;

use crate::{
    bytecode::{Bytecode, Instruction},
    resolve::{BoundCond, BoundExpr, BoundModule, BoundStmt, CompareOp},
};

pub fn emit_bytecode(module: &BoundModule) -> Bytecode {
    let mut slot_map: HashMap<&str, usize> = HashMap::new();
    for (slot, name) in module.declarations.iter().enumerate() {
        slot_map.insert(name.as_str(), slot);
    }

    let mut temps = TempSlotAllocator::new(module.declarations.len());
    let mut instructions = Vec::new();
    emit_stmt_list(&module.body, &slot_map, &mut temps, &mut instructions);

    instructions.push(Instruction::Halt);

    Bytecode {
        instructions,
        slot_count: temps.total_slots(),
        user_slot_count: module.declarations.len(),
    }
}

fn emit_stmt_list(
    stmts: &[BoundStmt],
    slot_map: &HashMap<&str, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
) {
    for stmt in stmts {
        emit_stmt(stmt, slot_map, temps, instructions);
    }
}

fn emit_stmt(
    stmt: &BoundStmt,
    slot_map: &HashMap<&str, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
) {
    match stmt {
        BoundStmt::Assign { target, expr } => {
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                emit_expr_into(expr, target_slot, slot_map, instructions);
            }
        }
        BoundStmt::IfCond { cond, then_body } => {
            let cond_slot = temps.alloc_temp();
            emit_cond_into(cond, cond_slot, slot_map, temps, instructions);
            let jump_patch = instructions.len();
            instructions.push(Instruction::JumpIfZero {
                cond_slot,
                target_pc: 0,
            });
            emit_stmt_list(then_body, slot_map, temps, instructions);
            let target = instructions.len();
            if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[jump_patch] {
                *target_pc = target;
            }
        }
        BoundStmt::ForRange {
            var,
            start,
            end,
            body,
        } => {
            if let Some(var_slot) = slot_map.get(var.as_str()).copied() {
                emit_expr_into(start, var_slot, slot_map, instructions);
                let end_slot = temps.alloc_temp();
                let cond_slot = temps.alloc_temp();
                emit_expr_into(end, end_slot, slot_map, instructions);

                let loop_head = instructions.len();
                instructions.push(Instruction::CmpLeSlots {
                    dst: cond_slot,
                    lhs: var_slot,
                    rhs: end_slot,
                });
                let exit_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot,
                    target_pc: 0,
                });
                emit_stmt_list(body, slot_map, temps, instructions);
                instructions.push(Instruction::IncSlot { slot: var_slot });
                instructions.push(Instruction::Jump {
                    target_pc: loop_head,
                });
                let exit_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[exit_patch] {
                    *target_pc = exit_target;
                }
            }
        }
        BoundStmt::Unsupported { .. } => {}
    }
}

fn emit_cond_into(
    cond: &BoundCond,
    dst: usize,
    slot_map: &HashMap<&str, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
) {
    match cond {
        BoundCond::Compare { op, lhs, rhs } => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_expr_into(lhs, lhs_slot, slot_map, instructions);
            emit_expr_into(rhs, rhs_slot, slot_map, instructions);
            match op {
                CompareOp::Eq => instructions.push(Instruction::CmpEqSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Ne => instructions.push(Instruction::CmpNeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Lt => instructions.push(Instruction::CmpLtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Le => instructions.push(Instruction::CmpLeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Gt => instructions.push(Instruction::CmpGtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Ge => instructions.push(Instruction::CmpGeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
            }
        }
        BoundCond::Truthy(expr) => {
            let expr_slot = temps.alloc_temp();
            let zero_slot = temps.alloc_temp();
            emit_expr_into(expr, expr_slot, slot_map, instructions);
            instructions.push(Instruction::LoadConstI32 {
                slot: zero_slot,
                value: 0,
            });
            instructions.push(Instruction::CmpNeSlots {
                dst,
                lhs: expr_slot,
                rhs: zero_slot,
            });
        }
        BoundCond::Not(inner) => {
            let inner_slot = temps.alloc_temp();
            emit_cond_into(inner, inner_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolNot {
                dst,
                src: inner_slot,
            });
        }
        BoundCond::And(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(lhs, lhs_slot, slot_map, temps, instructions);
            emit_cond_into(rhs, rhs_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolAnd {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
        BoundCond::Or(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(lhs, lhs_slot, slot_map, temps, instructions);
            emit_cond_into(rhs, rhs_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolOr {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
    }
}

fn emit_expr_into(
    expr: &BoundExpr,
    dst: usize,
    slot_map: &HashMap<&str, usize>,
    instructions: &mut Vec<Instruction>,
) {
    match expr {
        BoundExpr::IntConst(value) => instructions.push(Instruction::LoadConstI32 {
            slot: dst,
            value: *value,
        }),
        BoundExpr::Var(name) => {
            if let Some(src) = slot_map.get(name.as_str()).copied()
                && src != dst
            {
                instructions.push(Instruction::CopySlot { dst, src });
            }
        }
        BoundExpr::AddConst { var, delta } => {
            if let Some(src) = slot_map.get(var.as_str()).copied() {
                if src != dst {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
                instructions.push(Instruction::AddConstI32 {
                    slot: dst,
                    value: *delta,
                });
            }
        }
        BoundExpr::SubConst { var, delta } => {
            if let Some(src) = slot_map.get(var.as_str()).copied() {
                if src != dst {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
                instructions.push(Instruction::SubConstI32 {
                    slot: dst,
                    value: *delta,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
struct TempSlotAllocator {
    declared_count: usize,
    next_temp: usize,
}

impl TempSlotAllocator {
    fn new(declared_count: usize) -> Self {
        Self {
            declared_count,
            next_temp: declared_count,
        }
    }

    fn alloc_temp(&mut self) -> usize {
        let slot = self.next_temp;
        self.next_temp += 1;
        slot
    }

    fn total_slots(&self) -> usize {
        self.next_temp.max(self.declared_count)
    }
}

#[cfg(test)]
mod tests {
    use super::{Instruction, TempSlotAllocator, emit_bytecode};
    use crate::resolve::resolve_symbols;

    #[test]
    fn temp_slot_allocator_starts_after_declarations() {
        let mut alloc = TempSlotAllocator::new(2);
        let a = alloc.alloc_temp();
        let b = alloc.alloc_temp();
        assert_eq!(a, 2);
        assert_eq!(b, 3);
        assert_eq!(alloc.total_slots(), 4);
    }

    #[test]
    fn emits_if_and_for_control_flow() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nIf x = 0 Then\nx = 5\nEnd If\nFor i = 1 To 2\nx = x + 1\nNext i\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLeSlots { .. }))
        );
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::TempSlotAllocator;

    #[kani::proof]
    fn temp_slots_do_not_overlap_declared_slots() {
        let declared: usize = kani::any();
        kani::assume(declared < 1024);
        let mut alloc = TempSlotAllocator::new(declared);
        let a = alloc.alloc_temp();
        let b = alloc.alloc_temp();
        assert!(a >= declared);
        assert!(b >= declared);
        assert!(b > a);
    }
}
