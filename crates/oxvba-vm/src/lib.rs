//! oxvba-vm: register-window VM scaffolding.

pub mod broadword;
pub mod error_state;
pub mod interpreter;
pub mod register_file;

use std::sync::Arc;

use oxvba_compiler::Bytecode;
use oxvba_hal::{
    adapters,
    model::{HostPolicy, native_host_profile},
    traits::HostServices,
};
use oxvba_runtime::{RuntimeValue, value_tags::EMPTY_TAG};

pub use interpreter::Vm;

fn project_runtime_values_to_legacy_slots(values: Vec<RuntimeValue>) -> Vec<i32> {
    values
        .into_iter()
        .map(|value| value.to_legacy_i32().unwrap_or(EMPTY_TAG))
        .collect()
}

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

pub fn execute_and_legacy_snapshot(bytecode: &Bytecode) -> Result<Vec<i32>, String> {
    execute_and_snapshot(bytecode).map(project_runtime_values_to_legacy_slots)
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

pub fn execute_and_legacy_snapshot_with_typed_fastpaths(
    bytecode: &Bytecode,
    typed_fastpaths: bool,
) -> Result<Vec<i32>, String> {
    execute_and_snapshot_with_typed_fastpaths(bytecode, typed_fastpaths)
        .map(project_runtime_values_to_legacy_slots)
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

pub fn execute_and_legacy_snapshot_with_host(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<i32>, String> {
    execute_and_snapshot_with_host(bytecode, host_services)
        .map(project_runtime_values_to_legacy_slots)
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

pub fn execute_and_legacy_snapshot_with_host_and_typed_fastpaths(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
    typed_fastpaths: bool,
) -> Result<Vec<i32>, String> {
    execute_and_snapshot_with_host_and_typed_fastpaths(bytecode, host_services, typed_fastpaths)
        .map(project_runtime_values_to_legacy_slots)
}

fn default_host_services() -> Arc<dyn HostServices> {
    adapters::for_profile(native_host_profile(), HostPolicy::deterministic_runtime())
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
