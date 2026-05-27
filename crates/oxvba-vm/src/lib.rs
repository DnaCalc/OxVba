//! oxvba-vm: register-window VM scaffolding.

pub mod broadword;
pub mod error_state;
pub mod interpreter;
pub mod register_file;
pub mod semantics;

use std::sync::Arc;

use oxvba_compiler::{Bytecode, OxBundle};
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{HostPolicy, native_host_profile},
    traits::HostServices,
};
use oxvba_runtime::Variant;

pub use interpreter::{
    DebugBreakpoint, DebugRunResult, DebugRuntimeSnapshot, DebugSourceLocation, DebugStop,
    DebugStopReason, Vm, VmArrayShapeEvidence, VmCallSiteDescriptorEvidence, VmExecutionPackage,
    VmPackageIdentityEvidence, VmPackageOrigin, VmProcedureIdentityEvidence,
    VmSignatureCallEvidence,
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

/// Package-oriented retained value-model snapshot API.
pub fn execute_package_and_snapshot_variants(
    package: &VmExecutionPackage<'_>,
) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute_package(package)?;
    Ok(vm.snapshot_variants(package.bytecode.user_slot_count))
}

/// OxBundle-backed retained value-model snapshot API.
pub fn execute_bundle_and_snapshot_variants(bundle: &OxBundle) -> Result<Vec<Variant>, String> {
    let package = VmExecutionPackage::from_bundle(bundle);
    execute_package_and_snapshot_variants(&package)
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

/// Package-oriented retained value-model snapshot API with typed-fastpath selection.
pub fn execute_package_and_snapshot_variants_with_typed_fastpaths(
    package: &VmExecutionPackage<'_>,
    typed_fastpaths: bool,
) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(default_host_services());
    vm.execute_package_with_typed_fastpaths(package, typed_fastpaths)?;
    Ok(vm.snapshot_variants(package.bytecode.user_slot_count))
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

/// Package-oriented host-backed retained value-model snapshot API.
pub fn execute_package_and_snapshot_variants_with_host(
    package: &VmExecutionPackage<'_>,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<Variant>, String> {
    let mut vm = Vm::new(host_services);
    vm.execute_package(package)?;
    Ok(vm.snapshot_variants(package.bytecode.user_slot_count))
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
        ArgumentBindingKindDescriptor, ArgumentSourceKindDescriptor, CallTargetKindDescriptor,
        DeclareParamType, OxBundle, ParameterPassingMode, ParameterRole, ProcedureKindDescriptor,
        ProjectDynamicMemberKind, ProjectDynamicMemberRoute, ProjectDynamicObjectRoute,
        ResolvedParameterMechanism, RuntimeCarrierKind, SlotInitialState, SlotRole,
        SourceParameterMechanism, VbaTypeId, compile, compile_with_runtime_metadata,
    };
    use oxvba_runtime::{
        RuntimeInterfaceId, RuntimeMemberInvokeKind, RuntimeValueType, Variant, bstr::BStr,
    };

    use oxvba_hal::model::native_host_profile;

    use super::{
        Vm, VmExecutionPackage, default_host_services, execute_and_snapshot_variants,
        execute_bundle_and_snapshot_variants, execute_package_and_snapshot_variants,
    };

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
    fn execution_package_snapshot_matches_bytecode_snapshot() {
        let source = "Function Test(dbl As Double, str As String) As Variant\n\
                      Test = str\n\
                      End Function\n\
                      Sub Main()\n\
                      Dim observed\n\
                      observed = Test(2.5, \"kg\")\n\
                      End Sub";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let test_metadata = metadata
            .values()
            .find(|metadata| metadata.procedure_name.eq_ignore_ascii_case("Test"))
            .expect("function metadata should be present");
        assert_eq!(
            test_metadata.param_types,
            vec![DeclareParamType::Double, DeclareParamType::String]
        );
        assert_eq!(test_metadata.return_type, Some(DeclareParamType::Variant));

        let bundle = OxBundle::new(bytecode.clone(), metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let bytecode_snapshot =
            execute_and_snapshot_variants(&bytecode).expect("bytecode snapshot");
        let package_snapshot =
            execute_package_and_snapshot_variants(&package).expect("package snapshot");
        let bundle_snapshot =
            execute_bundle_and_snapshot_variants(&bundle).expect("bundle snapshot");

        assert_eq!(package_snapshot, bytecode_snapshot);
        assert_eq!(bundle_snapshot, bytecode_snapshot);
    }

    #[test]
    fn execution_package_invocation_loads_procedure_metadata() {
        let source = "Function Test(dbl As Double, str As String) As Variant\n\
                      Test = str\n\
                      End Function";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let test_metadata = metadata
            .values()
            .find(|metadata| metadata.procedure_name.eq_ignore_ascii_case("Test"))
            .expect("function metadata should be present")
            .clone();
        let bundle = OxBundle::new(bytecode, metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let mut vm = Vm::new(default_host_services());
        vm.invoke_package_procedure_with_variants(
            &package,
            test_metadata.entry_pc,
            &test_metadata.param_slots,
            &[
                Variant::from_f64(2.5),
                Variant::from_string(BStr::from("kg")),
            ],
        )
        .expect("package-backed function invocation should succeed");

        let return_slot = test_metadata.return_slot.expect("function return slot");
        let snapshot = vm.snapshot_variants(bundle.bytecode.slot_count);
        assert_eq!(
            snapshot[return_slot],
            Variant::from_string(BStr::from("kg"))
        );
        assert_eq!(
            vm.package_identity_evidence(),
            Some(&package.identity_evidence())
        );
    }

    #[test]
    fn execution_package_records_identity_evidence_without_snapshot_drift() {
        let source = "Function Test(dbl As Double, str As String) As Variant\n\
                      Test = str\n\
                      End Function\n\
                      Sub Main()\n\
                      Dim observed\n\
                      observed = Test(2.5, \"kg\")\n\
                      End Sub";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let test_metadata = metadata
            .values()
            .find(|metadata| metadata.procedure_name.eq_ignore_ascii_case("Test"))
            .expect("function metadata should be present")
            .clone();
        let bundle = OxBundle::new(bytecode.clone(), metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let bytecode_snapshot =
            execute_and_snapshot_variants(&bytecode).expect("bytecode snapshot");
        let mut vm = Vm::new(default_host_services());
        vm.execute_package(&package)
            .expect("package-backed execution should succeed");
        let package_snapshot = vm.snapshot_variants(package.bytecode.user_slot_count);
        let evidence = vm
            .package_identity_evidence()
            .expect("package identity evidence should be recorded");

        assert_eq!(package_snapshot, bytecode_snapshot);
        assert_eq!(evidence.package_origin, super::VmPackageOrigin::OxBundle);
        assert!(evidence.package_digest.starts_with("fnv1a64:"));
        assert!(evidence.bytecode_digest.starts_with("fnv1a64:"));
        assert_ne!(evidence.package_digest, evidence.bytecode_digest);
        assert_eq!(evidence.slot_count, package.bytecode.slot_count);
        assert_eq!(evidence.user_slot_count, package.bytecode.user_slot_count);
        let test_identity = evidence
            .procedures
            .iter()
            .find(|procedure| procedure.procedure_name.eq_ignore_ascii_case("Test"))
            .expect("Test procedure identity evidence should be present");
        let expected_module_name = if test_metadata.module_name.trim().is_empty() {
            "<anonymous>".to_string()
        } else {
            test_metadata.module_name.clone()
        };
        assert_eq!(test_identity.module_name, expected_module_name);
        assert_eq!(test_identity.entry_pc, test_metadata.entry_pc);
        assert_eq!(
            test_identity.procedure_id,
            format!(
                "proc:{}::{}@pc:{}",
                expected_module_name, test_metadata.procedure_name, test_metadata.entry_pc
            )
        );
        assert!(test_identity.slot_descriptor_digest.starts_with("fnv1a64:"));
        assert!(
            test_identity
                .slot_descriptors
                .iter()
                .any(|descriptor| descriptor.role == SlotRole::Parameter
                    && descriptor.declared_type == VbaTypeId::Double
                    && descriptor.initial_state == SlotInitialState::CallerProvided
                    && descriptor.carrier == RuntimeCarrierKind::F64),
            "descriptor evidence should report the Double parameter facts"
        );
        assert!(
            test_identity
                .slot_descriptors
                .iter()
                .any(|descriptor| descriptor.role == SlotRole::ReturnValue
                    && descriptor.declared_type == VbaTypeId::Variant
                    && descriptor.initial_state == SlotInitialState::Empty
                    && descriptor.carrier == RuntimeCarrierKind::Variant),
            "descriptor evidence should report the Variant return slot facts"
        );

        vm.execute(&bytecode)
            .expect("raw bytecode execution should still succeed");
        assert!(
            vm.package_identity_evidence().is_none(),
            "raw bytecode execution must not leave stale package identity evidence"
        );
    }

    #[test]
    fn execution_package_exposes_slot_type_descriptor_view() {
        let source = "Function Test(dbl As Double, str As String) As Variant\n\
                      Dim localValue As Long\n\
                      localValue = 3\n\
                      Test = CStr(dbl + localValue) & str\n\
                      End Function";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let bundle = OxBundle::new(bytecode, metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let descriptors_by_proc = package.slot_type_descriptors();
        let test_descriptors = descriptors_by_proc
            .get("test")
            .expect("Test procedure descriptors should be exposed");

        let dbl = test_descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("dbl"))
            })
            .expect("dbl parameter descriptor should be present");
        assert_eq!(dbl.role, SlotRole::Parameter);
        assert_eq!(dbl.declared_type, VbaTypeId::Double);
        assert_eq!(dbl.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(dbl.carrier, RuntimeCarrierKind::F64);

        let str_param = test_descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("str"))
            })
            .expect("str parameter descriptor should be present");
        assert_eq!(str_param.role, SlotRole::Parameter);
        assert_eq!(str_param.declared_type, VbaTypeId::String);
        assert_eq!(str_param.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(str_param.carrier, RuntimeCarrierKind::BStr);

        let return_value = test_descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("Test"))
            })
            .expect("return descriptor should be present");
        assert_eq!(return_value.role, SlotRole::ReturnValue);
        assert_eq!(return_value.declared_type, VbaTypeId::Variant);
        assert_eq!(return_value.initial_state, SlotInitialState::Empty);
        assert_eq!(return_value.carrier, RuntimeCarrierKind::Variant);

        let local = test_descriptors
            .iter()
            .find(|descriptor| {
                descriptor
                    .name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case("localValue"))
            })
            .expect("local descriptor should be present");
        assert_eq!(local.role, SlotRole::Local);
        assert_eq!(local.declared_type, VbaTypeId::Long);
        assert_eq!(local.initial_state, SlotInitialState::ScalarZero);
        assert_eq!(local.carrier, RuntimeCarrierKind::I32);

        assert!(
            test_descriptors
                .iter()
                .any(|descriptor| descriptor.role == SlotRole::Temporary),
            "temporary descriptors should survive to VM package setup"
        );
    }

    #[test]
    fn execution_package_exposes_procedure_signature_descriptor_view() {
        let source = "Sub Main()\n\
                      End Sub\n\
                      Function Test(ByVal dbl As Double, ByRef str As String) As Variant\n\
                      Test = str\n\
                      End Function\n\
                      Property Let Value(ByRef newValue As Long)\n\
                      End Property";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let bundle = OxBundle::new(bytecode, metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let signatures_by_proc = package.procedure_signature_descriptors();
        let test_signature = signatures_by_proc
            .get("test")
            .expect("Test signature descriptor should be exposed");
        assert_eq!(test_signature.kind, ProcedureKindDescriptor::Function);
        assert_eq!(test_signature.return_type, Some(VbaTypeId::Variant));
        assert_eq!(test_signature.parameters.len(), 2);
        assert_eq!(test_signature.parameters[0].name, "dbl");
        assert_eq!(
            test_signature.parameters[0].passing_mode,
            ParameterPassingMode::ByVal
        );
        assert_eq!(
            test_signature.parameters[0].source_mechanism,
            SourceParameterMechanism::ExplicitByVal
        );
        assert_eq!(
            test_signature.parameters[0].resolved_mechanism,
            ResolvedParameterMechanism::ByVal
        );
        assert_eq!(
            test_signature.parameters[0].declared_type,
            VbaTypeId::Double
        );
        assert_eq!(test_signature.parameters[1].name, "str");
        assert_eq!(
            test_signature.parameters[1].passing_mode,
            ParameterPassingMode::ByRef
        );
        assert_eq!(
            test_signature.parameters[1].source_mechanism,
            SourceParameterMechanism::ExplicitByRef
        );
        assert_eq!(
            test_signature.parameters[1].resolved_mechanism,
            ResolvedParameterMechanism::ByRef
        );
        assert_eq!(
            test_signature.parameters[1].declared_type,
            VbaTypeId::String
        );

        let property_signature = signatures_by_proc
            .get("property_let_value")
            .expect("Property Let signature descriptor should be exposed");
        assert_eq!(
            property_signature.kind,
            ProcedureKindDescriptor::PropertyLet
        );
        assert_eq!(property_signature.property_group.as_deref(), Some("value"));
        assert_eq!(
            property_signature.parameters[0].role,
            ParameterRole::PropertyValue
        );
        assert_eq!(
            property_signature.parameters[0].resolved_mechanism,
            ResolvedParameterMechanism::PropertyValueByVal
        );
    }

    #[test]
    fn execution_package_exposes_call_site_descriptor_view() {
        let source = "Sub Main()\n\
                      Dim target As Long\n\
                      Dim observed As Long\n\
                      target = 1\n\
                      Call Fill(target := target)\n\
                      observed = Echo(target)\n\
                      End Sub\n\
                      Sub Fill(ByRef target As Long, Optional ByVal value As Long = 7)\n\
                      target = value\n\
                      End Sub\n\
                      Function Echo(ByVal value As Long) As Long\n\
                      Echo = value\n\
                      End Function";
        let (bytecode, metadata) =
            compile_with_runtime_metadata(source).expect("compile should succeed");
        let bundle = OxBundle::new(bytecode, metadata);
        let package = VmExecutionPackage::from_bundle(&bundle);

        let call_sites_by_proc = package.call_site_descriptors();
        let main_call_sites = call_sites_by_proc
            .get("main")
            .expect("Main call-site descriptors should be exposed");
        assert_eq!(main_call_sites.len(), 2);

        let fill = main_call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Fill"))
            .expect("Fill call descriptor should be present");
        assert_eq!(fill.target_kind, CallTargetKindDescriptor::Procedure);
        assert!(fill.target_entry_pc.is_some());
        assert_eq!(
            fill.arguments[0].source_kind,
            ArgumentSourceKindDescriptor::Named
        );
        assert_eq!(
            fill.arguments[0].binding_kind,
            ArgumentBindingKindDescriptor::ByRefAlias
        );
        assert_eq!(
            fill.arguments[1].source_kind,
            ArgumentSourceKindDescriptor::Omitted
        );
        assert_eq!(
            fill.arguments[1].binding_kind,
            ArgumentBindingKindDescriptor::OptionalDefault
        );

        let echo = main_call_sites
            .iter()
            .find(|call| call.target_name.eq_ignore_ascii_case("Echo"))
            .expect("Echo call descriptor should be present");
        assert_eq!(echo.target_kind, CallTargetKindDescriptor::Function);
        assert!(
            echo.return_value
                .as_ref()
                .is_some_and(|ret| ret.copyout_required)
        );
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
                    param_types: Vec::new(),
                    return_type: Some(oxvba_compiler::DeclareParamType::Variant),
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
                    param_types: vec![oxvba_compiler::DeclareParamType::Variant],
                    return_type: None,
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

        let unhinted = vm
            .resolve_project_dynamic_unhinted_dispatch_plan_for_test(42, "refresh", 1)
            .expect("unhinted descriptor lookup should cache a unique member/arity plan");
        assert_eq!(unhinted.member_index, 1);
        assert_eq!(unhinted.invoke_kind, RuntimeMemberInvokeKind::Method);
        assert_eq!(vm.project_dynamic_dispatch_cache_len_for_test(42), 3);
        let unhinted_default = vm
            .resolve_project_dynamic_unhinted_default_dispatch_plan_for_test(42, 0)
            .expect("unhinted default descriptor lookup should cache a unique default plan");
        assert_eq!(unhinted_default.member_index, 0);
        assert_eq!(
            unhinted_default.invoke_kind,
            RuntimeMemberInvokeKind::PropertyGet
        );
        assert_eq!(vm.project_dynamic_dispatch_cache_len_for_test(42), 3);
    }
}
