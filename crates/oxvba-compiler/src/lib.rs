//! oxvba-compiler: semantic analysis and bytecode emission scaffolding.

pub mod bytecode;
pub mod emit;
pub mod lower_to_hir;
pub mod resolve;
pub mod typecheck;

use thiserror::Error;

pub use bytecode::{Bytecode, Instruction};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("empty source")]
    EmptySource,
    #[error("type error: {0}")]
    TypeError(String),
}

pub fn compile(source: &str) -> Result<Bytecode, CompileError> {
    if source.trim().is_empty() {
        return Err(CompileError::EmptySource);
    }

    let bound = resolve::resolve_symbols(source);
    let checked = typecheck::check_types(bound).map_err(CompileError::TypeError)?;
    let _hir = lower_to_hir::lower_to_hir(&checked);
    Ok(emit::emit_bytecode(&checked))
}

#[cfg(test)]
mod tests {
    use super::{Instruction, compile};

    #[test]
    fn compile_simple_module() {
        let out = compile("Sub Main()\nEnd Sub").expect("compile should succeed");
        assert_eq!(out.instructions, vec![Instruction::Halt]);
    }

    #[test]
    fn reject_empty_input() {
        assert!(compile(" \n ").is_err());
    }

    #[test]
    fn compile_dim_assign_and_add() {
        let source = "Sub Main()\nDim x\nx = 10\nx = x + 5\nEnd Sub";
        let out = compile(source).expect("compile should succeed");
        assert_eq!(out.slot_count, 1);
        assert_eq!(
            out.instructions,
            vec![
                Instruction::LoadConstI32 { slot: 0, value: 10 },
                Instruction::AddConstI32 { slot: 0, value: 5 },
                Instruction::Halt
            ]
        );
    }

    #[test]
    fn undeclared_variable_with_option_explicit_is_rejected() {
        let source = "Option Explicit\nSub Main()\nx = 1\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("undeclared variable"));
    }

    #[test]
    fn reject_unsupported_statement() {
        let source = "Sub Main()\nDim x\nx = y + 1\nEnd Sub";
        let err = compile(source).expect_err("typecheck should fail");
        assert!(err.to_string().contains("unsupported statement"));
    }

    #[test]
    fn undeclared_variable_without_option_explicit_is_accepted() {
        let source = "Sub Main()\nx = 1\nx = x + 1\nEnd Sub";
        let out = compile(source).expect("implicit declaration should compile");
        assert_eq!(out.slot_count, 1);
    }
}
