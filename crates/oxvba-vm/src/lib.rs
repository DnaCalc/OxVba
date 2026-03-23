//! oxvba-vm: register-window VM scaffolding.

pub mod broadword;
pub mod error_state;
pub mod interpreter;
pub mod register_file;
pub mod semantics;

use std::sync::Arc;

use oxvba_compiler::Bytecode;
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{HostPolicy, native_host_profile},
    traits::HostServices,
};
use oxvba_runtime::RuntimeValue;

pub use interpreter::Vm;

pub fn execute(bytecode: &Bytecode) -> Result<(), String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute(bytecode)
}

pub fn execute_and_snapshot(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute(bytecode)?;
    Ok(vm.snapshot(bytecode.user_slot_count))
}

pub fn execute_and_snapshot_values(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
    execute_and_snapshot(bytecode)
}

pub fn execute_and_snapshot_with_typed_fastpaths(
    bytecode: &Bytecode,
    typed_fastpaths: bool,
) -> Result<Vec<RuntimeValue>, String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute_with_typed_fastpaths(bytecode, typed_fastpaths)?;
    Ok(vm.snapshot(bytecode.user_slot_count))
}

pub fn execute_and_snapshot_values_with_typed_fastpaths(
    bytecode: &Bytecode,
    typed_fastpaths: bool,
) -> Result<Vec<RuntimeValue>, String> {
    execute_and_snapshot_with_typed_fastpaths(bytecode, typed_fastpaths)
}

pub fn execute_with_host(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<(), String> {
    let mut vm = Vm::new(host_services);
    vm.execute(bytecode)
}

pub fn execute_and_snapshot_with_host(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<RuntimeValue>, String> {
    let mut vm = Vm::new(host_services);
    vm.execute(bytecode)?;
    Ok(vm.snapshot(bytecode.user_slot_count))
}

pub fn execute_and_snapshot_values_with_host(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<RuntimeValue>, String> {
    execute_and_snapshot_with_host(bytecode, host_services)
}

pub fn execute_and_snapshot_with_host_and_typed_fastpaths(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
    typed_fastpaths: bool,
) -> Result<Vec<RuntimeValue>, String> {
    let mut vm = Vm::new(host_services);
    vm.execute_with_typed_fastpaths(bytecode, typed_fastpaths)?;
    Ok(vm.snapshot(bytecode.user_slot_count))
}

pub fn execute_and_snapshot_values_with_host_and_typed_fastpaths(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
    typed_fastpaths: bool,
) -> Result<Vec<RuntimeValue>, String> {
    execute_and_snapshot_with_host_and_typed_fastpaths(bytecode, host_services, typed_fastpaths)
}

fn default_host_services() -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(native_host_profile())
        .policy(HostPolicy::deterministic_runtime())
        .build()
}

#[cfg(test)]
mod tests {
    use oxvba_hal::model::native_host_profile;

    use super::default_host_services;

    #[test]
    fn default_host_services_follow_native_host_profile() {
        let host = default_host_services();
        assert_eq!(host.profile(), native_host_profile());
    }
}
