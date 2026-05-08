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

#[cfg(test)]
mod tests {
    use oxvba_com::DynamicCallKind;
    use oxvba_compiler::{
        ProjectDynamicMemberKind, ProjectDynamicMemberRoute, ProjectDynamicObjectRoute, compile,
    };
    use oxvba_runtime::{
        RuntimeInterfaceId, RuntimeMemberInvokeKind, RuntimeValueType, Variant, bstr::BStr,
    };

    use oxvba_hal::model::native_host_profile;

    use super::{Vm, default_host_services, execute_and_snapshot_variants};

    #[test]
    fn default_host_services_follow_native_host_profile() {
        let host = default_host_services();
        assert_eq!(host.profile(), native_host_profile());
    }

    #[test]
    fn snapshot_api_returns_variant_snapshot_results() {
        let bytecode =
            compile("Sub Main()\nDim x\nx = \"ABC\"\nEnd Sub").expect("compile should succeed");

        let variants = execute_and_snapshot_variants(&bytecode).expect("variant snapshot");

        assert_eq!(variants.len(), 1);
        assert_eq!(variants, vec![Variant::from_string(BStr::from("ABC"))]);
    }

    #[test]
    fn project_dynamic_objects_advertise_dual_dispatch_descriptors() {
        let mut vm = Vm::new(default_host_services());
        vm.set_project_dynamic_objects(vec![ProjectDynamicObjectRoute {
            object_handle: 42,
            project_name: "Project".to_string(),
            module_name: "Widget".to_string(),
            implements_interfaces: Vec::new(),
            members: vec![
                ProjectDynamicMemberRoute {
                    member_name: "Value".to_string(),
                    lowered_name: "project_widget_property_get_value".to_string(),
                    known_dispatch_token: None,
                    dispatch_id: Some(0),
                    member_flags: None,
                    is_default_member: true,
                    kind: ProjectDynamicMemberKind::PropertyGet,
                    visible_param_count: 0,
                    params: Vec::new(),
                    entry_pc: 10,
                    param_slots: vec![0],
                    return_slot: Some(1),
                },
                ProjectDynamicMemberRoute {
                    member_name: "Refresh".to_string(),
                    lowered_name: "project_widget_refresh".to_string(),
                    known_dispatch_token: Some(5),
                    dispatch_id: None,
                    member_flags: None,
                    is_default_member: false,
                    kind: ProjectDynamicMemberKind::Method,
                    visible_param_count: 1,
                    params: vec![oxvba_compiler::ProjectDynamicParamRoute {
                        name: "force".to_string(),
                        optional: true,
                        param_array: false,
                        default_value: Some(0),
                    }],
                    entry_pc: 20,
                    param_slots: vec![0, 2],
                    return_slot: None,
                },
            ],
        }]);

        let object = vm
            .project_dynamic_object_ref(42)
            .expect("dynamic object should be registered");
        let class_descriptor = object.class_descriptor();
        assert_eq!(class_descriptor.name, "Project.Widget");
        let dispatch = object
            .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
            .expect("project dynamic objects should advertise IDispatch descriptor metadata");
        assert!(dispatch.dual_dispatch);
        assert_eq!(dispatch.name, "Project.Widget._Default");
        assert_eq!(dispatch.members.len(), 2);
        assert_eq!(dispatch.members[0].name, "Value");
        assert_eq!(dispatch.members[0].dispatch_id, 0);
        assert_eq!(dispatch.members[0].vtable_slot, Some(7));
        assert_eq!(
            dispatch.members[0].invoke_kind,
            RuntimeMemberInvokeKind::PropertyGet
        );
        assert_eq!(dispatch.members[0].arity, 0);
        assert!(dispatch.members[0].is_default_member);
        assert_eq!(dispatch.members[1].name, "Refresh");
        assert_eq!(dispatch.members[1].dispatch_id, 5);
        assert_eq!(dispatch.members[1].vtable_slot, Some(8));
        assert_eq!(
            dispatch.members[1].invoke_kind,
            RuntimeMemberInvokeKind::Method
        );
        assert_eq!(dispatch.members[1].arity, 1);
        assert_eq!(dispatch.members[1].params.len(), 1);
        assert_eq!(dispatch.members[1].params[0].name, "force");
        assert_eq!(
            dispatch.members[1].params[0].value_type,
            RuntimeValueType::Variant
        );
        assert!(dispatch.members[1].params[0].optional);
        assert!(!dispatch.members[1].params[0].param_array);
        assert_eq!(dispatch.members[1].return_type, None);

        let first = vm
            .resolve_project_dynamic_dispatch_plan_for_test(
                42,
                " value ",
                DynamicCallKind::PropertyGet,
                0,
            )
            .expect("descriptor-backed dispatch plan should resolve");
        assert_eq!(first.member_index, 0);
        assert_eq!(vm.project_dynamic_dispatch_cache_len_for_test(42), 1);
        let second = vm
            .resolve_project_dynamic_dispatch_plan_for_test(
                42,
                "VALUE",
                DynamicCallKind::PropertyGet,
                0,
            )
            .expect("normalized descriptor-backed dispatch plan should be cached");
        assert_eq!(first, second);
        assert_eq!(vm.project_dynamic_dispatch_cache_len_for_test(42), 1);
        let default = vm
            .resolve_project_dynamic_default_dispatch_plan_for_test(
                42,
                DynamicCallKind::PropertyGet,
                0,
            )
            .expect("descriptor-backed default dispatch plan should resolve");
        assert_eq!(default.member_index, 0);
        assert!(default.is_default_member);
        assert_eq!(vm.project_dynamic_dispatch_cache_len_for_test(42), 2);
        assert!(
            vm.resolve_project_dynamic_dispatch_plan_for_test(
                42,
                "Value",
                DynamicCallKind::PropertyLet,
                0,
            )
            .is_none(),
            "call kind participates in VM project-object descriptor cache resolution"
        );
    }
}
