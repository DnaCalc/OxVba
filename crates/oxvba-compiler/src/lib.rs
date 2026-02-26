//! oxvba-compiler: semantic analysis and bytecode emission scaffolding.

pub mod bytecode;
pub mod emit;
pub mod lower_to_hir;
pub mod resolve;
pub mod typecheck;

use thiserror::Error;

pub use bytecode::Bytecode;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("empty source")]
    EmptySource,
}

pub fn compile(source: &str) -> Result<Bytecode, CompileError> {
    if source.trim().is_empty() {
        return Err(CompileError::EmptySource);
    }

    let bound = resolve::resolve_symbols(source);
    let checked = typecheck::check_types(bound);
    let hir = lower_to_hir::lower_to_hir(&checked);
    Ok(emit::emit_bytecode(&hir))
}

#[cfg(test)]
mod tests {
    use super::compile;

    #[test]
    fn compile_simple_module() {
        let out = compile("Sub Main()\nEnd Sub").expect("compile should succeed");
        assert_eq!(out.instructions.len(), 1);
    }

    #[test]
    fn reject_empty_input() {
        assert!(compile(" \n ").is_err());
    }
}
