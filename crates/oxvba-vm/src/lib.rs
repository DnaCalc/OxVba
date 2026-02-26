//! oxvba-vm: register-window VM scaffolding.

pub mod broadword;
pub mod error_state;
pub mod interpreter;
pub mod register_file;

use oxvba_compiler::Bytecode;

pub use interpreter::Vm;

pub fn execute(bytecode: &Bytecode) -> Result<(), String> {
    let mut vm = Vm::default();
    vm.execute(bytecode)
}
