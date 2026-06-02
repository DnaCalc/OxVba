use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fmt::Debug,
    sync::Arc,
};

use oxvba_com::{
    ComCallbackToken, ComSubscriptionToken, DynamicCallArg, DynamicCallKind, DynamicCallRequest,
    DynamicMemberSelector, DynamicObjectBridge, DynamicValue,
};
use oxvba_compiler::{
    ArgumentBindingDescriptor, ArgumentBindingKindDescriptor, ArgumentExpressionKindDescriptor,
    ArgumentWritebackDescriptor, ArrayShapeDescriptor, ArrayStorageKind, BundleCallableDescriptor,
    BundleProjectContext, Bytecode, CallReturnDescriptor, CallSiteDescriptor,
    CallTargetKindDescriptor, CarrierLayoutDescriptor, CoercionDescriptor, DescriptorFamily,
    DescriptorIdentity, DescriptorInventory, ExpressionSemanticsDescriptor, HostProcedureExport,
    Instruction, NameBindingDescriptor, ObjectMemberBindingDescriptor, ObjectTypeDescriptor,
    OperatorFamilyDescriptor, OperatorSemanticsDescriptor, OptionalDefaultValue,
    OptionalParameterDescriptor, OxBundle, ParameterDescriptor, ProcedureRuntimeMetadata,
    ProcedureRuntimeSlotKind, ProcedureSignatureDescriptor, ProjectComWithEventsRoute,
    ProjectDynamicMemberKind, ProjectDynamicMemberRoute, ProjectDynamicObjectRoute,
    ProjectDynamicParamRoute, ResolvedParameterMechanism, RuntimeCarrierKind, SlotRole,
    SlotTypeDescriptor, UdtFieldDescriptor, UdtTypeDescriptor, ValueStateDescriptor, VbaTypeId,
    bundle::ExportInventory,
    bytecode::{
        ComMemberSelectorDescriptor, ExternalCallDescriptor, ExternalCallWriteback,
        ExternalCallWritebackKind, ProjectMemberCallKind, RuntimeArrayElementType,
        StringCompareMode,
    },
    canonical_descriptor_id, descriptor_digest_debug, descriptor_digest_from_fields,
    descriptor_identity_debug,
};
use oxvba_hal::{
    HalComDynamicBridge,
    adapters::builder::HostBuilder,
    error::{HalError, HalErrorKind},
    model::{CapabilityId, HostPolicy, native_host_profile},
    traits::{DynLinkDescriptorView, HostServices},
};
use oxvba_runtime::safe_array::{
    SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
    VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_UI1_VALUE,
    VT_VARIANT_VALUE,
};
use oxvba_runtime::{
    BindingHandle, ObjectRef, RuntimeClassDescriptor, RuntimeInterfaceDescriptor,
    RuntimeInterfaceId, RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeParamDescriptor,
    RuntimeValueType, VarType, Variant, bstr::BStr,
};

use crate::register_file::{RegisterFile, RuntimeSlot};

fn parse_embedded_runtime_error_code(message: &str) -> Option<i32> {
    let marker = "runtime error: ";
    let start = message.find(marker)? + marker.len();
    message[start..]
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .next()
        .and_then(|s| s.parse::<i32>().ok())
}

#[derive(Debug, Default, Clone)]
struct WithEventsOwnerIterator {
    owners: Vec<ObjectRef>,
    next_index: usize,
}

#[derive(Debug, Clone)]
struct ForEachIteratorState {
    items: Vec<RuntimeSlot>,
    next_index: usize,
}

#[derive(Debug, Clone)]
struct ForEachInitError {
    code: i32,
    detail: String,
}

/// Saved error-handling state for one procedure activation.
#[derive(Debug, Clone, Default)]
struct ErrorFrame {
    on_error_resume_next: bool,
    on_error_goto_label_target: Option<usize>,
    last_error: i32,
    last_error_pc: Option<usize>,
    last_error_description: Option<String>,
    last_error_source: Option<String>,
}

#[derive(Debug, Clone)]
struct ComWithEventsSubscription {
    owner_object: ObjectRef,
    route: ProjectComWithEventsRoute,
}

#[derive(Debug, Clone)]
struct ProjectDynamicObjectState {
    object: ObjectRef,
    route: ProjectDynamicObjectRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugBreakpoint {
    pub module_name: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSourceLocation {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub statement_pc: usize,
    pub line_number: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugStopReason {
    Entry,
    Breakpoint,
    Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugStop {
    pub reason: DebugStopReason,
    pub location: DebugSourceLocation,
    pub call_stack_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugRunResult {
    Paused(DebugStop),
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugRuntimeSnapshot {
    pub last_pause: Option<DebugStop>,
    pub activation_entry_pcs: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct VmExecutionPackage<'a> {
    pub bytecode: &'a Bytecode,
    pub procedure_metadata: &'a BTreeMap<String, ProcedureRuntimeMetadata>,
    pub project_context: Option<&'a BundleProjectContext>,
    pub export_inventory: Option<&'a ExportInventory>,
    pub descriptor_inventory: Option<&'a DescriptorInventory>,
    pub dynamic_object_routes: Option<&'a [ProjectDynamicObjectRoute]>,
    pub com_withevents_routes: Option<&'a [ProjectComWithEventsRoute]>,
    pub package_origin: VmPackageOrigin,
}

impl<'a> VmExecutionPackage<'a> {
    pub fn new(
        bytecode: &'a Bytecode,
        procedure_metadata: &'a BTreeMap<String, ProcedureRuntimeMetadata>,
    ) -> Self {
        Self {
            bytecode,
            procedure_metadata,
            project_context: None,
            export_inventory: None,
            descriptor_inventory: None,
            dynamic_object_routes: None,
            com_withevents_routes: None,
            package_origin: VmPackageOrigin::InMemory,
        }
    }

    pub fn from_bundle(bundle: &'a OxBundle) -> Self {
        Self {
            bytecode: &bundle.bytecode,
            procedure_metadata: &bundle.procedure_metadata,
            project_context: bundle.project_context.as_ref(),
            export_inventory: bundle.export_inventory.as_ref(),
            descriptor_inventory: bundle.descriptor_inventory.as_ref(),
            dynamic_object_routes: bundle.dynamic_object_routes.as_deref(),
            com_withevents_routes: bundle.com_withevents_routes.as_deref(),
            package_origin: VmPackageOrigin::OxBundle,
        }
    }

    pub fn identity_evidence(&self) -> VmPackageIdentityEvidence {
        self.identity_evidence_with_runtime_slots(&[])
    }

    fn identity_evidence_with_runtime_slots(
        &self,
        runtime_slots: &[RuntimeSlot],
    ) -> VmPackageIdentityEvidence {
        let bytecode_descriptor_id =
            canonical_descriptor_id(DescriptorFamily::Bytecode, ["instruction-stream"]);
        let bytecode_digest = descriptor_digest_debug(
            DescriptorFamily::Bytecode,
            &bytecode_descriptor_id,
            self.bytecode,
        );
        let package_digest = digest_package(
            &bytecode_digest,
            self.procedure_metadata,
            self.project_context,
            self.export_inventory,
            self.descriptor_inventory,
            self.dynamic_object_routes,
            self.com_withevents_routes,
        );
        let procedures = self
            .procedure_metadata
            .values()
            .map(VmProcedureIdentityEvidence::from_metadata)
            .collect();
        let signature_call_evidence =
            collect_signature_call_evidence(self.bytecode, self.procedure_metadata);
        let call_site_evidence = collect_call_site_descriptor_evidence(self.procedure_metadata);
        let array_shape_evidence =
            collect_array_shape_evidence(self.procedure_metadata, runtime_slots);
        let udt_descriptor_evidence = collect_udt_descriptor_evidence(self.procedure_metadata);
        let package_route_object_descriptor_evidence =
            collect_package_route_object_descriptor_evidence(
                self.dynamic_object_routes,
                self.com_withevents_routes,
            );
        let mut object_descriptor_evidence =
            collect_object_descriptor_evidence(self.procedure_metadata);
        object_descriptor_evidence.extend(package_route_object_descriptor_evidence.clone());
        sort_object_descriptor_evidence(&mut object_descriptor_evidence);
        let interop_descriptor_evidence = collect_interop_descriptor_evidence(
            self.bytecode,
            self.export_inventory,
            self.descriptor_inventory,
        );
        let lifecycle_evidence = collect_lifecycle_evidence(self.procedure_metadata, runtime_slots);
        let error_descriptor_evidence = collect_error_descriptor_evidence(self.bytecode);
        let deopt_snapshot_evidence = collect_deopt_snapshot_evidence(
            self.bytecode,
            self.procedure_metadata,
            &lifecycle_evidence,
            self.project_context,
        );
        let host_policy_evidence = collect_host_policy_evidence(self.project_context);
        let carrier_layout_evidence = collect_carrier_layout_evidence(self.procedure_metadata);
        let value_state_evidence = collect_value_state_evidence(self.procedure_metadata);
        let expression_semantics_evidence =
            collect_expression_semantics_evidence(self.procedure_metadata);
        let operator_semantics_evidence =
            collect_operator_semantics_evidence(self.procedure_metadata);
        let coercion_evidence = collect_coercion_evidence(self.procedure_metadata);
        let name_binding_evidence = collect_name_binding_evidence(self.procedure_metadata);
        let object_member_binding_evidence =
            collect_object_member_binding_evidence(self.procedure_metadata);
        let descriptor_sources = DescriptorIdentityEvidenceSources {
            interop: &interop_descriptor_evidence,
            lifecycle: &lifecycle_evidence,
            error: &error_descriptor_evidence,
            deopt: &deopt_snapshot_evidence,
            host_policy: &host_policy_evidence,
            package_route_object: &package_route_object_descriptor_evidence,
        };
        let descriptor_identities = collect_descriptor_identity_evidence(
            self.bytecode,
            self.procedure_metadata,
            descriptor_sources,
        );
        VmPackageIdentityEvidence {
            package_origin: self.package_origin,
            package_digest,
            bytecode_digest,
            slot_count: self.bytecode.slot_count,
            user_slot_count: self.bytecode.user_slot_count,
            procedures,
            signature_call_evidence,
            call_site_evidence,
            array_shape_evidence,
            udt_descriptor_evidence,
            object_descriptor_evidence,
            interop_descriptor_evidence,
            lifecycle_evidence,
            error_descriptor_evidence,
            deopt_snapshot_evidence,
            host_policy_evidence,
            carrier_layout_evidence,
            value_state_evidence,
            expression_semantics_evidence,
            operator_semantics_evidence,
            coercion_evidence,
            name_binding_evidence,
            object_member_binding_evidence,
            descriptor_identities,
            project_context: self
                .project_context
                .map(VmProjectContextEvidence::from_context),
        }
    }

    pub fn slot_type_descriptors(&self) -> BTreeMap<String, Vec<SlotTypeDescriptor>> {
        self.procedure_metadata
            .iter()
            .map(|(name, metadata)| (name.clone(), metadata.slot_type_descriptors()))
            .collect()
    }

    pub fn procedure_signature_descriptors(
        &self,
    ) -> BTreeMap<String, ProcedureSignatureDescriptor> {
        self.procedure_metadata
            .iter()
            .map(|(name, metadata)| (name.clone(), metadata.procedure_signature_descriptor()))
            .collect()
    }

    pub fn call_site_descriptors(&self) -> BTreeMap<String, Vec<CallSiteDescriptor>> {
        self.procedure_metadata
            .iter()
            .map(|(name, metadata)| (name.clone(), metadata.call_sites.clone()))
            .collect()
    }

    pub fn udt_type_descriptors(&self) -> BTreeMap<String, Vec<UdtTypeDescriptor>> {
        self.procedure_metadata
            .iter()
            .map(|(name, metadata)| (name.clone(), metadata.udt_type_descriptors()))
            .collect()
    }

    pub fn object_type_descriptors(&self) -> BTreeMap<String, Vec<ObjectTypeDescriptor>> {
        self.procedure_metadata
            .iter()
            .map(|(name, metadata)| (name.clone(), metadata.object_type_descriptors()))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmPackageOrigin {
    InMemory,
    OxBundle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmProcedureIdentityEvidence {
    pub procedure_id: String,
    pub procedure_descriptor_id: String,
    pub procedure_descriptor_digest: String,
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub slot_descriptor_digest: String,
    pub slot_descriptors: Vec<SlotTypeDescriptor>,
}

impl VmProcedureIdentityEvidence {
    fn from_metadata(metadata: &ProcedureRuntimeMetadata) -> Self {
        let module_name = if metadata.module_name.trim().is_empty() {
            "<anonymous>".to_string()
        } else {
            metadata.module_name.clone()
        };
        let slot_descriptors = metadata.slot_type_descriptors();
        let procedure_id = format!(
            "proc:{}::{}@pc:{}",
            module_name, metadata.procedure_name, metadata.entry_pc
        );
        let procedure_descriptor_id = procedure_descriptor_id(metadata);
        let procedure_descriptor_digest = descriptor_digest_debug(
            DescriptorFamily::Procedure,
            &procedure_descriptor_id,
            metadata,
        );
        let slot_descriptor_identities = slot_descriptors
            .iter()
            .map(|descriptor| slot_descriptor_identity(&procedure_descriptor_id, descriptor))
            .collect::<Vec<_>>();
        let slot_descriptor_digest = descriptor_digest_debug(
            DescriptorFamily::DescriptorSet,
            &canonical_descriptor_id(
                DescriptorFamily::DescriptorSet,
                [procedure_descriptor_id.as_str(), "slot-descriptors"],
            ),
            &slot_descriptor_identities,
        );
        Self {
            procedure_id,
            procedure_descriptor_id,
            procedure_descriptor_digest,
            module_name,
            procedure_name: metadata.procedure_name.clone(),
            entry_pc: metadata.entry_pc,
            slot_descriptor_digest,
            slot_descriptors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmPackageIdentityEvidence {
    pub package_origin: VmPackageOrigin,
    pub package_digest: String,
    pub bytecode_digest: String,
    pub slot_count: usize,
    pub user_slot_count: usize,
    pub procedures: Vec<VmProcedureIdentityEvidence>,
    pub signature_call_evidence: Vec<VmSignatureCallEvidence>,
    pub call_site_evidence: Vec<VmCallSiteDescriptorEvidence>,
    pub array_shape_evidence: Vec<VmArrayShapeEvidence>,
    pub udt_descriptor_evidence: Vec<VmUdtDescriptorEvidence>,
    pub object_descriptor_evidence: Vec<VmObjectDescriptorEvidence>,
    pub interop_descriptor_evidence: Vec<VmInteropDescriptorEvidence>,
    pub lifecycle_evidence: Vec<VmLifecycleEvidence>,
    pub error_descriptor_evidence: Vec<VmErrorDescriptorEvidence>,
    pub deopt_snapshot_evidence: Vec<VmDeoptSnapshotEvidence>,
    pub host_policy_evidence: Vec<VmHostPolicyEvidence>,
    pub carrier_layout_evidence: Vec<VmCarrierLayoutEvidence>,
    pub value_state_evidence: Vec<VmValueStateEvidence>,
    pub expression_semantics_evidence: Vec<VmSemanticDescriptorEvidence>,
    pub operator_semantics_evidence: Vec<VmSemanticDescriptorEvidence>,
    pub coercion_evidence: Vec<VmSemanticDescriptorEvidence>,
    pub name_binding_evidence: Vec<VmSemanticDescriptorEvidence>,
    pub object_member_binding_evidence: Vec<VmSemanticDescriptorEvidence>,
    pub descriptor_identities: Vec<VmDescriptorIdentityEvidence>,
    pub project_context: Option<VmProjectContextEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmProjectContextEvidence {
    pub project_name: String,
    pub project_kind: String,
    pub compile_context: Vec<String>,
    pub modules: Vec<String>,
    pub module_compile_options: Vec<String>,
    pub references: Vec<String>,
    pub referenced_projects: Vec<String>,
    pub source_maps: Vec<String>,
    pub native_libraries: Vec<String>,
    pub host_capabilities: Vec<String>,
    pub package_diagnostics: Vec<String>,
    pub gap_classifications: Vec<String>,
}

impl VmProjectContextEvidence {
    fn from_context(context: &BundleProjectContext) -> Self {
        Self {
            project_name: context.project_name.clone(),
            project_kind: context.project_kind.clone(),
            compile_context: context
                .compile_context
                .builtin_conditional_constants
                .iter()
                .chain(
                    context
                        .compile_context
                        .manifest_conditional_constants
                        .iter(),
                )
                .map(|constant| {
                    format!("{}:{}={}", constant.source, constant.name, constant.value)
                })
                .chain(std::iter::once(format!(
                    "target:pointer-width={}",
                    context.compile_context.target_pointer_width_bits
                )))
                .chain(std::iter::once(format!(
                    "target:longptr-carrier={}",
                    context.compile_context.long_ptr_carrier
                )))
                .chain(std::iter::once(format!(
                    "target:vba7={}",
                    context.compile_context.vba7
                )))
                .chain(std::iter::once(format!(
                    "target:win64={}",
                    context.compile_context.win64
                )))
                .chain(std::iter::once(format!(
                    "target:longlong-supported={}",
                    context.compile_context.long_long_supported
                )))
                .collect(),
            modules: context
                .modules
                .iter()
                .map(|module| format!("{}:{}:{}", module.kind, module.name, module.module_id))
                .collect(),
            module_compile_options: context
                .modules
                .iter()
                .map(|module| {
                    format!(
                        "{}:explicit={}:compare={}:base={}:def={}:declares={}:ptrsafe={}:longptr={}:longlong={}:exposed={}:creatable={}:predeclared={}:global={}",
                        module.name,
                        module.option_explicit,
                        module.option_compare,
                        module.option_base,
                        module.default_type_families.len(),
                        module.external_declare_count,
                        module.ptrsafe_declare_count,
                        module.uses_long_ptr,
                        module.uses_long_long,
                        module.vb_exposed,
                        module.vb_creatable,
                        module.vb_predeclared_id,
                        module.vb_global_namespace,
                    )
                })
                .collect(),
            references: context
                .references
                .iter()
                .map(|reference| {
                    format!(
                        "{}:{}:{}",
                        reference.kind, reference.name, reference.resolution
                    )
                })
                .collect(),
            referenced_projects: context
                .referenced_projects
                .iter()
                .map(|project| {
                    format!(
                        "{}:{}:{}",
                        project.source, project.project_name, project.module_count
                    )
                })
                .collect(),
            source_maps: context
                .source_maps
                .iter()
                .map(|source_map| format!("{}:{}", source_map.module_name, source_map.lines.len()))
                .collect(),
            native_libraries: context
                .native_libraries
                .iter()
                .map(|library| {
                    format!(
                        "{}:{}:{}",
                        library.declared_name, library.library, library.calling_convention
                    )
                })
                .collect(),
            host_capabilities: context
                .host_capabilities
                .iter()
                .map(|capability| format!("{}:{}", capability.capability, capability.source))
                .collect(),
            package_diagnostics: context
                .package_diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "{}:{}:{}",
                        diagnostic.severity, diagnostic.code, diagnostic.fact_id
                    )
                })
                .collect(),
            gap_classifications: context
                .gap_classifications
                .iter()
                .map(|gap| format!("{}:{}:{}", gap.area, gap.status, gap.gap_id))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmDescriptorIdentityEvidence {
    pub family: String,
    pub descriptor_id: String,
    pub descriptor_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmSignatureCallEvidence {
    pub call_pc: usize,
    pub procedure_id: String,
    pub procedure_name: String,
    pub target_pc: usize,
    pub signature_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmCallSiteDescriptorEvidence {
    pub call_site_id: String,
    pub caller_procedure_name: String,
    pub call_pc: usize,
    pub target_name: String,
    pub call_site_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmArrayShapeEvidence {
    pub procedure_name: String,
    pub array_name: String,
    pub array_shape_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmUdtDescriptorEvidence {
    pub procedure_name: String,
    pub type_name: String,
    pub udt_descriptor_id: String,
    pub udt_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmObjectDescriptorEvidence {
    pub procedure_name: String,
    pub type_name: String,
    pub object_descriptor_id: String,
    pub object_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmInteropDescriptorEvidence {
    pub interop_kind: String,
    pub descriptor_id: String,
    pub descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmLifecycleEvidence {
    pub procedure_name: String,
    pub cleanup_scope_id: String,
    pub lifecycle_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmErrorDescriptorEvidence {
    pub error_scope_id: String,
    pub error_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmDeoptSnapshotEvidence {
    pub safepoint_id: String,
    pub deopt_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmHostPolicyEvidence {
    pub host_policy_id: String,
    pub host_policy_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmCarrierLayoutEvidence {
    pub procedure_name: String,
    pub carrier_key: String,
    pub carrier_layout_descriptor_id: String,
    pub carrier_layout_descriptor_digest: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmValueStateEvidence {
    pub procedure_name: String,
    pub value_state_descriptor_id: String,
    pub value_state_descriptor_digest: String,
    pub state: String,
    pub source: String,
    pub slot: Option<usize>,
    pub pc: Option<usize>,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmSemanticDescriptorEvidence {
    pub procedure_name: String,
    pub descriptor_id: String,
    pub descriptor_digest: String,
    pub descriptor_family: String,
    pub observations: Vec<String>,
}

const CALL_EVIDENCE_LOOKBACK: usize = 32;
const CALL_EVIDENCE_COPY_LIMIT: usize = 8;
const DESCRIPTOR_INTRINSIC_LOOKBACK: usize = 8;
const SELECTED_CALL_BYVAL_COERCION_ID: &str = "COERCE-CALL-BYVAL-DECLARED-TARGET";
const SELECTED_CALL_BYVAL_NUMERIC_WIDEN_ID: &str = "COERCE-LET-NUMERIC-WIDEN";
const SELECTED_CALL_BYVAL_RUNTIME_HELPER_ID: &str = "oxvba_runtime::coerce_to";

struct DescriptorIdentityEvidenceSources<'a> {
    interop: &'a [VmInteropDescriptorEvidence],
    lifecycle: &'a [VmLifecycleEvidence],
    error: &'a [VmErrorDescriptorEvidence],
    deopt: &'a [VmDeoptSnapshotEvidence],
    host_policy: &'a [VmHostPolicyEvidence],
    package_route_object: &'a [VmObjectDescriptorEvidence],
}

fn collect_descriptor_identity_evidence(
    bytecode: &Bytecode,
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    sources: DescriptorIdentityEvidenceSources<'_>,
) -> Vec<VmDescriptorIdentityEvidence> {
    let mut identities = Vec::new();
    identities.push(VmDescriptorIdentityEvidence::from(
        descriptor_identity_debug(
            DescriptorFamily::Bytecode,
            canonical_descriptor_id(DescriptorFamily::Bytecode, ["instruction-stream"]),
            bytecode,
        ),
    ));

    for metadata in procedure_metadata.values() {
        let procedure_id = procedure_descriptor_id(metadata);
        identities.push(VmDescriptorIdentityEvidence::from(
            descriptor_identity_debug(DescriptorFamily::Procedure, procedure_id.clone(), metadata),
        ));
        identities.push(VmDescriptorIdentityEvidence::from(
            descriptor_identity_debug(
                DescriptorFamily::ProcedureSignature,
                signature_descriptor_id(&procedure_id),
                &metadata.procedure_signature_descriptor(),
            ),
        ));
        for slot_descriptor in metadata.slot_type_descriptors() {
            identities.push(VmDescriptorIdentityEvidence::from(
                slot_descriptor_identity(&procedure_id, &slot_descriptor),
            ));
        }
        for call_site in &metadata.call_sites {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::CallSite,
                    call_site_descriptor_id(&procedure_id, call_site),
                    call_site,
                ),
            ));
        }
        for array_shape in &metadata.array_shapes {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::ArrayShape,
                    array_shape_descriptor_id(&procedure_id, array_shape),
                    array_shape,
                ),
            ));
        }
        for udt_type in &metadata.udt_types {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::UdtType,
                    udt_type.descriptor_id.clone(),
                    udt_type,
                ),
            ));
        }
        for object_type in &metadata.object_types {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::ObjectType,
                    object_type.descriptor_id.clone(),
                    object_type,
                ),
            ));
        }
        for carrier_layout in &metadata.carrier_layouts {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::CarrierLayout,
                    carrier_layout.descriptor_id.clone(),
                    carrier_layout,
                ),
            ));
        }
        for value_state in &metadata.value_states {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::ValueState,
                    value_state.descriptor_id.clone(),
                    value_state,
                ),
            ));
        }
        for descriptor in &metadata.expression_semantics {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::ExpressionSemantics,
                    descriptor.descriptor_id.clone(),
                    descriptor,
                ),
            ));
        }
        for descriptor in &metadata.operator_semantics {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::OperatorSemantics,
                    descriptor.descriptor_id.clone(),
                    descriptor,
                ),
            ));
        }
        for descriptor in &metadata.coercions {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::Coercion,
                    descriptor.descriptor_id.clone(),
                    descriptor,
                ),
            ));
        }
        for descriptor in &metadata.name_bindings {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::NameBinding,
                    descriptor.descriptor_id.clone(),
                    descriptor,
                ),
            ));
        }
        for descriptor in &metadata.object_member_bindings {
            identities.push(VmDescriptorIdentityEvidence::from(
                descriptor_identity_debug(
                    DescriptorFamily::ObjectMemberBinding,
                    descriptor.descriptor_id.clone(),
                    descriptor,
                ),
            ));
        }
    }

    identities.extend(
        sources
            .interop
            .iter()
            .map(|evidence| VmDescriptorIdentityEvidence {
                family: DescriptorFamily::Interop.registry_key().to_string(),
                descriptor_id: evidence.descriptor_id.clone(),
                descriptor_digest: evidence.descriptor_digest.clone(),
            }),
    );
    identities.extend(
        sources
            .lifecycle
            .iter()
            .map(|evidence| VmDescriptorIdentityEvidence {
                family: DescriptorFamily::Lifecycle.registry_key().to_string(),
                descriptor_id: evidence.cleanup_scope_id.clone(),
                descriptor_digest: evidence.lifecycle_descriptor_digest.clone(),
            }),
    );
    identities.extend(
        sources
            .error
            .iter()
            .map(|evidence| VmDescriptorIdentityEvidence {
                family: DescriptorFamily::ErrorRouting.registry_key().to_string(),
                descriptor_id: evidence.error_scope_id.clone(),
                descriptor_digest: evidence.error_descriptor_digest.clone(),
            }),
    );
    identities.extend(
        sources
            .deopt
            .iter()
            .map(|evidence| VmDescriptorIdentityEvidence {
                family: DescriptorFamily::DeoptSnapshot.registry_key().to_string(),
                descriptor_id: evidence.safepoint_id.clone(),
                descriptor_digest: evidence.deopt_descriptor_digest.clone(),
            }),
    );
    identities.extend(
        sources
            .host_policy
            .iter()
            .map(|evidence| VmDescriptorIdentityEvidence {
                family: DescriptorFamily::HostPolicy.registry_key().to_string(),
                descriptor_id: evidence.host_policy_id.clone(),
                descriptor_digest: evidence.host_policy_descriptor_digest.clone(),
            }),
    );
    identities.extend(sources.package_route_object.iter().map(|evidence| {
        VmDescriptorIdentityEvidence {
            family: DescriptorFamily::ObjectType.registry_key().to_string(),
            descriptor_id: evidence.object_descriptor_id.clone(),
            descriptor_digest: evidence.object_descriptor_digest.clone(),
        }
    }));
    identities.sort_by(|left, right| {
        left.family
            .cmp(&right.family)
            .then(left.descriptor_id.cmp(&right.descriptor_id))
    });
    identities
}

impl From<DescriptorIdentity> for VmDescriptorIdentityEvidence {
    fn from(identity: DescriptorIdentity) -> Self {
        Self {
            family: identity.family.registry_key().to_string(),
            descriptor_id: identity.descriptor_id,
            descriptor_digest: identity.descriptor_digest,
        }
    }
}

fn procedure_descriptor_id(metadata: &ProcedureRuntimeMetadata) -> String {
    let module_name = if metadata.module_name.trim().is_empty() {
        "<anonymous>"
    } else {
        metadata.module_name.as_str()
    };
    let entry_pc = metadata.entry_pc.to_string();
    canonical_descriptor_id(
        DescriptorFamily::Procedure,
        [
            module_name,
            metadata.procedure_name.as_str(),
            entry_pc.as_str(),
        ],
    )
}

fn signature_descriptor_id(procedure_id: &str) -> String {
    canonical_descriptor_id(DescriptorFamily::ProcedureSignature, [procedure_id])
}

fn slot_descriptor_id(procedure_id: &str, descriptor: &SlotTypeDescriptor) -> String {
    let slot = descriptor.slot.to_string();
    canonical_descriptor_id(DescriptorFamily::Slot, [procedure_id, slot.as_str()])
}

fn slot_descriptor_identity(
    procedure_id: &str,
    descriptor: &SlotTypeDescriptor,
) -> DescriptorIdentity {
    descriptor_identity_debug(
        DescriptorFamily::Slot,
        slot_descriptor_id(procedure_id, descriptor),
        descriptor,
    )
}

fn call_site_descriptor_id(procedure_id: &str, call_site: &CallSiteDescriptor) -> String {
    canonical_descriptor_id(
        DescriptorFamily::CallSite,
        [procedure_id, call_site.call_site_id.as_str()],
    )
}

fn array_shape_descriptor_id(procedure_id: &str, descriptor: &ArrayShapeDescriptor) -> String {
    canonical_descriptor_id(
        DescriptorFamily::ArrayShape,
        [procedure_id, descriptor.name.as_str()],
    )
}

fn collect_array_shape_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    runtime_slots: &[RuntimeSlot],
) -> Vec<VmArrayShapeEvidence> {
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            let procedure_id = procedure_descriptor_id(metadata);
            metadata
                .array_shapes
                .iter()
                .map(move |descriptor| VmArrayShapeEvidence {
                    procedure_name: metadata.procedure_name.clone(),
                    array_name: descriptor.name.clone(),
                    array_shape_descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::ArrayShape,
                        &array_shape_descriptor_id(&procedure_id, descriptor),
                        descriptor,
                    ),
                    observations: array_shape_observations(descriptor, runtime_slots),
                })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(left.array_name.cmp(&right.array_name))
    });
    evidence
}

fn array_shape_observations(
    descriptor: &ArrayShapeDescriptor,
    runtime_slots: &[RuntimeSlot],
) -> Vec<String> {
    let mut observations = vec![
        format!("rank={}", descriptor.rank),
        format!("storage={}", debug_token(&descriptor.storage)),
        format!("element-type={}", debug_token(&descriptor.element_type)),
        format!(
            "element-carrier={}",
            debug_token(&descriptor.element_carrier)
        ),
        format!("option-base={}", descriptor.option_base),
        format!(
            "bounds-policy={}",
            match descriptor.storage {
                ArrayStorageKind::ParamArrayPack => "paramarray-lower-0",
                _ if descriptor.bounds.is_empty() => "runtime-safearray",
                _ => "declared-descriptor",
            }
        ),
        format!(
            "element-lifecycle={}",
            array_element_lifecycle_token(&descriptor.element_carrier)
        ),
    ];
    match descriptor.storage {
        ArrayStorageKind::StaticFixed => {
            observations.push("allocation-policy=compile-time-fixed-slots".to_string());
            observations.push("erase-policy=reset-fixed-elements".to_string());
            observations.push("preserve-policy=not-resizable".to_string());
        }
        ArrayStorageKind::Dynamic => {
            observations.push("allocation-policy=runtime-safearray".to_string());
            observations.push("redim-policy=runtime-shape".to_string());
            observations.push("erase-policy=release-runtime-safearray".to_string());
            observations.push("preserve-policy=last-dimension-only".to_string());
        }
        ArrayStorageKind::ParamArrayPack => {
            observations.push("allocation-policy=call-entry-paramarray-pack".to_string());
            observations.push("preserve-policy=not-resizable".to_string());
        }
    }
    if descriptor.base_slot.is_some() {
        observations.push("base-slot-known".to_string());
    } else {
        observations.push("base-slot-missing".to_string());
    }
    if descriptor.bounds.is_empty() {
        observations.push("declared-bounds=runtime".to_string());
    } else {
        for bound in &descriptor.bounds {
            observations.push(format!(
                "declared-dim{}={}..{}",
                bound.dimension, bound.lower_bound, bound.upper_bound
            ));
        }
    }
    observations.extend(runtime_array_shape_observations(descriptor, runtime_slots));
    observations
}

fn array_element_lifecycle_token(carrier: &RuntimeCarrierKind) -> &'static str {
    match carrier {
        RuntimeCarrierKind::BStr => "owned-bstr-elements",
        RuntimeCarrierKind::ObjectRef => "owned-objectref-elements",
        RuntimeCarrierKind::SafeArray => "owned-safearray-elements",
        RuntimeCarrierKind::Variant => "owned-variant-payload-elements",
        RuntimeCarrierKind::UdtFields { .. } => "owned-udt-fields",
        _ => "plain-scalar-elements",
    }
}

fn runtime_array_shape_observations(
    descriptor: &ArrayShapeDescriptor,
    runtime_slots: &[RuntimeSlot],
) -> Vec<String> {
    if runtime_slots.is_empty() {
        return Vec::new();
    }
    let Some(slot) = descriptor.base_slot else {
        return Vec::new();
    };
    let Some(runtime_slot) = runtime_slots.get(slot) else {
        return vec!["runtime-slot-out-of-range".to_string()];
    };
    let RuntimeSlot::Variant(value) = runtime_slot else {
        return vec!["runtime-slot-non-variant".to_string()];
    };
    let Some(array) = value.as_safearray() else {
        return vec!["runtime-bounds=unallocated".to_string()];
    };
    let mut observations = vec![format!("runtime-rank={}", array.dimensions())];
    if let Some(bounds) = array.bounds() {
        for (index, bound) in bounds.iter().enumerate() {
            let upper = i64::from(bound.lower) + i64::from(bound.count) - 1;
            observations.push(format!(
                "runtime-dim{}={}..{}",
                index + 1,
                bound.lower,
                upper
            ));
        }
    } else {
        observations.push("runtime-bounds=unknown".to_string());
    }
    observations
}

fn collect_udt_descriptor_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmUdtDescriptorEvidence> {
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            metadata
                .udt_types
                .iter()
                .map(move |descriptor| VmUdtDescriptorEvidence {
                    procedure_name: metadata.procedure_name.clone(),
                    type_name: descriptor.type_name.clone(),
                    udt_descriptor_id: descriptor.descriptor_id.clone(),
                    udt_descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::UdtType,
                        &descriptor.descriptor_id,
                        descriptor,
                    ),
                    observations: udt_descriptor_observations(descriptor),
                })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(left.type_name.cmp(&right.type_name))
    });
    evidence
}

fn udt_descriptor_observations(descriptor: &UdtTypeDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("descriptor-id={}", descriptor.descriptor_id),
        format!("storage={}", debug_token(&descriptor.storage)),
        format!("copy={}", debug_token(&descriptor.copy_semantics)),
        "init=recursive-field-defaults".to_string(),
        "layout=descriptor-field-order".to_string(),
        format!(
            "cleanup=bstr:{}:object:{}:safearray:{}:variant:{}",
            descriptor.cleanup.owns_bstr,
            descriptor.cleanup.owns_object_ref,
            descriptor.cleanup.owns_safearray,
            descriptor.cleanup.owns_variant
        ),
    ];
    for instance in &descriptor.instances {
        observations.push(format!(
            "instance:{}:{}",
            instance.name.to_ascii_lowercase(),
            if instance.base_slot.is_some() {
                "base-slot-known"
            } else {
                "base-slot-missing"
            }
        ));
    }
    for field in &descriptor.fields {
        observations.extend(udt_field_observations(field));
    }
    observations
}

fn udt_field_observations(field: &UdtFieldDescriptor) -> Vec<String> {
    let field_name = field.name.to_ascii_lowercase();
    let mut observations = vec![
        format!("field:{field_name}:index={}", field.index),
        format!("field:{field_name}:layout-index={}", field.index),
        format!("field:{field_name}:init=declared-type-default"),
        format!(
            "field:{field_name}:type={}",
            debug_token(&field.declared_type)
        ),
        format!("field:{field_name}:carrier={}", debug_token(&field.carrier)),
    ];
    if let Some(nested) = &field.nested_udt_name {
        observations.push(format!(
            "field:{field_name}:nested-udt={}",
            nested.to_ascii_lowercase()
        ));
    }
    if let Some(len) = field.fixed_string_len {
        observations.push(format!("field:{field_name}:fixed-string-len={len}"));
    }
    if !field.array_bounds.is_empty() {
        for bound in &field.array_bounds {
            observations.push(format!(
                "field:{field_name}:array-dim{}={}..{}",
                bound.dimension, bound.lower_bound, bound.upper_bound
            ));
        }
    }
    for alias in &field.aliases {
        observations.push(format!(
            "field:{field_name}:alias:{}:{}",
            alias.slot_name.to_ascii_lowercase(),
            if alias.slot.is_some() {
                "slot-known"
            } else {
                "slot-missing"
            }
        ));
    }
    observations
}

fn collect_lifecycle_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    runtime_slots: &[RuntimeSlot],
) -> Vec<VmLifecycleEvidence> {
    let mut evidence = Vec::new();
    for metadata in procedure_metadata.values() {
        evidence.extend(collect_slot_lifecycle_evidence(metadata, runtime_slots));
        evidence.extend(collect_array_lifecycle_evidence(metadata, runtime_slots));
        evidence.extend(
            metadata
                .udt_types
                .iter()
                .filter(|descriptor| udt_has_selected_cleanup_obligation(descriptor))
                .map(|descriptor| {
                    let cleanup_scope_id = format!("cleanup:{}", descriptor.descriptor_id);
                    VmLifecycleEvidence {
                        procedure_name: metadata.procedure_name.clone(),
                        cleanup_scope_id: cleanup_scope_id.clone(),
                        lifecycle_descriptor_digest: descriptor_digest_debug(
                            DescriptorFamily::Lifecycle,
                            &cleanup_scope_id,
                            descriptor,
                        ),
                        observations: udt_lifecycle_observations(descriptor, runtime_slots),
                    }
                }),
        );
    }
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(left.cleanup_scope_id.cmp(&right.cleanup_scope_id))
    });
    evidence
}

fn collect_slot_lifecycle_evidence(
    metadata: &ProcedureRuntimeMetadata,
    runtime_slots: &[RuntimeSlot],
) -> Vec<VmLifecycleEvidence> {
    let procedure_key = metadata.procedure_name.to_ascii_lowercase();
    metadata
        .slot_type_descriptors()
        .into_iter()
        .filter_map(|descriptor| {
            let lifecycle_id = selected_slot_lifecycle_id(&descriptor)?;
            let slot_name = descriptor
                .name
                .clone()
                .unwrap_or_else(|| format!("slot{}", descriptor.slot))
                .to_ascii_lowercase();
            let cleanup_scope_id = format!("cleanup:slot:{procedure_key}:{slot_name}");
            Some(VmLifecycleEvidence {
                procedure_name: metadata.procedure_name.clone(),
                cleanup_scope_id: cleanup_scope_id.clone(),
                lifecycle_descriptor_digest: descriptor_digest_debug(
                    DescriptorFamily::Lifecycle,
                    &cleanup_scope_id,
                    &descriptor,
                ),
                observations: slot_lifecycle_observations(&descriptor, lifecycle_id, runtime_slots),
            })
        })
        .collect()
}

fn selected_slot_lifecycle_id(descriptor: &SlotTypeDescriptor) -> Option<&'static str> {
    if descriptor.role == SlotRole::Parameter {
        return None;
    }
    match descriptor.carrier {
        RuntimeCarrierKind::BStr => Some("LIFE-BSTR-SLOT"),
        RuntimeCarrierKind::SafeArray => Some("LIFE-SAFEARRAY-SLOT"),
        RuntimeCarrierKind::ObjectRef => Some("LIFE-OBJECTREF-SLOT"),
        RuntimeCarrierKind::Variant => Some("LIFE-VARIANT-PAYLOAD-SLOT"),
        _ => None,
    }
}

fn slot_lifecycle_observations(
    descriptor: &SlotTypeDescriptor,
    lifecycle_id: &str,
    _runtime_slots: &[RuntimeSlot],
) -> Vec<String> {
    let mut observations = vec![
        "source=SlotTypeDescriptor".to_string(),
        format!("lifecycle-id={lifecycle_id}"),
        format!("slot={}", descriptor.slot),
        format!("role={}", debug_token(&descriptor.role)),
        format!("declared-type={}", debug_token(&descriptor.declared_type)),
        format!("carrier={}", debug_token(&descriptor.carrier)),
        "cleanup-map=slot-owned-carrier".to_string(),
        "success-exit=drop-owned-slot".to_string(),
        "branch-exit=drop-owned-slot".to_string(),
        "error-exit=drop-owned-slot".to_string(),
        "helper-failure=drop-owned-slot".to_string(),
        "deopt=slot-ownership-map-required".to_string(),
    ];
    if let Some(name) = &descriptor.name {
        observations.push(format!("name={}", name.to_ascii_lowercase()));
    }
    observations
}

fn collect_array_lifecycle_evidence(
    metadata: &ProcedureRuntimeMetadata,
    runtime_slots: &[RuntimeSlot],
) -> Vec<VmLifecycleEvidence> {
    let procedure_key = metadata.procedure_name.to_ascii_lowercase();
    metadata
        .array_shapes
        .iter()
        .map(|descriptor| {
            let array_key = descriptor.name.to_ascii_lowercase();
            let cleanup_scope_id = format!("cleanup:array:{procedure_key}:{array_key}");
            VmLifecycleEvidence {
                procedure_name: metadata.procedure_name.clone(),
                cleanup_scope_id: cleanup_scope_id.clone(),
                lifecycle_descriptor_digest: descriptor_digest_debug(
                    DescriptorFamily::Lifecycle,
                    &cleanup_scope_id,
                    descriptor,
                ),
                observations: array_lifecycle_observations(descriptor, runtime_slots),
            }
        })
        .collect()
}

fn array_lifecycle_observations(
    descriptor: &ArrayShapeDescriptor,
    runtime_slots: &[RuntimeSlot],
) -> Vec<String> {
    let lifecycle_id = match descriptor.storage {
        ArrayStorageKind::StaticFixed => "LIFE-STATIC-ARRAY-ELEMENTS",
        ArrayStorageKind::Dynamic => "LIFE-SAFEARRAY-DYNAMIC",
        ArrayStorageKind::ParamArrayPack => "LIFE-PARAMARRAY-PACK",
    };
    let mut observations = vec![
        "source=ArrayShapeDescriptor".to_string(),
        format!("lifecycle-id={lifecycle_id}"),
        format!("array={}", descriptor.name.to_ascii_lowercase()),
        format!("storage={}", debug_token(&descriptor.storage)),
        format!("rank={}", descriptor.rank),
        format!(
            "element-carrier={}",
            debug_token(&descriptor.element_carrier)
        ),
        format!(
            "element-lifecycle={}",
            array_element_lifecycle_token(&descriptor.element_carrier)
        ),
        "cleanup-map=array-elements-and-safearray".to_string(),
        "success-exit=drop-array-carrier".to_string(),
        "branch-exit=drop-array-carrier".to_string(),
        "error-exit=drop-array-carrier".to_string(),
        "helper-failure=drop-array-carrier".to_string(),
        "deopt=array-ownership-map-required".to_string(),
    ];
    for bound in &descriptor.bounds {
        observations.push(format!(
            "declared-dim{}={}..{}",
            bound.dimension, bound.lower_bound, bound.upper_bound
        ));
    }
    observations.extend(runtime_array_shape_observations(descriptor, runtime_slots));
    observations
}

fn udt_has_selected_cleanup_obligation(descriptor: &UdtTypeDescriptor) -> bool {
    descriptor.cleanup.owns_bstr
        || descriptor.cleanup.owns_object_ref
        || descriptor.cleanup.owns_safearray
        || descriptor.cleanup.owns_variant
}

fn udt_lifecycle_observations(
    descriptor: &UdtTypeDescriptor,
    runtime_slots: &[RuntimeSlot],
) -> Vec<String> {
    let mut observations = vec![
        "source=UdtTypeDescriptor".to_string(),
        format!("descriptor-id={}", descriptor.descriptor_id),
        "descriptor-family=SlotLifecycleDescriptor".to_string(),
        "lifecycle-id=LIFE-UDT-FIELD-OWNING".to_string(),
        "cleanup-map=udt-recursive-owned-fields".to_string(),
        "success-exit=drop-owned-fields".to_string(),
        "branch-exit=drop-owned-fields".to_string(),
        "error-exit=drop-owned-fields".to_string(),
        "deopt=field-ownership-map-required".to_string(),
        format!(
            "cleanup=bstr:{}:object:{}:safearray:{}:variant:{}",
            descriptor.cleanup.owns_bstr,
            descriptor.cleanup.owns_object_ref,
            descriptor.cleanup.owns_safearray,
            descriptor.cleanup.owns_variant
        ),
    ];
    for field in &descriptor.fields {
        observations.extend(udt_field_lifecycle_observations(field, runtime_slots));
    }
    observations
}

fn udt_field_lifecycle_observations(
    field: &UdtFieldDescriptor,
    runtime_slots: &[RuntimeSlot],
) -> Vec<String> {
    let field_name = field.name.to_ascii_lowercase();
    let mut observations = Vec::new();
    if let Some(carrier_lifecycle_id) = selected_field_carrier_lifecycle_id(field) {
        observations.push(format!(
            "field:{field_name}:lifecycle-id=LIFE-UDT-FIELD-OWNING"
        ));
        observations.push(format!(
            "field:{field_name}:carrier-lifecycle-id={carrier_lifecycle_id}"
        ));
        observations.push(format!("field:{field_name}:drop-policy=drop-owned-carrier"));
        observations.push(format!(
            "field:{field_name}:helper-cleanup=recursive-by-field"
        ));
        for alias in &field.aliases {
            observations.push(format!(
                "field:{field_name}:alias:{}:{}",
                alias.slot_name.to_ascii_lowercase(),
                if alias.slot.is_some() {
                    "cleanup-slot-known"
                } else {
                    "cleanup-slot-missing"
                }
            ));
            if let Some(slot) = alias.slot.and_then(|slot| runtime_slots.get(slot)) {
                observations.push(format!(
                    "field:{field_name}:alias:{}:runtime-carrier={}",
                    alias.slot_name.to_ascii_lowercase(),
                    runtime_slot_lifecycle_token(slot)
                ));
            }
        }
    }
    observations
}

fn selected_field_carrier_lifecycle_id(field: &UdtFieldDescriptor) -> Option<&'static str> {
    match field.carrier {
        RuntimeCarrierKind::BStr => {
            if field.fixed_string_len.is_some() {
                Some("LIFE-BSTR-FIXED-STRING")
            } else {
                Some("LIFE-BSTR-VARIABLE-STRING")
            }
        }
        RuntimeCarrierKind::ObjectRef => Some("LIFE-OBJECTREF"),
        RuntimeCarrierKind::SafeArray => {
            if field.array_bounds.is_empty() {
                Some("LIFE-SAFEARRAY-DYNAMIC")
            } else {
                Some("LIFE-UDT-FIXED-ARRAY-FIELD")
            }
        }
        RuntimeCarrierKind::Variant => Some("LIFE-VARIANT-PAYLOAD"),
        _ => None,
    }
}

fn collect_error_descriptor_evidence(bytecode: &Bytecode) -> Vec<VmErrorDescriptorEvidence> {
    let mut evidence = bytecode
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(pc, instruction)| {
            let observations = error_descriptor_observations(pc, instruction)?;
            let pc_token = pc.to_string();
            let kind = error_instruction_kind(instruction);
            let error_scope_id = canonical_descriptor_id(
                DescriptorFamily::ErrorRouting,
                ["pc", pc_token.as_str(), kind],
            );
            Some(VmErrorDescriptorEvidence {
                error_scope_id: error_scope_id.clone(),
                error_descriptor_digest: descriptor_digest_from_fields(
                    DescriptorFamily::ErrorRouting,
                    &error_scope_id,
                    [
                        ("pc", pc_token),
                        ("instruction", format!("{instruction:#?}")),
                        ("observations", format!("{observations:#?}")),
                    ],
                ),
                observations,
            })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| left.error_scope_id.cmp(&right.error_scope_id));
    evidence
}

fn error_instruction_kind(instruction: &Instruction) -> &'static str {
    match instruction {
        Instruction::SetOnErrorResumeNext => "on-error-resume-next",
        Instruction::SetOnErrorGoto0 => "on-error-goto-0",
        Instruction::SetOnErrorGotoLabel { .. } => "on-error-goto-label",
        Instruction::ResumeNext => "resume-next",
        Instruction::Resume => "resume",
        Instruction::ResumeLabel { .. } => "resume-label",
        Instruction::RaiseError { .. } => "raise-error",
        Instruction::ClearErr => "err-clear",
        Instruction::CallProc { .. } => "call-frame-error-state",
        Instruction::DivSlots { .. } => "fallible-div",
        Instruction::IntDivSlots { .. } => "fallible-int-div",
        Instruction::ModSlots { .. } => "fallible-mod",
        instruction if instruction_host_boundary_kind(instruction).is_some() => {
            "host-boundary-error"
        }
        _ => "not-error-routing",
    }
}

fn error_descriptor_observations(pc: usize, instruction: &Instruction) -> Option<Vec<String>> {
    let mut observations = vec![
        "source=BytecodeInstruction".to_string(),
        format!("pc={pc}"),
        "err-state=number-description-source-last-error-pc".to_string(),
        "active-error-state=last-error-pc".to_string(),
        "error-policy=vm-error-frame-owned".to_string(),
    ];
    match instruction {
        Instruction::SetOnErrorResumeNext => {
            observations.extend([
                "kind=on-error-resume-next".to_string(),
                "state-transition=enable-resume-next".to_string(),
                "enabled-handler=resume-next".to_string(),
                "goto-label-target=cleared".to_string(),
                "fault-routing=auto-advance-next-pc".to_string(),
                "active-handler=last-error-pc-cleared-on-auto-advance".to_string(),
            ]);
        }
        Instruction::SetOnErrorGoto0 => {
            observations.extend([
                "kind=on-error-goto-0".to_string(),
                "state-transition=disable-handler".to_string(),
                "enabled-handler=none".to_string(),
                "goto-label-target=cleared".to_string(),
                "err-state=preserved-by-current-vm".to_string(),
            ]);
        }
        Instruction::SetOnErrorGotoLabel { target_pc } => {
            observations.extend([
                "kind=on-error-goto-label".to_string(),
                "state-transition=enable-label-handler".to_string(),
                format!("handler-target-pc={target_pc}"),
                "fault-routing=jump-to-handler-target".to_string(),
                "resume-required-to-clear-active-error".to_string(),
            ]);
        }
        Instruction::ResumeNext => {
            observations.extend([
                "kind=resume-next".to_string(),
                "requires-active-error=true".to_string(),
                "resume-target=fault-pc-plus-one".to_string(),
                "resume-without-error=runtime-error-20".to_string(),
                "err-reset=clear-on-success".to_string(),
            ]);
        }
        Instruction::Resume => {
            observations.extend([
                "kind=resume".to_string(),
                "requires-active-error=true".to_string(),
                "resume-target=fault-pc".to_string(),
                "resume-without-error=runtime-error-20".to_string(),
                "err-reset=clear-on-success".to_string(),
            ]);
        }
        Instruction::ResumeLabel { target_pc } => {
            observations.extend([
                "kind=resume-label".to_string(),
                "requires-active-error=true".to_string(),
                format!("resume-target-pc={target_pc}"),
                "resume-without-error=runtime-error-20".to_string(),
                "err-reset=clear-on-success".to_string(),
            ]);
        }
        Instruction::RaiseError { code } => {
            observations.extend([
                "kind=raise-error".to_string(),
                format!("raise-code={code}"),
                "fault-site=pc".to_string(),
                "route=route-runtime-error".to_string(),
                "resume-next-consumable=true".to_string(),
            ]);
        }
        Instruction::ClearErr => {
            observations.extend([
                "kind=err-clear".to_string(),
                "err-reset=number-description-source-last-error-pc".to_string(),
                "handler-state=unchanged".to_string(),
            ]);
        }
        Instruction::CallProc { target_pc, .. } => {
            observations.extend([
                "kind=call-frame-error-state".to_string(),
                format!("call-target-pc={target_pc}"),
                "caller-frame=save-handler-and-err-state".to_string(),
                "callee-entry=clear-handler-and-err-state".to_string(),
                "return-policy=restore-caller-error-frame".to_string(),
                "call-out-resume-next=caller-frame-owned".to_string(),
            ]);
        }
        Instruction::DivSlots { .. }
        | Instruction::IntDivSlots { .. }
        | Instruction::ModSlots { .. } => {
            observations.extend([
                "kind=fallible-helper".to_string(),
                format!("helper={}", error_instruction_kind(instruction)),
                "runtime-error=division-by-zero-11".to_string(),
                "route=route-runtime-error".to_string(),
                "resume-next-consumable=true".to_string(),
                "helper-failure=edge-recorded".to_string(),
            ]);
        }
        instruction => {
            let (operation, capability) = instruction_host_boundary_kind(instruction)?;
            observations.extend([
                "kind=host-boundary-error".to_string(),
                format!("host-operation={operation}"),
                format!("host-capability={capability}"),
                "route=route-host-error".to_string(),
                "runtime-error-map=hal-error-code".to_string(),
                "resume-next-consumable=true".to_string(),
            ]);
        }
    }
    Some(observations)
}

fn collect_deopt_snapshot_evidence(
    bytecode: &Bytecode,
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    lifecycle_evidence: &[VmLifecycleEvidence],
    project_context: Option<&BundleProjectContext>,
) -> Vec<VmDeoptSnapshotEvidence> {
    let slot_descriptor_count = procedure_metadata
        .values()
        .map(|metadata| metadata.slot_type_descriptors().len())
        .sum::<usize>();
    let carrier_layout_count = procedure_metadata
        .values()
        .map(|metadata| metadata.carrier_layouts.len())
        .sum::<usize>();
    let host_capability_count = project_context
        .map(|context| context.host_capabilities.len())
        .unwrap_or(0);
    let mut evidence = bytecode
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(pc, instruction)| {
            let (operation, observations) = deopt_snapshot_observations(
                pc,
                instruction,
                slot_descriptor_count,
                carrier_layout_count,
                lifecycle_evidence.len(),
                host_capability_count,
            )?;
            let pc_token = pc.to_string();
            let safepoint_id = canonical_descriptor_id(
                DescriptorFamily::DeoptSnapshot,
                ["pc", pc_token.as_str(), operation.as_str()],
            );
            Some(VmDeoptSnapshotEvidence {
                safepoint_id: safepoint_id.clone(),
                deopt_descriptor_digest: descriptor_digest_from_fields(
                    DescriptorFamily::DeoptSnapshot,
                    &safepoint_id,
                    [
                        ("pc", pc_token),
                        ("operation", operation),
                        ("instruction", format!("{instruction:#?}")),
                        ("observations", format!("{observations:#?}")),
                    ],
                ),
                observations,
            })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| left.safepoint_id.cmp(&right.safepoint_id));
    evidence
}

fn deopt_snapshot_observations(
    pc: usize,
    instruction: &Instruction,
    slot_descriptor_count: usize,
    carrier_layout_count: usize,
    cleanup_scope_count: usize,
    host_capability_count: usize,
) -> Option<(String, Vec<String>)> {
    let operation = deopt_operation_token(instruction)?;
    let mut observations = vec![
        "kind=deopt-safepoint".to_string(),
        format!("pc={pc}"),
        format!("operation={operation}"),
        "deopt-policy=vm-materialize-or-reject-before-jit-consumption".to_string(),
        "slot-map=procedure-slot-descriptors".to_string(),
        format!("slot-descriptor-count={slot_descriptor_count}"),
        "live-carrier-map=carrier-layout-descriptors".to_string(),
        format!("carrier-layout-count={carrier_layout_count}"),
        "error-state=err-number-description-source-last-error-pc".to_string(),
        "cleanup-state=lifecycle-descriptor-refs".to_string(),
        format!("cleanup-scope-count={cleanup_scope_count}"),
        "byref-state=call-site-and-external-writeback-descriptors".to_string(),
        "source-map=bytecode-pc".to_string(),
    ];
    if host_capability_count > 0 {
        observations.push("host-policy-state=project-context-host-capabilities".to_string());
        observations.push(format!("host-capability-count={host_capability_count}"));
    }
    if instruction_host_boundary_kind(instruction).is_some() {
        observations.push("boundary-state=host-or-interop-call-frame".to_string());
    }
    Some((operation.to_string(), observations))
}

fn deopt_operation_token(instruction: &Instruction) -> Option<&'static str> {
    match instruction {
        Instruction::CallProc { .. } => Some("call-procedure"),
        Instruction::DivSlots { .. } => Some("helper-div"),
        Instruction::IntDivSlots { .. } => Some("helper-int-div"),
        Instruction::ModSlots { .. } => Some("helper-mod"),
        Instruction::ConcatSlots { .. } => Some("helper-bstr-concat"),
        Instruction::IntrinsicCreateObjectHost { .. } => Some("com-createobject"),
        Instruction::IntrinsicDispatchInvokeHost { .. } => Some("com-dispatch-invoke"),
        Instruction::IntrinsicInvokeSymbolHost { .. } => Some("native-declare-invoke"),
        instruction if instruction_host_boundary_kind(instruction).is_some() => {
            Some("host-service-call")
        }
        _ => None,
    }
}

fn collect_host_policy_evidence(
    project_context: Option<&BundleProjectContext>,
) -> Vec<VmHostPolicyEvidence> {
    let Some(context) = project_context else {
        return Vec::new();
    };
    let host_gap = context
        .gap_classifications
        .iter()
        .find(|gap| gap.area == "host-capability")
        .map(|gap| format!("{}:{}", gap.status, gap.gap_id));
    let mut evidence = context
        .host_capabilities
        .iter()
        .map(|capability| {
            let host_policy_id = canonical_descriptor_id(
                DescriptorFamily::HostPolicy,
                [capability.capability_id.as_str()],
            );
            let mut observations = vec![
                "kind=host-capability-policy".to_string(),
                "source=BundleProjectContext.host_capabilities".to_string(),
                format!("bundle-capability-id={}", capability.capability_id),
                format!("capability={}", capability.capability),
                format!("source-operation={}", capability.source),
                "policy-evaluation=HostServices::policy".to_string(),
                "unsupported-diagnostic=HAL-E-CAP-UNAVAILABLE".to_string(),
                "policy-denied-diagnostic=HAL-E-POLICY-DENIED".to_string(),
                "runtime-error-map=Vm::hal_error_code".to_string(),
                "err-projection=route-host-error".to_string(),
                "deterministic-diagnostic=true".to_string(),
            ];
            if let Some(gap) = &host_gap {
                observations.push(format!("gap={gap}"));
            }
            VmHostPolicyEvidence {
                host_policy_id: host_policy_id.clone(),
                host_policy_descriptor_digest: descriptor_digest_from_fields(
                    DescriptorFamily::HostPolicy,
                    &host_policy_id,
                    [
                        ("capability_id", capability.capability_id.clone()),
                        ("capability", capability.capability.clone()),
                        ("source", capability.source.clone()),
                        ("observations", format!("{observations:#?}")),
                    ],
                ),
                observations,
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| left.host_policy_id.cmp(&right.host_policy_id));
    evidence
}

fn implementation_fastpath_operator_descriptor_ids(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<String> {
    let mut descriptor_ids = procedure_metadata
        .values()
        .flat_map(|metadata| {
            metadata
                .operator_semantics
                .iter()
                .filter(|descriptor| {
                    descriptor.family == OperatorFamilyDescriptor::ImplementationFastPath
                })
                .map(|descriptor| descriptor.descriptor_id.clone())
        })
        .collect::<Vec<_>>();
    descriptor_ids.sort();
    descriptor_ids.dedup();
    descriptor_ids
}

fn package_descriptor_fastpaths_enabled(package: &VmExecutionPackage<'_>) -> bool {
    !implementation_fastpath_operator_descriptor_ids(package.procedure_metadata).is_empty()
}

fn instruction_host_boundary_kind(
    instruction: &Instruction,
) -> Option<(&'static str, &'static str)> {
    match instruction {
        Instruction::IntrinsicShellHost { .. } => Some(("Shell", "process-env")),
        Instruction::IntrinsicEnvironHost { .. } => Some(("Environ", "process-env")),
        Instruction::IntrinsicDirHost { .. } => Some(("Dir", "process-env")),
        Instruction::IntrinsicDateNowHost { .. } => Some(("Date", "time-locale")),
        Instruction::IntrinsicTimeNowHost { .. } => Some(("Time", "time-locale")),
        Instruction::IntrinsicNowHost { .. } => Some(("Now", "time-locale")),
        Instruction::IntrinsicTimerHost { .. } => Some(("Timer", "time-locale")),
        Instruction::IntrinsicFreeFileHost { .. } => Some(("FreeFile", "file-system-io")),
        Instruction::IntrinsicFileOpenHost { .. } => Some(("Open", "file-system-io")),
        Instruction::IntrinsicFileCloseHost { .. } => Some(("Close", "file-system-io")),
        Instruction::IntrinsicFileKillHost { .. } => Some(("Kill", "file-system-io")),
        Instruction::IntrinsicFileReadHost { .. } => Some(("Input", "file-system-io")),
        Instruction::IntrinsicFileWriteHost { .. } => Some(("Write", "file-system-io")),
        Instruction::IntrinsicFilePrintHost { .. } => Some(("Print", "file-system-io")),
        Instruction::IntrinsicFileInputHost { .. } => Some(("Input", "file-system-io")),
        Instruction::IntrinsicFileLineInputHost { .. } => Some(("LineInput", "file-system-io")),
        Instruction::IntrinsicFileEofHost { .. } => Some(("EOF", "file-system-io")),
        Instruction::IntrinsicFileLofHost { .. } => Some(("LOF", "file-system-io")),
        Instruction::IntrinsicFileSeekHost { .. } => Some(("Seek", "file-system-io")),
        Instruction::IntrinsicFileLocHost { .. } => Some(("Loc", "file-system-io")),
        Instruction::IntrinsicConsolePrintHost { .. } => Some(("Print", "console-io")),
        Instruction::IntrinsicConsoleInputHost { .. } => Some(("Input", "console-io")),
        Instruction::IntrinsicConsoleLineInputHost { .. } => Some(("LineInput", "console-io")),
        Instruction::IntrinsicMsgBoxHost { .. } => Some(("MsgBox", "ui-interaction")),
        Instruction::IntrinsicInputBoxHost { .. } => Some(("InputBox", "ui-interaction")),
        Instruction::IntrinsicBeepHost { .. } => Some(("Beep", "ui-interaction")),
        Instruction::IntrinsicDebugPrintHost { .. } => {
            Some(("Debug.Print", "diagnostics-telemetry"))
        }
        Instruction::IntrinsicDoEventsHost { .. } => Some(("DoEvents", "event-pump")),
        Instruction::IntrinsicCreateObjectHost { .. } => {
            Some(("CreateObject", "com-activation-dispatch"))
        }
        Instruction::IntrinsicDispatchInvokeHost { .. } => {
            Some(("DispatchInvoke", "com-activation-dispatch"))
        }
        Instruction::IntrinsicComSubscribeEventHost { .. } => {
            Some(("ComSubscribeEvent", "com-activation-dispatch"))
        }
        Instruction::IntrinsicComUnsubscribeEventHost { .. } => {
            Some(("ComUnsubscribeEvent", "com-activation-dispatch"))
        }
        Instruction::IntrinsicComEventCallbackSubscriptionHost { .. } => {
            Some(("ComEventCallbackSubscription", "com-activation-dispatch"))
        }
        Instruction::IntrinsicComEventCallbackArgHost { .. } => {
            Some(("ComEventCallbackArg", "com-activation-dispatch"))
        }
        Instruction::IntrinsicComReleaseEventCallbackHost { .. } => {
            Some(("ComReleaseEventCallback", "com-activation-dispatch"))
        }
        Instruction::IntrinsicInvokeSymbolHost { .. } => Some(("DeclareInvoke", "dynamic-linking")),
        _ => None,
    }
}

fn runtime_slot_lifecycle_token(slot: &RuntimeSlot) -> String {
    match slot {
        RuntimeSlot::Variant(value) => format!("variant-{}", debug_token(&value.vtype())),
        RuntimeSlot::BindingHandle(handle) => format!("binding-handle-{}", handle.raw()),
    }
}

fn collect_carrier_layout_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmCarrierLayoutEvidence> {
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            metadata
                .carrier_layouts
                .iter()
                .map(move |descriptor| VmCarrierLayoutEvidence {
                    procedure_name: metadata.procedure_name.clone(),
                    carrier_key: descriptor.carrier_key.clone(),
                    carrier_layout_descriptor_id: descriptor.descriptor_id.clone(),
                    carrier_layout_descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::CarrierLayout,
                        &descriptor.descriptor_id,
                        descriptor,
                    ),
                    observations: carrier_layout_observations(descriptor),
                })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(left.carrier_key.cmp(&right.carrier_key))
    });
    evidence
}

fn carrier_layout_observations(descriptor: &CarrierLayoutDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("carrier={}", descriptor.carrier_key),
        format!("layout={}", debug_token(&descriptor.layout)),
        format!("storage-bits={:?}", descriptor.storage_bits),
        format!("native-frame-eligible={}", descriptor.native_frame_eligible),
        format!("variant-compatible={}", descriptor.variant_compatible),
    ];
    if let Some(com_variant_type) = &descriptor.com_variant_type {
        observations.push(format!("com-variant-type={com_variant_type}"));
    }
    observations.extend(descriptor.notes.iter().map(|note| format!("note={note}")));
    observations
}

fn collect_value_state_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmValueStateEvidence> {
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            metadata
                .value_states
                .iter()
                .map(move |descriptor| VmValueStateEvidence {
                    procedure_name: metadata.procedure_name.clone(),
                    value_state_descriptor_id: descriptor.descriptor_id.clone(),
                    value_state_descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::ValueState,
                        &descriptor.descriptor_id,
                        descriptor,
                    ),
                    state: debug_token(&descriptor.state),
                    source: debug_token(&descriptor.source),
                    slot: descriptor.slot,
                    pc: descriptor.pc,
                    observations: value_state_observations(descriptor),
                })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(
                left.value_state_descriptor_id
                    .cmp(&right.value_state_descriptor_id),
            )
    });
    evidence
}

fn value_state_observations(descriptor: &ValueStateDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("state={}", debug_token(&descriptor.state)),
        format!("source={}", debug_token(&descriptor.source)),
        format!("detail={}", descriptor.detail),
    ];
    if let Some(slot) = descriptor.slot {
        observations.push(format!("slot={slot}"));
    }
    if let Some(pc) = descriptor.pc {
        observations.push(format!("pc={pc}"));
    }
    if let Some(name) = &descriptor.name {
        observations.push(format!("name={}", name.to_ascii_lowercase()));
    }
    observations
}

fn collect_expression_semantics_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmSemanticDescriptorEvidence> {
    collect_semantic_descriptor_evidence(
        procedure_metadata,
        DescriptorFamily::ExpressionSemantics,
        |metadata| &metadata.expression_semantics,
    )
}

fn collect_operator_semantics_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmSemanticDescriptorEvidence> {
    collect_semantic_descriptor_evidence(
        procedure_metadata,
        DescriptorFamily::OperatorSemantics,
        |metadata| &metadata.operator_semantics,
    )
}

fn collect_coercion_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmSemanticDescriptorEvidence> {
    collect_semantic_descriptor_evidence(
        procedure_metadata,
        DescriptorFamily::Coercion,
        |metadata| &metadata.coercions,
    )
}

fn collect_name_binding_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmSemanticDescriptorEvidence> {
    collect_semantic_descriptor_evidence(
        procedure_metadata,
        DescriptorFamily::NameBinding,
        |metadata| &metadata.name_bindings,
    )
}

fn collect_object_member_binding_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmSemanticDescriptorEvidence> {
    collect_semantic_descriptor_evidence(
        procedure_metadata,
        DescriptorFamily::ObjectMemberBinding,
        |metadata| &metadata.object_member_bindings,
    )
}

trait VmSemanticDescriptorView: Debug {
    fn descriptor_id(&self) -> &str;
    fn observations(&self) -> Vec<String>;
}

impl VmSemanticDescriptorView for ExpressionSemanticsDescriptor {
    fn descriptor_id(&self) -> &str {
        &self.descriptor_id
    }

    fn observations(&self) -> Vec<String> {
        expression_semantics_observations(self)
    }
}

impl VmSemanticDescriptorView for OperatorSemanticsDescriptor {
    fn descriptor_id(&self) -> &str {
        &self.descriptor_id
    }

    fn observations(&self) -> Vec<String> {
        operator_semantics_observations(self)
    }
}

impl VmSemanticDescriptorView for CoercionDescriptor {
    fn descriptor_id(&self) -> &str {
        &self.descriptor_id
    }

    fn observations(&self) -> Vec<String> {
        coercion_observations(self)
    }
}

impl VmSemanticDescriptorView for NameBindingDescriptor {
    fn descriptor_id(&self) -> &str {
        &self.descriptor_id
    }

    fn observations(&self) -> Vec<String> {
        name_binding_observations(self)
    }
}

impl VmSemanticDescriptorView for ObjectMemberBindingDescriptor {
    fn descriptor_id(&self) -> &str {
        &self.descriptor_id
    }

    fn observations(&self) -> Vec<String> {
        object_member_binding_observations(self)
    }
}

fn collect_semantic_descriptor_evidence<T>(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    family: DescriptorFamily,
    descriptors: fn(&ProcedureRuntimeMetadata) -> &Vec<T>,
) -> Vec<VmSemanticDescriptorEvidence>
where
    T: VmSemanticDescriptorView,
{
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            descriptors(metadata).iter().map(move |descriptor| {
                let descriptor_id = descriptor.descriptor_id().to_string();
                VmSemanticDescriptorEvidence {
                    procedure_name: metadata.procedure_name.clone(),
                    descriptor_digest: descriptor_digest_debug(family, &descriptor_id, descriptor),
                    descriptor_id,
                    descriptor_family: family.registry_key().to_string(),
                    observations: descriptor.observations(),
                }
            })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(left.descriptor_id.cmp(&right.descriptor_id))
    });
    evidence
}

fn expression_semantics_observations(descriptor: &ExpressionSemanticsDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("expression-id={}", descriptor.expression_id),
        format!("classification={}", debug_token(&descriptor.classification)),
        format!("declared-type={}", debug_token(&descriptor.declared_type)),
        format!("carrier-hint={}", debug_token(&descriptor.carrier_hint)),
        format!(
            "default-member-policy={}",
            debug_token(&descriptor.default_member_policy)
        ),
        format!("source-context={}", debug_token(&descriptor.source_context)),
        format!("detail={}", descriptor.detail),
    ];
    for state in &descriptor.value_states {
        observations.push(format!("value-state={}", debug_token(state)));
    }
    observations
}

fn operator_semantics_observations(descriptor: &OperatorSemanticsDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("operator-id={}", descriptor.operator_id),
        format!("family={}", debug_token(&descriptor.family)),
        format!("operator={}", debug_token(&descriptor.operator)),
        format!(
            "result-declared-type={}",
            debug_token(&descriptor.result_declared_type)
        ),
        format!("helper-id={}", descriptor.helper_id),
        format!("runtime-error-policy={}", descriptor.runtime_error_policy),
        format!(
            "evaluation-order={}",
            debug_token(&descriptor.evaluation_order)
        ),
        format!("current-vm-status={}", descriptor.current_vm_status),
        format!("gap-classification={}", descriptor.gap_classification),
    ];
    if let Some(left) = descriptor.left_declared_type {
        observations.push(format!("left-declared-type={}", debug_token(&left)));
    }
    if let Some(right) = descriptor.right_declared_type {
        observations.push(format!("right-declared-type={}", debug_token(&right)));
    }
    if let Some(compare_mode) = descriptor.compare_mode {
        observations.push(format!("compare-mode={}", debug_token(&compare_mode)));
    }
    for state in &descriptor.result_value_states {
        observations.push(format!("result-value-state={}", debug_token(state)));
    }
    observations
}

fn coercion_observations(descriptor: &CoercionDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("coercion-id={}", descriptor.coercion_id),
        format!("kind={}", debug_token(&descriptor.kind)),
        format!(
            "source-declared-type={}",
            debug_token(&descriptor.source_declared_type)
        ),
        format!(
            "target-declared-type={}",
            debug_token(&descriptor.target_declared_type)
        ),
        format!("static-status={}", debug_token(&descriptor.static_status)),
        format!(
            "runtime-failure={}",
            debug_token(&descriptor.runtime_failure)
        ),
        format!("helper-id={}", descriptor.helper_id),
        format!("evidence-anchor={}", descriptor.evidence_anchor),
        format!("gap-classification={}", descriptor.gap_classification),
        format!("detail={}", descriptor.detail),
    ];
    for state in &descriptor.source_value_states {
        observations.push(format!("source-value-state={}", debug_token(state)));
    }
    observations
}

fn name_binding_observations(descriptor: &NameBindingDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("binding-id={}", descriptor.binding_id),
        format!("name={}", descriptor.name.to_ascii_lowercase()),
        format!("binding-kind={}", debug_token(&descriptor.binding_kind)),
        format!("precedence={}", debug_token(&descriptor.precedence)),
        format!("detail={}", descriptor.detail),
    ];
    if descriptor.binding_id == "NAME-BINDING-PROCEDURE-POLICY" {
        observations.push("name-binding-policy=local-scope-before-module-members".to_string());
    }
    if let Some(target) = &descriptor.target {
        observations.push(format!("target={}", target.to_ascii_lowercase()));
    }
    observations.extend(
        descriptor
            .diagnostics
            .iter()
            .map(|diagnostic| format!("diagnostic={diagnostic}")),
    );
    observations
}

fn object_member_binding_observations(descriptor: &ObjectMemberBindingDescriptor) -> Vec<String> {
    vec![
        format!("binding-id={}", descriptor.binding_id),
        format!(
            "target-declared-type={}",
            debug_token(&descriptor.target_declared_type)
        ),
        format!(
            "member-name={}",
            descriptor.member_name.to_ascii_lowercase()
        ),
        format!("member-kind={}", debug_token(&descriptor.member_kind)),
        format!("default-member={}", descriptor.default_member),
        format!("dispatch-kind={}", debug_token(&descriptor.dispatch_kind)),
        format!(
            "argument-binding-policy={}",
            descriptor.argument_binding_policy
        ),
        format!(
            "object-identity-policy={}",
            descriptor.object_identity_policy
        ),
        format!(
            "cache-invalidation-policy={}",
            descriptor.cache_invalidation_policy
        ),
        format!(
            "fallback-or-unsupported-policy={}",
            descriptor.fallback_or_unsupported_policy
        ),
        format!("current-vm-status={}", descriptor.current_vm_status),
        format!("gap-classification={}", descriptor.gap_classification),
    ]
}

fn collect_object_descriptor_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmObjectDescriptorEvidence> {
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            metadata
                .object_types
                .iter()
                .map(move |descriptor| VmObjectDescriptorEvidence {
                    procedure_name: metadata.procedure_name.clone(),
                    type_name: descriptor.type_name.clone(),
                    object_descriptor_id: descriptor.descriptor_id.clone(),
                    object_descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::ObjectType,
                        &descriptor.descriptor_id,
                        descriptor,
                    ),
                    observations: object_descriptor_observations(descriptor),
                })
        })
        .collect::<Vec<_>>();
    sort_object_descriptor_evidence(&mut evidence);
    evidence
}

fn object_descriptor_observations(descriptor: &ObjectTypeDescriptor) -> Vec<String> {
    let mut observations = vec![
        format!("descriptor-id={}", descriptor.descriptor_id),
        format!("kind={}", debug_token(&descriptor.kind)),
        format!("carrier={}", debug_token(&descriptor.carrier)),
        format!("initial={}", debug_token(&descriptor.initial_state)),
        format!("activation={}", debug_token(&descriptor.activation)),
        format!("event-binding={}", debug_token(&descriptor.event_binding)),
        format!("default-member={}", debug_token(&descriptor.default_member)),
        format!("support={}", debug_token(&descriptor.support)),
        "object-identity-policy=objectref-or-nothing".to_string(),
        "lifecycle-id=LIFE-OBJECTREF".to_string(),
        "set-policy=preserve-assigned-identity".to_string(),
    ];
    for instance in &descriptor.instances {
        observations.push(format!(
            "instance:{}:{}",
            instance.name.to_ascii_lowercase(),
            if instance.slot.is_some() {
                "slot-known"
            } else {
                "slot-missing"
            }
        ));
        observations.push(format!(
            "instance:{}:initial={}",
            instance.name.to_ascii_lowercase(),
            debug_token(&descriptor.initial_state)
        ));
        observations.push(format!(
            "instance:{}:carrier={}",
            instance.name.to_ascii_lowercase(),
            debug_token(&descriptor.carrier)
        ));
    }
    observations
}

fn collect_package_route_object_descriptor_evidence(
    dynamic_object_routes: Option<&[ProjectDynamicObjectRoute]>,
    com_withevents_routes: Option<&[ProjectComWithEventsRoute]>,
) -> Vec<VmObjectDescriptorEvidence> {
    let mut evidence = Vec::new();
    for route in dynamic_object_routes.into_iter().flatten() {
        let descriptor_id = project_dynamic_object_descriptor_id(route);
        evidence.push(VmObjectDescriptorEvidence {
            procedure_name: "<package-routes>".to_string(),
            type_name: route.module_name.clone(),
            object_descriptor_id: descriptor_id.clone(),
            object_descriptor_digest: descriptor_digest_debug(
                DescriptorFamily::ObjectType,
                &descriptor_id,
                route,
            ),
            observations: project_dynamic_object_observations(&descriptor_id, route, None),
        });
    }
    for route in com_withevents_routes.into_iter().flatten() {
        let descriptor_id = project_com_withevents_descriptor_id(route);
        evidence.push(VmObjectDescriptorEvidence {
            procedure_name: "<package-routes>".to_string(),
            type_name: route.prog_id_name.clone(),
            object_descriptor_id: descriptor_id.clone(),
            object_descriptor_digest: descriptor_digest_debug(
                DescriptorFamily::ObjectType,
                &descriptor_id,
                route,
            ),
            observations: project_com_withevents_observations(&descriptor_id, route),
        });
    }
    sort_object_descriptor_evidence(&mut evidence);
    evidence
}

fn collect_runtime_object_descriptor_evidence(
    project_dynamic_objects: &HashMap<i32, ProjectDynamicObjectState>,
    project_com_withevents_routes: &HashMap<i32, Vec<ProjectComWithEventsRoute>>,
) -> Vec<VmObjectDescriptorEvidence> {
    let mut evidence = project_dynamic_objects
        .values()
        .map(|state| {
            let route = &state.route;
            let descriptor_id = project_dynamic_object_descriptor_id(route);
            VmObjectDescriptorEvidence {
                procedure_name: "<project-runtime>".to_string(),
                type_name: route.module_name.clone(),
                object_descriptor_id: descriptor_id.clone(),
                object_descriptor_digest: descriptor_digest_debug(
                    DescriptorFamily::ObjectType,
                    &descriptor_id,
                    route,
                ),
                observations: project_dynamic_object_observations(
                    &descriptor_id,
                    route,
                    Some(state.object.raw()),
                ),
            }
        })
        .collect::<Vec<_>>();
    for routes in project_com_withevents_routes.values() {
        for route in routes {
            let descriptor_id = project_com_withevents_descriptor_id(route);
            evidence.push(VmObjectDescriptorEvidence {
                procedure_name: "<project-runtime>".to_string(),
                type_name: route.prog_id_name.clone(),
                object_descriptor_id: descriptor_id.clone(),
                object_descriptor_digest: descriptor_digest_debug(
                    DescriptorFamily::ObjectType,
                    &descriptor_id,
                    route,
                ),
                observations: project_com_withevents_observations(&descriptor_id, route),
            });
        }
    }
    sort_object_descriptor_evidence(&mut evidence);
    evidence
}

fn project_dynamic_object_descriptor_id(route: &ProjectDynamicObjectRoute) -> String {
    canonical_descriptor_id(
        DescriptorFamily::ObjectType,
        [
            "class",
            route.project_name.as_str(),
            route.module_name.as_str(),
        ],
    )
}

fn project_com_withevents_descriptor_id(route: &ProjectComWithEventsRoute) -> String {
    let binding_token = route.binding_token.to_string();
    canonical_descriptor_id(
        DescriptorFamily::ObjectType,
        [
            "com-withevents",
            route.prog_id_name.as_str(),
            binding_token.as_str(),
        ],
    )
}

fn project_dynamic_object_observations(
    descriptor_id: &str,
    route: &ProjectDynamicObjectRoute,
    object_ref: Option<i32>,
) -> Vec<String> {
    let default_member_count = route
        .members
        .iter()
        .filter(|member| member.is_default_member)
        .count();
    let mut observations = vec![
        format!("descriptor-id={descriptor_id}"),
        "source=ProjectDynamicObjectRoute".to_string(),
        "kind=vbaclass".to_string(),
        "carrier=objectref".to_string(),
        format!(
            "activation={}",
            project_dynamic_activation_token(route.object_handle)
        ),
        "event-binding=none".to_string(),
        "support=vmrunnablehosted".to_string(),
        format!(
            "default-member={}",
            if default_member_count > 0 {
                "hasdefaultmember"
            } else {
                "nodefaultmember"
            }
        ),
        format!(
            "default-member-policy={}",
            match default_member_count {
                0 => "none",
                1 => "single-route",
                _ => "ambiguous-diagnostic-required",
            }
        ),
        format!("project={}", route.project_name.to_ascii_lowercase()),
        format!("module={}", route.module_name.to_ascii_lowercase()),
        format!("class={}", route.module_name.to_ascii_lowercase()),
        format!("object-handle={}", route.object_handle),
        "object-identity-policy=stable-project-dynamic-object-handle".to_string(),
        "lifecycle-id=LIFE-OBJECTREF".to_string(),
        "cleanup-policy=release-objectref-on-slot-drop".to_string(),
        format!(
            "interface:_{}:kind=dispatch",
            route.module_name.to_ascii_lowercase()
        ),
        format!(
            "interface:_{}:descriptor-id={}",
            route.module_name.to_ascii_lowercase(),
            canonical_descriptor_id(
                DescriptorFamily::ObjectType,
                [
                    "interface",
                    route.project_name.as_str(),
                    route.module_name.as_str(),
                    &format!("_{}", route.module_name),
                ],
            )
        ),
    ];
    if let Some(object_ref) = object_ref {
        observations.push(format!("object-ref={object_ref}"));
    }
    for interface in &route.implements_interfaces {
        observations.push(format!(
            "interface:{}:kind=implemented",
            interface.to_ascii_lowercase()
        ));
        observations.push(format!(
            "interface:{}:descriptor-id={}",
            interface.to_ascii_lowercase(),
            canonical_descriptor_id(
                DescriptorFamily::ObjectType,
                [
                    "interface",
                    route.project_name.as_str(),
                    route.module_name.as_str(),
                    interface.as_str(),
                ],
            )
        ));
    }
    for member in &route.members {
        let member_name = member.member_name.to_ascii_lowercase();
        observations.push(format!(
            "member:{member_name}:kind={}",
            debug_token(&member.kind)
        ));
        observations.push(format!(
            "member:{member_name}:default={}",
            member.is_default_member
        ));
        if member.is_default_member {
            observations.push(format!("default-member-target={member_name}"));
        }
        observations.push(format!(
            "member:{member_name}:lowered={}",
            member.lowered_name.to_ascii_lowercase()
        ));
        observations.push(format!(
            "member:{member_name}:dispatch-kind=project-dynamic-route"
        ));
        if let Some(dispatch_id) = member.dispatch_id.or(member.known_dispatch_token) {
            observations.push(format!("member:{member_name}:dispatch-id={dispatch_id}"));
        }
        observations.push(format!(
            "member:{member_name}:visible-param-count={}",
            member.visible_param_count
        ));
        if let Some(return_type) = member.return_type {
            observations.push(format!(
                "member:{member_name}:return-type={}",
                debug_token(&return_type)
            ));
        } else {
            observations.push(format!("member:{member_name}:return-type=void"));
        }
        for param in &member.params {
            let param_name = param.name.to_ascii_lowercase();
            observations.push(format!(
                "member:{member_name}:param:{param_name}:optional={}",
                param.optional
            ));
            observations.push(format!(
                "member:{member_name}:param:{param_name}:paramarray={}",
                param.param_array
            ));
            if let Some(default_value) = param.default_value {
                observations.push(format!(
                    "member:{member_name}:param:{param_name}:default=i32-{default_value}"
                ));
            }
        }
    }
    observations
}

fn project_dynamic_activation_token(object_handle: i32) -> &'static str {
    match object_handle.cmp(&0) {
        std::cmp::Ordering::Greater => "asnew-project-class",
        std::cmp::Ordering::Equal => "default-instance",
        std::cmp::Ordering::Less => "exported-library-class",
    }
}

fn project_com_withevents_observations(
    descriptor_id: &str,
    route: &ProjectComWithEventsRoute,
) -> Vec<String> {
    vec![
        format!("descriptor-id={descriptor_id}"),
        "source=ProjectComWithEventsRoute".to_string(),
        "kind=witheventsobject".to_string(),
        "carrier=objectref".to_string(),
        "activation=hostprovided".to_string(),
        "event-binding=withevents".to_string(),
        "default-member=nodefaultmember".to_string(),
        "support=vmrunnablehosted".to_string(),
        "object-identity-policy=preserve-source-objectref".to_string(),
        "subscription-policy=set-assignment-updates-owner".to_string(),
        "cleanup-policy=clear-owner-on-class-terminate".to_string(),
        format!(
            "imported-com-class={}",
            route.prog_id_name.to_ascii_lowercase()
        ),
        format!("event-source={}", route.prog_id_name.to_ascii_lowercase()),
        format!("binding-token={}", route.binding_token),
        format!("event-token={}", route.event_token),
        format!("event={}", route.event_name.to_ascii_lowercase()),
        format!("handler={}", route.handler_symbol.to_ascii_lowercase()),
        format!(
            "guard-zero={}",
            route.guard_symbol_zero_arg.to_ascii_lowercase()
        ),
        format!(
            "guard-one={}",
            route.guard_symbol_one_arg.to_ascii_lowercase()
        ),
    ]
}

fn sort_object_descriptor_evidence(evidence: &mut [VmObjectDescriptorEvidence]) {
    evidence.sort_by(|left, right| {
        left.procedure_name
            .to_ascii_lowercase()
            .cmp(&right.procedure_name.to_ascii_lowercase())
            .then(left.type_name.cmp(&right.type_name))
            .then(left.object_descriptor_id.cmp(&right.object_descriptor_id))
    });
}

fn collect_interop_descriptor_evidence(
    bytecode: &Bytecode,
    export_inventory: Option<&ExportInventory>,
    descriptor_inventory: Option<&DescriptorInventory>,
) -> Vec<VmInteropDescriptorEvidence> {
    let mut evidence = bytecode
        .external_call_descriptors
        .iter()
        .map(|descriptor| {
            let descriptor_id = format!("native:{}", descriptor.descriptor_id);
            VmInteropDescriptorEvidence {
                interop_kind: "native-declare".to_string(),
                descriptor_id: descriptor_id.clone(),
                descriptor_digest: descriptor_digest_debug(
                    DescriptorFamily::Interop,
                    &descriptor_id,
                    descriptor,
                ),
                observations: external_call_descriptor_observations(descriptor),
            }
        })
        .collect::<Vec<_>>();

    for (pc, instruction) in bytecode.instructions.iter().enumerate() {
        match instruction {
            Instruction::IntrinsicCreateObjectHost { dst, prog_id } => {
                let pc_part = format!("pc-{pc}");
                let descriptor_id = canonical_descriptor_id(
                    DescriptorFamily::Interop,
                    ["com-createobject", pc_part.as_str()],
                );
                evidence.push(VmInteropDescriptorEvidence {
                    interop_kind: "com-createobject".to_string(),
                    descriptor_id: descriptor_id.clone(),
                    descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::Interop,
                        &descriptor_id,
                        instruction,
                    ),
                    observations: vec![
                        "kind=com-createobject".to_string(),
                        "boundary=host-com-activation".to_string(),
                        "support=vmrunnablehosted".to_string(),
                        "projection-source=runtime-prog-id-slot".to_string(),
                        "object-projection=objectref".to_string(),
                        "object-identity-policy=host-created-object-ref".to_string(),
                        "activation-error-policy=host-runtime-error-routing".to_string(),
                        "cleanup-policy=objectref-slot-lifecycle-or-host-owned".to_string(),
                        format!("dst-slot={dst}"),
                        format!("prog-id-slot={prog_id}"),
                    ],
                });
            }
            Instruction::IntrinsicDispatchInvokeHost {
                dst,
                object,
                member,
                args,
                early_bound,
                com_member,
                call_kind_hint,
            } => {
                let pc_part = format!("pc-{pc}");
                let descriptor_id = canonical_descriptor_id(
                    DescriptorFamily::Interop,
                    ["com-dispatch", pc_part.as_str()],
                );
                evidence.push(VmInteropDescriptorEvidence {
                    interop_kind: "com-dispatch-invoke".to_string(),
                    descriptor_id: descriptor_id.clone(),
                    descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::Interop,
                        &descriptor_id,
                        instruction,
                    ),
                    observations: com_dispatch_instruction_observations(
                        *dst,
                        *object,
                        *member,
                        args,
                        *early_bound,
                        com_member.as_ref(),
                        *call_kind_hint,
                    ),
                });
            }
            Instruction::IntrinsicInvokeSymbolHost {
                dst,
                descriptor_id,
                symbol,
                args,
                writeback_slots,
            } => {
                let descriptor_ref = descriptor_id.to_string();
                let pc_part = format!("pc-{pc}");
                let package_descriptor_id = canonical_descriptor_id(
                    DescriptorFamily::Interop,
                    ["native-invoke", descriptor_ref.as_str(), pc_part.as_str()],
                );
                evidence.push(VmInteropDescriptorEvidence {
                    interop_kind: "native-invoke".to_string(),
                    descriptor_id: package_descriptor_id.clone(),
                    descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::Interop,
                        &package_descriptor_id,
                        instruction,
                    ),
                    observations: native_invoke_instruction_observations(
                        *dst,
                        *descriptor_id,
                        symbol,
                        args,
                        writeback_slots,
                    ),
                });
            }
            _ => {}
        }
    }
    evidence.extend(collect_exported_callable_interop_evidence(
        export_inventory,
        descriptor_inventory,
    ));

    evidence.sort_by(|left, right| {
        left.interop_kind
            .cmp(&right.interop_kind)
            .then(left.descriptor_id.cmp(&right.descriptor_id))
    });
    evidence
}

fn collect_exported_callable_interop_evidence(
    export_inventory: Option<&ExportInventory>,
    descriptor_inventory: Option<&DescriptorInventory>,
) -> Vec<VmInteropDescriptorEvidence> {
    let Some(export_inventory) = export_inventory else {
        return Vec::new();
    };
    let callables = descriptor_inventory
        .map(|inventory| inventory.callables.as_slice())
        .unwrap_or(&[]);
    export_inventory
        .host_exports
        .iter()
        .map(|export| {
            let callable = callables
                .iter()
                .find(|callable| export_matches_callable(export, callable));
            let export_kind = debug_token(&export.kind);
            let descriptor_id = canonical_descriptor_id(
                DescriptorFamily::Interop,
                [
                    "exported-callable",
                    export.project_name.as_str(),
                    export.module_name.as_str(),
                    export.procedure_name.as_str(),
                    export_kind.as_str(),
                ],
            );
            VmInteropDescriptorEvidence {
                interop_kind: "exported-callable".to_string(),
                descriptor_id: descriptor_id.clone(),
                descriptor_digest: exported_callable_descriptor_digest(
                    &descriptor_id,
                    export,
                    callable,
                ),
                observations: exported_callable_descriptor_observations(export, callable),
            }
        })
        .collect()
}

fn export_matches_callable(
    export: &HostProcedureExport,
    callable: &BundleCallableDescriptor,
) -> bool {
    callable
        .project_id
        .eq_ignore_ascii_case(&export.project_name)
        && callable
            .module_name
            .eq_ignore_ascii_case(&export.module_name)
        && callable
            .procedure_name
            .eq_ignore_ascii_case(&export.procedure_name)
}

fn exported_callable_descriptor_digest(
    descriptor_id: &str,
    export: &HostProcedureExport,
    callable: Option<&BundleCallableDescriptor>,
) -> String {
    descriptor_digest_from_fields(
        DescriptorFamily::Interop,
        descriptor_id,
        [
            ("export_project", export.project_name.clone()),
            ("export_module", export.module_name.clone()),
            ("export_procedure", export.procedure_name.clone()),
            ("export_kind", debug_token(&export.kind)),
            (
                "callable_id",
                callable
                    .map(|callable| callable.callable_id.clone())
                    .unwrap_or_else(|| "<missing>".to_string()),
            ),
            (
                "callable_fingerprint",
                callable
                    .map(|callable| callable.descriptor_fingerprint.clone())
                    .unwrap_or_else(|| "<missing>".to_string()),
            ),
            (
                "callable_signature",
                callable
                    .map(|callable| format!("{:#?}", callable.signature))
                    .unwrap_or_else(|| "<missing>".to_string()),
            ),
        ],
    )
}

fn exported_callable_descriptor_observations(
    export: &HostProcedureExport,
    callable: Option<&BundleCallableDescriptor>,
) -> Vec<String> {
    let mut observations = vec![
        "kind=exported-callable".to_string(),
        "boundary=host-export-inbound".to_string(),
        "source=ExportInventory".to_string(),
        "callable-source=DescriptorInventory".to_string(),
        format!("project={}", export.project_name.to_ascii_lowercase()),
        format!("module={}", export.module_name.to_ascii_lowercase()),
        format!("procedure={}", export.procedure_name.to_ascii_lowercase()),
        format!("export-kind={}", debug_token(&export.kind)),
        "lane=variant-positional".to_string(),
        "inbound-projection=variant-positional-to-procedure-slots".to_string(),
        "byref-writeback-policy=byref-params-writeback-through-export-boundary".to_string(),
        "cleanup-policy=vm-frame-owned-slots-and-export-boundary-temporaries".to_string(),
        "error-policy=runtime-error-projected-to-host-failure".to_string(),
        "unsupported-shape-policy=descriptor-inventory-diagnostic-required".to_string(),
    ];
    let Some(callable) = callable else {
        observations.push("callable-descriptor=missing".to_string());
        observations.push("support=unsupported-missing-callable-descriptor".to_string());
        return observations;
    };
    observations.push("support=vmrunnablehosted".to_string());
    observations.push(format!(
        "callable-id={}",
        callable.callable_id.to_ascii_lowercase()
    ));
    observations.push(format!(
        "module-id={}",
        callable.module_id.to_ascii_lowercase()
    ));
    observations.push(format!(
        "callable-kind={}",
        callable.kind.to_ascii_lowercase()
    ));
    observations.push(format!("public={}", callable.is_public));
    observations.push(format!("option-private={}", callable.is_option_private));
    observations.push(format!("class-member={}", callable.is_class_member));
    observations.push(format!(
        "calling-shape={}",
        callable.signature.calling_shape.to_ascii_lowercase()
    ));
    observations.push(format!(
        "param-count={}",
        callable.signature.parameters.len()
    ));
    observations.push(match callable.entry_pc {
        Some(entry_pc) => format!("entry-pc={entry_pc}"),
        None => "entry-pc=unavailable".to_string(),
    });
    observations.push(match callable.return_slot {
        Some(slot) => format!("return-slot={slot}"),
        None => "return-slot=void".to_string(),
    });
    observations.push(format!(
        "return-type={}",
        callable
            .signature
            .return_type
            .as_ref()
            .map(bundle_type_token)
            .unwrap_or_else(|| "void".to_string())
    ));
    observations.push(if callable.return_slot.is_some() {
        "outbound-return-projection=return-slot-to-variant".to_string()
    } else {
        "outbound-return-projection=void".to_string()
    });
    for (index, param) in callable.signature.parameters.iter().enumerate() {
        let param_type = param
            .value_type
            .as_ref()
            .map(bundle_type_token)
            .unwrap_or_else(|| "variant".to_string());
        observations.push(format!(
            "param:{index}:name={}",
            param
                .name
                .as_deref()
                .unwrap_or("<unnamed>")
                .to_ascii_lowercase()
        ));
        observations.push(format!(
            "param:{index}:passing={}",
            param.passing_mode.to_ascii_lowercase()
        ));
        observations.push(format!("param:{index}:type={param_type}"));
        observations.push(match callable.param_slots.get(index).copied() {
            Some(slot) => format!("param:{index}:slot={slot}"),
            None => format!("param:{index}:slot=unavailable"),
        });
        observations.push(format!(
            "param:{index}:projection=variant-inbound-to-{param_type}"
        ));
        if param.passing_mode.eq_ignore_ascii_case("ByRef") {
            observations.push(format!("param:{index}:writeback=export-boundary-byref"));
        }
        if param.optional {
            observations.push(format!("param:{index}:optional=true"));
        }
        if param.param_array {
            observations.push(format!("param:{index}:paramarray=true"));
        }
    }
    observations
}

fn bundle_type_token(ty: &oxvba_compiler::BundleVbaTypeDescriptor) -> String {
    ty.normalized.to_ascii_lowercase()
}

fn external_call_descriptor_observations(descriptor: &ExternalCallDescriptor) -> Vec<String> {
    let mut observations = vec![
        "kind=native-declare".to_string(),
        format!("descriptor-id=native:{}", descriptor.descriptor_id),
        format!(
            "declared-name={}",
            descriptor.declared_name.to_ascii_lowercase()
        ),
        format!("library={}", descriptor.library.to_ascii_lowercase()),
        format!("alias={}", descriptor.alias.to_ascii_lowercase()),
        format!("ordinal-alias={}", descriptor.ordinal_alias),
        format!("symbol={}", debug_token(&descriptor.symbol)),
        format!(
            "marshal-lane={}",
            descriptor.marshal_lane.to_ascii_lowercase()
        ),
        format!(
            "calling-convention={}",
            descriptor.calling_convention.to_ascii_lowercase()
        ),
        format!(
            "selection-policy={}",
            descriptor.selection_policy.to_ascii_lowercase()
        ),
        format!("param-count={}", descriptor.param_count),
        format!(
            "return-type={}",
            descriptor
                .return_type
                .as_ref()
                .map(debug_token)
                .unwrap_or_else(|| "void".to_string())
        ),
        "boundary=host-native-declare".to_string(),
        "support=vmrunnablehosted".to_string(),
        "abi-descriptor-source=ExternalCallDescriptor".to_string(),
        "parameter-projection-policy=declare-param-type".to_string(),
        "return-projection-policy=declare-return-type".to_string(),
        "writeback-policy=ExternalCallWriteback-on-invoke".to_string(),
        "cleanup-policy=host-native-temporary-cleanup".to_string(),
        "error-policy=host-native-status-or-runtime-error".to_string(),
        "unsupported=generic-automation-variant-and-safearray-declared-parameter-abi".to_string(),
    ];
    for (index, param_type) in descriptor.param_types.iter().enumerate() {
        observations.push(format!("param:{index}:type={}", debug_token(param_type)));
        observations.push(format!(
            "param:{index}:projection={}",
            declare_param_projection_token(param_type)
        ));
        observations.push(format!(
            "param:{index}:byref={}",
            descriptor.param_by_ref.get(index).copied().unwrap_or(false)
        ));
    }
    observations
}

fn com_dispatch_instruction_observations(
    dst: usize,
    object: usize,
    member: usize,
    args: &[oxvba_compiler::bytecode::DispatchInvokeArg],
    early_bound: bool,
    com_member: Option<&oxvba_compiler::bytecode::ComMemberCallDescriptor>,
    call_kind_hint: Option<ProjectMemberCallKind>,
) -> Vec<String> {
    let named_arg_count = args.iter().filter(|arg| arg.name.is_some()).count();
    let mut observations = vec![
        "kind=com-dispatch-invoke".to_string(),
        "boundary=host-com-dispatch".to_string(),
        "support=vmrunnablehosted".to_string(),
        format!("early-bound={early_bound}"),
        format!("dst-slot={dst}"),
        format!("object-slot={object}"),
        format!("member-slot={member}"),
        format!("arg-count={}", args.len()),
        format!("named-arg-count={named_arg_count}"),
        format!(
            "call-kind-hint={}",
            call_kind_hint
                .map(project_member_call_kind_token)
                .unwrap_or("none")
        ),
        "hresult-excepinfo-argerr=runtime-owned".to_string(),
        "hresult-policy=runtime-owned".to_string(),
        "excepinfo-policy=runtime-owned".to_string(),
        "argerr-policy=runtime-owned".to_string(),
        "argument-projection=runtime-variant-dispatch".to_string(),
        "result-projection=runtime-owned-variant-or-object".to_string(),
        "cleanup-policy=host-dispatch-temporary-cleanup".to_string(),
        "unsupported=full-package-owned-com-boundary-abi-descriptor".to_string(),
    ];
    if let Some(com_member) = com_member {
        observations.push("selector-policy=descriptor-backed".to_string());
        observations.push(format!(
            "selector={}",
            com_member_selector_token(&com_member.selector)
        ));
        observations.push(format!("descriptor-arity={}", com_member.arity));
    } else {
        observations.push("selector-policy=runtime-name-slot".to_string());
        observations.push("selector=runtime-name-slot".to_string());
        observations.push("descriptor-arity=runtime-args".to_string());
    }
    for (index, arg) in args.iter().enumerate() {
        observations.push(format!(
            "arg:{index}:source={}",
            if arg.slot.is_some() {
                "slot-known"
            } else {
                "slot-missing"
            }
        ));
        if let Some(name) = &arg.name {
            observations.push(format!("arg:{index}:name={}", name.to_ascii_lowercase()));
            observations.push(format!("arg:{index}:named-policy=named-dispatch-arg"));
        }
        if arg.slot.is_none() {
            observations.push(format!("arg:{index}:missing-policy=runtime-missing-arg"));
        }
    }
    observations
}

fn com_member_selector_token(selector: &ComMemberSelectorDescriptor) -> String {
    match selector {
        ComMemberSelectorDescriptor::DispatchId(dispatch_id) => {
            format!("dispid:{dispatch_id}")
        }
        ComMemberSelectorDescriptor::Name(name) => format!("name:{}", name.to_ascii_lowercase()),
    }
}

fn project_member_call_kind_token(kind: ProjectMemberCallKind) -> &'static str {
    match kind {
        ProjectMemberCallKind::Method => "method",
        ProjectMemberCallKind::PropertyGet => "property-get",
        ProjectMemberCallKind::PropertyLet => "property-let",
        ProjectMemberCallKind::PropertySet => "property-set",
    }
}

fn native_invoke_instruction_observations(
    dst: usize,
    descriptor_id: u32,
    symbol: &oxvba_runtime::DynLinkSymbol,
    args: &[usize],
    writeback_slots: &[ExternalCallWriteback],
) -> Vec<String> {
    let mut observations = vec![
        "kind=native-invoke".to_string(),
        "boundary=host-native-invoke".to_string(),
        "support=vmrunnablehosted".to_string(),
        format!("descriptor-ref=native:{descriptor_id}"),
        format!("symbol={}", debug_token(symbol)),
        format!("dst-slot={dst}"),
        format!("arg-count={}", args.len()),
        format!("writeback-count={}", writeback_slots.len()),
        "abi-descriptor-source=ExternalCallDescriptor".to_string(),
        "argument-projection=runtime-variant-to-native-helper".to_string(),
        "return-projection=native-helper-to-variant".to_string(),
        "writeback-policy=external-call-writeback-slots".to_string(),
        "cleanup-policy=commit-writebacks-release-temporaries".to_string(),
        "error-policy=hal-native-call-result".to_string(),
    ];
    for (index, slot) in args.iter().enumerate() {
        observations.push(format!("arg:{index}:slot={slot}"));
    }
    for (index, writeback) in writeback_slots.iter().enumerate() {
        observations.push(format!(
            "writeback:{index}:arg-index={}",
            writeback.arg_index
        ));
        observations.push(format!(
            "writeback:{index}:source-slot={}",
            writeback.source_slot
        ));
        observations.push(format!(
            "writeback:{index}:kind={}",
            debug_token(&writeback.kind)
        ));
        observations.push(format!(
            "writeback:{index}:projection={}",
            external_writeback_projection_token(writeback.kind)
        ));
    }
    observations
}

fn declare_param_projection_token(param_type: &oxvba_compiler::DeclareParamType) -> &'static str {
    match param_type {
        oxvba_compiler::DeclareParamType::String => "bstr-string",
        oxvba_compiler::DeclareParamType::Variant => "variant-cell",
        oxvba_compiler::DeclareParamType::Any => "interop-any",
        oxvba_compiler::DeclareParamType::LongPtr => "pointer-sized-scalar",
        oxvba_compiler::DeclareParamType::LongLong => "i64-scalar",
        oxvba_compiler::DeclareParamType::Long => "i32-scalar",
        oxvba_compiler::DeclareParamType::Integer => "i16-scalar",
        oxvba_compiler::DeclareParamType::Byte => "u8-scalar",
        oxvba_compiler::DeclareParamType::Boolean => "bool-scalar",
        oxvba_compiler::DeclareParamType::Double => "f64-scalar",
        oxvba_compiler::DeclareParamType::Single => "f32-scalar",
        oxvba_compiler::DeclareParamType::Currency => "currency-scalar",
        oxvba_compiler::DeclareParamType::Date => "date-scalar",
    }
}

fn external_writeback_projection_token(kind: ExternalCallWritebackKind) -> &'static str {
    match kind {
        ExternalCallWritebackKind::ByRefValue => "byref-value",
        ExternalCallWritebackKind::PointerByteArrayPayload => "safearray-byte-buffer-pointer",
        ExternalCallWritebackKind::PointerStringPayload => "bstr-string-payload-pointer",
    }
}

fn collect_call_site_descriptor_evidence(
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmCallSiteDescriptorEvidence> {
    let metadata_by_entry_pc = procedure_metadata
        .values()
        .map(|metadata| (metadata.entry_pc, metadata))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = procedure_metadata
        .values()
        .flat_map(|metadata| {
            metadata.call_sites.iter().map(|call_site| {
                let target_metadata = call_site
                    .target_entry_pc
                    .and_then(|entry_pc| metadata_by_entry_pc.get(&entry_pc).copied());
                VmCallSiteDescriptorEvidence {
                    call_site_id: call_site.call_site_id.clone(),
                    caller_procedure_name: metadata.procedure_name.clone(),
                    call_pc: call_site.call_pc,
                    target_name: call_site.target_name.clone(),
                    call_site_descriptor_digest: descriptor_digest_debug(
                        DescriptorFamily::CallSite,
                        &call_site_descriptor_id(&procedure_descriptor_id(metadata), call_site),
                        call_site,
                    ),
                    observations: call_site_descriptor_observations(
                        call_site,
                        metadata,
                        target_metadata,
                    ),
                }
            })
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        left.caller_procedure_name
            .to_ascii_lowercase()
            .cmp(&right.caller_procedure_name.to_ascii_lowercase())
            .then(left.call_pc.cmp(&right.call_pc))
            .then(left.target_name.cmp(&right.target_name))
    });
    evidence
}

fn call_site_descriptor_observations(
    call_site: &CallSiteDescriptor,
    caller_metadata: &ProcedureRuntimeMetadata,
    target_metadata: Option<&ProcedureRuntimeMetadata>,
) -> Vec<String> {
    let mut observations = Vec::new();
    observations.push(format!(
        "target-kind:{}",
        debug_token(&call_site.target_kind)
    ));
    observations.push(if call_site.target_entry_pc.is_some() {
        "target-entry-known".to_string()
    } else {
        "target-entry-missing".to_string()
    });
    observations.push(format!(
        "default-member-policy:{}",
        debug_token(&call_site.default_member_policy)
    ));
    observations.push(format!(
        "invocation-syntax:{}",
        debug_token(&call_site.invocation_syntax)
    ));
    if !call_site.argument_evaluation_order.is_empty() {
        observations.push(format!(
            "arg-eval-order:{}",
            call_site
                .argument_evaluation_order
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    for argument in &call_site.arguments {
        observations.extend(call_site_argument_observations(
            argument,
            caller_metadata,
            target_metadata,
        ));
    }
    for policy in &call_site.diagnostic_policies {
        observations.push(format!(
            "diagnostic:{}:owner={}:detail={}",
            debug_token(&policy.diagnostic),
            debug_token(&policy.owner),
            policy.detail
        ));
    }
    if let Some(return_value) = &call_site.return_value {
        observations.push(if return_value.copyout_required {
            "return:copyout-required".to_string()
        } else {
            "return:no-copyout".to_string()
        });
        if return_value.assign_target_slot.is_some() {
            observations.push("return:assign-target-known".to_string());
        }
    }
    observations
}

fn call_site_argument_observations(
    argument: &ArgumentBindingDescriptor,
    caller_metadata: &ProcedureRuntimeMetadata,
    target_metadata: Option<&ProcedureRuntimeMetadata>,
) -> Vec<String> {
    let name = argument
        .parameter_name
        .as_deref()
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| format!("arg{}", argument.argument_index));
    let mut observations = vec![
        format!("arg:{name}:source={}", debug_token(&argument.source_kind)),
        format!("arg:{name}:expr={}", debug_token(&argument.expression_kind)),
        format!("arg:{name}:binding={}", debug_token(&argument.binding_kind)),
    ];
    if argument.force_byval {
        observations.push(format!("arg:{name}:force-byval"));
    }
    if argument.source_slot.is_some() {
        observations.push(format!("arg:{name}:source-slot-known"));
    }
    if argument.parameter_slot.is_some() {
        observations.push(format!("arg:{name}:parameter-slot-known"));
    }
    if argument
        .writeback
        .as_ref()
        .is_some_and(|writeback| writeback.required)
    {
        observations.push(format!("arg:{name}:writeback-required"));
    }
    if let Some(default_value) = &argument.optional_default {
        observations.push(format!(
            "arg:{name}:optional-default={}",
            optional_default_token(default_value)
        ));
    }
    if let Some(param_array) = &argument.param_array {
        observations.push(format!(
            "arg:{name}:paramarray-count={}",
            param_array.element_count
        ));
    }
    if argument.binding_kind == ArgumentBindingKindDescriptor::ByRefExpressionTemp {
        observations.push(format!("arg:{name}:no-writeback-temp"));
    }
    if target_metadata
        .and_then(|target_metadata| {
            selected_long_to_double_byval_call_entry_slots(
                argument,
                caller_metadata,
                target_metadata,
            )
        })
        .is_some()
    {
        observations.push(format!(
            "arg:{name}:coercion-id={SELECTED_CALL_BYVAL_COERCION_ID}"
        ));
        observations.push(format!(
            "arg:{name}:coercion-helper-id={SELECTED_CALL_BYVAL_NUMERIC_WIDEN_ID}"
        ));
        observations.push(format!(
            "arg:{name}:runtime-helper={SELECTED_CALL_BYVAL_RUNTIME_HELPER_ID}"
        ));
    }
    observations
}

fn selected_long_to_double_byval_call_entry_slots(
    argument: &ArgumentBindingDescriptor,
    caller_metadata: &ProcedureRuntimeMetadata,
    target_metadata: &ProcedureRuntimeMetadata,
) -> Option<(usize, usize)> {
    if argument.binding_kind != ArgumentBindingKindDescriptor::ByValCopy
        || argument.expression_kind != ArgumentExpressionKindDescriptor::Variable
    {
        return None;
    }
    let source_slot = argument.source_slot?;
    let parameter_slot = argument.parameter_slot?;
    let parameter = target_metadata
        .signature
        .parameters
        .iter()
        .find(|parameter| {
            parameter.slot == Some(parameter_slot)
                && argument
                    .parameter_index
                    .is_none_or(|index| parameter.index == index)
        })?;
    if parameter.resolved_mechanism != ResolvedParameterMechanism::ByVal
        || parameter.declared_type != VbaTypeId::Double
    {
        return None;
    }
    let caller_slot = caller_metadata
        .slots
        .iter()
        .find(|slot| slot.slot == source_slot)?;
    let source_declared_type = argument
        .source_declared_type
        .unwrap_or(caller_slot.declared_type);
    if source_declared_type != VbaTypeId::Long {
        return None;
    }
    if argument.source_declared_type.is_none()
        && (caller_slot.kind != ProcedureRuntimeSlotKind::Local
            || caller_slot.carrier != RuntimeCarrierKind::I32)
    {
        return None;
    }
    let callee_slot = target_metadata
        .slots
        .iter()
        .find(|slot| slot.slot == parameter_slot)?;
    if callee_slot.kind != ProcedureRuntimeSlotKind::Parameter
        || callee_slot.declared_type != VbaTypeId::Double
        || callee_slot.carrier != RuntimeCarrierKind::F64
    {
        return None;
    }
    Some((parameter_slot, source_slot))
}

fn optional_default_token(default_value: &OptionalDefaultValue) -> String {
    match default_value {
        OptionalDefaultValue::Unknown => "unknown".to_string(),
        OptionalDefaultValue::ExplicitI32(value) => format!("i32-{value}"),
        OptionalDefaultValue::ExplicitBool(value) => format!("bool-{value}"),
        OptionalDefaultValue::ExplicitString(value) => format!("string-{value}"),
        OptionalDefaultValue::DeclaredTypeDefault => "declared-type-default".to_string(),
        OptionalDefaultValue::VariantMissingError448 => "variant-missing-error-448".to_string(),
        OptionalDefaultValue::ImplementationDefined => "implementation-defined".to_string(),
    }
}

fn debug_token(value: &impl Debug) -> String {
    format!("{value:?}").to_ascii_lowercase()
}

fn collect_signature_call_evidence(
    bytecode: &Bytecode,
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<VmSignatureCallEvidence> {
    let metadata_by_entry_pc = procedure_metadata
        .values()
        .map(|metadata| (metadata.entry_pc, metadata))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = Vec::new();
    for (call_pc, instruction) in bytecode.instructions.iter().enumerate() {
        let Instruction::CallProc { target_pc, .. } = instruction else {
            continue;
        };
        let Some(metadata) = metadata_by_entry_pc.get(target_pc).copied() else {
            evidence.push(VmSignatureCallEvidence {
                call_pc,
                procedure_id: format!("proc:<unknown>@pc:{target_pc}"),
                procedure_name: "<unknown>".to_string(),
                target_pc: *target_pc,
                signature_descriptor_digest: "missing".to_string(),
                observations: vec!["mismatch:target-metadata-missing".to_string()],
            });
            continue;
        };
        let module_name = if metadata.module_name.trim().is_empty() {
            "<anonymous>".to_string()
        } else {
            metadata.module_name.clone()
        };
        let signature = metadata.procedure_signature_descriptor();
        let procedure_descriptor_id = procedure_descriptor_id(metadata);
        let call_site = procedure_metadata
            .values()
            .flat_map(|metadata| metadata.call_sites.iter())
            .find(|call_site| {
                call_site.call_pc == call_pc && call_site.target_entry_pc == Some(*target_pc)
            });
        evidence.push(VmSignatureCallEvidence {
            call_pc,
            procedure_id: format!(
                "proc:{}::{}@pc:{}",
                module_name, metadata.procedure_name, metadata.entry_pc
            ),
            procedure_name: metadata.procedure_name.clone(),
            target_pc: *target_pc,
            signature_descriptor_digest: descriptor_digest_debug(
                DescriptorFamily::ProcedureSignature,
                &signature_descriptor_id(&procedure_descriptor_id),
                &signature,
            ),
            observations: signature_call_observations(
                bytecode, call_pc, metadata, &signature, call_site,
            ),
        });
    }
    evidence
}

fn signature_call_observations(
    bytecode: &Bytecode,
    call_pc: usize,
    metadata: &ProcedureRuntimeMetadata,
    signature: &ProcedureSignatureDescriptor,
    call_site: Option<&CallSiteDescriptor>,
) -> Vec<String> {
    let mut observations = Vec::new();
    observations.push(format!(
        "descriptor:kind={:?}:params={}:return_slot={:?}",
        signature.kind,
        signature.parameters.len(),
        signature.return_slot
    ));
    if signature.parameters.len() == metadata.param_slots.len() {
        observations.push("match:param-count".to_string());
    } else {
        observations.push(format!(
            "mismatch:param-count:signature={}:metadata={}",
            signature.parameters.len(),
            metadata.param_slots.len()
        ));
    }
    if signature.return_slot == metadata.return_slot {
        observations.push("match:return-slot".to_string());
    } else {
        observations.push(format!(
            "mismatch:return-slot:signature={:?}:metadata={:?}",
            signature.return_slot, metadata.return_slot
        ));
    }

    for parameter in &signature.parameters {
        observations.extend(parameter_call_observations(
            bytecode, call_pc, parameter, call_site,
        ));
    }
    if let Some(return_slot) = signature.return_slot {
        if post_call_copy_from_slot(bytecode, call_pc, return_slot)
            || call_site
                .and_then(|call_site| call_site.return_value.as_ref())
                .is_some_and(|return_value| {
                    return_value.return_slot == Some(return_slot) && return_value.copyout_required
                })
        {
            observations.push("return:copy-observed".to_string());
        } else {
            observations.push("gap:return-copy-not-observed".to_string());
        }
    }
    observations
}

fn parameter_call_observations(
    bytecode: &Bytecode,
    call_pc: usize,
    parameter: &ParameterDescriptor,
    call_site: Option<&CallSiteDescriptor>,
) -> Vec<String> {
    let name = parameter.name.to_ascii_lowercase();
    let Some(slot) = parameter.slot else {
        return vec![format!("param:{name}:mismatch:slot-missing")];
    };
    let descriptor_argument = call_site.and_then(|call_site| {
        call_site
            .arguments
            .iter()
            .find(|argument| argument.parameter_slot == Some(slot))
    });
    let has_copyback = post_call_copy_from_slot(bytecode, call_pc, slot)
        || descriptor_argument.is_some_and(|argument| {
            argument
                .writeback
                .as_ref()
                .is_some_and(|writeback| writeback.parameter_slot == Some(slot))
        });
    let mut observations = Vec::new();
    match parameter.resolved_mechanism {
        ResolvedParameterMechanism::ByRef => {
            if has_copyback {
                observations.push(format!("param:{name}:byref-copyback-observed"));
            } else {
                observations.push(format!("param:{name}:gap:byref-copyback-not-observed"));
            }
        }
        ResolvedParameterMechanism::ByVal => {
            if has_copyback {
                observations.push(format!("param:{name}:mismatch:byval-copyback-observed"));
            } else {
                observations.push(format!("param:{name}:byval-no-copyback"));
            }
        }
        ResolvedParameterMechanism::PropertyValueByVal => {
            if has_copyback {
                observations.push(format!(
                    "param:{name}:mismatch:property-value-copyback-observed"
                ));
            } else {
                observations.push(format!("param:{name}:property-value-byval-no-copyback"));
            }
        }
        ResolvedParameterMechanism::Unknown | ResolvedParameterMechanism::EventSignatureOnly => {
            observations.push(format!(
                "param:{name}:gap:resolved-mechanism={:?}",
                parameter.resolved_mechanism
            ));
        }
    }
    if parameter.param_array {
        if recent_pre_call_array_literal_to_slot(bytecode, call_pc, slot)
            || descriptor_argument
                .and_then(|argument| argument.param_array.as_ref())
                .is_some()
        {
            observations.push(format!("param:{name}:paramarray-pack-observed"));
        } else {
            observations.push(format!("param:{name}:gap:paramarray-pack-not-observed"));
        }
    }
    if let OptionalParameterDescriptor::Optional {
        default_value: OptionalDefaultValue::ExplicitI32(default_value),
        ..
    } = &parameter.optional_descriptor
    {
        if recent_pre_call_explicit_i32_to_slot(bytecode, call_pc, slot, *default_value)
            || descriptor_argument.is_some_and(|argument| {
                argument.optional_default == Some(OptionalDefaultValue::ExplicitI32(*default_value))
            })
        {
            observations.push(format!("param:{name}:optional-default-i32-observed"));
        } else {
            observations.push(format!(
                "param:{name}:gap:optional-default-i32-not-observed"
            ));
        }
    }
    observations
}

fn post_call_copy_from_slot(bytecode: &Bytecode, call_pc: usize, slot: usize) -> bool {
    bytecode
        .instructions
        .iter()
        .skip(call_pc + 1)
        .take(CALL_EVIDENCE_COPY_LIMIT)
        .take_while(|instruction| matches!(instruction, Instruction::CopySlot { .. }))
        .any(|instruction| matches!(instruction, Instruction::CopySlot { src, .. } if *src == slot))
}

fn recent_pre_call_array_literal_to_slot(bytecode: &Bytecode, call_pc: usize, slot: usize) -> bool {
    recent_pre_call_instruction(
        bytecode,
        call_pc,
        |instruction| matches!(instruction, Instruction::IntrinsicArrayLiteral { dst, .. } if *dst == slot),
    )
}

fn recent_pre_call_explicit_i32_to_slot(
    bytecode: &Bytecode,
    call_pc: usize,
    slot: usize,
    value: i32,
) -> bool {
    recent_pre_call_instruction(
        bytecode,
        call_pc,
        |instruction| matches!(instruction, Instruction::LoadConstI32 { slot: dst, value: actual } if *dst == slot && *actual == value),
    )
}

fn recent_pre_call_instruction(
    bytecode: &Bytecode,
    call_pc: usize,
    mut matches_instruction: impl FnMut(&Instruction) -> bool,
) -> bool {
    let mut index = call_pc;
    let lower_bound = call_pc.saturating_sub(CALL_EVIDENCE_LOOKBACK);
    while index > lower_bound {
        index -= 1;
        let instruction = &bytecode.instructions[index];
        if matches_instruction(instruction) {
            return true;
        }
        if is_call_evidence_boundary(instruction) {
            break;
        }
    }
    false
}

fn is_call_evidence_boundary(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::CallProc { .. }
            | Instruction::Return
            | Instruction::Halt
            | Instruction::Jump { .. }
            | Instruction::JumpIfZero { .. }
    )
}

fn digest_package(
    bytecode_digest: &str,
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    project_context: Option<&BundleProjectContext>,
    export_inventory: Option<&ExportInventory>,
    descriptor_inventory: Option<&DescriptorInventory>,
    dynamic_object_routes: Option<&[ProjectDynamicObjectRoute]>,
    com_withevents_routes: Option<&[ProjectComWithEventsRoute]>,
) -> String {
    descriptor_digest_from_fields(
        DescriptorFamily::Package,
        &canonical_descriptor_id(DescriptorFamily::Package, ["vm-execution-package"]),
        [
            ("bytecode_digest", bytecode_digest.to_string()),
            ("procedure_metadata", format!("{procedure_metadata:#?}")),
            ("project_context", format!("{project_context:#?}")),
            ("export_inventory", format!("{export_inventory:#?}")),
            ("descriptor_inventory", format!("{descriptor_inventory:#?}")),
            (
                "dynamic_object_routes",
                format!("{dynamic_object_routes:#?}"),
            ),
            (
                "com_withevents_routes",
                format!("{com_withevents_routes:#?}"),
            ),
        ],
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DebugStepMode {
    Into,
    Over { depth: usize },
    Out { depth: usize },
}

#[derive(Debug, Clone)]
struct DebugRuntimeState {
    current_pc: usize,
    return_halts_when_stack_empty: bool,
    breakpoints: Vec<DebugBreakpoint>,
    pause_on_entry: bool,
    skip_pause_once_at_pc: Option<usize>,
    step_mode: Option<DebugStepMode>,
    last_pause: Option<DebugStop>,
}

fn runtime_invoke_kind_for_project_dynamic_member(
    kind: ProjectDynamicMemberKind,
) -> RuntimeMemberInvokeKind {
    match kind {
        ProjectDynamicMemberKind::Method | ProjectDynamicMemberKind::Function => {
            RuntimeMemberInvokeKind::Method
        }
        ProjectDynamicMemberKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        ProjectDynamicMemberKind::PropertyLet => RuntimeMemberInvokeKind::PropertyLet,
        ProjectDynamicMemberKind::PropertySet => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn runtime_invoke_kind_for_dynamic_call_hint(hint: DynamicCallKind) -> RuntimeMemberInvokeKind {
    match hint {
        DynamicCallKind::Method => RuntimeMemberInvokeKind::Method,
        DynamicCallKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        DynamicCallKind::PropertyLet => RuntimeMemberInvokeKind::PropertyLet,
        DynamicCallKind::PropertySet => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn dynamic_call_kind_for_project_member(hint: ProjectMemberCallKind) -> DynamicCallKind {
    match hint {
        ProjectMemberCallKind::Method => DynamicCallKind::Method,
        ProjectMemberCallKind::PropertyGet => DynamicCallKind::PropertyGet,
        ProjectMemberCallKind::PropertyLet => DynamicCallKind::PropertyLet,
        ProjectMemberCallKind::PropertySet => DynamicCallKind::PropertySet,
    }
}

fn leak_runtime_descriptor_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn optional_default_runtime_value(default: &OptionalDefaultValue) -> Variant {
    match default {
        OptionalDefaultValue::ExplicitI32(value) => Variant::from_i32(*value),
        OptionalDefaultValue::ExplicitBool(value) => Variant::from_bool(*value),
        OptionalDefaultValue::ExplicitString(value) => Variant::from_string(value.as_str()),
        OptionalDefaultValue::Unknown
        | OptionalDefaultValue::DeclaredTypeDefault
        | OptionalDefaultValue::VariantMissingError448
        | OptionalDefaultValue::ImplementationDefined => Variant::empty(),
    }
}

pub struct Vm {
    registers: RegisterFile,
    host_services: Arc<dyn HostServices>,
    call_stack: Vec<CallFrame>,
    activation_frames: Vec<ActivationFrame>,
    activation_entry_pcs: Vec<usize>,
    completed_activation_frames: HashMap<usize, HashMap<usize, RuntimeSlot>>,
    procedure_runtime_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    /// Union of every procedure's statement-entry PCs. A PC in this set is the first instruction
    /// of a statement, i.e. a statement boundary — where pending `Class_Terminate` work is drained
    /// (VBA terminates at statement granularity, never between a statement's sub-operations).
    statement_boundary_pcs: HashSet<usize>,
    /// Per-procedure (keyed by `entry_pc`) local-variable slot indices, released at the procedure
    /// epilogue so a local object reference going out of scope drops its reference (and may
    /// terminate). Excludes parameters, the return value, and temporaries.
    object_local_slots_by_entry_pc: HashMap<usize, Vec<usize>>,
    /// Per-procedure (keyed by `entry_pc`) temporary slot indices, released at each statement
    /// boundary so an expression temporary holding the only reference to an object (e.g. the
    /// result of `New Foo` before it is assigned, or `MakeFoo()` in a larger expression) drops
    /// that reference at end of statement — VBA's temporary lifetime — rather than lingering.
    object_temp_slots_by_entry_pc: HashMap<usize, Vec<usize>>,
    project_dynamic_objects: HashMap<i32, ProjectDynamicObjectState>,
    project_dynamic_dispatch_caches: HashMap<i32, oxvba_runtime::RuntimeDispatchPlanCache>,
    /// Monotonic allocator for per-instance project-object identities. Each `New` mints a fresh
    /// id from a high base so it never collides with compile-time route handles (which key the
    /// per-class route map). The id is the instance's `compat_identity` / state key, 1:1 with
    /// its IUnknown box.
    next_project_instance_id: i32,
    foreach_iterators: HashMap<i32, ForEachIteratorState>,
    next_foreach_iterator_id: i32,
    project_com_withevents_routes: HashMap<i32, Vec<ProjectComWithEventsRoute>>,
    withevents_bindings: HashMap<i64, RuntimeSlot>,
    com_withevents_subscriptions: HashMap<ComSubscriptionToken, ComWithEventsSubscription>,
    com_withevents_binding_subscriptions: HashMap<i64, Vec<ComSubscriptionToken>>,
    pending_callback_tokens: VecDeque<ComCallbackToken>,
    withevents_owner_iters: Vec<WithEventsOwnerIterator>,
    on_error_resume_next: bool,
    on_error_goto_label_target: Option<usize>,
    last_error: i32,
    last_error_pc: Option<usize>,
    last_error_description: Option<String>,
    last_error_source: Option<String>,
    rnd_state: u32,
    debug_runtime: Option<DebugRuntimeState>,
    last_package_identity_evidence: Option<VmPackageIdentityEvidence>,
}

#[derive(Clone)]
struct ActivationFrame {
    slots: HashMap<usize, RuntimeSlot>,
}

#[derive(Clone)]
struct CallFrame {
    return_pc: usize,
    error_frame: ErrorFrame,
    return_value: Option<CallReturnDescriptor>,
    return_value_fallback_slot: Option<usize>,
    writebacks: Vec<ArgumentWritebackDescriptor>,
}

const FIN_MAX_ITERS: usize = 60;
const FIN_EPS: f64 = 1e-10;
const FIN_DERIVATIVE_STEP: f64 = 1e-7;
const FIN_RATE_ERROR_CODE: i32 = 2001;
const FIN_NPER_ERROR_CODE: i32 = 2002;

fn default_host_services() -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(native_host_profile())
        .policy(HostPolicy::deterministic_runtime())
        .build()
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(default_host_services())
    }
}

impl Vm {
    pub fn new(host_services: Arc<dyn HostServices>) -> Self {
        Self {
            registers: RegisterFile::with_capacity(256),
            host_services,
            call_stack: Vec::new(),
            activation_frames: Vec::new(),
            activation_entry_pcs: Vec::new(),
            completed_activation_frames: HashMap::new(),
            procedure_runtime_metadata: BTreeMap::new(),
            statement_boundary_pcs: HashSet::new(),
            object_local_slots_by_entry_pc: HashMap::new(),
            object_temp_slots_by_entry_pc: HashMap::new(),
            project_dynamic_objects: HashMap::new(),
            project_dynamic_dispatch_caches: HashMap::new(),
            next_project_instance_id: 0x4000_0000,
            foreach_iterators: HashMap::new(),
            next_foreach_iterator_id: 1,
            project_com_withevents_routes: HashMap::new(),
            withevents_bindings: HashMap::new(),
            com_withevents_subscriptions: HashMap::new(),
            com_withevents_binding_subscriptions: HashMap::new(),
            pending_callback_tokens: VecDeque::new(),
            withevents_owner_iters: Vec::new(),
            on_error_resume_next: false,
            on_error_goto_label_target: None,
            last_error: 0,
            last_error_pc: None,
            last_error_description: None,
            last_error_source: None,
            rnd_state: 0x50000,
            debug_runtime: None,
            last_package_identity_evidence: None,
        }
    }

    fn clear_error_state(&mut self) {
        self.last_error = 0;
        self.last_error_pc = None;
        self.last_error_description = None;
        self.last_error_source = None;
    }

    fn route_runtime_error(
        &mut self,
        pc: usize,
        code: i32,
        detail: Option<&str>,
    ) -> Result<usize, String> {
        self.last_error = code;
        self.last_error_pc = Some(pc);
        self.last_error_description = detail.map(|s| s.to_string());
        if self.on_error_resume_next {
            // Error is handled by auto-advance; clear last_error_pc so that
            // Resume/Resume Next statements correctly detect "no pending error"
            // (VBA raises error 20 on Resume without a pending error).
            self.last_error_pc = None;
            return Ok(pc + 1);
        }
        if let Some(target_pc) = self.on_error_goto_label_target {
            return Ok(target_pc);
        }
        match detail {
            Some(detail) => Err(format!("runtime error: {code} ({detail})")),
            None => Err(format!("runtime error: {code}")),
        }
    }

    fn route_host_error(&mut self, pc: usize, err: HalError) -> Result<usize, String> {
        let detail = format!("{} [{}] {}", err.stable_code, err.operation, err.message);
        if let Some(code) = parse_embedded_runtime_error_code(&err.message) {
            return self.route_runtime_error(pc, code, Some(detail.as_str()));
        }
        let code = Self::hal_error_code(err.kind, err.capability);
        self.route_runtime_error(pc, code, Some(detail.as_str()))
    }

    fn hal_error_code(kind: HalErrorKind, capability: CapabilityId) -> i32 {
        let kind_code = match kind {
            HalErrorKind::CapabilityUnavailable => 1,
            HalErrorKind::PolicyDenied => 2,
            HalErrorKind::AdapterFault => 3,
            HalErrorKind::UnsupportedProfile => 4,
        };
        let capability_code = match capability {
            CapabilityId::UiInteraction => 1,
            CapabilityId::EventPump => 2,
            CapabilityId::FileSystemIo => 3,
            CapabilityId::ProcessEnv => 4,
            CapabilityId::ComActivationDispatch => 5,
            CapabilityId::TimeLocale => 6,
            CapabilityId::DynamicLinking => 7,
            CapabilityId::DiagnosticsTelemetry => 8,
            CapabilityId::ProjectCatalog => 9,
            CapabilityId::ProjectReferenceProvider => 10,
            CapabilityId::ProjectMutation => 11,
            CapabilityId::ConsoleIo => 12,
        };
        53_000 + capability_code * 10 + kind_code
    }

    fn ensure_slot_count(&mut self, slot_count: usize) {
        if slot_count > self.registers.registers.len() {
            self.registers
                .registers
                .resize(slot_count, RuntimeSlot::default());
        }
    }

    /// Retained value-model snapshot API.
    pub fn snapshot_variants(&self, slot_count: usize) -> Vec<Variant> {
        let end = slot_count.min(self.registers.registers.len());
        (0..end)
            .map(
                |slot| match self.read_runtime_slot(slot).unwrap_or_default() {
                    RuntimeSlot::Variant(value) => value,
                    RuntimeSlot::BindingHandle(handle) => {
                        panic!(
                            "VM register slot contains non-VBA BindingHandle {}",
                            handle.raw()
                        )
                    }
                },
            )
            .collect()
    }

    pub fn snapshot_procedure_variants(&self, entry_pc: usize, slots: &[usize]) -> Vec<Variant> {
        slots
            .iter()
            .map(|slot| {
                self.completed_activation_frames
                    .get(&entry_pc)
                    .and_then(|frame| frame.get(slot))
                    .cloned()
                    .or_else(|| self.read_runtime_slot(*slot).ok())
                    .unwrap_or_default()
            })
            .map(|value| match value {
                RuntimeSlot::Variant(value) => value,
                RuntimeSlot::BindingHandle(handle) => {
                    panic!(
                        "VM procedure snapshot contains non-VBA BindingHandle {}",
                        handle.raw()
                    )
                }
            })
            .collect()
    }

    pub fn package_identity_evidence(&self) -> Option<&VmPackageIdentityEvidence> {
        self.last_package_identity_evidence.as_ref()
    }

    fn package_identity_evidence_with_runtime_context(
        &self,
        package: &VmExecutionPackage<'_>,
    ) -> VmPackageIdentityEvidence {
        let mut evidence = package.identity_evidence_with_runtime_slots(&self.registers.registers);
        evidence
            .object_descriptor_evidence
            .extend(collect_runtime_object_descriptor_evidence(
                &self.project_dynamic_objects,
                &self.project_com_withevents_routes,
            ));
        sort_object_descriptor_evidence(&mut evidence.object_descriptor_evidence);
        evidence
    }

    pub fn set_project_dynamic_objects(&mut self, routes: Vec<ProjectDynamicObjectRoute>) {
        self.project_dynamic_dispatch_caches.clear();
        self.project_dynamic_objects = routes
            .into_iter()
            .map(|route| {
                let raw = route.object_handle;
                let descriptor = Self::leak_project_dynamic_class_descriptor(&route);
                (
                    raw,
                    ProjectDynamicObjectState {
                        object: ObjectRef::from_compat_identity_with_descriptor(raw, descriptor),
                        route,
                    },
                )
            })
            .collect();
    }

    fn leak_project_dynamic_class_descriptor(
        route: &ProjectDynamicObjectRoute,
    ) -> &'static RuntimeClassDescriptor {
        let members = route
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| {
                let params = member
                    .params
                    .iter()
                    .map(|param| RuntimeParamDescriptor {
                        name: leak_runtime_descriptor_str(param.name.clone()),
                        value_type: RuntimeValueType::Variant,
                        by_ref: false,
                        optional: param.optional,
                        param_array: param.param_array,
                    })
                    .collect::<Vec<_>>();
                RuntimeMemberDescriptor {
                    name: leak_runtime_descriptor_str(member.member_name.clone()),
                    dispatch_id: member
                        .dispatch_id
                        .or(member.known_dispatch_token)
                        .unwrap_or_else(|| (index as i32) + 1),
                    vtable_slot: Some((7 + index) as u16),
                    invoke_kind: runtime_invoke_kind_for_project_dynamic_member(member.kind),
                    arity: member.visible_param_count,
                    params: Box::leak(params.into_boxed_slice()),
                    return_type: member.return_slot.map(|_| RuntimeValueType::Variant),
                    is_default_member: member.is_default_member,
                }
            })
            .collect::<Vec<_>>();
        let interface_name = leak_runtime_descriptor_str(format!(
            "{}.{}._Default",
            route.project_name, route.module_name
        ));
        let class_name =
            leak_runtime_descriptor_str(format!("{}.{}", route.project_name, route.module_name));
        let members = Box::leak(members.into_boxed_slice());
        let interfaces = Box::leak(
            vec![RuntimeInterfaceDescriptor {
                id: RuntimeInterfaceId::IDispatch,
                identity: oxvba_runtime::RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
                name: interface_name,
                members,
                dual_dispatch: true,
            }]
            .into_boxed_slice(),
        );
        Box::leak(Box::new(RuntimeClassDescriptor {
            name: class_name,
            interfaces,
        }))
    }

    pub fn project_dynamic_object_ref(&self, raw: i32) -> Option<ObjectRef> {
        self.project_dynamic_objects
            .get(&raw)
            .map(|state| state.object.clone())
    }

    /// Mints a fresh per-instance project-object identity (see `next_project_instance_id`).
    fn alloc_project_instance_id(&mut self) -> i32 {
        let id = self.next_project_instance_id;
        self.next_project_instance_id = self.next_project_instance_id.wrapping_add(1);
        id
    }

    #[cfg(test)]
    pub fn resolve_project_dynamic_dispatch_plan_for_test(
        &mut self,
        raw: i32,
        member_name: &str,
        hint: DynamicCallKind,
        arity: usize,
    ) -> Option<oxvba_runtime::RuntimeDispatchPlan> {
        let object = self.project_dynamic_objects.get(&raw)?.object.clone();
        let interface = object.query_interface_descriptor(RuntimeInterfaceId::IDispatch)?;
        self.project_dynamic_dispatch_caches
            .entry(raw)
            .or_default()
            .resolve_member(
                interface,
                member_name,
                runtime_invoke_kind_for_dynamic_call_hint(hint),
                arity,
            )
    }

    #[cfg(test)]
    pub fn resolve_project_dynamic_default_dispatch_plan_for_test(
        &mut self,
        raw: i32,
        hint: DynamicCallKind,
        arity: usize,
    ) -> Option<oxvba_runtime::RuntimeDispatchPlan> {
        let object = self.project_dynamic_objects.get(&raw)?.object.clone();
        let interface = object.query_interface_descriptor(RuntimeInterfaceId::IDispatch)?;
        self.project_dynamic_dispatch_caches
            .entry(raw)
            .or_default()
            .resolve_default_member(
                interface,
                runtime_invoke_kind_for_dynamic_call_hint(hint),
                arity,
            )
    }

    pub fn resolve_project_dynamic_unhinted_dispatch_plan_for_test(
        &mut self,
        raw: i32,
        member_name: &str,
        arity: usize,
    ) -> Option<oxvba_runtime::RuntimeDispatchPlan> {
        let object = self.project_dynamic_objects.get(&raw)?.object.clone();
        let interface = object.query_interface_descriptor(RuntimeInterfaceId::IDispatch)?;
        self.project_dynamic_dispatch_caches
            .entry(raw)
            .or_default()
            .resolve_member_unhinted(interface, member_name, arity)
    }

    pub fn resolve_project_dynamic_unhinted_default_dispatch_plan_for_test(
        &mut self,
        raw: i32,
        arity: usize,
    ) -> Option<oxvba_runtime::RuntimeDispatchPlan> {
        let object = self.project_dynamic_objects.get(&raw)?.object.clone();
        let interface = object.query_interface_descriptor(RuntimeInterfaceId::IDispatch)?;
        self.project_dynamic_dispatch_caches
            .entry(raw)
            .or_default()
            .resolve_default_member_unhinted(interface, arity)
    }

    pub fn project_dynamic_dispatch_cache_len_for_test(&self, raw: i32) -> usize {
        self.project_dynamic_dispatch_caches
            .get(&raw)
            .map(|cache| cache.len())
            .unwrap_or_default()
    }

    pub fn set_project_procedure_runtime_metadata(
        &mut self,
        metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) {
        self.statement_boundary_pcs.clear();
        self.object_local_slots_by_entry_pc.clear();
        self.object_temp_slots_by_entry_pc.clear();
        for meta in metadata.values() {
            for &boundary_pc in &meta.statement_entry_pcs {
                self.statement_boundary_pcs.insert(boundary_pc);
            }
            let locals: Vec<usize> = meta
                .slots
                .iter()
                .filter(|slot| {
                    // Locals go out of scope at the epilogue. So does the hidden ByVal `Me`
                    // parameter of a class member (`__oxvba_this_instance`): the callee holds its
                    // own AddRef'd copy of the receiver, so releasing it on return is required for
                    // the receiver to reach refcount 0 promptly (e.g. after its Class_Initialize).
                    slot.kind == ProcedureRuntimeSlotKind::Local
                        || (slot.kind == ProcedureRuntimeSlotKind::Parameter
                            && slot.name.eq_ignore_ascii_case("__oxvba_this_instance"))
                })
                .map(|slot| slot.slot)
                .collect();
            if !locals.is_empty() {
                self.object_local_slots_by_entry_pc
                    .insert(meta.entry_pc, locals);
            }
            let temps: Vec<usize> = meta
                .slots
                .iter()
                .filter(|slot| slot.kind == ProcedureRuntimeSlotKind::Temporary)
                .map(|slot| slot.slot)
                .collect();
            if !temps.is_empty() {
                self.object_temp_slots_by_entry_pc
                    .insert(meta.entry_pc, temps);
            }
        }
        self.procedure_runtime_metadata = metadata;
    }

    pub fn load_execution_package_metadata(&mut self, package: &VmExecutionPackage<'_>) {
        self.set_project_procedure_runtime_metadata(package.procedure_metadata.clone());
    }

    pub fn set_project_com_withevents_routes(&mut self, routes: Vec<ProjectComWithEventsRoute>) {
        self.project_com_withevents_routes.clear();
        for route in routes {
            self.project_com_withevents_routes
                .entry(route.binding_token)
                .or_default()
                .push(route);
        }
    }

    pub fn debug_set_breakpoints(&mut self, breakpoints: Vec<DebugBreakpoint>) {
        if let Some(state) = &mut self.debug_runtime {
            state.breakpoints = breakpoints;
        } else {
            self.debug_runtime = Some(DebugRuntimeState {
                current_pc: 0,
                return_halts_when_stack_empty: false,
                breakpoints,
                pause_on_entry: false,
                skip_pause_once_at_pc: None,
                step_mode: None,
                last_pause: None,
            });
        }
    }

    pub fn debug_start_package(
        &mut self,
        package: &VmExecutionPackage<'_>,
    ) -> Result<DebugRunResult, String> {
        self.last_package_identity_evidence = None;
        self.load_execution_package_metadata(package);
        self.reset_execution_state(package.bytecode.slot_count, false);
        let mut state = self.debug_runtime.take().unwrap_or(DebugRuntimeState {
            current_pc: 0,
            return_halts_when_stack_empty: false,
            breakpoints: Vec::new(),
            pause_on_entry: true,
            skip_pause_once_at_pc: None,
            step_mode: None,
            last_pause: None,
        });
        state.current_pc = 0;
        state.return_halts_when_stack_empty = false;
        state.pause_on_entry = true;
        state.skip_pause_once_at_pc = None;
        state.step_mode = None;
        state.last_pause = None;
        self.debug_runtime = Some(state);
        self.resume_debug_package_session(package)
    }

    pub fn debug_continue_package(
        &mut self,
        package: &VmExecutionPackage<'_>,
    ) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = None;
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_package_session(package)
    }

    pub fn debug_step_into_package(
        &mut self,
        package: &VmExecutionPackage<'_>,
    ) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = Some(DebugStepMode::Into);
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_package_session(package)
    }

    pub fn debug_step_over_package(
        &mut self,
        package: &VmExecutionPackage<'_>,
    ) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = Some(DebugStepMode::Over {
            depth: self.activation_entry_pcs.len().max(1),
        });
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_package_session(package)
    }

    pub fn debug_step_out_package(
        &mut self,
        package: &VmExecutionPackage<'_>,
    ) -> Result<DebugRunResult, String> {
        let state = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| "debug session is not active".to_string())?;
        state.pause_on_entry = false;
        state.step_mode = Some(DebugStepMode::Out {
            depth: self.activation_entry_pcs.len().saturating_sub(1),
        });
        state.skip_pause_once_at_pc = Some(state.current_pc);
        state.last_pause = None;
        self.resume_debug_package_session(package)
    }

    pub fn debug_snapshot(&self) -> Option<DebugRuntimeSnapshot> {
        let state = self.debug_runtime.as_ref()?;
        Some(DebugRuntimeSnapshot {
            last_pause: state.last_pause.clone(),
            activation_entry_pcs: self.activation_entry_pcs.clone(),
        })
    }

    pub fn execute_package(&mut self, package: &VmExecutionPackage<'_>) -> Result<(), String> {
        self.load_execution_package_metadata(package);
        let fastpaths = package_descriptor_fastpaths_enabled(package);
        let result = self.execute_with_descriptor_metadata(package.bytecode, fastpaths);
        let identity_evidence = self.package_identity_evidence_with_runtime_context(package);
        self.last_package_identity_evidence = Some(identity_evidence);
        result
    }

    fn execute_with_descriptor_metadata(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        self.last_package_identity_evidence = None;
        self.reset_execution_state(bytecode.slot_count, false);
        self.execute_loop(bytecode, 0, 0, descriptor_selected_fastpaths, false)
    }

    fn invoke_procedure_with_i32_args_and_descriptor_metadata(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[i32],
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        self.last_package_identity_evidence = None;
        if arg_slots.len() != args.len() {
            return Err(format!(
                "argument shape mismatch: {} slots for {} values",
                arg_slots.len(),
                args.len()
            ));
        }
        if entry_pc >= bytecode.instructions.len() {
            return Err(format!("procedure entry out of range: {entry_pc}"));
        }
        self.reset_execution_state(bytecode.slot_count, true);
        for (slot, value) in arg_slots.iter().zip(args.iter()) {
            self.write_i32_slot(*slot, *value)?;
        }
        self.execute_loop(
            bytecode,
            entry_pc,
            entry_pc,
            descriptor_selected_fastpaths,
            true,
        )
    }

    pub fn invoke_package_procedure_with_i32_args(
        &mut self,
        package: &VmExecutionPackage<'_>,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[i32],
    ) -> Result<(), String> {
        self.load_execution_package_metadata(package);
        let fastpaths = package_descriptor_fastpaths_enabled(package);
        let result = self.invoke_procedure_with_i32_args_and_descriptor_metadata(
            package.bytecode,
            entry_pc,
            arg_slots,
            args,
            fastpaths,
        );
        let identity_evidence = self.package_identity_evidence_with_runtime_context(package);
        self.last_package_identity_evidence = Some(identity_evidence);
        result
    }

    fn invoke_procedure_with_variants_and_descriptor_metadata(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[Variant],
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        self.last_package_identity_evidence = None;
        if arg_slots.len() != args.len() {
            return Err(format!(
                "argument shape mismatch: {} slots for {} values",
                arg_slots.len(),
                args.len()
            ));
        }
        if entry_pc >= bytecode.instructions.len() {
            return Err(format!("procedure entry out of range: {entry_pc}"));
        }

        // Save caller's error frame so cross-module calls preserve the caller's
        // On Error handler. This is critical for VBA parity: On Error Resume Next
        // in the caller must catch Err.Raise from called procedures.
        let saved = ErrorFrame {
            on_error_resume_next: self.on_error_resume_next,
            on_error_goto_label_target: self.on_error_goto_label_target,
            last_error: self.last_error,
            last_error_pc: self.last_error_pc,
            last_error_description: self.last_error_description.take(),
            last_error_source: self.last_error_source.take(),
        };

        self.reset_execution_state(bytecode.slot_count, true);
        self.clear_invoked_procedure_slots(entry_pc)?;
        // Clearing stale entry-proc slots can release leftover objects from a prior host call;
        // discard any terminations that produced so this run begins with a clean queue.
        oxvba_runtime::reset_pending_terminations();
        self.push_activation_frame(entry_pc);
        for (slot, value) in arg_slots.iter().zip(args.iter()) {
            self.write_variant_slot(*slot, value.clone())?;
        }

        let result = self.execute_loop(
            bytecode,
            entry_pc,
            entry_pc,
            descriptor_selected_fastpaths,
            true,
        );
        self.flush_current_frame_to_registers();
        self.activation_entry_pcs.clear();
        self.activation_frames.clear();

        // Restore caller's error handling mode.
        self.on_error_resume_next = saved.on_error_resume_next;
        self.on_error_goto_label_target = saved.on_error_goto_label_target;

        match result {
            Ok(()) => {
                if self.last_error == 0 {
                    self.last_error = saved.last_error;
                    self.last_error_pc = saved.last_error_pc;
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                }
                Ok(())
            }
            Err(msg) => {
                if saved.on_error_resume_next {
                    let code = msg
                        .strip_prefix("runtime error: ")
                        .and_then(|rest| {
                            rest.split(|c: char| !c.is_ascii_digit() && c != '-')
                                .next()
                                .and_then(|s| s.parse::<i32>().ok())
                        })
                        .unwrap_or(5);
                    self.last_error = code;
                    self.last_error_pc = None;
                    self.last_error_description = Some(msg);
                    self.last_error_source = None;
                    Ok(())
                } else {
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                    Err(msg)
                }
            }
        }
    }

    pub fn invoke_package_procedure_with_variants(
        &mut self,
        package: &VmExecutionPackage<'_>,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[Variant],
    ) -> Result<(), String> {
        self.load_execution_package_metadata(package);
        let fastpaths = package_descriptor_fastpaths_enabled(package);
        let result = self.invoke_procedure_with_variants_and_descriptor_metadata(
            package.bytecode,
            entry_pc,
            arg_slots,
            args,
            fastpaths,
        );
        let identity_evidence = self.package_identity_evidence_with_runtime_context(package);
        self.last_package_identity_evidence = Some(identity_evidence);
        result
    }

    fn reset_execution_state(&mut self, slot_count: usize, preserve_withevents_bindings: bool) {
        self.ensure_slot_count(slot_count);
        oxvba_runtime::reset_pending_terminations();
        self.call_stack.clear();
        self.activation_frames.clear();
        self.activation_entry_pcs.clear();
        self.completed_activation_frames.clear();
        if !preserve_withevents_bindings {
            self.clear_all_com_withevents_state_best_effort();
            self.withevents_bindings.clear();
        }
        self.foreach_iterators.clear();
        self.next_foreach_iterator_id = 1;
        self.withevents_owner_iters.clear();
        self.on_error_resume_next = false;
        self.on_error_goto_label_target = None;
        self.clear_error_state();
    }

    fn frame_slot_set(&self, entry_pc: usize) -> HashSet<usize> {
        self.procedure_metadata_by_entry_pc(entry_pc)
            .map(|metadata| metadata.slots.iter().map(|slot| slot.slot).collect())
            .unwrap_or_default()
    }

    fn push_activation_frame(&mut self, entry_pc: usize) {
        let slot_set = self.frame_slot_set(entry_pc);
        let slots = slot_set
            .into_iter()
            .map(|slot| (slot, RuntimeSlot::default()))
            .collect();
        self.activation_entry_pcs.push(entry_pc);
        self.activation_frames.push(ActivationFrame { slots });
    }

    fn pop_activation_frame(&mut self) {
        self.activation_entry_pcs.pop();
        self.activation_frames.pop();
    }

    fn capture_current_activation_frame(&mut self, entry_pc: usize) {
        let Some(frame) = self.activation_frames.last() else {
            return;
        };
        let slots = frame
            .slots
            .iter()
            .map(|(slot, value)| (*slot, Self::completed_snapshot_slot(value)))
            .collect();
        self.completed_activation_frames.insert(entry_pc, slots);
    }

    fn completed_snapshot_slot(value: &RuntimeSlot) -> RuntimeSlot {
        match value {
            RuntimeSlot::Variant(value)
                if value
                    .as_object_ref()
                    .is_some_and(|object| object.terminates_on_release()) =>
            {
                RuntimeSlot::Variant(Variant::empty())
            }
            _ => value.clone(),
        }
    }

    fn current_frame_contains_slot(&self, slot: usize) -> bool {
        self.activation_frames
            .last()
            .is_some_and(|frame| frame.slots.contains_key(&slot))
    }

    fn flush_current_frame_to_registers(&mut self) {
        let Some(frame) = self.activation_frames.last() else {
            return;
        };
        for (slot, value) in &frame.slots {
            if *slot < self.registers.registers.len() {
                self.registers.registers[*slot] = value.clone();
            }
        }
    }

    fn clear_invoked_procedure_slots(&mut self, entry_pc: usize) -> Result<(), String> {
        let slots = self
            .procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
            .map(|metadata| metadata.slots.clone())
            .unwrap_or_default();
        for slot in slots {
            if matches!(
                slot.kind,
                ProcedureRuntimeSlotKind::Parameter
                    | ProcedureRuntimeSlotKind::Local
                    | ProcedureRuntimeSlotKind::ReturnValue
                    | ProcedureRuntimeSlotKind::CompilerGenerated
            ) {
                self.write_variant_slot(slot.slot, Variant::empty())?;
            }
        }
        Ok(())
    }

    fn resume_debug_session(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
    ) -> Result<DebugRunResult, String> {
        let (entry_pc, return_halts_when_stack_empty) = {
            let state = self
                .debug_runtime
                .as_ref()
                .ok_or_else(|| "debug session is not active".to_string())?;
            (state.current_pc, state.return_halts_when_stack_empty)
        };
        self.execute_loop(
            bytecode,
            entry_pc,
            self.activation_entry_pcs.last().copied().unwrap_or(0),
            descriptor_selected_fastpaths,
            return_halts_when_stack_empty,
        )?;
        let Some(state) = &self.debug_runtime else {
            return Ok(DebugRunResult::Completed);
        };
        if let Some(stop) = state.last_pause.clone() {
            Ok(DebugRunResult::Paused(stop))
        } else {
            self.debug_runtime = None;
            Ok(DebugRunResult::Completed)
        }
    }

    fn resume_debug_package_session(
        &mut self,
        package: &VmExecutionPackage<'_>,
    ) -> Result<DebugRunResult, String> {
        self.load_execution_package_metadata(package);
        let fastpaths = package_descriptor_fastpaths_enabled(package);
        let result = self.resume_debug_session(package.bytecode, fastpaths);
        let identity_evidence = self.package_identity_evidence_with_runtime_context(package);
        self.last_package_identity_evidence = Some(identity_evidence);
        result
    }

    fn debug_metadata_for_entry_pc(&self, entry_pc: usize) -> Option<&ProcedureRuntimeMetadata> {
        self.procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
    }

    fn debug_resolve_stop_location(&self, pc: usize) -> Option<DebugSourceLocation> {
        let entry_pc = self.activation_entry_pcs.last().copied().unwrap_or(0);
        let metadata = self.debug_metadata_for_entry_pc(entry_pc)?;
        let statement_index = metadata
            .statement_entry_pcs
            .iter()
            .position(|candidate| *candidate == pc)?;
        Some(DebugSourceLocation {
            module_name: metadata.module_name.clone(),
            procedure_name: metadata.procedure_name.clone(),
            entry_pc,
            statement_pc: pc,
            line_number: metadata
                .statement_line_numbers
                .get(statement_index)
                .copied(),
        })
    }

    fn debug_breakpoint_matches(
        breakpoint: &DebugBreakpoint,
        location: &DebugSourceLocation,
    ) -> bool {
        location.line_number == Some(breakpoint.line_number)
            && location
                .module_name
                .eq_ignore_ascii_case(&breakpoint.module_name)
    }

    fn maybe_pause_before_pc(&mut self, pc: usize) -> Option<DebugStop> {
        let location = self.debug_resolve_stop_location(pc)?;
        let current_depth = self.activation_entry_pcs.len().max(1);
        let state = self.debug_runtime.as_mut()?;

        if state.skip_pause_once_at_pc == Some(pc) {
            state.skip_pause_once_at_pc = None;
            return None;
        }

        let reason = if state.pause_on_entry {
            state.pause_on_entry = false;
            Some(DebugStopReason::Entry)
        } else if state
            .breakpoints
            .iter()
            .any(|breakpoint| Self::debug_breakpoint_matches(breakpoint, &location))
        {
            Some(DebugStopReason::Breakpoint)
        } else {
            match state.step_mode.clone() {
                Some(DebugStepMode::Into) => Some(DebugStopReason::Step),
                Some(DebugStepMode::Over { depth }) if current_depth <= depth => {
                    Some(DebugStopReason::Step)
                }
                Some(DebugStepMode::Out { depth }) if current_depth <= depth => {
                    Some(DebugStopReason::Step)
                }
                _ => None,
            }
        }?;

        state.current_pc = pc;
        state.step_mode = None;
        let stop = DebugStop {
            reason,
            location,
            call_stack_depth: current_depth,
        };
        state.last_pause = Some(stop.clone());
        Some(stop)
    }

    fn apply_descriptor_driven_call_entry_bindings(
        &mut self,
        call_pc: usize,
        target_pc: usize,
    ) -> Result<(), String> {
        let actions = self.descriptor_driven_call_entry_coercions(call_pc, target_pc)?;
        for (slot, value) in actions {
            self.write_variant_slot(slot, value)?;
        }
        Ok(())
    }

    fn descriptor_driven_call_entry_coercions(
        &self,
        call_pc: usize,
        target_pc: usize,
    ) -> Result<Vec<(usize, Variant)>, String> {
        let Some((caller_metadata, call_site)) =
            self.call_site_descriptor_for_call(call_pc, target_pc)
        else {
            return Ok(Vec::new());
        };
        if !matches!(
            call_site.target_kind,
            CallTargetKindDescriptor::Function | CallTargetKindDescriptor::Procedure
        ) {
            return Ok(Vec::new());
        }
        let Some(target_metadata) = self.procedure_metadata_by_entry_pc(target_pc) else {
            return Ok(Vec::new());
        };

        let mut actions = Vec::new();
        for argument in &call_site.arguments {
            let Some((parameter_slot, source_slot)) =
                selected_long_to_double_byval_call_entry_slots(
                    argument,
                    caller_metadata,
                    target_metadata,
                )
            else {
                continue;
            };
            if source_slot == parameter_slot {
                continue;
            }
            let source_value = self.read_variant_slot(parameter_slot)?;
            if source_value.vtype() != VarType::Long {
                continue;
            }
            let coerced = oxvba_runtime::coerce::coerce_to(&source_value, VarType::Double)
                .map_err(|err| {
                    format!(
                        "descriptor-driven call-entry coercion failed at call pc {call_pc}: {err}"
                    )
                })?;
            actions.push((parameter_slot, coerced));
        }
        Ok(actions)
    }

    fn call_site_descriptor_for_call(
        &self,
        call_pc: usize,
        target_pc: usize,
    ) -> Option<(&ProcedureRuntimeMetadata, &CallSiteDescriptor)> {
        self.procedure_runtime_metadata
            .values()
            .find_map(|metadata| {
                metadata
                    .call_sites
                    .iter()
                    .find(|call_site| {
                        call_site.call_pc == call_pc && call_site.target_entry_pc == Some(target_pc)
                    })
                    .map(|call_site| (metadata, call_site))
            })
    }

    fn procedure_metadata_by_entry_pc(&self, entry_pc: usize) -> Option<&ProcedureRuntimeMetadata> {
        self.procedure_runtime_metadata
            .values()
            .find(|metadata| metadata.entry_pc == entry_pc)
    }

    fn descriptor_declared_array_bound(
        &self,
        source_slot: usize,
        upper_bound: bool,
    ) -> Option<i32> {
        self.procedure_runtime_metadata
            .values()
            .flat_map(|metadata| metadata.array_shapes.iter())
            .find(|descriptor| {
                descriptor.base_slot == Some(source_slot)
                    && descriptor.rank == 1
                    && descriptor.storage == ArrayStorageKind::StaticFixed
                    && descriptor.bounds.len() == 1
            })
            .and_then(|descriptor| descriptor.bounds.first())
            .map(|bound| {
                if upper_bound {
                    bound.upper_bound
                } else {
                    bound.lower_bound
                }
            })
    }

    fn descriptor_declared_array_bound_for_intrinsic(
        &self,
        bytecode: &Bytecode,
        pc: usize,
        source_slot: usize,
        upper_bound: bool,
    ) -> Option<i32> {
        let mut candidate = source_slot;
        for _ in 0..DESCRIPTOR_INTRINSIC_LOOKBACK {
            if let Some(bound) = self.descriptor_declared_array_bound(candidate, upper_bound) {
                return Some(bound);
            }
            let copied_source = Self::recent_pre_intrinsic_copy_source(bytecode, pc, candidate)?;
            if copied_source == candidate {
                return None;
            }
            candidate = copied_source;
        }
        None
    }

    fn recent_pre_intrinsic_copy_source(
        bytecode: &Bytecode,
        pc: usize,
        dst_slot: usize,
    ) -> Option<usize> {
        let mut index = pc;
        let lower_bound = pc.saturating_sub(DESCRIPTOR_INTRINSIC_LOOKBACK);
        while index > lower_bound {
            index -= 1;
            let instruction = &bytecode.instructions[index];
            if let Instruction::CopySlot { dst, src } = instruction
                && *dst == dst_slot
            {
                return Some(*src);
            }
            if is_call_evidence_boundary(instruction) {
                break;
            }
        }
        None
    }

    fn execute_loop(
        &mut self,
        bytecode: &Bytecode,
        start_pc: usize,
        activation_entry_pc: usize,
        descriptor_selected_fastpaths: bool,
        return_halts_when_stack_empty: bool,
    ) -> Result<(), String> {
        let activation_depth = self.activation_entry_pcs.len();
        let frame_depth = self.activation_frames.len();
        if self.activation_entry_pcs.last().copied() != Some(activation_entry_pc) {
            self.push_activation_frame(activation_entry_pc);
        }
        let mut pc = start_pc;
        let len = bytecode.instructions.len();
        while pc < len {
            if self.maybe_pause_before_pc(pc).is_some() {
                return Ok(());
            }
            // Statement boundary: the previous statement is fully evaluated and `pc` is the first
            // instruction of the next. Release that statement's expression temporaries (their VBA
            // lifetime ends here) and run any `Class_Terminate` thereby queued, so teardown lands
            // between statements (VBA never interleaves it mid-statement).
            if self.statement_boundary_pcs.contains(&pc) {
                if let Some(entry_pc) = self.activation_entry_pcs.last().copied() {
                    self.release_object_temps(entry_pc);
                }
                if oxvba_runtime::has_pending_terminations() {
                    self.drain_pending_terminations(bytecode, descriptor_selected_fastpaths)?;
                }
            }
            match &bytecode.instructions[pc] {
                Instruction::LoadConstI32 { slot, value } => {
                    self.write_variant_slot(*slot, Variant::from_i32(*value))?;
                    pc += 1;
                }
                Instruction::CoerceNumeric { slot, target } => {
                    // Overflow guard for a fixed-width integer result/target: out of range is
                    // VBA error 6; in-range values pass through unchanged.
                    let value = self.read_variant_slot(*slot)?;
                    match crate::semantics::variant_overflow_check(&value, *target) {
                        Ok(()) => {
                            pc += 1;
                        }
                        Err(()) => {
                            pc = self.route_runtime_error(pc, 6, Some("Overflow"))?;
                        }
                    }
                }
                Instruction::LoadProjectObjectRef { dst, handle } => {
                    // Allocate a FRESH per-instance project object: a distinct IUnknown box
                    // (hence a distinct identity for `Is`) carrying the class's descriptor and
                    // route key, refcount 1, not pinned by the route map — so its lifetime
                    // tracks real references and same-site `New`s are distinct instances.
                    // Dispatch resolves the route via the object's route key; per-instance
                    // state keys on the fresh `compat_identity`.
                    let route_handle = self.read_i32_slot(*handle)?;
                    let route_info = self
                        .project_dynamic_objects
                        .get(&route_handle)
                        .map(|state| {
                            (
                                state.object.class_descriptor(),
                                state.route.class_terminate.is_some(),
                            )
                        });
                    let object = match route_info {
                        Some((descriptor, has_terminate)) => {
                            let instance_id = self.alloc_project_instance_id();
                            oxvba_runtime::ObjectRef::from_project_instance(
                                instance_id,
                                route_handle,
                                has_terminate,
                                descriptor,
                            )
                        }
                        None => oxvba_runtime::ObjectRef::from_compat_identity(route_handle),
                    };
                    self.write_variant_slot(*dst, Variant::from_object_ref(object))?;
                    pc += 1;
                }
                Instruction::LoadConstBool { slot, value } => {
                    self.write_variant_slot(*slot, Variant::from_bool(*value))?;
                    pc += 1;
                }
                Instruction::LoadConstString { slot, value } => {
                    self.write_variant_slot(
                        *slot,
                        Variant::from_string(BStr::from(value.clone())),
                    )?;
                    pc += 1;
                }
                Instruction::LoadConstF64 { slot, bits } => {
                    self.write_variant_slot(*slot, Variant::from_f64(f64::from_bits(*bits)))?;
                    pc += 1;
                }
                Instruction::AddConstI32 { slot, value } => {
                    if descriptor_selected_fastpaths && self.fast_add_const(*slot, *value) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*slot)?;
                    let out = crate::semantics::variant_add_const_value(
                        &lhs,
                        *value,
                        "add-const operand",
                    )?;
                    self.write_variant_slot(*slot, out)?;
                    pc += 1;
                }
                Instruction::AddSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_add_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::SubConstI32 { slot, value } => {
                    if descriptor_selected_fastpaths && self.fast_sub_const(*slot, *value) {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*slot)?;
                    let out = crate::semantics::variant_add_const_value(
                        &lhs,
                        -*value,
                        "sub-const operand",
                    )?;
                    self.write_variant_slot(*slot, out)?;
                    pc += 1;
                }
                Instruction::SubSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_sub_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::MulSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_mul_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::DivSlots { dst, lhs, rhs } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    match crate::semantics::variant_div_values(&lhs_val, &rhs_val)? {
                        Ok(out) => {
                            self.write_variant_slot(*dst, out)?;
                            pc += 1;
                        }
                        Err(11) => {
                            pc = self.route_runtime_error(pc, 11, Some("Division by zero"))?;
                        }
                        Err(code) => {
                            pc = self.route_runtime_error(pc, code, Some("Division failed"))?;
                        }
                    }
                }
                Instruction::IntDivSlots { dst, lhs, rhs } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    match crate::semantics::variant_intdiv_values(&lhs_val, &rhs_val)? {
                        Ok(out) => {
                            self.write_variant_slot(*dst, out)?;
                            pc += 1;
                        }
                        Err(11) => {
                            pc = self.route_runtime_error(pc, 11, Some("Division by zero"))?;
                        }
                        Err(code) => {
                            pc = self.route_runtime_error(
                                pc,
                                code,
                                Some("Integer division failed"),
                            )?;
                        }
                    }
                }
                Instruction::ModSlots { dst, lhs, rhs } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    match crate::semantics::variant_mod_values(&lhs_val, &rhs_val)? {
                        Ok(out) => {
                            self.write_variant_slot(*dst, out)?;
                            pc += 1;
                        }
                        Err(11) => {
                            pc = self.route_runtime_error(pc, 11, Some("Division by zero"))?;
                        }
                        Err(code) => {
                            pc = self.route_runtime_error(pc, code, Some("Modulo failed"))?;
                        }
                    }
                }
                Instruction::PowSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_pow_values(&lhs, &rhs)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::ConcatSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_concat_values(&lhs, &rhs);
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::NegSlot { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    let out = crate::semantics::variant_neg_value(&val)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::CopySlot { dst, src } => {
                    if descriptor_selected_fastpaths && self.fast_copy_slot(*dst, *src) {
                        pc += 1;
                        continue;
                    }
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicLenDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "Len operand")?;
                    self.write_variant_slot(*dst, Variant::from_i32(text.len() as i32))?;
                    pc += 1;
                }
                Instruction::IntrinsicLeftDigits { dst, src, count } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&src_val, "Left src")?;
                    let count_val = self.read_variant_slot(*count)?;
                    let n =
                        crate::semantics::runtime_variant_to_i32_compat(&count_val, "Left count")
                            .and_then(|value| {
                            usize::try_from(value)
                                .map_err(|_| format!("Left count cannot be negative: {value}"))
                        })?;
                    let result = if n >= text.len() {
                        text
                    } else {
                        text[..n].to_string()
                    };
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicRightDigits { dst, src, count } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&src_val, "Right src")?;
                    let count_val = self.read_variant_slot(*count)?;
                    let n =
                        crate::semantics::runtime_variant_to_i32_compat(&count_val, "Right count")
                            .and_then(|value| {
                                usize::try_from(value)
                                    .map_err(|_| format!("Right count cannot be negative: {value}"))
                            })?;
                    let len = text.len();
                    let result = if n >= len {
                        text
                    } else {
                        text[len - n..].to_string()
                    };
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidDigits {
                    dst,
                    src,
                    start,
                    count,
                } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&src_val, "Mid src")?;
                    let start_val = self.read_variant_slot(*start)?;
                    let st =
                        crate::semantics::runtime_variant_to_i32_compat(&start_val, "Mid start")
                            .and_then(|value| {
                                usize::try_from(value)
                                    .map_err(|_| format!("Mid start cannot be negative: {value}"))
                            })?;
                    let cnt = match count {
                        Some(slot) => {
                            let cv = self.read_variant_slot(*slot)?;
                            Some(
                                crate::semantics::runtime_variant_to_i32_compat(&cv, "Mid count")
                                    .and_then(|value| {
                                    usize::try_from(value).map_err(|_| {
                                        format!("Mid count cannot be negative: {value}")
                                    })
                                })?,
                            )
                        }
                        None => None,
                    };
                    let len = text.len();
                    let begin = if st == 0 { 0 } else { (st - 1).min(len) };
                    let end = match cnt {
                        Some(c) => (begin + c).min(len),
                        None => len,
                    };
                    let result = text[begin..end].to_string();
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicMidStmtDigits {
                    target,
                    start,
                    count,
                    value,
                } => {
                    let target_value = self.read_variant_slot(*target)?;
                    let start_value = self.read_variant_slot(*start)?;
                    let count_value = match count {
                        Some(slot) => Some(self.read_variant_slot(*slot)?),
                        None => None,
                    };
                    let value_value = self.read_variant_slot(*value)?;
                    self.write_variant_slot(
                        *target,
                        crate::semantics::runtime_mid_stmt_variant_bounded(
                            &target_value,
                            &start_value,
                            count_value.as_ref(),
                            &value_value,
                        )?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrDigits {
                    dst,
                    haystack,
                    needle,
                    mode,
                } => {
                    let hay_val = self.read_variant_slot(*haystack)?;
                    let nee_val = self.read_variant_slot(*needle)?;
                    let h = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&hay_val, "InStr haystack")?,
                        *mode,
                    );
                    let n = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&nee_val, "InStr needle")?,
                        *mode,
                    );
                    let pos = h.find(&n).map_or(0, |idx| (idx + 1) as i32);
                    self.write_variant_slot(*dst, Variant::from_i32(pos))?;
                    pc += 1;
                }
                Instruction::IntrinsicInStrRevDigits {
                    dst,
                    haystack,
                    needle,
                    mode,
                } => {
                    let hay_val = self.read_variant_slot(*haystack)?;
                    let nee_val = self.read_variant_slot(*needle)?;
                    let h = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&hay_val, "InStrRev haystack")?,
                        *mode,
                    );
                    let n = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&nee_val, "InStrRev needle")?,
                        *mode,
                    );
                    let pos = h.rfind(&n).map_or(0, |idx| (idx + 1) as i32);
                    self.write_variant_slot(*dst, Variant::from_i32(pos))?;
                    pc += 1;
                }
                Instruction::IntrinsicLowerDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "LCase operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.to_ascii_lowercase())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicUpperDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "UCase operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.to_ascii_uppercase())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSplitCountDigits {
                    dst,
                    src,
                    delimiter,
                } => {
                    let value = self.read_variant_slot(*src)?;
                    let delimiter = self.read_variant_slot(*delimiter)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_split_count_variant_bounded(&value, &delimiter)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicJoinDigits {
                    dst,
                    src,
                    delimiter,
                } => {
                    let value = self.read_variant_slot(*src)?;
                    let delimiter = self.read_variant_slot(*delimiter)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_join_variant_bounded(&value, &delimiter)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicReplaceDigits {
                    dst,
                    src,
                    find,
                    replace,
                } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let find_val = self.read_variant_slot(*find)?;
                    let replace_val = self.read_variant_slot(*replace)?;
                    let src_text =
                        crate::semantics::runtime_variant_to_text(&src_val, "Replace src")?;
                    let find_text =
                        crate::semantics::runtime_variant_to_text(&find_val, "Replace find")?;
                    let replace_text =
                        crate::semantics::runtime_variant_to_text(&replace_val, "Replace replace")?;
                    let result = src_text.replace(&find_text, &replace_text);
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IntrinsicTrimDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "Trim operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.trim().to_string())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicLTrimDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "LTrim operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.trim_start().to_string())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRTrimDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let text = crate::semantics::runtime_variant_to_text(&value, "RTrim operand")?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_string(BStr::from(text.trim_end().to_string())),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicStrCompDigits {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    let lhs_val = self.read_variant_slot(*lhs)?;
                    let rhs_val = self.read_variant_slot(*rhs)?;
                    let l = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&lhs_val, "StrComp lhs")?,
                        *mode,
                    );
                    let r = Self::normalize_for_compare(
                        crate::semantics::runtime_variant_to_text(&rhs_val, "StrComp rhs")?,
                        *mode,
                    );
                    let result = match l.cmp(&r) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(result))?;
                    pc += 1;
                }
                Instruction::IntrinsicLikeDigits {
                    dst,
                    lhs,
                    pattern,
                    mode,
                } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let pattern = self.read_variant_slot(*pattern)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_like_variant_bounded(&lhs, &pattern, *mode)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicDateSerialDigits {
                    dst,
                    year,
                    month,
                    day,
                } => {
                    let year = self.read_variant_slot(*year)?;
                    let month = self.read_variant_slot(*month)?;
                    let day = self.read_variant_slot(*day)?;
                    let out =
                        crate::semantics::runtime_date_serial_variant_bounded(&year, &month, &day)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimeSerialDigits {
                    dst,
                    hour,
                    minute,
                    second,
                } => {
                    let hour = self.read_variant_slot(*hour)?;
                    let minute = self.read_variant_slot(*minute)?;
                    let second = self.read_variant_slot(*second)?;
                    let out = crate::semantics::runtime_time_serial_variant_bounded(
                        &hour, &minute, &second,
                    )?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateValueDigits { dst, src } => {
                    let src = self.read_variant_slot(*src)?;
                    let out = crate::semantics::runtime_variant_to_datevalue(&src)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimeValueDigits { dst, src } => {
                    let src = self.read_variant_slot(*src)?;
                    let out = crate::semantics::runtime_variant_to_timevalue(&src)?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateAddDigits {
                    dst,
                    interval,
                    number,
                    date,
                } => {
                    let interval = self.read_variant_slot(*interval)?;
                    let number = self.read_variant_slot(*number)?;
                    let date = self.read_variant_slot(*date)?;
                    let out = crate::semantics::runtime_date_add_variant_bounded(
                        &interval, &number, &date,
                    )?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateDiffDigits {
                    dst,
                    interval,
                    date1,
                    date2,
                } => {
                    let interval = self.read_variant_slot(*interval)?;
                    let date1 = self.read_variant_slot(*date1)?;
                    let date2 = self.read_variant_slot(*date2)?;
                    let out = crate::semantics::runtime_date_diff_variant_bounded(
                        &interval, &date1, &date2,
                    )?;
                    self.write_variant_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicYearDigits { dst, src } => {
                    let v = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_variant_date_year(&v)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicMonthDigits { dst, src } => {
                    let v = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_variant_date_month(&v)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicDayDigits { dst, src } => {
                    let v = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, crate::semantics::runtime_variant_date_day(&v)?)?;
                    pc += 1;
                }
                Instruction::IntrinsicDateNowHost { dst } => {
                    match self.host_services.time_locale().date_serial_now_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicTimeNowHost { dst } => {
                    match self.host_services.time_locale().time_serial_now_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicNowHost { dst } => {
                    let date = match self.host_services.time_locale().date_serial_now_variant() {
                        Ok(value) => value,
                        Err(err) => {
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let time = match self.host_services.time_locale().time_serial_now_variant() {
                        Ok(value) => value,
                        Err(err) => {
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let value = crate::semantics::variant_host_now_value(&date, &time)?;
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicTimerHost { dst } => {
                    match self.host_services.time_locale().timer_ticks_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileOpenHost {
                    dst,
                    path,
                    mode,
                    file_number,
                } => {
                    let path = self.read_variant_slot(*path)?;
                    let mode_val = self.read_variant_slot(*mode)?;
                    let file_num = self.read_variant_slot(*file_number)?;
                    // Encode file_number into upper 16 bits of mode so the HAL
                    // can allocate the specific handle requested by the VBA source.
                    let mode_i32 = crate::semantics::variant_to_i32_compat(&mode_val, "Open mode")?;
                    let fnum_i32 =
                        crate::semantics::variant_to_i32_compat(&file_num, "Open file number")?;
                    let combined_mode = Variant::from_i32(mode_i32 | (fnum_i32 << 16));
                    match self.host_services.fs().open_variant(path, combined_mode) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileCloseHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().close_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileKillHost { dst, path } => {
                    let path = self.read_variant_slot(*path)?;
                    match self.host_services.fs().kill_variant(path) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFreeFileHost {
                    dst,
                    range_selector,
                } => {
                    let selector = if let Some(slot) = range_selector {
                        self.read_variant_slot(*slot)?
                    } else {
                        Variant::from_i32(0)
                    };
                    match self.host_services.fs().free_file_variant(selector) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileReadHost { dst, handle, count } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let count = self.read_variant_slot(*count)?;
                    match self.host_services.fs().read_bytes_variant(handle, count) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileWriteHost { dst, handle, data } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.fs().write_bytes_variant(handle, data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFilePrintHost { dst, handle, data } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.fs().print_line_variant(handle, data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicConsolePrintHost { dst, data } => {
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.console().print_line_variant(data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileInputHost { dst, handle, count } => {
                    let handle = self.read_variant_slot(*handle)?;
                    let count = self.read_variant_slot(*count)?;
                    match self.host_services.fs().input_fields_variant(handle, count) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicConsoleInputHost { dst, count } => {
                    let count = self.read_variant_slot(*count)?;
                    match self.host_services.console().input_fields_variant(count) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileLineInputHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().line_input_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicConsoleLineInputHost { dst } => {
                    match self.host_services.console().line_input_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileEofHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().eof_variant(handle) {
                        Ok(value) => {
                            let value = crate::semantics::variant_truthy_value(&value)?;
                            self.write_variant_slot(*dst, Variant::from_bool(value))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileLofHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().lof_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileSeekHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().loc_variant(handle) {
                        Ok(value) => {
                            let value = crate::semantics::variant_to_i32_compat(&value, "Loc")?;
                            self.write_variant_slot(*dst, Variant::from_i32(value + 1))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicFileLocHost { dst, handle } => {
                    let handle = self.read_variant_slot(*handle)?;
                    match self.host_services.fs().loc_variant(handle) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicMsgBoxHost { dst, prompt, style } => {
                    let prompt = self.read_variant_slot(*prompt)?;
                    let style = if let Some(slot) = style {
                        self.read_variant_slot(*slot)?
                    } else {
                        Variant::from_i32(1)
                    };
                    match self.host_services.ui().msg_box_variant(prompt, style) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicInputBoxHost {
                    dst,
                    prompt,
                    default_value,
                } => {
                    let prompt = self.read_variant_slot(*prompt)?;
                    let default_value = if let Some(slot) = default_value {
                        self.read_variant_slot(*slot)?
                    } else {
                        Variant::from_i32(0)
                    };
                    match self
                        .host_services
                        .ui()
                        .input_box_variant(prompt, default_value)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicBeepHost { dst } => {
                    match self
                        .host_services
                        .diag()
                        .emit_variant(Variant::from_i32(7), Variant::from_i32(0))
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDebugPrintHost { dst, data } => {
                    let data = self.read_variant_slot(*data)?;
                    match self.host_services.diag().debug_print_variant(data) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicStrPtr { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer = match value.vtype() {
                        oxvba_runtime::VarType::Empty | oxvba_runtime::VarType::Null => 0,
                        oxvba_runtime::VarType::String => {
                            let text = value.as_bstr().ok_or_else(|| {
                                "runtime error: invalid String Variant payload".to_string()
                            })?;
                            let utf8 = text.as_str();
                            oxvba_runtime::pointer_helpers::register_utf16_string(&utf8)?
                        }
                        _ => return Err("runtime error: 13 (Type mismatch)".to_string()),
                    };
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicVarPtr { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer = if let Some(array) = value.as_safearray() {
                        oxvba_runtime::pointer_helpers::register_array_payload_pointer(&array)?
                    } else {
                        oxvba_runtime::pointer_helpers::register_variant_pointer(&value)?
                    };
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicVarPtrStringVar { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer =
                        oxvba_runtime::pointer_helpers::register_string_variant_pointer(&value)?;
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicVarPtrVariantVar { dst, src } => {
                    let pointer = self.variant_cell_pointer(*src)?;
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicObjPtr { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let pointer =
                        oxvba_runtime::pointer_helpers::register_object_variant_pointer(&value)?;
                    self.write_variant_slot(*dst, Variant::from_i64(pointer))?;
                    pc += 1;
                }
                Instruction::IntrinsicDoEventsHost { dst } => {
                    if let Some(callback) = self.pending_callback_tokens.pop_front() {
                        self.write_variant_slot(*dst, Variant::from_i32(callback.raw()))?;
                        pc += 1;
                        continue;
                    }
                    match self.host_services.events().do_events_variant() {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicAbsI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_abs_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIntI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, crate::semantics::variant_int_value(&value)?)?;
                    pc += 1;
                }
                Instruction::IntrinsicFixI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(*dst, crate::semantics::variant_fix_value(&value)?)?;
                    pc += 1;
                }
                Instruction::IntrinsicSgnI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_sgn_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRoundI32 { dst, src, digits } => {
                    let value = self.read_variant_slot(*src)?;
                    let digits = match digits {
                        Some(slot) => Some(self.read_variant_slot(*slot)?),
                        None => None,
                    };
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_round_variant_bounded(&value, digits.as_ref())?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSqrI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_sqr_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSinI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_sin_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicCosI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_cos_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicLogI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_log_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicExpI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_exp_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicAtnI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_atn_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicTanI32 { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_tan_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicChrDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_chr_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicAscDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_asc_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicSpaceDigits { dst, count } => {
                    let count = self.read_variant_slot(*count)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_space_variant_bounded(&count)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicStringRepeatDigits { dst, count, ch } => {
                    let count_val = self.read_variant_slot(*count)?;
                    let ch_val = self.read_variant_slot(*ch)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_string_repeat_variant_bounded(
                            &count_val, &ch_val,
                        )?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicCStrDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    match oxvba_runtime::variant_to_vba_string(&src_val) {
                        Ok(result) => {
                            self.write_variant_slot(*dst, Variant::from_string(result))?;
                            pc += 1;
                        }
                        Err(msg) => {
                            pc = self.route_runtime_error(pc, 13, Some(&msg))?;
                        }
                    }
                }
                Instruction::IntrinsicStrFuncDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    match crate::semantics::runtime_variant_to_vba_str(&src_val) {
                        Ok(result) => {
                            self.write_variant_slot(*dst, result)?;
                            pc += 1;
                        }
                        Err(msg) => {
                            pc = self.route_runtime_error(pc, 13, Some(&msg))?;
                        }
                    }
                }
                Instruction::IntrinsicValDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let result = crate::semantics::runtime_val_variant_bounded(&src_val)?;
                    self.write_variant_slot(*dst, result)?;
                    pc += 1;
                }
                Instruction::IntrinsicCDateValue { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let result = crate::semantics::runtime_variant_to_cdate(&src_val)?;
                    self.write_variant_slot(*dst, result)?;
                    pc += 1;
                }
                Instruction::IntrinsicHexDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_hex_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicOctDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_oct_variant_bounded(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicWeekdayDigits { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_variant_date_weekday(&value)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicMonthNameDigits { dst, src } => {
                    let month = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_month_name_variant_bounded(&month)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicFvI32 {
                    dst,
                    rate,
                    nper,
                    pmt,
                    pv,
                    due,
                } => {
                    let rate = self.read_i32_slot(*rate)?;
                    let nper = self.read_i32_slot(*nper)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let pv = match pv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::fv_i32(rate, nper, pmt, pv, due)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicPvI32 {
                    dst,
                    rate,
                    nper,
                    pmt,
                    fv,
                    due,
                } => {
                    let rate = self.read_i32_slot(*rate)?;
                    let nper = self.read_i32_slot(*nper)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::pv_i32(rate, nper, pmt, fv, due)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicPmtI32 {
                    dst,
                    rate,
                    nper,
                    pv,
                    fv,
                    due,
                } => {
                    let rate = self.read_i32_slot(*rate)?;
                    let nper = self.read_i32_slot(*nper)?;
                    let pv = self.read_i32_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::pmt_i32(rate, nper, pv, fv, due)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicNpvI32 { dst, rate, values } => {
                    let rate = self.read_i32_slot(*rate)?;
                    let mut cash_flows = Vec::with_capacity(values.len());
                    for slot in values {
                        cash_flows.push(self.read_i32_slot(*slot)?);
                    }
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::npv_i32(rate, &cash_flows)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIrrI32 { dst, value, guess } => {
                    let value = self.read_i32_slot(*value)?;
                    let guess = match guess {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 10,
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(Self::irr_i32(value, guess)))?;
                    pc += 1;
                }
                Instruction::IntrinsicMirrI32 {
                    dst,
                    value,
                    finance_rate,
                    reinvest_rate,
                } => {
                    let value = self.read_i32_slot(*value)?;
                    let finance_rate = self.read_i32_slot(*finance_rate)?;
                    let reinvest_rate = self.read_i32_slot(*reinvest_rate)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(Self::mirr_i32(value, finance_rate, reinvest_rate)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRateI32 {
                    dst,
                    nper,
                    pmt,
                    pv,
                    fv,
                    due,
                    guess,
                } => {
                    let nper = self.read_i32_slot(*nper)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let pv = self.read_i32_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let guess = match guess {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 10,
                    };
                    let value = match Self::rate_i32(nper, pmt, pv, fv, due, guess) {
                        Ok(value) => Variant::from_i32(value),
                        Err(code) => Variant::from_error_code(code),
                    };
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicNPerI32 {
                    dst,
                    rate,
                    pmt,
                    pv,
                    fv,
                    due,
                } => {
                    let rate = self.read_i32_slot(*rate)?;
                    let pmt = self.read_i32_slot(*pmt)?;
                    let pv = self.read_i32_slot(*pv)?;
                    let fv = match fv {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let due = match due {
                        Some(slot) => self.read_i32_slot(*slot)?,
                        None => 0,
                    };
                    let value = match Self::nper_i32(rate, pmt, pv, fv, due) {
                        Ok(value) => Variant::from_i32(value),
                        Err(code) => Variant::from_error_code(code),
                    };
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayLiteral { dst, values } => {
                    let elements = values
                        .iter()
                        .map(|slot| self.read_variant_slot(*slot))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_safearray(SafeArray::from_variants(elements)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayAppend { dst, array, item } => {
                    let current = self.read_variant_slot(*array)?;
                    let item = self.read_variant_slot(*item)?;
                    let mut elements = if let Some(array) = current.as_safearray() {
                        array.variant_elements().unwrap_or_default()
                    } else if current.vtype() == oxvba_runtime::VarType::Empty
                        || current.as_i32() == Some(0)
                    {
                        Vec::new()
                    } else {
                        pc = self.route_runtime_error(
                            pc,
                            13,
                            Some(&format!(
                                "__oxvba_array_append expects an array or empty source, got {current:?}"
                            )),
                        )?;
                        continue;
                    };
                    elements.push(item);
                    self.write_variant_slot(
                        *dst,
                        Variant::from_safearray(SafeArray::from_variants(elements)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayResize {
                    dst,
                    upper_bounds,
                    lower_bounds,
                    element_type,
                } => {
                    let mut resolved_upper_bounds = Vec::with_capacity(upper_bounds.len());
                    let mut upper_error = None;
                    for upper_bound in upper_bounds {
                        match crate::semantics::runtime_variant_to_i32_compat(
                            &self.read_variant_slot(*upper_bound)?,
                            "ReDim upper bound",
                        ) {
                            Ok(value) => resolved_upper_bounds.push(value),
                            Err(detail) => {
                                upper_error = Some(detail);
                                break;
                            }
                        }
                    }
                    if let Some(detail) = upper_error {
                        pc = self.route_runtime_error(pc, 13, Some(&detail))?;
                        continue;
                    }
                    let array = match runtime_resized_array(
                        lower_bounds,
                        &resolved_upper_bounds,
                        *element_type,
                    ) {
                        Ok(array) => array,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*dst, Variant::from_safearray(array))?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayResizePreserve {
                    dst,
                    upper_bounds,
                    lower_bounds,
                    element_type,
                } => {
                    let mut resolved_upper_bounds = Vec::with_capacity(upper_bounds.len());
                    let mut upper_error = None;
                    for upper_bound in upper_bounds {
                        match crate::semantics::runtime_variant_to_i32_compat(
                            &self.read_variant_slot(*upper_bound)?,
                            "ReDim Preserve upper bound",
                        ) {
                            Ok(value) => resolved_upper_bounds.push(value),
                            Err(detail) => {
                                upper_error = Some(detail);
                                break;
                            }
                        }
                    }
                    if let Some(detail) = upper_error {
                        pc = self.route_runtime_error(pc, 13, Some(&detail))?;
                        continue;
                    }
                    let current = self.read_variant_slot(*dst)?;
                    let array = match runtime_resized_array_preserve(
                        &current,
                        lower_bounds,
                        &resolved_upper_bounds,
                        *element_type,
                    ) {
                        Ok(array) => array,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*dst, Variant::from_safearray(array))?;
                    pc += 1;
                }
                Instruction::IntrinsicArrayGet {
                    dst,
                    array,
                    indices,
                } => {
                    let array_value = self.read_variant_slot(*array)?;
                    let index_values = indices
                        .iter()
                        .map(|slot| self.read_variant_slot(*slot))
                        .collect::<Result<Vec<_>, _>>()?;
                    let value = match crate::semantics::runtime_array_get_variant(
                        &array_value,
                        &index_values,
                        "array index",
                    ) {
                        Ok(value) => value,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicArraySet {
                    array,
                    indices,
                    src,
                } => {
                    let array_value = self.read_variant_slot(*array)?;
                    let index_values = indices
                        .iter()
                        .map(|slot| self.read_variant_slot(*slot))
                        .collect::<Result<Vec<_>, _>>()?;
                    let src_value = self.read_variant_slot(*src)?;
                    let value = match crate::semantics::runtime_array_set_variant(
                        &array_value,
                        &index_values,
                        &src_value,
                        "array index",
                    ) {
                        Ok(value) => value,
                        Err(detail) => {
                            pc = self.route_runtime_error(pc, 9, Some(&detail))?;
                            continue;
                        }
                    };
                    self.write_variant_slot(*array, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicForEachInit { iter, src } => {
                    let iterable = self.read_variant_slot(*src)?;
                    match self.materialize_foreach_items(
                        bytecode,
                        descriptor_selected_fastpaths,
                        &iterable,
                    ) {
                        Ok(items) => {
                            let id = self.next_foreach_iterator_id;
                            self.next_foreach_iterator_id =
                                self.next_foreach_iterator_id.saturating_add(1);
                            self.foreach_iterators.insert(
                                id,
                                ForEachIteratorState {
                                    items,
                                    next_index: 0,
                                },
                            );
                            self.write_variant_slot(*iter, Variant::from_i32(id))?;
                            pc += 1;
                        }
                        Err(err) => {
                            pc = self.route_runtime_error(pc, err.code, Some(&err.detail))?
                        }
                    }
                }
                Instruction::IntrinsicForEachNext {
                    iter,
                    item,
                    has_value,
                } => {
                    let iter_id = crate::semantics::runtime_variant_to_i32_compat(
                        &self.read_variant_slot(*iter)?,
                        "For Each iterator slot",
                    )?;
                    let next = if let Some(state) = self.foreach_iterators.get_mut(&iter_id) {
                        if state.next_index < state.items.len() {
                            let value = state.items[state.next_index].clone();
                            state.next_index += 1;
                            Some(value)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if next.is_none() {
                        self.foreach_iterators.remove(&iter_id);
                    }
                    self.write_variant_slot(*has_value, Variant::from_bool(next.is_some()))?;
                    self.write_runtime_slot(*item, next.unwrap_or_default())?;
                    pc += 1;
                }
                Instruction::IntrinsicLBoundArray { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out = match crate::semantics::runtime_array_lbound_variant(
                        &value,
                        "LBound operand",
                    ) {
                        Ok(out) => out,
                        Err(detail) => self
                            .descriptor_declared_array_bound_for_intrinsic(
                                bytecode, pc, *src, false,
                            )
                            .ok_or_else(|| format!("runtime error: 13 ({detail})"))?,
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::IntrinsicUBoundArray { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out = match crate::semantics::runtime_array_ubound_variant(
                        &value,
                        "UBound operand",
                    ) {
                        Ok(out) => out,
                        Err(detail) => self
                            .descriptor_declared_array_bound_for_intrinsic(bytecode, pc, *src, true)
                            .ok_or_else(|| format!("runtime error: 13 ({detail})"))?,
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsArrayTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(
                            if matches!(value.vtype(), oxvba_runtime::VarType::ArrayVariant) {
                                1
                            } else {
                                0
                            },
                        ),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicVarTypeTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(crate::semantics::runtime_vartype_tag_bounded_variant(
                            &value,
                        )),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicVarType { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    let code = crate::semantics::runtime_vartype_compat_bounded_variant(&val);
                    self.write_variant_slot(*dst, Variant::from_i32(code))?;
                    pc += 1;
                }
                Instruction::IntrinsicTypeNameTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(crate::semantics::runtime_typename_tag_bounded_variant(
                            &value,
                        )),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNumericTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_i32(
                            crate::semantics::runtime_is_numeric_tag_bounded_variant(&value),
                        ),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNumeric { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    let is_numeric = matches!(
                        val.vtype(),
                        oxvba_runtime::VarType::Integer
                            | oxvba_runtime::VarType::Long
                            | oxvba_runtime::VarType::LongLong
                            | oxvba_runtime::VarType::Single
                            | oxvba_runtime::VarType::Double
                            | oxvba_runtime::VarType::Date
                            | oxvba_runtime::VarType::Currency
                            | oxvba_runtime::VarType::Decimal
                            | oxvba_runtime::VarType::Boolean
                            | oxvba_runtime::VarType::Byte
                    );
                    self.write_variant_slot(*dst, Variant::from_bool(is_numeric))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsError { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_bool(matches!(val.vtype(), oxvba_runtime::VarType::Error)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicCVErr { dst, src } => {
                    let code = self.read_i32_slot(*src)?.saturating_abs();
                    self.write_variant_slot(*dst, Variant::from_error_code(code))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsDateTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out = if crate::semantics::runtime_variant_is_date(&value) {
                        1
                    } else {
                        0
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsObjectTag { dst, src } => {
                    let value = self.read_variant_slot(*src)?;
                    let out = if crate::semantics::runtime_variant_is_object(&value) {
                        1
                    } else {
                        0
                    };
                    self.write_variant_slot(*dst, Variant::from_i32(out))?;
                    pc += 1;
                }
                Instruction::ValidateRuntimeAssignment {
                    src,
                    intent,
                    target_kind,
                    target_name,
                    target_type_name,
                } => {
                    let value = self.read_variant_slot(*src)?;
                    crate::semantics::validate_runtime_assignment_variant(
                        &value,
                        *intent,
                        *target_kind,
                        target_name,
                        target_type_name,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicShellHost { dst, command } => {
                    let command = self.read_variant_slot(*command)?;
                    match self
                        .host_services
                        .process()
                        .shell_variant(command, Variant::from_i32(0))
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicEnvironHost { dst, key } => {
                    let key = self.read_variant_slot(*key)?;
                    match self.host_services.process().environ_variant(key) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDirHost { dst, path } => {
                    let path = self.read_variant_slot(*path)?;
                    match self
                        .host_services
                        .process()
                        .dir_variant(path, Variant::from_i32(0))
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicCollectionAdd { dst, count, item } => {
                    let count = self.read_i32_slot(*count)?;
                    let _item = self.read_i32_slot(*item)?;
                    self.write_i32_slot(*dst, (count + 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionItem { dst, count, index } => {
                    let count = self.read_i32_slot(*count)?;
                    let index = self.read_i32_slot(*index)?;
                    let out = if index >= 1 && index <= count {
                        index
                    } else {
                        0
                    };
                    self.write_i32_slot(*dst, out)?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionRemove { dst, count, index } => {
                    let count = self.read_i32_slot(*count)?;
                    let _index = self.read_i32_slot(*index)?;
                    self.write_i32_slot(*dst, (count - 1).max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCollectionCount { dst, count } => {
                    let count = self.read_i32_slot(*count)?;
                    self.write_i32_slot(*dst, count.max(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicCreateObjectHost { dst, prog_id } => {
                    let prog_id = self.read_variant_slot(*prog_id)?;
                    match self.host_services.com().create_object_variant(prog_id) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicDispatchInvokeHost {
                    dst,
                    object,
                    member,
                    args,
                    early_bound,
                    call_kind_hint,
                    ..
                } => {
                    let invoke_label = if *early_bound {
                        "early_bound_com_invoke"
                    } else {
                        "dispatch_invoke"
                    };
                    let object_value = self.read_variant_slot(*object)?;
                    // VBA error 91: Object variable or With block variable not set.
                    if matches!(object_value.vtype(), oxvba_runtime::VarType::Empty) {
                        pc = self.route_runtime_error(
                            pc,
                            91,
                            Some("Object variable or With block variable not set"),
                        )?;
                        continue;
                    }
                    let object = match crate::semantics::variant_to_com_object(
                        &object_value,
                        "dispatch_invoke.object",
                    ) {
                        Ok(object) => {
                            // Also check for Nothing (ObjectRef raw identity 0).
                            if object.raw() == 0 {
                                pc = self.route_runtime_error(
                                    pc,
                                    91,
                                    Some("Object variable or With block variable not set"),
                                )?;
                                continue;
                            }
                            object
                        }
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                invoke_label,
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let member_value = self.read_variant_slot(*member)?;
                    let mut request = DynamicCallRequest {
                        object,
                        member: match crate::semantics::variant_to_dynamic_member_selector(
                            &member_value,
                            "dispatch_invoke.member",
                        ) {
                            Ok(member) => member,
                            Err(detail) => {
                                let err = HalError::adapter_fault(
                                    self.host_services.profile(),
                                    CapabilityId::ComActivationDispatch,
                                    invoke_label,
                                    detail,
                                );
                                pc = self.route_host_error(pc, err)?;
                                continue;
                            }
                        },
                        args: Vec::new(),
                        call_kind_hint: call_kind_hint.map(dynamic_call_kind_for_project_member),
                    };
                    for arg in args {
                        request.args.push(DynamicCallArg {
                            value: arg
                                .slot
                                .map(|slot| self.read_variant_slot(slot))
                                .transpose()?
                                .map(DynamicValue::from_variant),
                            name: arg.name.clone(),
                        });
                    }
                    match self.try_invoke_project_dynamic(
                        bytecode,
                        descriptor_selected_fastpaths,
                        &request,
                    ) {
                        Ok(Some(value)) => {
                            self.write_runtime_slot(*dst, value)?;
                            pc += 1;
                            continue;
                        }
                        Ok(None) => {}
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                invoke_label,
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    }
                    let bridge = HalComDynamicBridge::new(
                        self.host_services.profile(),
                        self.host_services.com(),
                    );
                    match bridge.invoke_dynamic(&request) {
                        Ok(value) => {
                            self.write_variant_slot(
                                *dst,
                                Self::normalize_dynamic_result_variant(value.variant()),
                            )?;
                            self.pump_project_com_withevents_callbacks(
                                bytecode,
                                descriptor_selected_fastpaths,
                            )?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComSubscribeEventHost { dst, object, event } => {
                    let object = self.read_variant_slot(*object)?;
                    let event = self.read_variant_slot(*event)?;
                    let object = match crate::semantics::variant_to_com_object(
                        &object,
                        "com_subscribe_event.object",
                    ) {
                        Ok(object) => object,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "subscribe_event",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let event = match crate::semantics::variant_to_com_member_token(
                        &event,
                        "com_subscribe_event.event",
                    ) {
                        Ok(event) => event,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "subscribe_event",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self.host_services.com().subscribe_event(object, event) {
                        Ok(value) => {
                            self.write_variant_slot(*dst, Variant::from_i32(value.raw()))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComUnsubscribeEventHost { dst, subscription } => {
                    let subscription = self.read_variant_slot(*subscription)?;
                    let subscription = match crate::semantics::variant_to_com_subscription_token(
                        &subscription,
                        "com_unsubscribe_event.subscription",
                    ) {
                        Ok(subscription) => subscription,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "unsubscribe_event",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .unsubscribe_event_variant(subscription)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComEventCallbackSubscriptionHost { dst, callback } => {
                    let callback = self.read_variant_slot(*callback)?;
                    let callback = match crate::semantics::variant_to_com_callback_token(
                        &callback,
                        "com_event_callback_subscription.callback",
                    ) {
                        Ok(callback) => callback,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "event_callback_subscription",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .event_callback_subscription(callback)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, Variant::from_i32(value.raw()))?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComEventCallbackArgHost {
                    dst,
                    callback,
                    index,
                } => {
                    let callback = self.read_variant_slot(*callback)?;
                    let index = self.read_variant_slot(*index)?;
                    let callback = match crate::semantics::variant_to_com_callback_token(
                        &callback,
                        "com_event_callback_arg.callback",
                    ) {
                        Ok(callback) => callback,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "event_callback_arg",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    let index = match if matches!(index.vtype(), oxvba_runtime::VarType::Empty) {
                        Ok(0)
                    } else {
                        crate::semantics::variant_to_usize_index(
                            &index,
                            "com_event_callback_arg.index",
                        )
                    } {
                        Ok(index) => index,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "event_callback_arg",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .event_callback_variant(callback, index)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicComReleaseEventCallbackHost { dst, callback } => {
                    let callback = self.read_variant_slot(*callback)?;
                    let callback = match crate::semantics::variant_to_com_callback_token(
                        &callback,
                        "com_release_event_callback.callback",
                    ) {
                        Ok(callback) => callback,
                        Err(detail) => {
                            let err = HalError::adapter_fault(
                                self.host_services.profile(),
                                CapabilityId::ComActivationDispatch,
                                "release_event_callback",
                                detail,
                            );
                            pc = self.route_host_error(pc, err)?;
                            continue;
                        }
                    };
                    match self
                        .host_services
                        .com()
                        .release_event_callback_variant(callback)
                    {
                        Ok(value) => {
                            self.write_variant_slot(*dst, value)?;
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicInvokeSymbolHost {
                    dst,
                    descriptor_id,
                    symbol,
                    args,
                    writeback_slots,
                } => {
                    let arg_variants: Vec<Variant> = args
                        .iter()
                        .map(|slot| self.read_variant_slot(*slot))
                        .collect::<Result<_, _>>()?;

                    if bytecode.external_call_descriptors.is_empty() {
                        let first_arg = arg_variants
                            .first()
                            .cloned()
                            .unwrap_or_else(|| Variant::from_i32(0));
                        match self
                            .host_services
                            .dynlink()
                            .invoke_symbol_variant(*symbol, &first_arg)
                        {
                            Ok(value) => {
                                self.write_variant_slot(*dst, value)?;
                                pc += 1;
                            }
                            Err(err) => pc = self.route_host_error(pc, err)?,
                        }
                        continue;
                    }

                    let Some(descriptor) = bytecode
                        .external_call_descriptors
                        .iter()
                        .find(|entry| entry.descriptor_id == *descriptor_id)
                    else {
                        let err = HalError::adapter_fault(
                            self.host_services.profile(),
                            CapabilityId::DynamicLinking,
                            "invoke_descriptor",
                            format!("unknown external descriptor id {}", descriptor_id),
                        );
                        pc = self.route_host_error(pc, err)?;
                        continue;
                    };

                    if descriptor.symbol != *symbol {
                        let err = HalError::adapter_fault(
                            self.host_services.profile(),
                            CapabilityId::DynamicLinking,
                            "invoke_descriptor",
                            format!(
                                "descriptor {} symbol mismatch: instruction={}, descriptor={}",
                                descriptor_id, symbol, descriptor.symbol
                            ),
                        );
                        pc = self.route_host_error(pc, err)?;
                        continue;
                    }

                    let param_type_strings: Vec<String> = descriptor
                        .param_types
                        .iter()
                        .map(|pt| format!("{:?}", pt))
                        .collect();
                    let view = DynLinkDescriptorView {
                        descriptor_id: descriptor.descriptor_id,
                        declared_name: descriptor.declared_name.as_str(),
                        library: descriptor.library.as_str(),
                        alias: descriptor.alias.as_str(),
                        ordinal_alias: descriptor.ordinal_alias,
                        symbol: descriptor.symbol,
                        marshal_lane: descriptor.marshal_lane.as_str(),
                        calling_convention: descriptor.calling_convention.as_str(),
                        selection_policy: descriptor.selection_policy.as_str(),
                        param_count: descriptor.param_count,
                        param_types: &param_type_strings,
                        param_by_ref: &descriptor.param_by_ref,
                        return_type: descriptor
                            .return_type
                            .as_ref()
                            .map(|rt| Cow::Owned(format!("{:?}", rt))),
                    };
                    if let Some(violation) = view.contract_violation() {
                        let err = HalError::adapter_fault(
                            self.host_services.profile(),
                            CapabilityId::DynamicLinking,
                            "invoke_descriptor",
                            format!(
                                "external descriptor contract violation for id {}: {}",
                                descriptor_id, violation
                            ),
                        );
                        pc = self.route_host_error(pc, err)?;
                        continue;
                    }

                    match self
                        .host_services
                        .dynlink()
                        .invoke_descriptor_variants(&view, &arg_variants)
                    {
                        Ok((ret_value, wb_values)) => {
                            self.write_variant_slot(*dst, ret_value)?;
                            if let Err(detail) = self.apply_external_writebacks(
                                writeback_slots,
                                &arg_variants,
                                &wb_values,
                            ) {
                                let err = HalError::adapter_fault(
                                    self.host_services.profile(),
                                    CapabilityId::DynamicLinking,
                                    "invoke_descriptor",
                                    detail,
                                );
                                pc = self.route_host_error(pc, err)?;
                                continue;
                            }
                            pc += 1;
                        }
                        Err(err) => pc = self.route_host_error(pc, err)?,
                    }
                }
                Instruction::IntrinsicWithEventsGet {
                    dst,
                    owner,
                    binding,
                } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let binding = self.read_variant_slot(*binding)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    let binding = crate::semantics::variant_to_withevents_binding_handle(
                        &binding, "binding",
                    )?;
                    let value = if self.is_project_instance_field_token(&owner, binding) {
                        RuntimeSlot::Variant(
                            owner
                                .project_field_get(binding.raw())
                                .unwrap_or_else(|| Variant::from_i32(0)),
                        )
                    } else {
                        let key = Self::withevents_binding_key(&owner, binding);
                        self.withevents_bindings
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| RuntimeSlot::Variant(Variant::from_i32(0)))
                    };
                    self.write_runtime_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsSet {
                    dst,
                    owner,
                    binding,
                    value,
                } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let binding = self.read_variant_slot(*binding)?;
                    let value = self.read_variant_slot(*value)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    let binding = crate::semantics::variant_to_withevents_binding_handle(
                        &binding, "binding",
                    )?;
                    if self.is_project_instance_field_token(&owner, binding) {
                        owner.project_field_set(binding.raw(), value.clone());
                    } else {
                        let key = Self::withevents_binding_key(&owner, binding);
                        self.clear_com_withevents_binding_subscriptions(key)?;
                        if value.as_i32() == Some(0) {
                            self.withevents_bindings.remove(&key);
                        } else {
                            self.withevents_bindings
                                .insert(key, RuntimeSlot::Variant(value.clone()));
                            self.sync_project_com_withevents_binding(owner, binding, &value)?;
                        }
                    }
                    self.write_variant_slot(*dst, value)?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsClearOwner { dst, owner } => {
                    let owner = self.read_variant_slot(*owner)?;
                    let owner =
                        crate::semantics::variant_to_withevents_owner_handle(&owner, "owner")?;
                    self.clear_com_withevents_owner_subscriptions(owner.clone())?;
                    self.withevents_bindings.retain(|key, _| {
                        Self::withevents_owner_from_key(*key).raw() != owner.raw()
                    });
                    self.write_variant_slot(*dst, Variant::from_i32(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsFirstOwner {
                    dst,
                    source,
                    binding,
                } => {
                    let source_slot = *source;
                    let source = self.read_variant_slot(source_slot)?;
                    let source_variant = if source.as_i32() == Some(0)
                        || source.as_i64() == Some(0)
                        || source.as_bool() == Some(false)
                    {
                        None
                    } else {
                        Some(source)
                    };
                    let binding = self.read_variant_slot(*binding)?;
                    let binding = crate::semantics::variant_to_withevents_binding_handle(
                        &binding, "binding",
                    )?;
                    let mut owners =
                        self.withevents_matching_owners(source_variant.as_ref(), binding);
                    owners.sort_unstable_by_key(|owner| owner.raw());
                    if owners.is_empty() {
                        self.write_variant_slot(*dst, Variant::from_i32(0))?;
                    } else {
                        let first = owners[0].clone();
                        self.withevents_owner_iters.push(WithEventsOwnerIterator {
                            owners,
                            next_index: 1,
                        });
                        self.write_variant_slot(*dst, Variant::from_object_ref(first))?;
                    }
                    pc += 1;
                }
                Instruction::IntrinsicWithEventsNextOwner { dst } => {
                    let next = if let Some(iter) = self.withevents_owner_iters.last_mut() {
                        if iter.next_index < iter.owners.len() {
                            let owner = iter.owners[iter.next_index].clone();
                            iter.next_index += 1;
                            Some(owner)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if next.is_none() {
                        let _ = self.withevents_owner_iters.pop();
                    }
                    let result = next
                        .map(Variant::from_object_ref)
                        .unwrap_or_else(|| Variant::from_i32(0));
                    self.write_variant_slot(*dst, result)?;
                    pc += 1;
                }
                Instruction::CmpEqSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if descriptor_selected_fastpaths
                        && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l == r)
                    {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
                        ord == std::cmp::Ordering::Equal
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpNeSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if descriptor_selected_fastpaths
                        && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l != r)
                    {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
                        ord != std::cmp::Ordering::Equal
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpLtSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if descriptor_selected_fastpaths
                        && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l < r)
                    {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
                        ord == std::cmp::Ordering::Less
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpLeSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if descriptor_selected_fastpaths
                        && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l <= r)
                    {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
                        ord != std::cmp::Ordering::Greater
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpGtSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if descriptor_selected_fastpaths
                        && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l > r)
                    {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
                        ord == std::cmp::Ordering::Greater
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpGeSlots {
                    dst,
                    lhs,
                    rhs,
                    mode,
                } => {
                    if descriptor_selected_fastpaths
                        && self.fast_cmp_slots(*dst, *lhs, *rhs, |l, r| l >= r)
                    {
                        pc += 1;
                        continue;
                    }
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::typed_compare_variants(&lhs, &rhs, *mode, |ord| {
                        ord != std::cmp::Ordering::Less
                    })?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::CmpObjectIsSlots { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::runtime_object_identity_is(&lhs, &rhs);
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::LoadErrNumber { slot } => {
                    self.write_variant_slot(*slot, Variant::from_i32(self.last_error))?;
                    pc += 1;
                }
                Instruction::LoadErrDescription { slot } => {
                    let text = self
                        .last_error_description
                        .as_deref()
                        .unwrap_or("")
                        .to_string();
                    self.write_variant_slot(*slot, Variant::from_string(BStr::from(text)))?;
                    pc += 1;
                }
                Instruction::LoadErrSource { slot } => {
                    let text = self.last_error_source.as_deref().unwrap_or("").to_string();
                    self.write_variant_slot(*slot, Variant::from_string(BStr::from(text)))?;
                    pc += 1;
                }
                Instruction::IntrinsicTypeOfIs {
                    dst,
                    object_slot,
                    type_name,
                } => {
                    let val = self.read_variant_slot(*object_slot)?;
                    let is_match = match val.as_object_ref() {
                        Some(handle) => {
                            if let Some(state) =
                                self.project_dynamic_objects.get(&handle.route_key())
                            {
                                state.route.module_name.eq_ignore_ascii_case(type_name)
                                    || state
                                        .route
                                        .implements_interfaces
                                        .iter()
                                        .any(|iface| iface.eq_ignore_ascii_case(type_name))
                            } else {
                                false
                            }
                        }
                        None => false,
                    };
                    self.write_variant_slot(*dst, Variant::from_bool(is_match))?;
                    pc += 1;
                }
                Instruction::IntrinsicIsNull { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_bool(matches!(val.vtype(), oxvba_runtime::VarType::Null)),
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicIsEmpty { dst, src } => {
                    let val = self.read_variant_slot(*src)?;
                    self.write_variant_slot(
                        *dst,
                        Variant::from_bool(matches!(val.vtype(), oxvba_runtime::VarType::Empty)),
                    )?;
                    pc += 1;
                }
                Instruction::LoadNull { slot } => {
                    self.write_variant_slot(*slot, Variant::null())?;
                    pc += 1;
                }
                Instruction::LoadEmpty { slot } => {
                    self.write_variant_slot(*slot, Variant::empty())?;
                    pc += 1;
                }
                Instruction::BoolNot { dst, src } => {
                    let src = self.read_variant_slot(*src)?;
                    let out = !crate::semantics::variant_truthy_value(&src)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::BoolAnd { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_truthy_value(&lhs)?
                        && crate::semantics::variant_truthy_value(&rhs)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::BoolOr { dst, lhs, rhs } => {
                    let lhs = self.read_variant_slot(*lhs)?;
                    let rhs = self.read_variant_slot(*rhs)?;
                    let out = crate::semantics::variant_truthy_value(&lhs)?
                        || crate::semantics::variant_truthy_value(&rhs)?;
                    self.write_variant_slot(*dst, Variant::from_bool(out))?;
                    pc += 1;
                }
                Instruction::JumpIfZero {
                    cond_slot,
                    target_pc,
                } => {
                    let cond = self.read_variant_slot(*cond_slot)?;
                    pc = Self::next_pc_for_jump_if_zero_variant(&cond, *target_pc, len, pc)?;
                }
                Instruction::Jump { target_pc } => {
                    pc = Self::next_pc_for_jump(*target_pc, len)?;
                }
                Instruction::CallProc { target_pc, .. } => {
                    if *target_pc >= bytecode.instructions.len() {
                        return Err(format!("call target out of range: {target_pc}"));
                    }
                    let call_site = self.call_site_for_call_cloned(pc, *target_pc);
                    let argument_values =
                        self.call_argument_values(call_site.as_ref(), *target_pc)?;
                    // Save caller's error frame and clear for new procedure.
                    let saved = ErrorFrame {
                        on_error_resume_next: self.on_error_resume_next,
                        on_error_goto_label_target: self.on_error_goto_label_target,
                        last_error: self.last_error,
                        last_error_pc: self.last_error_pc,
                        last_error_description: self.last_error_description.take(),
                        last_error_source: self.last_error_source.take(),
                    };
                    self.call_stack.push(CallFrame {
                        return_pc: pc + 1,
                        error_frame: saved,
                        return_value: call_site
                            .as_ref()
                            .and_then(|call_site| call_site.return_value.clone()),
                        return_value_fallback_slot: if call_site.is_none() {
                            self.procedure_metadata_by_entry_pc(*target_pc)
                                .and_then(|metadata| metadata.return_slot)
                        } else {
                            None
                        },
                        writebacks: Self::call_writebacks(call_site.as_ref()),
                    });
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = None;
                    self.clear_error_state();
                    self.push_activation_frame(*target_pc);
                    self.bind_call_argument_values(argument_values)?;
                    self.apply_descriptor_driven_call_entry_bindings(pc, *target_pc)?;
                    pc = *target_pc;
                }
                Instruction::SetOnErrorResumeNext => {
                    self.on_error_resume_next = true;
                    self.on_error_goto_label_target = None;
                    pc += 1;
                }
                Instruction::SetOnErrorGoto0 => {
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = None;
                    pc += 1;
                }
                Instruction::SetOnErrorGotoLabel { target_pc } => {
                    if *target_pc >= len {
                        return Err(format!("error handler target out of range: {target_pc}"));
                    }
                    self.on_error_resume_next = false;
                    self.on_error_goto_label_target = Some(*target_pc);
                    pc += 1;
                }
                Instruction::ResumeNext => {
                    if self.last_error_pc.is_none() {
                        // VBA error 20: Resume without error.
                        pc = self.route_runtime_error(pc, 20, Some("Resume without error"))?;
                    } else {
                        // Jump to the statement after the one that caused the error.
                        let resume_target = self.last_error_pc.unwrap() + 1;
                        self.clear_error_state();
                        pc = resume_target;
                    }
                }
                Instruction::Resume => {
                    if let Some(target_pc) = self.last_error_pc {
                        self.clear_error_state();
                        pc = target_pc;
                    } else {
                        // VBA error 20: Resume without error.
                        pc = self.route_runtime_error(pc, 20, Some("Resume without error"))?;
                    }
                }
                Instruction::ResumeLabel { target_pc } => {
                    if *target_pc >= len {
                        return Err(format!("resume target out of range: {target_pc}"));
                    }
                    if self.last_error_pc.is_none() {
                        // VBA error 20: Resume without error.
                        pc = self.route_runtime_error(pc, 20, Some("Resume without error"))?;
                    } else {
                        self.clear_error_state();
                        pc = *target_pc;
                    }
                }
                Instruction::RaiseError { code } => {
                    pc = self.route_runtime_error(pc, *code, None)?;
                }
                Instruction::ClearErr => {
                    self.clear_error_state();
                    pc += 1;
                }
                Instruction::Return => {
                    let returning_entry_pc = self.activation_entry_pcs.last().copied();
                    if let Some(call_frame) = self.call_stack.pop() {
                        let return_value = call_frame
                            .return_value
                            .as_ref()
                            .and_then(|descriptor| {
                                descriptor
                                    .return_slot
                                    .map(|slot| self.read_runtime_slot(slot))
                            })
                            .transpose()?;
                        let fallback_return_value = call_frame
                            .return_value_fallback_slot
                            .map(|slot| self.read_runtime_slot(slot))
                            .transpose()?;
                        let mut writeback_values = Vec::new();
                        for writeback in &call_frame.writebacks {
                            let value = writeback
                                .parameter_slot
                                .map(|slot| self.read_runtime_slot(slot))
                                .transpose()?;
                            writeback_values.push((writeback.clone(), value));
                        }
                        // Procedure epilogue: the callee's locals and temporaries go out of scope
                        // while the callee frame is still current.
                        if let Some(entry_pc) = returning_entry_pc {
                            self.release_object_locals(entry_pc);
                            self.release_object_temps(entry_pc);
                        }
                        if oxvba_runtime::has_pending_terminations() {
                            self.drain_pending_terminations(
                                bytecode,
                                descriptor_selected_fastpaths,
                            )?;
                        }
                        if let Some(entry_pc) = returning_entry_pc {
                            self.capture_current_activation_frame(entry_pc);
                        }
                        self.pop_activation_frame();
                        // Restore caller's error-handling state.
                        self.on_error_resume_next = call_frame.error_frame.on_error_resume_next;
                        self.on_error_goto_label_target =
                            call_frame.error_frame.on_error_goto_label_target;
                        self.last_error = call_frame.error_frame.last_error;
                        self.last_error_pc = call_frame.error_frame.last_error_pc;
                        self.last_error_description = call_frame.error_frame.last_error_description;
                        self.last_error_source = call_frame.error_frame.last_error_source;
                        for (writeback, value) in writeback_values {
                            if writeback.required
                                && let (Some(caller_slot), Some(value)) =
                                    (writeback.caller_slot, value)
                            {
                                self.write_runtime_slot(caller_slot, value)?;
                            }
                        }
                        if let Some(descriptor) = &call_frame.return_value
                            && let (Some(assign_target), Some(value)) =
                                (descriptor.assign_target_slot, return_value)
                        {
                            self.write_runtime_slot(assign_target, value)?;
                        }
                        if let (Some(slot), Some(value)) =
                            (call_frame.return_value_fallback_slot, fallback_return_value)
                        {
                            self.write_runtime_slot(slot, value)?;
                        }
                        pc = call_frame.return_pc;
                    } else if return_halts_when_stack_empty {
                        if let Some(entry_pc) = returning_entry_pc {
                            self.release_object_locals(entry_pc);
                            self.release_object_temps(entry_pc);
                        }
                        if oxvba_runtime::has_pending_terminations() {
                            self.drain_pending_terminations(
                                bytecode,
                                descriptor_selected_fastpaths,
                            )?;
                        }
                        if let Some(entry_pc) = returning_entry_pc {
                            self.capture_current_activation_frame(entry_pc);
                        }
                        self.flush_current_frame_to_registers();
                        self.activation_entry_pcs.truncate(activation_depth);
                        self.activation_frames.truncate(frame_depth);
                        break;
                    } else {
                        return Err("return with empty call stack".to_string());
                    }
                }
                Instruction::IntrinsicStrConvDigits {
                    dst,
                    src,
                    conversion,
                } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let conv_val = self.read_variant_slot(*conversion)?;
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_strconv_variant_bounded(&src_val, &conv_val)?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicRndDigits { dst, seed } => {
                    if let Some(seed_slot) = seed {
                        let seed_val = self.read_variant_slot(*seed_slot)?;
                        let seed_val = crate::semantics::runtime_random_seed_variant_bounded(
                            &seed_val, "Rnd seed",
                        )?;
                        if seed_val < 0 {
                            self.rnd_state = (seed_val as u32) & 0x00FF_FFFF;
                        } else if seed_val == 0 {
                            let result = self.rnd_state as f64 / 16_777_216.0;
                            self.write_variant_slot(*dst, Variant::from_f64(result))?;
                            pc += 1;
                            continue;
                        }
                    }
                    self.rnd_state = self
                        .rnd_state
                        .wrapping_mul(0x43FD_43FD)
                        .wrapping_add(0x0026_9EC3)
                        & 0x00FF_FFFF;
                    let result = self.rnd_state as f64 / 16_777_216.0;
                    self.write_variant_slot(*dst, Variant::from_f64(result))?;
                    pc += 1;
                }
                Instruction::IntrinsicRandomizeDigits { dst, seed } => {
                    if let Some(seed_slot) = seed {
                        let seed_val = self.read_variant_slot(*seed_slot)?;
                        let seed_val = crate::semantics::runtime_random_seed_variant_bounded(
                            &seed_val,
                            "Randomize seed",
                        )?;
                        self.rnd_state = (seed_val as u32) & 0x00FF_FFFF;
                    } else {
                        self.rnd_state = 0x50000;
                    }
                    self.write_variant_slot(*dst, Variant::from_i32(0))?;
                    pc += 1;
                }
                Instruction::IntrinsicFormatDigits {
                    dst,
                    value,
                    format_string,
                } => {
                    let val = self.read_variant_slot(*value)?;
                    let fmt_variant = if let Some(fmt_slot) = format_string {
                        Some(self.read_variant_slot(*fmt_slot)?)
                    } else {
                        None
                    };
                    self.write_variant_slot(
                        *dst,
                        crate::semantics::runtime_format_variant_bounded(
                            &val,
                            fmt_variant.as_ref(),
                        )?,
                    )?;
                    pc += 1;
                }
                Instruction::IntrinsicStrReverseDigits { dst, src } => {
                    let src_val = self.read_variant_slot(*src)?;
                    let s =
                        crate::semantics::runtime_variant_to_text(&src_val, "StrReverse source")?;
                    let result: String = s.chars().rev().collect();
                    self.write_variant_slot(*dst, Variant::from_string(BStr::from(result)))?;
                    pc += 1;
                }
                Instruction::IncSlot { slot } => {
                    if descriptor_selected_fastpaths && self.fast_add_const(*slot, 1) {
                        pc += 1;
                        continue;
                    }
                    let value = self.read_variant_slot(*slot)?;
                    let out = crate::semantics::variant_increment_value(&value)?;
                    self.write_variant_slot(*slot, out)?;
                    pc += 1;
                }
                Instruction::Halt => {
                    // End of the entry procedure: release its locals and temporaries and drain
                    // any final terminations before control returns to the host.
                    if let Some(entry_pc) = self.activation_entry_pcs.last().copied() {
                        self.release_object_locals(entry_pc);
                        self.release_object_temps(entry_pc);
                    }
                    if oxvba_runtime::has_pending_terminations() {
                        self.drain_pending_terminations(bytecode, descriptor_selected_fastpaths)?;
                    }
                    if let Some(entry_pc) = self.activation_entry_pcs.last().copied() {
                        self.capture_current_activation_frame(entry_pc);
                    }
                    self.flush_current_frame_to_registers();
                    break;
                }
            }
        }
        self.activation_entry_pcs.truncate(activation_depth);
        self.activation_frames.truncate(frame_depth);
        Ok(())
    }

    fn call_site_for_call_cloned(
        &self,
        call_pc: usize,
        target_pc: usize,
    ) -> Option<CallSiteDescriptor> {
        self.call_site_descriptor_for_call(call_pc, target_pc)
            .map(|(_, call_site)| call_site.clone())
    }

    fn call_argument_values(
        &self,
        call_site: Option<&CallSiteDescriptor>,
        target_pc: usize,
    ) -> Result<Vec<(usize, RuntimeSlot)>, String> {
        let Some(call_site) = call_site else {
            return self
                .procedure_metadata_by_entry_pc(target_pc)
                .map(|metadata| {
                    metadata
                        .param_slots
                        .iter()
                        .map(|slot| Ok((*slot, self.read_runtime_slot(*slot)?)))
                        .collect()
                })
                .unwrap_or_else(|| Ok(Vec::new()));
        };
        let mut values = Vec::new();
        for argument in &call_site.arguments {
            let Some(parameter_slot) = argument.parameter_slot else {
                continue;
            };
            let value = if let Some(source_slot) = argument.source_slot {
                self.read_runtime_slot(source_slot)?
            } else if let Some(default) = &argument.optional_default {
                RuntimeSlot::Variant(optional_default_runtime_value(default))
            } else {
                RuntimeSlot::Variant(Variant::empty())
            };
            values.push((parameter_slot, value));
        }
        Ok(values)
    }

    fn bind_call_argument_values(
        &mut self,
        values: Vec<(usize, RuntimeSlot)>,
    ) -> Result<(), String> {
        for (slot, value) in values {
            self.write_runtime_slot(slot, value)?;
        }
        Ok(())
    }

    fn call_writebacks(call_site: Option<&CallSiteDescriptor>) -> Vec<ArgumentWritebackDescriptor> {
        call_site
            .map(|call_site| {
                call_site
                    .arguments
                    .iter()
                    .filter_map(|argument| argument.writeback.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn read_i32_slot(&self, slot: usize) -> Result<i32, String> {
        crate::semantics::runtime_variant_to_i32_compat(
            &self.read_variant_slot(slot)?,
            "i32 intrinsic operand",
        )
        .map_err(|detail| format!("runtime value in slot {slot} cannot be read as i32: {detail}"))
    }

    fn write_i32_slot(&mut self, slot: usize, value: i32) -> Result<(), String> {
        self.write_variant_slot(slot, Variant::from_i32(value))
    }

    fn read_variant_slot(&self, slot: usize) -> Result<Variant, String> {
        if let Some(frame) = self.activation_frames.last()
            && let Some(value) = frame.slots.get(&slot)
        {
            return match value {
                RuntimeSlot::Variant(value) => Ok(value.clone()),
                RuntimeSlot::BindingHandle(handle) => Err(format!(
                    "slot {slot} contains internal BindingHandle {} and cannot be passed as a Variant",
                    handle.raw()
                )),
            };
        }
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        match &self.registers.registers[slot] {
            RuntimeSlot::Variant(value) => Ok(value.clone()),
            RuntimeSlot::BindingHandle(handle) => Err(format!(
                "slot {slot} contains internal BindingHandle {} and cannot be passed as a Variant",
                handle.raw()
            )),
        }
    }

    fn variant_cell_pointer(&self, slot: usize) -> Result<i64, String> {
        if let Some(frame) = self.activation_frames.last()
            && let Some(value) = frame.slots.get(&slot)
        {
            return value.variant_cell_pointer();
        }
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot].variant_cell_pointer()
    }

    fn write_variant_slot(&mut self, slot: usize, value: Variant) -> Result<(), String> {
        if self.current_frame_contains_slot(slot) {
            let frame = self
                .activation_frames
                .last_mut()
                .expect("current frame checked above");
            frame.slots.insert(slot, RuntimeSlot::Variant(value));
            return Ok(());
        }
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = RuntimeSlot::Variant(value);
        Ok(())
    }

    fn read_runtime_slot(&self, slot: usize) -> Result<RuntimeSlot, String> {
        if let Some(frame) = self.activation_frames.last()
            && let Some(value) = frame.slots.get(&slot)
        {
            return Ok(value.clone());
        }
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        Ok(self.registers.registers[slot].clone())
    }

    fn write_runtime_slot(&mut self, slot: usize, value: RuntimeSlot) -> Result<(), String> {
        if self.current_frame_contains_slot(slot) {
            let frame = self
                .activation_frames
                .last_mut()
                .expect("current frame checked above");
            frame.slots.insert(slot, value);
            return Ok(());
        }
        if slot >= self.registers.registers.len() {
            return Err(format!("slot out of range: {slot}"));
        }
        self.registers.registers[slot] = value;
        Ok(())
    }

    fn apply_external_writebacks(
        &mut self,
        writebacks: &[ExternalCallWriteback],
        arg_values: &[Variant],
        wb_values: &[Variant],
    ) -> Result<(), String> {
        for writeback in writebacks {
            let value = match writeback.kind {
                ExternalCallWritebackKind::ByRefValue => {
                    let Some(value) = wb_values.get(writeback.arg_index) else {
                        continue;
                    };
                    value.clone()
                }
                ExternalCallWritebackKind::PointerByteArrayPayload => {
                    let Some(pointer) = arg_values
                        .get(writeback.arg_index)
                        .and_then(Variant::as_i64)
                    else {
                        return Err(format!(
                            "pointer writeback arg {} is not a LongPtr value",
                            writeback.arg_index
                        ));
                    };
                    oxvba_runtime::pointer_helpers::read_back_byte_array_payload_variant(pointer)?
                }
                ExternalCallWritebackKind::PointerStringPayload => {
                    let Some(pointer) = arg_values
                        .get(writeback.arg_index)
                        .and_then(Variant::as_i64)
                    else {
                        return Err(format!(
                            "pointer writeback arg {} is not a LongPtr value",
                            writeback.arg_index
                        ));
                    };
                    oxvba_runtime::pointer_helpers::read_back_string_payload_variant(pointer)?
                }
            };
            self.write_variant_slot(writeback.source_slot, value)?;
        }
        Ok(())
    }

    fn withevents_binding_key(owner: &ObjectRef, binding: BindingHandle) -> i64 {
        crate::semantics::withevents_binding_key(owner, binding)
    }

    fn is_project_instance_field_token(&self, owner: &ObjectRef, binding: BindingHandle) -> bool {
        owner.is_project_instance()
            && self
                .project_dynamic_objects
                .get(&owner.route_key())
                .is_some_and(|state| state.route.field_tokens.contains(&binding.raw()))
    }

    fn withevents_binding_from_key(key: i64) -> BindingHandle {
        crate::semantics::withevents_binding_from_key(key)
    }

    fn withevents_owner_from_key(key: i64) -> ObjectRef {
        crate::semantics::withevents_owner_from_key(key)
    }

    fn materialize_foreach_items(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
        iterable: &Variant,
    ) -> Result<Vec<RuntimeSlot>, ForEachInitError> {
        if let Some(array) = iterable.as_safearray() {
            let values = array.variant_elements().ok_or_else(|| ForEachInitError {
                code: 13,
                detail: "For Each array source is missing materialized element payload".to_string(),
            })?;
            return Ok(Self::variants_to_slots(values));
        }

        if let Some(object) = iterable.as_object_ref() {
            return self.materialize_foreach_items_from_object(
                bytecode,
                descriptor_selected_fastpaths,
                object,
            );
        }

        if let Some(handle) = iterable.as_i32()
            && self.project_dynamic_objects.contains_key(&handle)
        {
            return self.materialize_foreach_items_from_object(
                bytecode,
                descriptor_selected_fastpaths,
                ObjectRef::from_compat_identity(handle),
            );
        }

        Err(ForEachInitError {
            code: 13,
            detail: format!("For Each expects an array or object source, got {iterable:?}"),
        })
    }

    fn materialize_foreach_items_from_object(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
        object: oxvba_runtime::ObjectRef,
    ) -> Result<Vec<RuntimeSlot>, ForEachInitError> {
        let request = DynamicCallRequest {
            object: object.clone(),
            member: DynamicMemberSelector::Token(-4),
            args: Vec::new(),
            call_kind_hint: Some(DynamicCallKind::PropertyGet),
        };
        let result_slot = match self.try_invoke_project_dynamic(
            bytecode,
            descriptor_selected_fastpaths,
            &request,
        ) {
            Ok(Some(value)) => value,
            Ok(None) => {
                let bridge = HalComDynamicBridge::new(
                    self.host_services.profile(),
                    self.host_services.com(),
                );
                bridge
                    .invoke_dynamic(&request)
                    .map(|value| RuntimeSlot::Variant(value.variant().clone()))
                    .map_err(|err| ForEachInitError {
                        code: 438,
                        detail: err.message,
                    })?
            }
            Err(detail) => return Err(ForEachInitError { code: 438, detail }),
        };

        match &result_slot {
            RuntimeSlot::Variant(value) => {
                let Some(array) = value.as_safearray() else {
                    return Err(ForEachInitError {
                        code: 13,
                        detail: format!(
                            "For Each NewEnum source on object {object} returned unsupported value {value:?}"
                        ),
                    });
                };
                let values = array.variant_elements().ok_or_else(|| ForEachInitError {
                    code: 13,
                    detail: format!(
                        "For Each NewEnum source on object {object} is missing element payload"
                    ),
                })?;
                Ok(Self::variants_to_slots(values))
            }
            RuntimeSlot::BindingHandle(handle) => Err(ForEachInitError {
                code: 13,
                detail: format!(
                    "For Each NewEnum source on object {object} returned unsupported BindingHandle {}",
                    handle.raw()
                ),
            }),
        }
    }

    fn variants_to_slots(values: Vec<Variant>) -> Vec<RuntimeSlot> {
        values.into_iter().map(RuntimeSlot::Variant).collect()
    }

    fn normalize_dynamic_result_variant(value: &Variant) -> Variant {
        value.clone()
    }

    fn project_dynamic_member_matches_hint(
        member: &ProjectDynamicMemberRoute,
        hint: DynamicCallKind,
    ) -> bool {
        match hint {
            DynamicCallKind::Method => matches!(
                member.kind,
                ProjectDynamicMemberKind::Method | ProjectDynamicMemberKind::Function
            ),
            DynamicCallKind::PropertyGet => member.kind == ProjectDynamicMemberKind::PropertyGet,
            DynamicCallKind::PropertyLet => member.kind == ProjectDynamicMemberKind::PropertyLet,
            DynamicCallKind::PropertySet => member.kind == ProjectDynamicMemberKind::PropertySet,
        }
    }

    fn default_project_dynamic_param_slot(param: &ProjectDynamicParamRoute) -> RuntimeSlot {
        RuntimeSlot::Variant(Variant::from_i32(param.default_value.unwrap_or(0)))
    }

    fn bind_project_dynamic_member_args(
        member: &ProjectDynamicMemberRoute,
        request_args: &[DynamicCallArg],
    ) -> Result<Vec<RuntimeSlot>, String> {
        if member.params.len() != member.visible_param_count {
            return Err(format!(
                "member metadata mismatch: {} visible params but {} param routes",
                member.visible_param_count,
                member.params.len()
            ));
        }

        let mut bound: Vec<Option<RuntimeSlot>> = vec![None; member.params.len()];
        let mut param_array_items = Vec::new();
        let param_array_index = member.params.iter().position(|param| param.param_array);
        let mut next_positional = 0usize;
        let mut named_seen = false;

        for arg in request_args {
            if let Some(name) = &arg.name {
                named_seen = true;
                let Some(index) = member
                    .params
                    .iter()
                    .position(|param| param.name.eq_ignore_ascii_case(name))
                else {
                    return Err(format!("unknown named argument `{name}`"));
                };
                let param = &member.params[index];
                if param.param_array {
                    return Err(format!(
                        "named arguments are not supported for ParamArray parameter `{}`",
                        param.name
                    ));
                }
                if bound[index].is_some() {
                    return Err(format!("argument `{}` is bound more than once", param.name));
                }
                bound[index] = Some(match &arg.value {
                    Some(value) => RuntimeSlot::Variant(value.variant().clone()),
                    None if param.optional => Self::default_project_dynamic_param_slot(param),
                    None => {
                        return Err(format!(
                            "required argument `{}` cannot be omitted",
                            param.name
                        ));
                    }
                });
                continue;
            }

            if named_seen {
                return Err("positional argument cannot follow named argument".to_string());
            }

            let Some(index) = (next_positional..member.params.len())
                .find(|&idx| member.params[idx].param_array || bound[idx].is_none())
            else {
                return Err(format!(
                    "too many arguments: request supplied {} visible args for {} parameters",
                    request_args.len(),
                    member.params.len()
                ));
            };
            let param = &member.params[index];
            if param.param_array {
                let Some(value) = &arg.value else {
                    return Err(format!(
                        "ParamArray parameter `{}` does not support omitted elements",
                        param.name
                    ));
                };
                param_array_items.push(value.variant().clone());
                next_positional = index;
                continue;
            }

            bound[index] = Some(match &arg.value {
                Some(value) => RuntimeSlot::Variant(value.variant().clone()),
                None if param.optional => Self::default_project_dynamic_param_slot(param),
                None => {
                    return Err(format!(
                        "required argument `{}` cannot be omitted",
                        param.name
                    ));
                }
            });
            next_positional = index + 1;
        }

        if let Some(index) = param_array_index {
            bound[index] = Some(RuntimeSlot::Variant(Variant::from_safearray(
                SafeArray::from_variants(param_array_items),
            )));
        }

        for (index, param) in member.params.iter().enumerate() {
            if bound[index].is_some() {
                continue;
            }
            bound[index] = Some(if param.param_array {
                RuntimeSlot::Variant(Variant::from_safearray(
                    SafeArray::from_variants(Vec::new()),
                ))
            } else if param.optional {
                Self::default_project_dynamic_param_slot(param)
            } else {
                return Err(format!("missing required argument `{}`", param.name));
            });
        }

        Ok(bound.into_iter().flatten().collect())
    }

    fn try_invoke_project_dynamic(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
        request: &DynamicCallRequest,
    ) -> Result<Option<RuntimeSlot>, String> {
        let object = request.object.clone();
        // Route (class dispatch table) is resolved by the object's route key; the per-instance
        // `raw()` identity is used only for state and the dispatch-plan cache.
        let Some(state) = self
            .project_dynamic_objects
            .get(&object.route_key())
            .cloned()
        else {
            return Ok(None);
        };
        let route = state.route;
        let cached_descriptor_candidate = match (&request.member, request.call_kind_hint) {
            (DynamicMemberSelector::Name(name), Some(hint)) => object
                .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
                .and_then(|interface| {
                    self.project_dynamic_dispatch_caches
                        .entry(object.raw())
                        .or_default()
                        .resolve_member(
                            interface,
                            name,
                            runtime_invoke_kind_for_dynamic_call_hint(hint),
                            request.args.len(),
                        )
                })
                .and_then(|plan| route.members.get(plan.member_index).cloned()),
            (DynamicMemberSelector::Name(name), None) => object
                .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
                .and_then(|interface| {
                    self.project_dynamic_dispatch_caches
                        .entry(object.raw())
                        .or_default()
                        .resolve_member_unhinted(interface, name, request.args.len())
                })
                .and_then(|plan| route.members.get(plan.member_index).cloned()),
            (DynamicMemberSelector::DefaultMember, Some(hint)) => object
                .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
                .and_then(|interface| {
                    self.project_dynamic_dispatch_caches
                        .entry(object.raw())
                        .or_default()
                        .resolve_default_member(
                            interface,
                            runtime_invoke_kind_for_dynamic_call_hint(hint),
                            request.args.len(),
                        )
                })
                .and_then(|plan| route.members.get(plan.member_index).cloned()),
            (DynamicMemberSelector::DefaultMember, None) => object
                .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
                .and_then(|interface| {
                    self.project_dynamic_dispatch_caches
                        .entry(object.raw())
                        .or_default()
                        .resolve_default_member_unhinted(interface, request.args.len())
                })
                .and_then(|plan| route.members.get(plan.member_index).cloned()),
            _ => None,
        };
        let mut candidates = match &request.member {
            DynamicMemberSelector::Name(name) => cached_descriptor_candidate.clone().map_or_else(
                || {
                    route
                        .members
                        .iter()
                        .filter(|member| member.member_name.eq_ignore_ascii_case(name))
                        .cloned()
                        .collect::<Vec<_>>()
                },
                |member| vec![member],
            ),
            DynamicMemberSelector::Token(token) => route
                .members
                .iter()
                .filter(|member| {
                    member.known_dispatch_token == Some(*token)
                        || member.dispatch_id == Some(*token)
                })
                .cloned()
                .collect::<Vec<_>>(),
            DynamicMemberSelector::DefaultMember => cached_descriptor_candidate.map_or_else(
                || {
                    route
                        .members
                        .iter()
                        .filter(|member| member.is_default_member)
                        .cloned()
                        .collect::<Vec<_>>()
                },
                |member| vec![member],
            ),
        };
        if let Some(hint) = request.call_kind_hint {
            candidates.retain(|member| Self::project_dynamic_member_matches_hint(member, hint));
        }
        let selector_label = match &request.member {
            DynamicMemberSelector::Name(name) => {
                format!("name `{}`", name.trim().to_ascii_lowercase())
            }
            DynamicMemberSelector::Token(token) => format!("token {}", token),
            DynamicMemberSelector::DefaultMember => "default member".to_string(),
        };
        candidates.sort_by(|lhs, rhs| {
            lhs.lowered_name
                .cmp(&rhs.lowered_name)
                .then(lhs.entry_pc.cmp(&rhs.entry_pc))
        });
        let mut bound_candidates = Vec::new();
        let mut binding_failures = Vec::new();
        for member in candidates {
            match Self::bind_project_dynamic_member_args(&member, &request.args) {
                Ok(values) => bound_candidates.push((member, values)),
                Err(detail) => binding_failures.push(format!("{:?}: {detail}", member.kind)),
            }
        }
        let (member, mut values) = match bound_candidates.as_slice() {
            [] => {
                let available = route
                    .members
                    .iter()
                    .map(|member| {
                        format!(
                            "{}/{}:{:?}/arity={}/default={}",
                            member
                                .known_dispatch_token
                                .map(|token| token.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            member
                                .dispatch_id
                                .map(|token| token.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            member.kind,
                            member.visible_param_count,
                            member.is_default_member
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let binding_context = if binding_failures.is_empty() {
                    String::new()
                } else {
                    format!("; binding failures: [{}]", binding_failures.join("; "))
                };
                return Err(format!(
                    "project dynamic dispatch target {} on `{}` object {} is unresolved for {} explicit args (available: [{}]{})",
                    selector_label,
                    route.module_name,
                    object.raw(),
                    request.args.len(),
                    available,
                    binding_context
                ));
            }
            [(member, values)] => (member.clone(), values.clone()),
            _ => {
                return Err(format!(
                    "project dynamic dispatch target {} on `{}` object {} is ambiguous for {} explicit args",
                    selector_label,
                    route.module_name,
                    object.raw(),
                    request.args.len()
                ));
            }
        };
        values.insert(
            0,
            RuntimeSlot::Variant(Variant::from_object_ref(object.clone())),
        );
        if member.param_slots.len() != values.len() {
            return Err(format!(
                "project dynamic dispatch target {} on `{}` object {} expects {} runtime slots but request built {} values",
                selector_label,
                route.module_name,
                object,
                member.param_slots.len(),
                values.len()
            ));
        }
        self.invoke_procedure_inline_with_slots(
            bytecode,
            member.entry_pc,
            &member.param_slots,
            &values,
            descriptor_selected_fastpaths,
        )?;
        Ok(Some(
            member
                .return_slot
                .map(|slot| self.read_runtime_slot(slot))
                .transpose()?
                .unwrap_or_default(),
        ))
    }

    fn invoke_procedure_inline_with_slots(
        &mut self,
        bytecode: &Bytecode,
        entry_pc: usize,
        arg_slots: &[usize],
        args: &[RuntimeSlot],
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        if arg_slots.len() != args.len() {
            return Err(format!(
                "argument shape mismatch: {} slots for {} values",
                arg_slots.len(),
                args.len()
            ));
        }
        if entry_pc >= bytecode.instructions.len() {
            return Err(format!("procedure entry out of range: {entry_pc}"));
        }

        // Save caller's error frame — each procedure has its own error context.
        let saved = ErrorFrame {
            on_error_resume_next: self.on_error_resume_next,
            on_error_goto_label_target: self.on_error_goto_label_target,
            last_error: self.last_error,
            last_error_pc: self.last_error_pc,
            last_error_description: self.last_error_description.take(),
            last_error_source: self.last_error_source.take(),
        };
        let call_stack_depth = self.call_stack.len();
        let activation_depth = self.activation_entry_pcs.len();
        let frame_depth = self.activation_frames.len();
        self.call_stack.push(CallFrame {
            return_pc: bytecode.instructions.len(),
            error_frame: saved.clone(),
            return_value: None,
            return_value_fallback_slot: self
                .procedure_metadata_by_entry_pc(entry_pc)
                .and_then(|metadata| metadata.return_slot),
            writebacks: Vec::new(),
        });

        // Callee starts with no error handler (VBA semantics).
        self.on_error_resume_next = false;
        self.on_error_goto_label_target = None;
        self.clear_error_state();

        self.push_activation_frame(entry_pc);
        for (slot, value) in arg_slots.iter().zip(args.iter()) {
            self.write_runtime_slot(*slot, value.clone())?;
        }

        let result = self.execute_loop(
            bytecode,
            entry_pc,
            entry_pc,
            descriptor_selected_fastpaths,
            true,
        );
        self.call_stack.truncate(call_stack_depth);
        self.activation_entry_pcs.truncate(activation_depth);
        self.activation_frames.truncate(frame_depth);

        // Restore caller's error handling mode.
        self.on_error_resume_next = saved.on_error_resume_next;
        self.on_error_goto_label_target = saved.on_error_goto_label_target;

        match result {
            Ok(()) => {
                // Callee succeeded. If the callee set an error (e.g., via On Error
                // Resume Next + Err.Raise inside the callee), preserve the callee's
                // error state for the caller's Err.Number. Otherwise restore the
                // caller's prior error state.
                if self.last_error == 0 {
                    self.last_error = saved.last_error;
                    self.last_error_pc = saved.last_error_pc;
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                }
                Ok(())
            }
            Err(msg) => {
                // Callee raised an unhandled error. Check if the caller can catch it.
                if saved.on_error_resume_next {
                    // Caller's On Error Resume Next absorbs the error.
                    let code = msg
                        .strip_prefix("runtime error: ")
                        .and_then(|rest| {
                            rest.split(|c: char| !c.is_ascii_digit() && c != '-')
                                .next()
                                .and_then(|s| s.parse::<i32>().ok())
                        })
                        .unwrap_or(5);
                    self.last_error = code;
                    self.last_error_pc = None;
                    self.last_error_description = Some(msg);
                    self.last_error_source = None;
                    Ok(())
                } else {
                    // No error handler in caller — propagate.
                    self.last_error_description = saved.last_error_description;
                    self.last_error_source = saved.last_error_source;
                    Err(msg)
                }
            }
        }
    }

    /// Sets each given slot that currently holds a terminating project object to Empty, dropping
    /// its reference (which may bring the instance to refcount 0 and queue its `Class_Terminate`).
    /// Scoped to objects whose class defines `Class_Terminate`: other object references (foreign
    /// COM, no-terminate project classes) are left intact, so their post-run slot value is
    /// observable and we avoid perturbing programs whose classes have no teardown.
    fn release_terminating_object_slots(&mut self, slots: &[usize]) {
        for &slot in slots {
            let terminates = matches!(
                self.read_runtime_slot(slot).ok(),
                Some(RuntimeSlot::Variant(value))
                    if value.as_object_ref().is_some_and(|object| object.terminates_on_release())
            );
            if terminates {
                let _ = self.write_variant_slot(slot, Variant::empty());
            }
        }
    }

    /// Releases a procedure's local references to terminating project objects at its epilogue
    /// (parameters, the return value, and temporaries are handled elsewhere or left intact).
    fn release_object_locals(&mut self, entry_pc: usize) {
        if let Some(slots) = self.object_local_slots_by_entry_pc.get(&entry_pc).cloned() {
            self.release_terminating_object_slots(&slots);
        }
    }

    /// Releases a procedure's expression temporaries that hold terminating project objects. Called
    /// at statement boundaries (temporaries do not outlive their statement in VBA) and at the
    /// procedure epilogue.
    fn release_object_temps(&mut self, entry_pc: usize) {
        if let Some(slots) = self.object_temp_slots_by_entry_pc.get(&entry_pc).cloned() {
            self.release_terminating_object_slots(&slots);
        }
    }

    /// Runs queued `Class_Terminate` teardown for project instances whose last reference was
    /// released. Called at statement boundaries and procedure epilogues so termination is
    /// statement-granular (VBA timing). Termination runs with `Me` bound to the original parked
    /// object box, so instance fields remain readable during `Class_Terminate` and are released
    /// afterwards only if `Me` was not resurrected.
    fn drain_pending_terminations(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        while oxvba_runtime::has_pending_terminations() {
            for (instance_id, route_key) in oxvba_runtime::take_pending_terminations() {
                let resolved = self
                    .project_dynamic_objects
                    .get(&route_key)
                    .and_then(|state| {
                        state
                            .route
                            .class_terminate
                            .clone()
                            .map(|member| (member, state.object.class_descriptor()))
                    });
                if let Some((member, descriptor)) = resolved {
                    let mut terminate_result = Ok(());
                    if let Some(me) = oxvba_runtime::retained_parked_termination_object(instance_id)
                    {
                        debug_assert_eq!(me.class_descriptor().name, descriptor.name);
                        let values = vec![RuntimeSlot::Variant(Variant::from_object_ref(me))];
                        if member.param_slots.len() == values.len() {
                            terminate_result = self.invoke_procedure_inline_with_slots(
                                bytecode,
                                member.entry_pc,
                                &member.param_slots,
                                &values,
                                descriptor_selected_fastpaths,
                            );
                        }
                    }
                    let freed = oxvba_runtime::finish_pending_termination(instance_id);
                    if freed {
                        let owner = ObjectRef::from_compat_identity(instance_id);
                        self.clear_com_withevents_owner_subscriptions(owner.clone())?;
                        self.withevents_bindings.retain(|key, _| {
                            Self::withevents_owner_from_key(*key).raw() != owner.raw()
                        });
                    }
                    terminate_result?;
                    continue;
                }
                if oxvba_runtime::finish_pending_termination(instance_id) {
                    let owner = ObjectRef::from_compat_identity(instance_id);
                    self.clear_com_withevents_owner_subscriptions(owner.clone())?;
                    self.withevents_bindings.retain(|key, _| {
                        Self::withevents_owner_from_key(*key).raw() != owner.raw()
                    });
                }
            }
        }
        Ok(())
    }

    fn withevents_matching_owners(
        &self,
        source_variant: Option<&Variant>,
        binding: BindingHandle,
    ) -> Vec<ObjectRef> {
        let Some(source_variant) = source_variant else {
            return Vec::new();
        };
        let mut owners = Vec::new();
        for (key, value) in &self.withevents_bindings {
            let RuntimeSlot::Variant(value) = value else {
                continue;
            };
            if !Self::withevents_source_matches(value, source_variant)
                || Self::withevents_binding_from_key(*key) != binding
            {
                continue;
            }
            owners.push(Self::withevents_owner_from_key(*key));
        }
        owners
    }

    fn withevents_source_matches(bound: &Variant, source: &Variant) -> bool {
        if bound == source {
            return true;
        }
        matches!(
            (
                Self::withevents_source_identity(bound),
                Self::withevents_source_identity(source)
            ),
            (Some(bound), Some(source)) if bound == source
        )
    }

    fn withevents_source_identity(value: &Variant) -> Option<i32> {
        if let Some(object) = value.as_object_ref() {
            let raw = object.raw();
            if raw > 0 {
                return Some(raw);
            }
        }
        value.as_i32().filter(|raw| *raw > 0)
    }

    fn clear_all_com_withevents_state_best_effort(&mut self) {
        for subscription in self
            .com_withevents_subscriptions
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            let _ = self
                .host_services
                .com()
                .unsubscribe_event_variant(subscription);
        }
        self.com_withevents_subscriptions.clear();
        self.com_withevents_binding_subscriptions.clear();
        self.pending_callback_tokens.clear();
    }

    fn clear_com_withevents_binding_subscriptions(&mut self, key: i64) -> Result<(), String> {
        let Some(subscriptions) = self.com_withevents_binding_subscriptions.remove(&key) else {
            return Ok(());
        };
        for subscription in subscriptions {
            self.com_withevents_subscriptions.remove(&subscription);
            self.host_services
                .com()
                .unsubscribe_event_variant(subscription)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    fn clear_com_withevents_owner_subscriptions(&mut self, owner: ObjectRef) -> Result<(), String> {
        let keys = self
            .com_withevents_binding_subscriptions
            .keys()
            .copied()
            .filter(|key| Self::withevents_owner_from_key(*key).raw() == owner.raw())
            .collect::<Vec<_>>();
        for key in keys {
            self.clear_com_withevents_binding_subscriptions(key)?;
        }
        Ok(())
    }

    fn sync_project_com_withevents_binding(
        &mut self,
        owner: ObjectRef,
        binding: BindingHandle,
        value: &Variant,
    ) -> Result<(), String> {
        let Some(routes) = self
            .project_com_withevents_routes
            .get(&binding.raw())
            .cloned()
        else {
            return Ok(());
        };
        if value.as_i32() == Some(0) {
            return Ok(());
        }
        let object = value
            .as_object_ref()
            .ok_or_else(|| "withevents.value is not an object Variant".to_string())?;
        if object.raw() == 0 {
            return Ok(());
        }
        let descriptor = self
            .host_services
            .com()
            .describe_object(object.clone())
            .map_err(|err| err.to_string())?;
        let key = Self::withevents_binding_key(&owner, binding);
        let Some(descriptor) = descriptor else {
            return Ok(());
        };
        for route in routes {
            if !descriptor
                .prog_id_name
                .eq_ignore_ascii_case(&route.prog_id_name)
            {
                continue;
            }
            let subscription = self
                .host_services
                .com()
                .subscribe_event(object.clone(), route.event_token.into())
                .map_err(|err| err.to_string())?;
            self.com_withevents_subscriptions.insert(
                subscription,
                ComWithEventsSubscription {
                    owner_object: owner.clone(),
                    route,
                },
            );
            self.com_withevents_binding_subscriptions
                .entry(key)
                .or_default()
                .push(subscription);
        }
        Ok(())
    }

    fn poll_host_callback_token(&self) -> Result<Option<ComCallbackToken>, String> {
        let value = self
            .host_services
            .events()
            .do_events_variant()
            .map_err(|err| err.to_string())?;
        if value.as_i32() == Some(0) || value.as_i64() == Some(0) || value.as_bool() == Some(false)
        {
            return Ok(None);
        }
        if let Some(raw) = value.as_i32() {
            return Ok(Some(ComCallbackToken::new(raw)));
        }
        if let Some(raw) = value.as_i64() {
            let raw = i32::try_from(raw).map_err(|_| {
                format!("do_events callback exceeds i32 callback-token range: {raw}")
            })?;
            return Ok(Some(ComCallbackToken::new(raw)));
        }
        Err(format!(
            "do_events callback requires callback-token-compatible Variant, got {:?}",
            value.vtype()
        ))
    }

    fn invoke_project_symbol_inline_with_variants(
        &mut self,
        bytecode: &Bytecode,
        symbol: &str,
        args: &[Variant],
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        let normalized = symbol.trim().to_ascii_lowercase();
        let metadata = self
            .procedure_runtime_metadata
            .get(&normalized)
            .cloned()
            .ok_or_else(|| format!("project procedure metadata missing for `{normalized}`"))?;
        let args = args
            .iter()
            .cloned()
            .map(RuntimeSlot::Variant)
            .collect::<Vec<_>>();
        self.invoke_procedure_inline_with_slots(
            bytecode,
            metadata.entry_pc,
            &metadata.param_slots,
            &args,
            descriptor_selected_fastpaths,
        )
    }

    fn pump_project_com_withevents_callbacks(
        &mut self,
        bytecode: &Bytecode,
        descriptor_selected_fastpaths: bool,
    ) -> Result<(), String> {
        if self.com_withevents_subscriptions.is_empty() {
            return Ok(());
        }
        loop {
            let Some(callback) = self.poll_host_callback_token()? else {
                return Ok(());
            };
            let subscription = self
                .host_services
                .com()
                .event_callback_subscription(callback)
                .map_err(|err| err.to_string())?;
            let Some(bound) = self
                .com_withevents_subscriptions
                .get(&subscription)
                .cloned()
            else {
                self.pending_callback_tokens.push_back(callback);
                return Ok(());
            };
            let callback_arity = self
                .host_services
                .com()
                .event_callback_arity(callback)
                .map_err(|err| err.to_string())?;
            let mut args = vec![Variant::from_object_ref(bound.owner_object.clone())];
            let target_symbol = match callback_arity {
                0 => bound.route.handler_symbol.clone(),
                1 => {
                    let arg0 = self
                        .host_services
                        .com()
                        .event_callback_variant(callback, 0)
                        .map_err(|err| err.to_string())?;
                    args.push(arg0);
                    bound.route.handler_symbol.clone()
                }
                _ => {
                    let _ = self
                        .host_services
                        .com()
                        .release_event_callback_variant(callback);
                    return Err(format!(
                        "project COM WithEvents handler `{}` supports at most 1 callback argument, got {}",
                        bound.route.event_name, callback_arity
                    ));
                }
            };
            let invoke_result = self.invoke_project_symbol_inline_with_variants(
                bytecode,
                &target_symbol,
                &args,
                descriptor_selected_fastpaths,
            );
            let release_result = self
                .host_services
                .com()
                .release_event_callback_variant(callback)
                .map_err(|err| err.to_string());
            invoke_result?;
            release_result?;
        }
    }

    fn fast_read_slot(&self, slot: usize) -> Option<i32> {
        if self.current_frame_contains_slot(slot) {
            return None;
        }
        self.registers.registers.get(slot)?.as_i32_lossy()
    }

    fn fast_add_const(&mut self, slot: usize, value: i32) -> bool {
        if self.current_frame_contains_slot(slot) {
            return false;
        }
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        // Null propagation: Null + anything = Null.
        if dst.is_null() {
            return true;
        }
        let Some(current) = dst.as_i32_lossy() else {
            return false;
        };
        // Bail to the promoting slow path on i32 overflow so the result matches
        // VBA arithmetic (Long overflow promotes to Double) instead of
        // wrapping/panicking on the raw i32 add.
        let Some(sum) = current.checked_add(value) else {
            return false;
        };
        *dst = RuntimeSlot::Variant(Variant::from_i32(sum));
        true
    }

    fn fast_sub_const(&mut self, slot: usize, value: i32) -> bool {
        if self.current_frame_contains_slot(slot) {
            return false;
        }
        let Some(dst) = self.registers.registers.get_mut(slot) else {
            return false;
        };
        // Null propagation: Null - anything = Null.
        if dst.is_null() {
            return true;
        }
        let Some(current) = dst.as_i32_lossy() else {
            return false;
        };
        // Bail to the promoting slow path on i32 overflow (see fast_add_const).
        let Some(diff) = current.checked_sub(value) else {
            return false;
        };
        *dst = RuntimeSlot::Variant(Variant::from_i32(diff));
        true
    }

    fn fast_copy_slot(&mut self, dst: usize, src: usize) -> bool {
        if self.current_frame_contains_slot(dst) || self.current_frame_contains_slot(src) {
            return false;
        }
        let Some(value) = self.registers.registers.get(src).cloned() else {
            return false;
        };
        let Some(dst) = self.registers.registers.get_mut(dst) else {
            return false;
        };
        *dst = value;
        true
    }

    fn fast_cmp_slots<F>(&mut self, dst: usize, lhs: usize, rhs: usize, pred: F) -> bool
    where
        F: FnOnce(i32, i32) -> bool,
    {
        if self.current_frame_contains_slot(dst)
            || self.current_frame_contains_slot(lhs)
            || self.current_frame_contains_slot(rhs)
        {
            return false;
        }
        // Null comparisons yield Null (falsy), and object comparisons are identity (IUnknown
        // pointer) comparisons — both bail to the slow path. The i32 fast path must not read an
        // object's `raw()` (it is the per-instance handle for our objects and undefined for real
        // COM pointers); object `Is` is handled by pointer equality in `typed_compare_variants`.
        if let (Some(l), Some(r)) = (
            self.registers.registers.get(lhs),
            self.registers.registers.get(rhs),
        ) && (l.is_null() || r.is_null() || l.is_object() || r.is_object())
        {
            return false; // bail to the general comparison handler
        }
        let (Some(lhs), Some(rhs)) = (self.fast_read_slot(lhs), self.fast_read_slot(rhs)) else {
            return false;
        };
        self.write_variant_slot(dst, Variant::from_bool(pred(lhs, rhs)))
            .is_ok()
    }

    fn next_pc_for_jump(target_pc: usize, instruction_len: usize) -> Result<usize, String> {
        if target_pc > instruction_len {
            return Err(format!("jump target out of range: {target_pc}"));
        }
        Ok(target_pc)
    }

    fn next_pc_for_jump_if_zero(
        cond: i32,
        target_pc: usize,
        instruction_len: usize,
        current_pc: usize,
    ) -> Result<usize, String> {
        if cond == 0 {
            Self::next_pc_for_jump(target_pc, instruction_len)
        } else {
            Ok(current_pc + 1)
        }
    }

    fn next_pc_for_jump_if_zero_variant(
        cond: &Variant,
        target_pc: usize,
        instruction_len: usize,
        current_pc: usize,
    ) -> Result<usize, String> {
        let cond = crate::semantics::variant_truthy_value(cond)?;
        Self::next_pc_for_jump_if_zero(
            if cond { -1 } else { 0 },
            target_pc,
            instruction_len,
            current_pc,
        )
    }

    fn normalize_for_compare(text: String, mode: StringCompareMode) -> String {
        crate::semantics::normalize_for_compare(text, mode)
    }

    fn fv_i32(rate: i32, nper: i32, pmt: i32, pv: i32, due: i32) -> i32 {
        if nper == 0 {
            return 0;
        }
        if rate == 0 {
            return -(pv + pmt.saturating_mul(nper));
        }
        let r = rate as f64 / 100.0;
        let n = nper as f64;
        let growth = (1.0 + r).powf(n);
        let due_adj = if due != 0 { 1.0 + r } else { 1.0 };
        let out = -(pv as f64 * growth + pmt as f64 * due_adj * ((growth - 1.0) / r));
        out.round() as i32
    }

    fn pv_i32(rate: i32, nper: i32, pmt: i32, fv: i32, due: i32) -> i32 {
        if nper == 0 {
            return 0;
        }
        if rate == 0 {
            return -(fv + pmt.saturating_mul(nper));
        }
        let r = rate as f64 / 100.0;
        let n = nper as f64;
        let growth = (1.0 + r).powf(n);
        let due_adj = if due != 0 { 1.0 + r } else { 1.0 };
        let out = -(fv as f64 + pmt as f64 * due_adj * ((growth - 1.0) / r)) / growth;
        out.round() as i32
    }

    fn pmt_i32(rate: i32, nper: i32, pv: i32, fv: i32, due: i32) -> i32 {
        if nper == 0 {
            return 0;
        }
        if rate == 0 {
            return -((pv + fv) / nper);
        }
        let r = rate as f64 / 100.0;
        let n = nper as f64;
        let growth = (1.0 + r).powf(n);
        let due_adj = if due != 0 { 1.0 + r } else { 1.0 };
        let denom = due_adj * ((growth - 1.0) / r);
        if denom == 0.0 {
            return 0;
        }
        let out = -(pv as f64 * growth + fv as f64) / denom;
        out.round() as i32
    }

    fn npv_i32(rate: i32, values: &[i32]) -> i32 {
        if values.is_empty() {
            return 0;
        }
        let r = rate as f64 / 100.0;
        let mut total = 0.0f64;
        for (idx, value) in values.iter().enumerate() {
            let period = (idx + 1) as i32;
            let discount = (1.0 + r).powi(period);
            if discount == 0.0 {
                continue;
            }
            total += *value as f64 / discount;
        }
        total.round() as i32
    }

    fn irr_i32(value: i32, guess: i32) -> i32 {
        let mut r = guess as f64 / 100.0;
        let value = value as f64;
        for _ in 0..20 {
            let denom = 1.0 + r;
            if denom.abs() < 1e-9 {
                break;
            }
            let f = -100.0 + (value / denom);
            let fp = -value / (denom * denom);
            if fp.abs() < 1e-12 {
                break;
            }
            let next = (r - f / fp).clamp(-0.99, 10.0);
            if (next - r).abs() < 1e-10 {
                r = next;
                break;
            }
            r = next;
        }
        (r * 100.0).round() as i32
    }

    fn mirr_i32(value: i32, finance_rate: i32, reinvest_rate: i32) -> i32 {
        let value = value as f64;
        let fr = finance_rate as f64 / 100.0;
        let rr = reinvest_rate as f64 / 100.0;
        let pv_neg = 100.0 / (1.0 + fr).max(1e-9);
        let fv_pos = value * (1.0 + rr);
        let out = (fv_pos / pv_neg) - 1.0;
        (out * 100.0).round() as i32
    }

    fn rate_func(r: f64, nper: f64, pmt: f64, pv: f64, fv: f64, due: f64) -> f64 {
        if r.abs() < 1e-9 {
            pv + pmt * nper + fv
        } else {
            let growth = (1.0 + r).powf(nper);
            pv * growth + pmt * (1.0 + r * due) * ((growth - 1.0) / r) + fv
        }
    }

    fn rate_func_derivative(r: f64, nper: f64, pmt: f64, pv: f64, fv: f64, due: f64) -> f64 {
        if r.abs() < 1e-8 {
            let h = FIN_DERIVATIVE_STEP;
            return (Self::rate_func(r + h, nper, pmt, pv, fv, due)
                - Self::rate_func(r - h, nper, pmt, pv, fv, due))
                / (2.0 * h);
        }

        let base = 1.0 + r;
        if base <= 0.0 {
            return f64::NAN;
        }
        let growth = base.powf(nper);
        let growth_prime = nper * base.powf(nper - 1.0);
        let c = (growth - 1.0) / r;
        let c_prime = (growth_prime * r - (growth - 1.0)) / (r * r);
        pv * growth_prime + pmt * (due * c + (1.0 + r * due) * c_prime)
    }

    fn rate_i32(nper: i32, pmt: i32, pv: i32, fv: i32, due: i32, guess: i32) -> Result<i32, i32> {
        if nper == 0 {
            return Err(FIN_RATE_ERROR_CODE);
        }
        let n = nper as f64;
        let pmt = pmt as f64;
        let pv = pv as f64;
        let fv = fv as f64;
        let due = if due != 0 { 1.0 } else { 0.0 };

        let mut r = (guess as f64 / 100.0).clamp(-0.99, 10.0);
        for _ in 0..FIN_MAX_ITERS {
            let f = Self::rate_func(r, n, pmt, pv, fv, due);
            let fp = Self::rate_func_derivative(r, n, pmt, pv, fv, due);
            if fp.abs() < 1e-12 {
                return Err(FIN_RATE_ERROR_CODE);
            }
            let next = (r - f / fp).clamp(-0.99, 10.0);
            if !next.is_finite() {
                return Err(FIN_RATE_ERROR_CODE);
            }
            if (next - r).abs() < FIN_EPS {
                r = next;
                return Ok((r * 100.0).round() as i32);
            }
            r = next;
        }
        Err(FIN_RATE_ERROR_CODE)
    }

    fn nper_i32(rate: i32, pmt: i32, pv: i32, fv: i32, due: i32) -> Result<i32, i32> {
        let pmt = pmt as f64;
        let pv = pv as f64;
        let fv = fv as f64;
        let due = if due != 0 { 1.0 } else { 0.0 };

        if rate == 0 {
            if pmt == 0.0 {
                return Err(FIN_NPER_ERROR_CODE);
            }
            return Ok((-(pv + fv) / pmt).round() as i32);
        }

        let r = rate as f64 / 100.0;
        let numerator = pmt * (1.0 + r * due) - fv * r;
        let denominator = pv * r + pmt * (1.0 + r * due);
        if numerator <= 0.0 || denominator <= 0.0 || (1.0 + r) <= 0.0 {
            return Err(FIN_NPER_ERROR_CODE);
        }

        let n = (numerator / denominator).ln() / (1.0 + r).ln();
        if !n.is_finite() {
            return Err(FIN_NPER_ERROR_CODE);
        }
        Ok(n.round() as i32)
    }
}

fn runtime_resized_array(
    lower_bounds: &[i32],
    upper_bounds: &[i32],
    element_type: RuntimeArrayElementType,
) -> Result<SafeArray, String> {
    if lower_bounds.is_empty() || lower_bounds.len() != upper_bounds.len() {
        return Err("runtime ReDim requires at least one dimension".to_string());
    }
    let mut len = 1usize;
    let mut bounds = Vec::with_capacity(lower_bounds.len());
    for (&lower_bound, &upper_bound) in lower_bounds.iter().zip(upper_bounds.iter()) {
        if upper_bound < lower_bound {
            return Err(format!(
                "runtime ReDim upper bound {upper_bound} is below lower bound {lower_bound}"
            ));
        }
        let count = i64::from(upper_bound) - i64::from(lower_bound) + 1;
        let width = usize::try_from(count)
            .map_err(|_| format!("runtime ReDim bound span {count} cannot fit in host memory"))?;
        len = len
            .checked_mul(width)
            .ok_or_else(|| "runtime ReDim total array length overflowed".to_string())?;
        bounds.push(SafeArrayBound {
            lower: lower_bound,
            count: u32::try_from(width)
                .map_err(|_| format!("runtime ReDim length {width} exceeds SAFEARRAY capacity"))?,
        });
    }
    let default = runtime_array_default_variant(element_type);
    let values = vec![default; len];
    SafeArray::from_typed_variants_nd(bounds, runtime_array_element_vartype(element_type), values)
}

fn runtime_resized_array_preserve(
    current: &Variant,
    lower_bounds: &[i32],
    upper_bounds: &[i32],
    element_type: RuntimeArrayElementType,
) -> Result<SafeArray, String> {
    let previous = current.as_safearray().ok_or_else(|| {
        "runtime ReDim Preserve requires an existing runtime array value".to_string()
    })?;
    if previous.dimensions() as usize != lower_bounds.len()
        || lower_bounds.len() != upper_bounds.len()
    {
        return Err(
            "runtime ReDim Preserve requires the existing and resized array to have the same rank"
                .to_string(),
        );
    }
    let previous_bounds_binding = previous.bounds();
    let previous_bounds = previous_bounds_binding
        .as_ref()
        .ok_or_else(|| "runtime ReDim Preserve requires bounds metadata".to_string())?;
    let previous_values_binding = previous.variant_elements();
    let previous_values = previous_values_binding
        .as_ref()
        .ok_or_else(|| "runtime ReDim Preserve requires an owned array payload".to_string())?;
    let resized = runtime_resized_array(lower_bounds, upper_bounds, element_type)?;
    let resized_bounds = resized
        .bounds()
        .as_ref()
        .ok_or_else(|| "runtime ReDim Preserve failed to materialize bounds metadata".to_string())?
        .clone();
    let mut resized_values = resized.variant_elements().ok_or_else(|| {
        "runtime ReDim Preserve failed to materialize an owned array payload".to_string()
    })?;
    for dim in 0..previous_bounds.len() {
        let previous_bound = &previous_bounds[dim];
        let resized_bound = &resized_bounds[dim];
        if previous_bound.lower != resized_bound.lower {
            return Err(
                "runtime ReDim Preserve requires lower bounds to remain unchanged".to_string(),
            );
        }
        if dim + 1 != previous_bounds.len() && previous_bound.count != resized_bound.count {
            return Err("runtime ReDim Preserve only supports resizing the upper bound of the last dimension".to_string());
        }
    }
    let last = previous_bounds.len() - 1;
    let previous_last = previous_bounds[last].count as usize;
    let resized_last = resized_bounds[last].count as usize;
    let overlap = previous_last.min(resized_last);
    let mut block_count = 1usize;
    for bound in &previous_bounds[..last] {
        block_count = block_count
            .checked_mul(bound.count as usize)
            .ok_or_else(|| "runtime ReDim Preserve block count overflowed".to_string())?;
    }
    for block in 0..block_count.max(1) {
        let previous_start = block
            .checked_mul(previous_last)
            .ok_or_else(|| "runtime ReDim Preserve previous block offset overflowed".to_string())?;
        let resized_start = block
            .checked_mul(resized_last)
            .ok_or_else(|| "runtime ReDim Preserve resized block offset overflowed".to_string())?;
        // Keep this slice-based for V170 evidence: text[start..end] / text[start..].
        resized_values[resized_start..(resized_start + overlap)]
            .clone_from_slice(&previous_values[previous_start..(previous_start + overlap)]);
    }
    resized.replace_variant_elements(resized_values)
}

fn runtime_array_default_variant(element_type: RuntimeArrayElementType) -> Variant {
    match element_type {
        RuntimeArrayElementType::Variant => Variant::empty(),
        RuntimeArrayElementType::Integer => Variant::from_i16(0),
        RuntimeArrayElementType::Long => Variant::from_i32(0),
        RuntimeArrayElementType::Byte => Variant::from_u8(0),
        RuntimeArrayElementType::LongLong | RuntimeArrayElementType::LongPtr => {
            Variant::from_i64(0)
        }
        RuntimeArrayElementType::Single => Variant::from_f32(0.0),
        RuntimeArrayElementType::Double => Variant::from_f64(0.0),
        RuntimeArrayElementType::Currency => Variant::from_currency_scaled_i64(0),
        RuntimeArrayElementType::Date => Variant::from_date_f64(0.0),
        RuntimeArrayElementType::String => Variant::from_string(BStr::empty()),
        RuntimeArrayElementType::Boolean => Variant::from_bool(false),
    }
}

fn runtime_array_element_vartype(element_type: RuntimeArrayElementType) -> u16 {
    match element_type {
        RuntimeArrayElementType::Variant => VT_VARIANT_VALUE,
        RuntimeArrayElementType::Integer => VT_I2_VALUE,
        RuntimeArrayElementType::Long => VT_I4_VALUE,
        RuntimeArrayElementType::LongLong | RuntimeArrayElementType::LongPtr => VT_I8_VALUE,
        RuntimeArrayElementType::Byte => VT_UI1_VALUE,
        RuntimeArrayElementType::Single => VT_R4_VALUE,
        RuntimeArrayElementType::Double => VT_R8_VALUE,
        RuntimeArrayElementType::Currency => VT_CY_VALUE,
        RuntimeArrayElementType::Date => VT_DATE_VALUE,
        RuntimeArrayElementType::String => VT_BSTR_VALUE,
        RuntimeArrayElementType::Boolean => VT_BOOL_VALUE,
    }
}
