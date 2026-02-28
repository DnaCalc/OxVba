use oxvba_compiler::{Bytecode, Instruction};

use crate::register_file::RegisterFile;

#[derive(Debug)]
pub struct Vm {
    registers: RegisterFile,
    call_stack: Vec<usize>,
    on_error_resume_next: bool,
    on_error_goto_label_target: Option<usize>,
    last_error: i32,
}

impl Default for Vm {
    fn default() -> Self {
        Self {
            registers: RegisterFile::with_capacity(256),
            call_stack: Vec::new(),
            on_error_resume_next: false,
            on_error_goto_label_target: None,
            last_error: 0,
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
        let len = bytecode.instructions.len();

        while pc < len {
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
                Instruction::IntrinsicLenDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::len_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicLeftDigits { dst, src, count } => {
                    let value = self.read_slot(*src)?;
                    let count = self.read_slot(*count)?;
                    self.write_slot(*dst, Self::left_digits(value, count))?;
                    pc += 1;
                }
                Instruction::IntrinsicRightDigits { dst, src, count } => {
                    let value = self.read_slot(*src)?;
                    let count = self.read_slot(*count)?;
                    self.write_slot(*dst, Self::right_digits(value, count))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidDigits {
                    dst,
                    src,
                    start,
                    count,
                } => {
                    let value = self.read_slot(*src)?;
                    let start = self.read_slot(*start)?;
                    let count = match count {
                        Some(slot) => Some(self.read_slot(*slot)?),
                        None => None,
                    };
                    self.write_slot(*dst, Self::mid_digits(value, start, count))?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrDigits {
                    dst,
                    haystack,
                    needle,
                } => {
                    let haystack = self.read_slot(*haystack)?;
                    let needle = self.read_slot(*needle)?;
                    self.write_slot(*dst, Self::instr_digits(haystack, needle))?;
                    pc += 1;
                }
                Instruction::IntrinsicLowerDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::to_lower_digits(value))?;
                    pc += 1;
                }
                Instruction::IntrinsicUpperDigits { dst, src } => {
                    let value = self.read_slot(*src)?;
                    self.write_slot(*dst, Self::to_upper_digits(value))?;
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
                Instruction::LoadErrNumber { slot } => {
                    self.write_slot(*slot, self.last_error)?;
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
                    pc = Self::next_pc_for_jump_if_zero(cond, *target_pc, len, pc)?;
                }
                Instruction::Jump { target_pc } => {
                    pc = Self::next_pc_for_jump(*target_pc, len)?;
                }
                Instruction::CallProc { target_pc } => {
                    if *target_pc >= bytecode.instructions.len() {
                        return Err(format!("call target out of range: {target_pc}"));
                    }
                    self.call_stack.push(pc + 1);
                    pc = *target_pc;
                }
                Instruction::SetOnErrorResumeNext => {
                    self.on_error_resume_next = true;
                    self.on_error_goto_label_target = None;
                    pc += 1;
                }
                Instruction::SetOnErrorGoto0 => {
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = None;
                    pc += 1;
                }
                Instruction::SetOnErrorGotoLabel { target_pc } => {
                    if *target_pc >= len {
                        return Err(format!("error handler target out of range: {target_pc}"));
                    }
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = Some(*target_pc);
                    pc += 1;
                }
                Instruction::ResumeNext => {
                    pc += 1;
                }
                Instruction::RaiseError { code } => {
                    self.last_error = *code;
                    if self.on_error_resume_next {
                        pc += 1;
                    } else if let Some(target_pc) = self.on_error_goto_label_target {
                        pc = target_pc;
                    } else {
                        return Err(format!("runtime error: {code}"));
                    }
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

    fn next_pc_for_jump(target_pc: usize, instruction_len: usize) -> Result<usize, String> {
        if target_pc > instruction_len {
            return Err(format!("jump target out of range: {target_pc}"));
        }
        Ok(target_pc)
    }

    fn next_pc_for_jump_if_zero(
        cond: i32,
        target_pc: usize,
        instruction_len: usize,
        current_pc: usize,
    ) -> Result<usize, String> {
        if cond == 0 {
            Self::next_pc_for_jump(target_pc, instruction_len)
        } else {
            Ok(current_pc + 1)
        }
    }

    fn len_digits(value: i32) -> i32 {
        value.to_string().chars().count() as i32
    }

    fn left_digits(value: i32, count: i32) -> i32 {
        Self::slice_digits(value, 0, Some(count))
    }

    fn right_digits(value: i32, count: i32) -> i32 {
        if count <= 0 {
            return 0;
        }
        let text = value.to_string();
        let chars = text.chars().collect::<Vec<_>>();
        let take = (count as usize).min(chars.len());
        let start = chars.len().saturating_sub(take);
        let out = chars[start..].iter().collect::<String>();
        out.parse::<i32>().unwrap_or(0)
    }

    fn mid_digits(value: i32, start: i32, count: Option<i32>) -> i32 {
        let zero_based_start = if start <= 1 { 0 } else { (start - 1) as usize };
        Self::slice_digits(value, zero_based_start, count)
    }

    fn slice_digits(value: i32, start: usize, count: Option<i32>) -> i32 {
        let text = value.to_string();
        let chars = text.chars().collect::<Vec<_>>();
        if start >= chars.len() {
            return 0;
        }
        let end = match count {
            Some(c) if c <= 0 => start,
            Some(c) => (start + c as usize).min(chars.len()),
            None => chars.len(),
        };
        let out = chars[start..end].iter().collect::<String>();
        out.parse::<i32>().unwrap_or(0)
    }

    fn instr_digits(haystack: i32, needle: i32) -> i32 {
        let hay = haystack.to_string();
        let nee = needle.to_string();
        hay.find(&nee).map_or(0, |idx| (idx + 1) as i32)
    }

    fn to_lower_digits(value: i32) -> i32 {
        value
            .to_string()
            .to_ascii_lowercase()
            .parse::<i32>()
            .unwrap_or(0)
    }

    fn to_upper_digits(value: i32) -> i32 {
        value
            .to_string()
            .to_ascii_uppercase()
            .parse::<i32>()
            .unwrap_or(0)
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
    fn executes_intrinsic_digit_string_subset() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 {
                    slot: 0,
                    value: 12345,
                },
                Instruction::LoadConstI32 { slot: 1, value: 2 },
                Instruction::LoadConstI32 { slot: 2, value: 3 },
                Instruction::IntrinsicLenDigits { dst: 3, src: 0 },
                Instruction::IntrinsicLeftDigits {
                    dst: 4,
                    src: 0,
                    count: 1,
                },
                Instruction::IntrinsicRightDigits {
                    dst: 5,
                    src: 0,
                    count: 1,
                },
                Instruction::IntrinsicMidDigits {
                    dst: 6,
                    src: 0,
                    start: 1,
                    count: Some(2),
                },
                Instruction::IntrinsicInStrDigits {
                    dst: 7,
                    haystack: 0,
                    needle: 2,
                },
                Instruction::IntrinsicLowerDigits { dst: 8, src: 0 },
                Instruction::IntrinsicUpperDigits { dst: 9, src: 0 },
                Instruction::Halt,
            ],
            slot_count: 10,
            user_slot_count: 10,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(
            vm.snapshot_slots(10),
            vec![12345, 2, 3, 5, 12, 45, 234, 3, 12345, 12345]
        );
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
    fn jump_if_zero_pc_progression_helper() {
        assert_eq!(Vm::next_pc_for_jump_if_zero(0, 3, 4, 1).expect("jump"), 3);
        assert_eq!(
            Vm::next_pc_for_jump_if_zero(1, 3, 4, 1).expect("fallthrough"),
            2
        );
        assert!(Vm::next_pc_for_jump_if_zero(0, 9, 4, 1).is_err());
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

    #[test]
    fn resume_next_records_error_number_and_continues() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorResumeNext,
                Instruction::RaiseError { code: 5 },
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should continue on error");
        assert_eq!(vm.snapshot_slots(1), vec![5]);
    }

    #[test]
    fn goto_label_handler_receives_error_and_jumps() {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::SetOnErrorGotoLabel { target_pc: 4 },
                Instruction::RaiseError { code: 7 },
                Instruction::LoadConstI32 { slot: 0, value: 99 },
                Instruction::Halt,
                Instruction::LoadErrNumber { slot: 0 },
                Instruction::Halt,
            ],
            slot_count: 1,
            user_slot_count: 1,
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode)
            .expect("vm should jump to label handler");
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
        let instruction_len: usize = kani::any();
        kani::assume(instruction_len > 0);
        kani::assume(instruction_len < 64);

        let current_pc: usize = kani::any();
        kani::assume(current_pc < instruction_len);

        let target_pc: usize = kani::any();
        kani::assume(target_pc <= instruction_len);

        let cond: i32 = kani::any();
        let next = Vm::next_pc_for_jump_if_zero(cond, target_pc, instruction_len, current_pc)
            .expect("assumed valid target");
        assert!(next <= instruction_len);
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
