use oxvba_compiler::Bytecode;

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
    pub fn execute(&mut self, bytecode: &Bytecode) -> Result<(), String> {
        for (ix, instr) in bytecode.instructions.iter().enumerate() {
            if ix < self.registers.registers.len() {
                self.registers.registers[ix] = instr.clone();
            }
        }
        Ok(())
    }
}
