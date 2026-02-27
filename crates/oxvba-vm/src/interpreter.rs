use oxvba_compiler::{Bytecode, Instruction};

use crate::register_file::RegisterFile;

#[derive(Debug)]
pub struct Vm {
    registers: RegisterFile,
    call_stack: Vec<usize>,
}

impl Default for Vm {
    fn default() -> Self {
        Self {
            registers: RegisterFile::with_capacity(256),
            call_stack: Vec::new(),
        }
    }
}

impl Vm {
    fn ensure_slot_count(&mut self, slot_count: usize) {
        if slot_count > self.registers.registers.len() {
            self.registers.registers.resize(slot_count, 0);
        }
    }

    pub fn snapshot_slots(&self, slot_count: usize) -> Vec<i32> {
        let end = slot_count.min(self.registers.registers.len());
        self.registers.registers[..end].to_vec()
    }

    pub fn execute(&mut self, bytecode: &Bytecode) -> Result<(), String> {
        self.ensure_slot_count(bytecode.slot_count);
        let mut pc = 0usize;

        while pc < bytecode.instructions.len() {
            match &bytecode.instructions[pc] {
                Instruction::LoadConstI32 { slot, value } => {
                    self.write_slot(*slot, *value)?;
                    pc += 1;
                }
                Instruction::AddConstI32 { slot, value } => {
                    let lhs = self.read_slot(*slot)?;
                    self.write_slot(*slot, lhs + *value)?;
                    pc += 1;
                }
                Instruction::SubConstI32 { slot, value } => {
                    let lhs = self.read_slot(*slot)?;
                    self.write_slot(*slot, lhs - *value)?;
                    pc += 1;
                }
                Instruction::CopySlot { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::CmpEqSlots { dst, lhs, rhs } => {
                    let out = if self.read_slot(*lhs)? == self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpNeSlots { dst, lhs, rhs } => {
                    let out = if self.read_slot(*lhs)? != self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpLtSlots { dst, lhs, rhs } => {
                    let out = if self.read_slot(*lhs)? < self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpLeSlots { dst, lhs, rhs } => {
                    let out = if self.read_slot(*lhs)? <= self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpGtSlots { dst, lhs, rhs } => {
                    let out = if self.read_slot(*lhs)? > self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CmpGeSlots { dst, lhs, rhs } => {
                    let out = if self.read_slot(*lhs)? >= self.read_slot(*rhs)? {
                        1
                    } else {
                        0
                    };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::BoolNot { dst, src } => {
                    let out = if self.read_slot(*src)? == 0 { 1 } else { 0 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::BoolAnd { dst, lhs, rhs } => {
                    let lhs_val = self.read_slot(*lhs)?;
                    let rhs_val = self.read_slot(*rhs)?;
                    let out = if lhs_val != 0 && rhs_val != 0 { 1 } else { 0 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::BoolOr { dst, lhs, rhs } => {
                    let lhs_val = self.read_slot(*lhs)?;
                    let rhs_val = self.read_slot(*rhs)?;
                    let out = if lhs_val != 0 || rhs_val != 0 { 1 } else { 0 };
                    self.write_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::JumpIfZero {
                    cond_slot,
                    target_pc,
                } => {
                    let cond = self.read_slot(*cond_slot)?;
                    if cond == 0 {
                        if *target_pc > bytecode.instructions.len() {
                            return Err(format!("jump target out of range: {target_pc}"));
                        }
                        pc = *target_pc;
                    } else {
                        pc += 1;
                    }
                }
                Instruction::Jump { target_pc } => {
                    if *target_pc > bytecode.instructions.len() {
                        return Err(format!("jump target out of range: {target_pc}"));
                    }
                    pc = *target_pc;
                }
                Instruction::CallProc { target_pc } => {
                    if *target_pc >= bytecode.instructions.len() {
                        return Err(format!("call target out of range: {target_pc}"));
                    }
                    self.call_stack.push(pc + 1);
                    pc = *target_pc;
                }
                Instruction::Return => {
                    if let Some(return_pc) = self.call_stack.pop() {
                        pc = return_pc;
                    } else {
                        return Err("return with empty call stack".to_string());
                    }
                }
                Instruction::IncSlot { slot } => {
                    let value = self.read_slot(*slot)?;
                    self.write_slot(*slot, value + 1)?;
                    pc += 1;
                }
                Instruction::Halt => break,
            }
        }
        Ok(())
    }

    fn read_slot(&self, slot: usize) -> Result<i32, String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        Ok(self.registers.registers[slot])
    }

    fn write_slot(&mut self, slot: usize, value: i32) -> Result<(), String> {
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Vm;
    use oxvba_compiler::{Bytecode, Instruction};

    #[test]
    fn executes_load_and_add_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::Halt,
            ],
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![15]);
    }

    #[test]
    fn executes_load_and_sub_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::SubConstI32 { slot: 0, value: 3 },
                Instruction::Halt,
            ],
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![7]);
    }

    #[test]
    fn executes_branch_and_loop_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 0 },
                Instruction::LoadConstI32 { slot: 1, value: 0 },
                Instruction::LoadConstI32 { slot: 2, value: 3 },
                Instruction::CmpEqSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::JumpIfZero {
                    cond_slot: 3,
                    target_pc: 6,
                },
                Instruction::LoadConstI32 { slot: 4, value: 10 },
                Instruction::LoadConstI32 { slot: 5, value: 1 },
                Instruction::CmpLeSlots {
                    dst: 6,
                    lhs: 5,
                    rhs: 2,
                },
                Instruction::JumpIfZero {
                    cond_slot: 6,
                    target_pc: 12,
                },
                Instruction::AddConstI32 { slot: 4, value: 1 },
                Instruction::IncSlot { slot: 5 },
                Instruction::Jump { target_pc: 7 },
                Instruction::Halt,
            ],
            slot_count: 7,
            user_slot_count: 7,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(7), vec![0, 0, 3, 1, 13, 4, 0]);
    }

    #[test]
    fn rejects_invalid_jump_target() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Jump { target_pc: 10 }, Instruction::Halt],
            slot_count: 0,
            user_slot_count: 0,
        };
        let mut vm = Vm::default();
        let err = vm.execute(&bytecode).expect_err("invalid jump should fail");
        assert!(err.contains("jump target out of range"));
    }

    #[test]
    fn executes_comparators_and_boolean_ops() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 5 },
                Instruction::LoadConstI32 { slot: 1, value: 3 },
                Instruction::CmpGtSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpLtSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpNeSlots {
                    dst: 4,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::BoolAnd {
                    dst: 5,
                    lhs: 2,
                    rhs: 4,
                },
                Instruction::BoolNot { dst: 6, src: 3 },
                Instruction::BoolOr {
                    dst: 7,
                    lhs: 3,
                    rhs: 6,
                },
                Instruction::Halt,
            ],
            slot_count: 8,
            user_slot_count: 8,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(8), vec![5, 3, 1, 0, 1, 1, 1, 1]);
    }

    #[test]
    fn executes_call_and_return_sequence() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 1 },
                Instruction::CallProc { target_pc: 4 },
                Instruction::AddConstI32 { slot: 0, value: 1 },
                Instruction::Halt,
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::Return,
            ],
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![7]);
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use crate::interpreter::Vm;
    use oxvba_compiler::{Bytecode, Instruction};

    #[kani::proof]
    fn pc_progression_is_safe_for_valid_jump_target() {
        let branch: bool = kani::any();
        let cond_value = if branch { 0 } else { 1 };
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: cond_value,
                },
                Instruction::JumpIfZero {
                    cond_slot: 0,
                    target_pc: 3,
                },
                Instruction::IncSlot { slot: 0 },
                Instruction::Halt,
            ],
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut vm = Vm::default();
        assert!(vm.execute(&bytecode).is_ok());
    }

    #[kani::proof]
    fn comparator_ops_produce_boolean_values() {
        let a: i32 = kani::any();
        let b: i32 = kani::any();
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: a },
                Instruction::LoadConstI32 { slot: 1, value: b },
                Instruction::CmpEqSlots {
                    dst: 2,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpNeSlots {
                    dst: 3,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpLtSlots {
                    dst: 4,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpLeSlots {
                    dst: 5,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpGtSlots {
                    dst: 6,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::CmpGeSlots {
                    dst: 7,
                    lhs: 0,
                    rhs: 1,
                },
                Instruction::Halt,
            ],
            slot_count: 8,
            user_slot_count: 8,
        };

        let mut vm = Vm::default();
        assert!(vm.execute(&bytecode).is_ok());
        let out = vm.snapshot_slots(8);
        for idx in 2..=7 {
            assert!(out[idx] == 0 || out[idx] == 1);
        }
    }
}
