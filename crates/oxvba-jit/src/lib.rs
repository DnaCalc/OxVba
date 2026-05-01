//! oxvba-jit: JIT scaffolding and CLIF lowering placeholders.

pub mod cranelift;
pub mod jit_context;
pub mod runtime_helpers;
pub mod slot_abi;

use std::sync::Arc;

use oxvba_compiler::Bytecode;
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{HalProfileId, HostPolicy},
    traits::HostServices,
};
use oxvba_runtime::Variant;
use oxvba_vm::execute_and_snapshot_variants_with_host;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JitError {
    #[error("jit execution failed: {0}")]
    Execution(String),
}

#[derive(Debug, Default)]
pub struct JitEngine;

impl JitEngine {
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Ok(())
    }

    /// Retained value-model snapshot API.
    pub fn execute_and_snapshot_variants(
        &self,
        bytecode: &Bytecode,
    ) -> Result<Vec<Variant>, JitError> {
        self.execute_and_snapshot_variants_with_host(bytecode, default_host_services())
    }

    /// Retained value-model host-backed snapshot API.
    pub fn execute_and_snapshot_variants_with_host(
        &self,
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<Variant>, JitError> {
        // Try the RtSlot path first (supports more instructions).
        // On failure, fall back to VM for proper error handling with detailed messages.
        if cranelift::supports_bytecode_rtslot(bytecode) {
            match cranelift::execute_bytecode_rtslot_variants(bytecode, host_services.clone()) {
                Ok(values) => return Ok(values),
                Err(_) => {
                    return execute_and_snapshot_variants_with_host(bytecode, host_services)
                        .map_err(JitError::Execution);
                }
            }
        }
        // Fall back to VM interpreter for unsupported bytecode.
        execute_and_snapshot_variants_with_host(bytecode, host_services)
            .map_err(JitError::Execution)
    }
}

fn default_host_services() -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(HalProfileId::Windows)
        .policy(HostPolicy::deterministic_runtime())
        .build()
}
