use oxvba_compiler::{Bytecode, Instruction};

use crate::register_file::RegisterFile;

#[derive(Debug)]
pub struct Vm {
    registers: RegisterFile,
}

impl Default for Vm {
    fn default() -> Self {
        Self {
            registers: RegisterFile::with_capacity(256),
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

        for instr in &bytecode.instructions {
            match instr {
                Instruction::LoadConstI32 { slot, value } => {
                    if *slot >= self.registers.registers.len() {
                        return Err(format!("slot out of range: {slot}"));
                    }
                    self.registers.registers[*slot] = *value;
                }
                Instruction::AddConstI32 { slot, value } => {
                    if *slot >= self.registers.registers.len() {
                        return Err(format!("slot out of range: {slot}"));
                    }
                    self.registers.registers[*slot] += *value;
                }
                Instruction::SubConstI32 { slot, value } => {
                    if *slot >= self.registers.registers.len() {
                        return Err(format!("slot out of range: {slot}"));
                    }
                    self.registers.registers[*slot] -= *value;
                }
                Instruction::Halt => break,
            }
        }
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
        };

        let mut vm = Vm::default();
        vm.execute(&bytecode).expect("vm should execute bytecode");
        assert_eq!(vm.snapshot_slots(1), vec![7]);
    }
}
