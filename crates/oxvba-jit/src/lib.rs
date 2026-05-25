//! oxvba-jit: public JIT boundary.
//!
//! The previous Cranelift prototype was intentionally removed because it
//! silently fell back to VM execution and used the interpreter's `Variant` slot
//! file as its core execution model. This crate stays as the stable boundary
//! for the future JIT v2 design, but no executable JIT is currently available.

use std::sync::Arc;

use oxvba_compiler::Bytecode;
use oxvba_hal::traits::HostServices;
use oxvba_runtime::Variant;
use thiserror::Error;

pub const JIT_NOT_IMPLEMENTED_MESSAGE: &str =
    "JIT execution is not implemented; use the VM backend until the JIT v2 design lands";

#[derive(Debug, Error)]
pub enum JitError {
    #[error("{0}")]
    NotImplemented(&'static str),
}

#[derive(Debug, Default)]
pub struct JitEngine;

impl JitEngine {
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Err(JitError::NotImplemented(JIT_NOT_IMPLEMENTED_MESSAGE))
    }

    /// Retained value-model snapshot API.
    pub fn execute_and_snapshot_variants(
        &self,
        _bytecode: &Bytecode,
    ) -> Result<Vec<Variant>, JitError> {
        Err(JitError::NotImplemented(JIT_NOT_IMPLEMENTED_MESSAGE))
    }

    /// Retained value-model host-backed snapshot API.
    pub fn execute_and_snapshot_variants_with_host(
        &self,
        _bytecode: &Bytecode,
        _host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<Variant>, JitError> {
        Err(JitError::NotImplemented(JIT_NOT_IMPLEMENTED_MESSAGE))
    }
}

#[cfg(test)]
mod tests {
    use super::{JIT_NOT_IMPLEMENTED_MESSAGE, JitEngine};

    #[test]
    fn jit_api_reports_not_implemented() {
        let jit = JitEngine;
        let err = jit.compile_function("main").expect_err("jit is disabled");

        assert_eq!(err.to_string(), JIT_NOT_IMPLEMENTED_MESSAGE);
    }
}
