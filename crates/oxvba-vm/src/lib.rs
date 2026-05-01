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
use oxvba_runtime::Variant;

pub use interpreter::{
    DebugBreakpoint, DebugRunResult, DebugRuntimeSnapshot, DebugSourceLocation, DebugStop,
    DebugStopReason, Vm,
};

pub fn execute(bytecode: &Bytecode) -> Result<(), String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute(bytecode)
}

/// Retained value-model snapshot API.
pub fn execute_and_snapshot_variants(bytecode: &Bytecode) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute(bytecode)?;
    Ok(vm.snapshot_variants(bytecode.user_slot_count))
}

/// Retained value-model snapshot API with typed-fastpath selection.
pub fn execute_and_snapshot_variants_with_typed_fastpaths(
    bytecode: &Bytecode,
    typed_fastpaths: bool,
) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute_with_typed_fastpaths(bytecode, typed_fastpaths)?;
    Ok(vm.snapshot_variants(bytecode.user_slot_count))
}

pub fn execute_with_host(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<(), String> {
    let mut vm = Vm::new(host_services);
    vm.execute(bytecode)
}

/// Retained value-model host-backed snapshot API.
pub fn execute_and_snapshot_variants_with_host(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(host_services);
    vm.execute(bytecode)?;
    Ok(vm.snapshot_variants(bytecode.user_slot_count))
}

/// Retained value-model host-backed snapshot API with typed-fastpath selection.
pub fn execute_and_snapshot_variants_with_host_and_typed_fastpaths(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
    typed_fastpaths: bool,
) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(host_services);
    vm.execute_with_typed_fastpaths(bytecode, typed_fastpaths)?;
    Ok(vm.snapshot_variants(bytecode.user_slot_count))
}

fn default_host_services() -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(native_host_profile())
        .policy(HostPolicy::deterministic_runtime())
        .build()
}

pub mod compat {
    //! Explicit compatibility adapters for legacy `RuntimeValue` snapshots.
    //!
    //! Core VM execution APIs expose retained `Variant` snapshots. Callers that
    //! still need semantic `RuntimeValue` projections must opt into this module
    //! so the compatibility boundary is visible at the call site.

    use std::sync::Arc;

    use oxvba_compiler::Bytecode;
    use oxvba_hal::traits::HostServices;
    use oxvba_runtime::{RuntimeValue, Variant};

    fn project_snapshot_variants_to_values(
        values: Vec<Variant>,
    ) -> Result<Vec<RuntimeValue>, String> {
        values
            .into_iter()
            .map(|value| value.to_runtime_value())
            .collect()
    }

    pub trait RuntimeValueCompatVmExt {
        fn invoke_procedure_with_values(
            &mut self,
            bytecode: &Bytecode,
            entry_pc: usize,
            arg_slots: &[usize],
            args: &[RuntimeValue],
        ) -> Result<(), String>;
    }

    impl RuntimeValueCompatVmExt for super::Vm {
        fn invoke_procedure_with_values(
            &mut self,
            bytecode: &Bytecode,
            entry_pc: usize,
            arg_slots: &[usize],
            args: &[RuntimeValue],
        ) -> Result<(), String> {
            let variants = args
                .iter()
                .map(RuntimeValue::to_variant)
                .collect::<Result<Vec<_>, _>>()?;
            self.invoke_procedure_with_variants(bytecode, entry_pc, arg_slots, &variants)
        }
    }

    pub fn execute_and_snapshot(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
        execute_and_snapshot_values(bytecode)
    }

    pub fn execute_and_snapshot_values(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
        project_snapshot_variants_to_values(super::execute_and_snapshot_variants(bytecode)?)
    }

    pub fn execute_and_snapshot_with_typed_fastpaths(
        bytecode: &Bytecode,
        typed_fastpaths: bool,
    ) -> Result<Vec<RuntimeValue>, String> {
        execute_and_snapshot_values_with_typed_fastpaths(bytecode, typed_fastpaths)
    }

    pub fn execute_and_snapshot_values_with_typed_fastpaths(
        bytecode: &Bytecode,
        typed_fastpaths: bool,
    ) -> Result<Vec<RuntimeValue>, String> {
        project_snapshot_variants_to_values(
            super::execute_and_snapshot_variants_with_typed_fastpaths(bytecode, typed_fastpaths)?,
        )
    }

    pub fn execute_and_snapshot_with_host(
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, String> {
        execute_and_snapshot_values_with_host(bytecode, host_services)
    }

    pub fn execute_and_snapshot_values_with_host(
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
    ) -> Result<Vec<RuntimeValue>, String> {
        project_snapshot_variants_to_values(super::execute_and_snapshot_variants_with_host(
            bytecode,
            host_services,
        )?)
    }

    pub fn execute_and_snapshot_with_host_and_typed_fastpaths(
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
        typed_fastpaths: bool,
    ) -> Result<Vec<RuntimeValue>, String> {
        execute_and_snapshot_values_with_host_and_typed_fastpaths(
            bytecode,
            host_services,
            typed_fastpaths,
        )
    }

    pub fn execute_and_snapshot_values_with_host_and_typed_fastpaths(
        bytecode: &Bytecode,
        host_services: Arc<dyn HostServices>,
        typed_fastpaths: bool,
    ) -> Result<Vec<RuntimeValue>, String> {
        project_snapshot_variants_to_values(
            super::execute_and_snapshot_variants_with_host_and_typed_fastpaths(
                bytecode,
                host_services,
                typed_fastpaths,
            )?,
        )
    }
}

#[cfg(test)]
mod tests {
    use oxvba_compiler::compile;
    use oxvba_runtime::{RuntimeValue, bstr::BStr};

    use oxvba_hal::model::native_host_profile;

    use super::{compat, default_host_services, execute_and_snapshot_variants};

    #[test]
    fn default_host_services_follow_native_host_profile() {
        let host = default_host_services();
        assert_eq!(host.profile(), native_host_profile());
    }

    #[test]
    fn compat_snapshot_api_projects_variant_snapshot_results() {
        let bytecode =
            compile("Sub Main()\nDim x\nx = \"ABC\"\nEnd Sub").expect("compile should succeed");

        let variants = execute_and_snapshot_variants(&bytecode).expect("variant snapshot");
        let compat = compat::execute_and_snapshot_values(&bytecode).expect("compat snapshot");

        assert_eq!(variants.len(), 1);
        assert_eq!(compat, vec![RuntimeValue::String(BStr::from("ABC"))]);
        assert_eq!(
            compat,
            variants
                .into_iter()
                .map(|value| value.to_runtime_value().expect("variant projection"))
                .collect::<Vec<_>>()
        );
    }
}
