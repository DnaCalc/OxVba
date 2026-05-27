//! OxBundle: portable compiled bytecode container.
//!
//! Bundles a compiled `Bytecode` together with `ProcedureRuntimeMetadata`
//! into a single serializable unit that can be persisted to disk (.oxb files)
//! and later deserialized for execution.

use std::collections::BTreeMap;

use rkyv::{Archive, Deserialize, Serialize};

use crate::bytecode::Bytecode;
use crate::emit::{
    ArrayShapeDescriptor, CallSiteDescriptor, OptionalDefaultValue, OptionalMissingStatePolicy,
    OptionalParameterDescriptor, ParamArrayDescriptor, ParameterDescriptor, ParameterPassingMode,
    ParameterRole, ProcedureKindDescriptor, ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind,
    ProcedureRuntimeSlotMetadata, ProcedureSignatureDescriptor, ResolvedParameterMechanism,
    SourceParameterMechanism, VbaTypeId, legacy_procedure_signature_descriptor,
};
use crate::project::{
    CallableCapability, CallingShape, HostProcedureExport, InvocationLane, ModuleDescriptor,
    ModuleKind, ModuleVisibility, PassingMode, ProcedureAnnotation, ProcedureDescriptor,
    ProcedureKind, ProcedureParameterDescriptor, ProcedureSignature, ProcedureVisibility,
    ProjectComWithEventsRoute, ProjectDynamicMemberKind, ProjectDynamicObjectRoute,
    ProjectEventDispatchBinding, ProjectIdentity, ProjectReflection, RuntimeProcedureRoute,
    VbaType, VbaTypeDescriptor,
};

/// Magic header bytes for the OxBundle binary format.
const MAGIC: [u8; 4] = *b"OXVB";
/// Current bundle format version.
const FORMAT_VERSION: u32 = 9;
/// Header size in bytes (padded to 16 for rkyv alignment).
const HEADER_SIZE: usize = 16;

/// Snapshot of project manifest at compile time.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ManifestSnapshot {
    pub project_name: String,
    pub project_kind: String,
    pub module_names: Vec<String>,
    pub reference_names: Vec<String>,
}

/// Export inventory for the bundle.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ExportInventory {
    pub host_exports: Vec<HostProcedureExport>,
    pub com_class_exports: Vec<ComClassExportEntry>,
}

/// Descriptor inventory consumed by generated wrappers and hosts.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct DescriptorInventory {
    pub com_classes: Vec<BundleComClassDescriptor>,
    pub com_events: Vec<BundleComEventDescriptor>,
    pub callables: Vec<BundleCallableDescriptor>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleComClassDescriptor {
    pub stable_class_id: String,
    pub project_name: String,
    pub module_name: String,
    pub class_name: String,
    pub object_handle: i32,
    pub prog_id: Option<String>,
    pub instancing: Option<String>,
    pub clsid: Option<String>,
    pub description: Option<String>,
    pub interfaces: Vec<BundleComInterfaceDescriptor>,
    pub members: Vec<BundleComMemberDescriptor>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleComInterfaceDescriptor {
    pub stable_interface_id: String,
    pub name: String,
    pub kind: String,
    pub source_interface: bool,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleComMemberDescriptor {
    pub stable_member_id: String,
    pub member_name: String,
    pub lowered_name: String,
    pub kind: ProjectDynamicMemberKind,
    pub dispatch_id: Option<i32>,
    pub member_flags: Option<u32>,
    pub is_default_member: bool,
    pub visible_param_count: usize,
    pub params: Vec<BundleComParamDescriptor>,
    pub param_types: Vec<crate::bytecode::DeclareParamType>,
    pub return_type: Option<crate::bytecode::DeclareParamType>,
    pub entry_pc: usize,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleComParamDescriptor {
    pub name: String,
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<i32>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleComEventDescriptor {
    pub stable_event_id: String,
    pub source_project_name: String,
    pub source_module_name: String,
    pub event_name: String,
    pub event_token: Option<i32>,
    pub binding_token: Option<i32>,
    pub prog_id_name: Option<String>,
    pub handler_symbol: String,
    pub guard_symbol_zero_arg: String,
    pub guard_symbol_one_arg: String,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleCallableDescriptor {
    pub callable_id: String,
    pub project_id: String,
    pub module_id: String,
    pub module_name: String,
    pub procedure_name: String,
    pub kind: String,
    pub is_public: bool,
    pub is_option_private: bool,
    pub is_class_member: bool,
    pub signature: BundleProcedureSignature,
    pub entry_pc: Option<usize>,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
    pub descriptor_fingerprint: String,
    pub annotations: Vec<BundleProcedureAnnotation>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleProcedureSignature {
    pub parameters: Vec<BundleProcedureParameterDescriptor>,
    pub return_type: Option<BundleVbaTypeDescriptor>,
    pub calling_shape: String,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleProcedureParameterDescriptor {
    pub name: Option<String>,
    pub passing_mode: String,
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<i32>,
    pub value_type: Option<BundleVbaTypeDescriptor>,
    pub source_type_text: Option<String>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleVbaTypeDescriptor {
    pub normalized: String,
    pub source_text: Option<String>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BundleProcedureAnnotation {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleDescriptorInventoryError {
    Unavailable,
}

/// Toolchain fingerprint for cache invalidation.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ToolchainFingerprint {
    pub oxvba_version: String,
    pub build_profile: String,
}

/// COM class export entry.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct ComClassExportEntry {
    pub class_name: String,
    pub prog_id: Option<String>,
    pub instancing: Option<String>,
    pub clsid: Option<String>,
    pub description: Option<String>,
}

/// Compiled bytecode bundle — the unit of persistence.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct OxBundle {
    /// Compiled bytecode (instructions, slot counts, external call descriptors).
    pub bytecode: Bytecode,
    /// Per-procedure metadata (entry points, parameter slots, return slots).
    pub procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    /// Optional fields added across bundle format revisions.
    pub manifest_snapshot: Option<ManifestSnapshot>,
    pub export_inventory: Option<ExportInventory>,
    pub source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    pub toolchain_fingerprint: Option<ToolchainFingerprint>,
    pub event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    pub com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    pub dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    pub descriptor_inventory: Option<DescriptorInventory>,
}

/// v1 bundle layout for backward-compatible deserialization.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV1 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadata>,
}

/// v2 bundle layout for backward-compatible deserialization.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV2 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadata>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
}

/// v3 bundle layout for backward-compatible deserialization.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV3 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadata>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v4 bundle layout before procedure signature descriptors were added to
/// `ProcedureRuntimeMetadata`.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV4 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV4>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v5 bundle layout before call-relevant signature descriptor fields were
/// added to `ProcedureSignatureDescriptor` and `ParameterDescriptor`.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV5 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV5>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v6 bundle layout before procedure call-site descriptors were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV6 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV6>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v7 bundle layout before procedure array-shape descriptors were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV7 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV7>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v8 bundle layout before procedure UDT descriptors were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV8 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV8>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeSlotMetadata {
    name: String,
    slot: usize,
    kind: ProcedureRuntimeSlotKind,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadata {
    module_name: String,
    procedure_name: String,
    entry_pc: usize,
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    statement_entry_pcs: Vec<usize>,
    slots: Vec<LegacyProcedureRuntimeSlotMetadata>,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
    param_types: Vec<crate::bytecode::DeclareParamType>,
    return_type: Option<crate::bytecode::DeclareParamType>,
}

impl From<LegacyProcedureRuntimeMetadata> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadata) -> Self {
        let module_name = legacy.module_name;
        let procedure_name = legacy.procedure_name;
        let entry_pc = legacy.entry_pc;
        let source_line_start = legacy.source_line_start;
        let source_line_end = legacy.source_line_end;
        let statement_line_numbers = legacy.statement_line_numbers;
        let statement_entry_pcs = legacy.statement_entry_pcs;
        let param_slots = legacy.param_slots;
        let return_slot = legacy.return_slot;
        let param_types = legacy.param_types;
        let return_type = legacy.return_type;
        let legacy_type_metadata = ProcedureRuntimeMetadata {
            module_name: module_name.clone(),
            procedure_name: procedure_name.clone(),
            entry_pc,
            source_line_start,
            source_line_end,
            statement_line_numbers: statement_line_numbers.clone(),
            statement_entry_pcs: statement_entry_pcs.clone(),
            slots: Vec::new(),
            param_slots: param_slots.clone(),
            return_slot,
            param_types: param_types.clone(),
            return_type,
            signature: legacy_procedure_signature_descriptor(
                &procedure_name,
                &param_slots,
                &param_types,
                return_slot,
                return_type,
                &[],
            ),
            call_sites: Vec::new(),
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
        };
        let slots = legacy
            .slots
            .into_iter()
            .map(|slot| {
                let declared_type =
                    legacy_type_metadata.legacy_declared_type_for_slot(slot.slot, slot.kind);
                ProcedureRuntimeSlotMetadata::new(slot.name, slot.slot, slot.kind, declared_type)
            })
            .collect::<Vec<_>>();
        let signature = legacy_procedure_signature_descriptor(
            &procedure_name,
            &param_slots,
            &param_types,
            return_slot,
            return_type,
            &slots,
        );
        ProcedureRuntimeMetadata {
            module_name,
            procedure_name,
            entry_pc,
            source_line_start,
            source_line_end,
            statement_line_numbers,
            statement_entry_pcs,
            slots,
            param_slots,
            return_slot,
            param_types,
            return_type,
            signature,
            call_sites: Vec::new(),
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV4 {
    module_name: String,
    procedure_name: String,
    entry_pc: usize,
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    statement_entry_pcs: Vec<usize>,
    slots: Vec<ProcedureRuntimeSlotMetadata>,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
    param_types: Vec<crate::bytecode::DeclareParamType>,
    return_type: Option<crate::bytecode::DeclareParamType>,
}

impl From<LegacyProcedureRuntimeMetadataV4> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV4) -> Self {
        let signature = legacy_procedure_signature_descriptor(
            &legacy.procedure_name,
            &legacy.param_slots,
            &legacy.param_types,
            legacy.return_slot,
            legacy.return_type,
            &legacy.slots,
        );
        ProcedureRuntimeMetadata {
            module_name: legacy.module_name,
            procedure_name: legacy.procedure_name,
            entry_pc: legacy.entry_pc,
            source_line_start: legacy.source_line_start,
            source_line_end: legacy.source_line_end,
            statement_line_numbers: legacy.statement_line_numbers,
            statement_entry_pcs: legacy.statement_entry_pcs,
            slots: legacy.slots,
            param_slots: legacy.param_slots,
            return_slot: legacy.return_slot,
            param_types: legacy.param_types,
            return_type: legacy.return_type,
            signature,
            call_sites: Vec::new(),
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV5 {
    module_name: String,
    procedure_name: String,
    entry_pc: usize,
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    statement_entry_pcs: Vec<usize>,
    slots: Vec<ProcedureRuntimeSlotMetadata>,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
    param_types: Vec<crate::bytecode::DeclareParamType>,
    return_type: Option<crate::bytecode::DeclareParamType>,
    signature: LegacyProcedureSignatureDescriptorV5,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureSignatureDescriptorV5 {
    procedure_name: String,
    kind: ProcedureKindDescriptor,
    parameters: Vec<LegacyParameterDescriptorV5>,
    return_type: Option<VbaTypeId>,
    return_slot: Option<usize>,
    property_group: Option<String>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyParameterDescriptorV5 {
    index: usize,
    name: String,
    slot: Option<usize>,
    role: ParameterRole,
    passing_mode: ParameterPassingMode,
    declared_type: VbaTypeId,
    optional: bool,
    param_array: bool,
    default_value: Option<i32>,
}

impl From<LegacyProcedureRuntimeMetadataV5> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV5) -> Self {
        ProcedureRuntimeMetadata {
            module_name: legacy.module_name,
            procedure_name: legacy.procedure_name,
            entry_pc: legacy.entry_pc,
            source_line_start: legacy.source_line_start,
            source_line_end: legacy.source_line_end,
            statement_line_numbers: legacy.statement_line_numbers,
            statement_entry_pcs: legacy.statement_entry_pcs,
            slots: legacy.slots,
            param_slots: legacy.param_slots,
            return_slot: legacy.return_slot,
            param_types: legacy.param_types,
            return_type: legacy.return_type,
            signature: upgrade_v5_signature_descriptor(legacy.signature),
            call_sites: Vec::new(),
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV6 {
    module_name: String,
    procedure_name: String,
    entry_pc: usize,
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    statement_entry_pcs: Vec<usize>,
    slots: Vec<ProcedureRuntimeSlotMetadata>,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
    param_types: Vec<crate::bytecode::DeclareParamType>,
    return_type: Option<crate::bytecode::DeclareParamType>,
    signature: ProcedureSignatureDescriptor,
}

impl From<LegacyProcedureRuntimeMetadataV6> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV6) -> Self {
        ProcedureRuntimeMetadata {
            module_name: legacy.module_name,
            procedure_name: legacy.procedure_name,
            entry_pc: legacy.entry_pc,
            source_line_start: legacy.source_line_start,
            source_line_end: legacy.source_line_end,
            statement_line_numbers: legacy.statement_line_numbers,
            statement_entry_pcs: legacy.statement_entry_pcs,
            slots: legacy.slots,
            param_slots: legacy.param_slots,
            return_slot: legacy.return_slot,
            param_types: legacy.param_types,
            return_type: legacy.return_type,
            signature: legacy.signature,
            call_sites: Vec::new(),
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV7 {
    module_name: String,
    procedure_name: String,
    entry_pc: usize,
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    statement_entry_pcs: Vec<usize>,
    slots: Vec<ProcedureRuntimeSlotMetadata>,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
    param_types: Vec<crate::bytecode::DeclareParamType>,
    return_type: Option<crate::bytecode::DeclareParamType>,
    signature: ProcedureSignatureDescriptor,
    call_sites: Vec<CallSiteDescriptor>,
}

impl From<LegacyProcedureRuntimeMetadataV7> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV7) -> Self {
        ProcedureRuntimeMetadata {
            module_name: legacy.module_name,
            procedure_name: legacy.procedure_name,
            entry_pc: legacy.entry_pc,
            source_line_start: legacy.source_line_start,
            source_line_end: legacy.source_line_end,
            statement_line_numbers: legacy.statement_line_numbers,
            statement_entry_pcs: legacy.statement_entry_pcs,
            slots: legacy.slots,
            param_slots: legacy.param_slots,
            return_slot: legacy.return_slot,
            param_types: legacy.param_types,
            return_type: legacy.return_type,
            signature: legacy.signature,
            call_sites: legacy.call_sites,
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV8 {
    module_name: String,
    procedure_name: String,
    entry_pc: usize,
    source_line_start: usize,
    source_line_end: usize,
    statement_line_numbers: Vec<usize>,
    statement_entry_pcs: Vec<usize>,
    slots: Vec<ProcedureRuntimeSlotMetadata>,
    param_slots: Vec<usize>,
    return_slot: Option<usize>,
    param_types: Vec<crate::bytecode::DeclareParamType>,
    return_type: Option<crate::bytecode::DeclareParamType>,
    signature: ProcedureSignatureDescriptor,
    call_sites: Vec<CallSiteDescriptor>,
    array_shapes: Vec<ArrayShapeDescriptor>,
}

impl From<LegacyProcedureRuntimeMetadataV8> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV8) -> Self {
        ProcedureRuntimeMetadata {
            module_name: legacy.module_name,
            procedure_name: legacy.procedure_name,
            entry_pc: legacy.entry_pc,
            source_line_start: legacy.source_line_start,
            source_line_end: legacy.source_line_end,
            statement_line_numbers: legacy.statement_line_numbers,
            statement_entry_pcs: legacy.statement_entry_pcs,
            slots: legacy.slots,
            param_slots: legacy.param_slots,
            return_slot: legacy.return_slot,
            param_types: legacy.param_types,
            return_type: legacy.return_type,
            signature: legacy.signature,
            call_sites: legacy.call_sites,
            array_shapes: legacy.array_shapes,
            udt_types: Vec::new(),
        }
    }
}

fn upgrade_v5_signature_descriptor(
    legacy: LegacyProcedureSignatureDescriptorV5,
) -> ProcedureSignatureDescriptor {
    let kind = legacy.kind;
    let parameters = legacy
        .parameters
        .into_iter()
        .map(|param| upgrade_v5_parameter_descriptor(kind, param))
        .collect();
    ProcedureSignatureDescriptor {
        procedure_name: legacy.procedure_name,
        kind,
        parameters,
        return_type: legacy.return_type,
        return_slot: legacy.return_slot,
        property_group: legacy.property_group,
        implicit_current_object: None,
    }
}

fn upgrade_v5_parameter_descriptor(
    kind: ProcedureKindDescriptor,
    legacy: LegacyParameterDescriptorV5,
) -> ParameterDescriptor {
    let resolved_mechanism = if matches!(
        (kind, legacy.role),
        (
            ProcedureKindDescriptor::PropertyLet | ProcedureKindDescriptor::PropertySet,
            ParameterRole::PropertyValue
        )
    ) {
        ResolvedParameterMechanism::PropertyValueByVal
    } else {
        match legacy.passing_mode {
            ParameterPassingMode::ByRef => ResolvedParameterMechanism::ByRef,
            ParameterPassingMode::ByVal => ResolvedParameterMechanism::ByVal,
            ParameterPassingMode::Unknown => ResolvedParameterMechanism::Unknown,
        }
    };
    let optional_descriptor = if legacy.optional {
        let (default_value, missing_state) = match legacy.default_value {
            Some(value) => (
                OptionalDefaultValue::ExplicitI32(value),
                OptionalMissingStatePolicy::AssignDefaultLocal,
            ),
            None if legacy.declared_type == VbaTypeId::Variant => (
                OptionalDefaultValue::VariantMissingError448,
                OptionalMissingStatePolicy::PreserveMissingArgumentState,
            ),
            None => (
                OptionalDefaultValue::DeclaredTypeDefault,
                OptionalMissingStatePolicy::AssignDefaultLocal,
            ),
        };
        OptionalParameterDescriptor::Optional {
            default_value,
            missing_state,
        }
    } else {
        OptionalParameterDescriptor::Required
    };
    let param_array_descriptor = legacy.param_array.then_some(ParamArrayDescriptor {
        element_type: VbaTypeId::Variant,
        array_lower_bound: 0,
        empty_upper_bound: -1,
    });
    ParameterDescriptor {
        index: legacy.index,
        name: legacy.name,
        slot: legacy.slot,
        role: legacy.role,
        source_mechanism: SourceParameterMechanism::Unknown,
        resolved_mechanism,
        passing_mode: legacy.passing_mode,
        declared_type: legacy.declared_type,
        optional: legacy.optional,
        param_array: legacy.param_array,
        default_value: legacy.default_value,
        optional_descriptor,
        param_array_descriptor,
    }
}

fn upgrade_legacy_procedure_metadata(
    metadata: BTreeMap<String, LegacyProcedureRuntimeMetadata>,
) -> BTreeMap<String, ProcedureRuntimeMetadata> {
    metadata
        .into_iter()
        .map(|(name, metadata)| (name, metadata.into()))
        .collect()
}

impl OxBundle {
    /// Create a new bundle from a compiled bytecode and its procedure metadata.
    ///
    /// New v2 fields default to `None`.
    pub fn new(
        bytecode: Bytecode,
        procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) -> Self {
        Self {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        }
    }

    pub fn callable_descriptors(
        &self,
    ) -> Result<&[BundleCallableDescriptor], BundleDescriptorInventoryError> {
        self.descriptor_inventory
            .as_ref()
            .map(|inventory| inventory.callables.as_slice())
            .ok_or(BundleDescriptorInventoryError::Unavailable)
    }

    pub fn project_reflection(&self) -> Result<ProjectReflection, BundleDescriptorInventoryError> {
        let callables = self.callable_descriptors()?;
        let project_name = self
            .manifest_snapshot
            .as_ref()
            .map(|snapshot| snapshot.project_name.clone())
            .or_else(|| {
                callables
                    .first()
                    .map(|callable| callable.project_id.clone())
            })
            .unwrap_or_default();
        let project_id = callables
            .first()
            .map(|callable| callable.project_id.clone())
            .unwrap_or_else(|| project_name.to_ascii_lowercase());
        let mut modules = BTreeMap::<String, ModuleDescriptor>::new();
        let mut procedures = Vec::new();
        let mut capabilities = Vec::new();
        for callable in callables {
            modules
                .entry(callable.module_id.clone())
                .or_insert_with(|| ModuleDescriptor {
                    module_id: callable.module_id.clone(),
                    project_id: callable.project_id.clone(),
                    name: callable.module_name.clone(),
                    kind: if callable.is_class_member {
                        ModuleKind::Class
                    } else {
                        ModuleKind::Procedural
                    },
                    visibility: ModuleVisibility {
                        option_private_module: callable.is_option_private,
                        vb_exposed: false,
                        vb_creatable: false,
                    },
                    source_fingerprint: String::new(),
                    source_span: None,
                });
            procedures.push(procedure_descriptor_from_bundle_callable(callable));
            capabilities.push(CallableCapability {
                callable_id: callable.callable_id.clone(),
                invocable_in_prepared_session: callable.entry_pc.is_some(),
                supported_invocation_lanes: vec![InvocationLane::VariantPositional],
                unsupported_reasons: Vec::new(),
            });
        }
        Ok(ProjectReflection {
            identity: ProjectIdentity {
                project_name,
                project_id,
                source_fingerprint: String::new(),
            },
            modules: modules.into_values().collect(),
            procedures,
            capabilities,
        })
    }

    /// Create a bundle from a `CompiledProject`, populating v2 metadata fields.
    pub fn from_compiled_project(
        compiled: &crate::project::CompiledProject,
        project_name: &str,
    ) -> Self {
        let manifest_snapshot = ManifestSnapshot {
            project_name: project_name.to_string(),
            project_kind: "compiled".to_string(),
            module_names: compiled
                .host_exports
                .iter()
                .map(|e| e.module_name.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
            reference_names: Vec::new(),
        };

        let host_exports = compiled.host_exports.clone();
        let export_inventory = ExportInventory {
            host_exports,
            com_class_exports: Vec::new(),
        };

        Self {
            bytecode: compiled.bytecode.clone(),
            procedure_metadata: compiled.procedure_runtime_metadata.clone(),
            manifest_snapshot: Some(manifest_snapshot),
            export_inventory: Some(export_inventory),
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: if compiled.event_dispatch_bindings.is_empty() {
                None
            } else {
                Some(compiled.event_dispatch_bindings.clone())
            },
            com_withevents_routes: if compiled.project_com_withevents_routes.is_empty() {
                None
            } else {
                Some(compiled.project_com_withevents_routes.clone())
            },
            dynamic_object_routes: if compiled.project_dynamic_objects.is_empty() {
                None
            } else {
                Some(compiled.project_dynamic_objects.clone())
            },
            descriptor_inventory: descriptor_inventory_from_compiled_project(compiled),
        }
    }

    /// Serialize the bundle to bytes with a header.
    ///
    /// Wire format (16-byte header, aligned for rkyv):
    /// ```text
    /// [4 bytes: magic "OXVB"]
    /// [4 bytes: format version, little-endian u32]
    /// [4 bytes: payload length, little-endian u32]
    /// [4 bytes: reserved/padding (zeroes)]
    /// [N bytes: rkyv-serialized OxBundle payload]
    /// ```
    pub fn serialize_to_bytes(&self) -> Result<Vec<u8>, String> {
        let payload =
            rkyv::to_bytes::<rkyv::rancor::Error>(self).map_err(|e| format!("serialize: {e}"))?;

        let payload_len =
            u32::try_from(payload.len()).map_err(|_| "bundle payload exceeds 4 GiB".to_string())?;

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved padding
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Deserialize a bundle from bytes produced by `serialize_to_bytes`.
    ///
    /// Accepts legacy format versions and upgrades missing fields.
    pub fn deserialize_from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < HEADER_SIZE {
            return Err("bundle too short for header".to_string());
        }

        // Validate magic.
        if data[0..4] != MAGIC {
            return Err("invalid bundle magic (expected OXVB)".to_string());
        }

        // Read version.
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != 1
            && version != 2
            && version != 3
            && version != 4
            && version != 5
            && version != 6
            && version != 7
            && version != 8
            && version != FORMAT_VERSION
        {
            return Err(format!(
                "unsupported bundle version {version} (expected 1, 2, 3, 4, 5, 6, 7, 8, or {FORMAT_VERSION})"
            ));
        }

        // Read payload length.
        let payload_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        if data.len() < HEADER_SIZE + payload_len {
            return Err(format!(
                "bundle truncated: expected {} payload bytes, got {}",
                payload_len,
                data.len() - HEADER_SIZE
            ));
        }

        let payload = &data[HEADER_SIZE..HEADER_SIZE + payload_len];

        // rkyv requires aligned data. Copy to an aligned buffer (16-byte alignment).
        let mut aligned: rkyv::util::AlignedVec<16> =
            rkyv::util::AlignedVec::with_capacity(payload.len());
        aligned.extend_from_slice(payload);

        if version == 1 {
            // v1 layout: just bytecode + procedure_metadata
            let legacy: LegacyOxBundleV1 =
                rkyv::from_bytes::<LegacyOxBundleV1, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v1: {e}"))?;
            Ok(OxBundle::new(
                legacy.bytecode,
                upgrade_legacy_procedure_metadata(legacy.procedure_metadata),
            ))
        } else if version == 2 {
            let legacy: LegacyOxBundleV2 =
                rkyv::from_bytes::<LegacyOxBundleV2, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v2: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: upgrade_legacy_procedure_metadata(legacy.procedure_metadata),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: None,
            })
        } else if version == 3 {
            let legacy: LegacyOxBundleV3 =
                rkyv::from_bytes::<LegacyOxBundleV3, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v3: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: upgrade_legacy_procedure_metadata(legacy.procedure_metadata),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: legacy.descriptor_inventory,
            })
        } else if version == 4 {
            let legacy: LegacyOxBundleV4 =
                rkyv::from_bytes::<LegacyOxBundleV4, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v4: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: legacy
                    .procedure_metadata
                    .into_iter()
                    .map(|(name, metadata)| (name, metadata.into()))
                    .collect(),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: legacy.descriptor_inventory,
            })
        } else if version == 5 {
            let legacy: LegacyOxBundleV5 =
                rkyv::from_bytes::<LegacyOxBundleV5, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v5: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: legacy
                    .procedure_metadata
                    .into_iter()
                    .map(|(name, metadata)| (name, metadata.into()))
                    .collect(),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: legacy.descriptor_inventory,
            })
        } else if version == 6 {
            let legacy: LegacyOxBundleV6 =
                rkyv::from_bytes::<LegacyOxBundleV6, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v6: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: legacy
                    .procedure_metadata
                    .into_iter()
                    .map(|(name, metadata)| (name, metadata.into()))
                    .collect(),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: legacy.descriptor_inventory,
            })
        } else if version == 7 {
            let legacy: LegacyOxBundleV7 =
                rkyv::from_bytes::<LegacyOxBundleV7, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v7: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: legacy
                    .procedure_metadata
                    .into_iter()
                    .map(|(name, metadata)| (name, metadata.into()))
                    .collect(),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: legacy.descriptor_inventory,
            })
        } else if version == 8 {
            let legacy: LegacyOxBundleV8 =
                rkyv::from_bytes::<LegacyOxBundleV8, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v8: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: legacy
                    .procedure_metadata
                    .into_iter()
                    .map(|(name, metadata)| (name, metadata.into()))
                    .collect(),
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: legacy.descriptor_inventory,
            })
        } else {
            let bundle: OxBundle = rkyv::from_bytes::<OxBundle, rkyv::rancor::Error>(&aligned)
                .map_err(|e| format!("deserialize: {e}"))?;
            Ok(bundle)
        }
    }
}

fn descriptor_inventory_from_compiled_project(
    compiled: &crate::project::CompiledProject,
) -> Option<DescriptorInventory> {
    let com_classes = compiled
        .project_dynamic_objects
        .iter()
        .map(com_class_descriptor_from_route)
        .collect::<Vec<_>>();
    let mut com_events = compiled
        .event_dispatch_bindings
        .iter()
        .map(event_descriptor_from_dispatch_binding)
        .collect::<Vec<_>>();
    com_events.extend(
        compiled
            .project_com_withevents_routes
            .iter()
            .map(event_descriptor_from_withevents_route),
    );
    com_events.sort_by(|lhs, rhs| lhs.stable_event_id.cmp(&rhs.stable_event_id));
    com_events.dedup_by(|lhs, rhs| lhs.stable_event_id == rhs.stable_event_id);

    let callables = compiled
        .project_reflection
        .procedures
        .iter()
        .map(bundle_callable_descriptor_from_procedure)
        .collect::<Vec<_>>();

    if com_classes.is_empty() && com_events.is_empty() && callables.is_empty() {
        None
    } else {
        Some(DescriptorInventory {
            com_classes,
            com_events,
            callables,
        })
    }
}

fn com_class_descriptor_from_route(route: &ProjectDynamicObjectRoute) -> BundleComClassDescriptor {
    let stable_class_id = stable_id(["com-class", &route.project_name, &route.module_name]);
    let default_interface_name = format!("_{}", route.module_name);
    BundleComClassDescriptor {
        stable_class_id: stable_class_id.clone(),
        project_name: route.project_name.clone(),
        module_name: route.module_name.clone(),
        class_name: route.module_name.clone(),
        object_handle: route.object_handle,
        prog_id: None,
        instancing: None,
        clsid: None,
        description: None,
        interfaces: std::iter::once(BundleComInterfaceDescriptor {
            stable_interface_id: stable_id([
                "com-interface",
                &route.project_name,
                &route.module_name,
                &default_interface_name,
            ]),
            name: default_interface_name,
            kind: "dispatch".to_string(),
            source_interface: false,
        })
        .chain(
            route
                .implements_interfaces
                .iter()
                .map(|name| BundleComInterfaceDescriptor {
                    stable_interface_id: stable_id([
                        "com-interface",
                        &route.project_name,
                        &route.module_name,
                        name,
                    ]),
                    name: name.clone(),
                    kind: "implemented".to_string(),
                    source_interface: false,
                }),
        )
        .collect(),
        members: route
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| BundleComMemberDescriptor {
                stable_member_id: stable_id([
                    "com-member",
                    &route.project_name,
                    &route.module_name,
                    &member.member_name,
                    &format!("{:?}", member.kind),
                    &member
                        .dispatch_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| format!("ordinal-{index}")),
                ]),
                member_name: member.member_name.clone(),
                lowered_name: member.lowered_name.clone(),
                kind: member.kind,
                dispatch_id: member.dispatch_id,
                member_flags: member.member_flags,
                is_default_member: member.is_default_member,
                visible_param_count: member.visible_param_count,
                params: member
                    .params
                    .iter()
                    .map(|param| BundleComParamDescriptor {
                        name: param.name.clone(),
                        optional: param.optional,
                        param_array: param.param_array,
                        default_value: param.default_value,
                    })
                    .collect(),
                param_types: member.param_types.clone(),
                return_type: member.return_type,
                entry_pc: member.entry_pc,
                param_slots: member.param_slots.clone(),
                return_slot: member.return_slot,
            })
            .collect(),
    }
}

fn event_descriptor_from_dispatch_binding(
    binding: &ProjectEventDispatchBinding,
) -> BundleComEventDescriptor {
    BundleComEventDescriptor {
        stable_event_id: stable_id([
            "event",
            &binding.source_project_name,
            &binding.source_module_name,
            &binding.event_name,
            &binding.handler_symbol,
        ]),
        source_project_name: binding.source_project_name.clone(),
        source_module_name: binding.source_module_name.clone(),
        event_name: binding.event_name.clone(),
        event_token: None,
        binding_token: None,
        prog_id_name: None,
        handler_symbol: binding.handler_symbol.clone(),
        guard_symbol_zero_arg: binding.guard_symbol_zero_arg.clone(),
        guard_symbol_one_arg: binding.guard_symbol_one_arg.clone(),
    }
}

fn event_descriptor_from_withevents_route(
    route: &ProjectComWithEventsRoute,
) -> BundleComEventDescriptor {
    BundleComEventDescriptor {
        stable_event_id: stable_id([
            "event",
            &route.prog_id_name,
            &route.event_name,
            &route.event_token.to_string(),
            &route.handler_symbol,
        ]),
        source_project_name: String::new(),
        source_module_name: String::new(),
        event_name: route.event_name.clone(),
        event_token: Some(route.event_token),
        binding_token: Some(route.binding_token),
        prog_id_name: Some(route.prog_id_name.clone()),
        handler_symbol: route.handler_symbol.clone(),
        guard_symbol_zero_arg: route.guard_symbol_zero_arg.clone(),
        guard_symbol_one_arg: route.guard_symbol_one_arg.clone(),
    }
}

fn procedure_descriptor_from_bundle_callable(
    callable: &BundleCallableDescriptor,
) -> ProcedureDescriptor {
    ProcedureDescriptor {
        callable_id: callable.callable_id.clone(),
        project_id: callable.project_id.clone(),
        module_id: callable.module_id.clone(),
        module_name: callable.module_name.clone(),
        procedure_name: callable.procedure_name.clone(),
        kind: procedure_kind_from_name(&callable.kind),
        visibility: ProcedureVisibility {
            is_public: callable.is_public,
            is_option_private: callable.is_option_private,
            is_class_member: callable.is_class_member,
        },
        signature: ProcedureSignature {
            parameters: callable
                .signature
                .parameters
                .iter()
                .map(|param| ProcedureParameterDescriptor {
                    name: param.name.clone(),
                    passing_mode: passing_mode_from_name(&param.passing_mode),
                    optional: param.optional,
                    param_array: param.param_array,
                    default_value: param.default_value,
                    value_type: param
                        .value_type
                        .as_ref()
                        .map(vba_type_descriptor_from_bundle),
                    source_type_text: param.source_type_text.clone(),
                })
                .collect(),
            return_type: callable
                .signature
                .return_type
                .as_ref()
                .map(vba_type_descriptor_from_bundle),
            calling_shape: calling_shape_from_name(&callable.signature.calling_shape),
        },
        runtime_route: callable.entry_pc.map(|entry_pc| RuntimeProcedureRoute {
            entry_pc,
            param_slots: callable.param_slots.clone(),
            return_slot: callable.return_slot,
        }),
        source_span: None,
        descriptor_fingerprint: callable.descriptor_fingerprint.clone(),
        annotations: callable
            .annotations
            .iter()
            .map(|annotation| ProcedureAnnotation {
                name: annotation.name.clone(),
                value: annotation.value.clone(),
            })
            .collect(),
    }
}

fn bundle_callable_descriptor_from_procedure(
    procedure: &ProcedureDescriptor,
) -> BundleCallableDescriptor {
    let runtime_route = procedure.runtime_route.as_ref();
    BundleCallableDescriptor {
        callable_id: procedure.callable_id.clone(),
        project_id: procedure.project_id.clone(),
        module_id: procedure.module_id.clone(),
        module_name: procedure.module_name.clone(),
        procedure_name: procedure.procedure_name.clone(),
        kind: procedure_kind_name(procedure.kind).to_string(),
        is_public: procedure.visibility.is_public,
        is_option_private: procedure.visibility.is_option_private,
        is_class_member: procedure.visibility.is_class_member,
        signature: BundleProcedureSignature {
            parameters: procedure
                .signature
                .parameters
                .iter()
                .map(|param| BundleProcedureParameterDescriptor {
                    name: param.name.clone(),
                    passing_mode: passing_mode_name(param.passing_mode).to_string(),
                    optional: param.optional,
                    param_array: param.param_array,
                    default_value: param.default_value,
                    value_type: param.value_type.as_ref().map(bundle_vba_type_descriptor),
                    source_type_text: param.source_type_text.clone(),
                })
                .collect(),
            return_type: procedure
                .signature
                .return_type
                .as_ref()
                .map(bundle_vba_type_descriptor),
            calling_shape: calling_shape_name(procedure.signature.calling_shape).to_string(),
        },
        entry_pc: runtime_route.map(|route| route.entry_pc),
        param_slots: runtime_route
            .map(|route| route.param_slots.clone())
            .unwrap_or_default(),
        return_slot: runtime_route.and_then(|route| route.return_slot),
        descriptor_fingerprint: procedure.descriptor_fingerprint.clone(),
        annotations: procedure
            .annotations
            .iter()
            .map(|annotation| BundleProcedureAnnotation {
                name: annotation.name.clone(),
                value: annotation.value.clone(),
            })
            .collect(),
    }
}

fn vba_type_descriptor_from_bundle(ty: &BundleVbaTypeDescriptor) -> VbaTypeDescriptor {
    VbaTypeDescriptor {
        normalized: vba_type_from_name(&ty.normalized),
        source_text: ty.source_text.clone(),
    }
}

fn bundle_vba_type_descriptor(ty: &crate::project::VbaTypeDescriptor) -> BundleVbaTypeDescriptor {
    BundleVbaTypeDescriptor {
        normalized: vba_type_name(&ty.normalized).to_string(),
        source_text: ty.source_text.clone(),
    }
}

fn procedure_kind_from_name(name: &str) -> ProcedureKind {
    match name {
        "Sub" => ProcedureKind::Sub,
        "Function" => ProcedureKind::Function,
        "PropertyGet" => ProcedureKind::PropertyGet,
        "PropertyLet" => ProcedureKind::PropertyLet,
        "PropertySet" => ProcedureKind::PropertySet,
        "Event" => ProcedureKind::Event,
        _ => ProcedureKind::Sub,
    }
}

fn procedure_kind_name(kind: ProcedureKind) -> &'static str {
    match kind {
        ProcedureKind::Sub => "Sub",
        ProcedureKind::Function => "Function",
        ProcedureKind::PropertyGet => "PropertyGet",
        ProcedureKind::PropertyLet => "PropertyLet",
        ProcedureKind::PropertySet => "PropertySet",
        ProcedureKind::Event => "Event",
    }
}

fn calling_shape_from_name(name: &str) -> CallingShape {
    match name {
        "PropertyAccessor" => CallingShape::PropertyAccessor,
        "EventHandler" => CallingShape::EventHandler,
        _ => CallingShape::Procedure,
    }
}

fn calling_shape_name(shape: CallingShape) -> &'static str {
    match shape {
        CallingShape::Procedure => "Procedure",
        CallingShape::PropertyAccessor => "PropertyAccessor",
        CallingShape::EventHandler => "EventHandler",
    }
}

fn passing_mode_from_name(name: &str) -> PassingMode {
    match name {
        "ByVal" => PassingMode::ByVal,
        "ByRef" => PassingMode::ByRef,
        _ => PassingMode::Unknown,
    }
}

fn passing_mode_name(mode: PassingMode) -> &'static str {
    match mode {
        PassingMode::ByVal => "ByVal",
        PassingMode::ByRef => "ByRef",
        PassingMode::Unknown => "Unknown",
    }
}

fn vba_type_from_name(name: &str) -> VbaType {
    match name {
        "Variant" => VbaType::Variant,
        "Boolean" => VbaType::Boolean,
        "Byte" => VbaType::Byte,
        "Integer" => VbaType::Integer,
        "Long" => VbaType::Long,
        "LongLong" => VbaType::LongLong,
        "LongPtr" => VbaType::LongPtr,
        "Single" => VbaType::Single,
        "Double" => VbaType::Double,
        "Currency" => VbaType::Currency,
        "Date" => VbaType::Date,
        "String" => VbaType::String,
        "Object" => VbaType::Object,
        "Array" => VbaType::Array,
        "Any" => VbaType::Any,
        "Unknown" => VbaType::Unknown,
        user_defined if user_defined.starts_with("UserDefined:") => {
            VbaType::UserDefined(user_defined[12..].to_string())
        }
        _ => VbaType::Unknown,
    }
}

fn vba_type_name(ty: &VbaType) -> String {
    match ty {
        VbaType::Variant => "Variant".to_string(),
        VbaType::Boolean => "Boolean".to_string(),
        VbaType::Byte => "Byte".to_string(),
        VbaType::Integer => "Integer".to_string(),
        VbaType::Long => "Long".to_string(),
        VbaType::LongLong => "LongLong".to_string(),
        VbaType::LongPtr => "LongPtr".to_string(),
        VbaType::Single => "Single".to_string(),
        VbaType::Double => "Double".to_string(),
        VbaType::Currency => "Currency".to_string(),
        VbaType::Date => "Date".to_string(),
        VbaType::String => "String".to_string(),
        VbaType::Object => "Object".to_string(),
        VbaType::Array => "Array".to_string(),
        VbaType::UserDefined(name) => format!("UserDefined:{name}"),
        VbaType::Any => "Any".to_string(),
        VbaType::Unknown => "Unknown".to_string(),
    }
}

fn stable_id<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .map(|part| part.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{Bytecode, DeclareParamType, Instruction};
    use crate::emit::{
        OptionalParameterDescriptor, ParameterPassingMode, ParameterRole, ProcedureKindDescriptor,
        ResolvedParameterMechanism, RuntimeCarrierKind, SlotInitialState, SourceParameterMechanism,
        VbaTypeId,
    };
    use crate::project::ExportKind;

    fn sample_bundle() -> OxBundle {
        let bytecode = Bytecode {
            instructions: vec![
                Instruction::LoadConstI32 { slot: 0, value: 42 },
                Instruction::Halt,
            ],
            external_call_descriptors: vec![],
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "Main".to_string(),
            ProcedureRuntimeMetadata {
                module_name: "Main".to_string(),
                procedure_name: "Main".to_string(),
                entry_pc: 0,
                source_line_start: 1,
                source_line_end: 1,
                statement_line_numbers: vec![1],
                statement_entry_pcs: vec![1],
                slots: vec![ProcedureRuntimeSlotMetadata::new(
                    "x".to_string(),
                    0,
                    ProcedureRuntimeSlotKind::Local,
                    VbaTypeId::Variant,
                )],
                param_slots: vec![],
                return_slot: None,
                param_types: vec![],
                return_type: None,
                signature: legacy_procedure_signature_descriptor("Main", &[], &[], None, None, &[]),
                call_sites: Vec::new(),
                array_shapes: Vec::new(),
                udt_types: Vec::new(),
            },
        );
        OxBundle::new(bytecode, metadata)
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");

        assert_eq!(restored.bytecode.instructions.len(), 2);
        assert_eq!(restored.bytecode.slot_count, 1);
        assert_eq!(restored.bytecode.user_slot_count, 1);
        assert!(restored.procedure_metadata.contains_key("Main"));
        let meta = &restored.procedure_metadata["Main"];
        assert_eq!(meta.entry_pc, 0);
        assert!(meta.param_slots.is_empty());
        assert_eq!(meta.return_slot, None);
        // v2 fields are None by default
        assert!(restored.manifest_snapshot.is_none());
        assert!(restored.export_inventory.is_none());
    }

    #[test]
    fn v9_roundtrip_preserves_call_site_array_shape_and_udt_descriptors() {
        let source = "Type Point\n\
                      X As Long\n\
                      Caption As String\n\
                      End Type\n\
                      Sub Main()\n\
                      Dim x As Long\n\
                      Dim a(1 To 2) As Long\n\
                      Dim p As Point\n\
                      x = 1\n\
                      a(1) = x\n\
                      p.X = x\n\
                      p.Caption = \"pt\"\n\
                      Call Touch(x)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub";
        let (bytecode, metadata) =
            crate::compile_with_runtime_metadata(source).expect("compile should succeed");
        let bundle = OxBundle::new(bytecode, metadata);
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");

        let call_sites = &restored.procedure_metadata["main"].call_sites;
        assert_eq!(call_sites.len(), 1);
        assert!(call_sites[0].target_name.eq_ignore_ascii_case("Touch"));
        assert_eq!(
            call_sites[0].target_entry_pc,
            Some(restored.procedure_metadata["touch"].entry_pc)
        );
        let array_shapes = &restored.procedure_metadata["main"].array_shapes;
        assert_eq!(array_shapes.len(), 1);
        assert_eq!(array_shapes[0].name, "a");
        assert_eq!(array_shapes[0].rank, 1);
        assert_eq!(array_shapes[0].bounds[0].lower_bound, 1);
        assert_eq!(array_shapes[0].bounds[0].upper_bound, 2);
        let udt_types = &restored.procedure_metadata["main"].udt_types;
        assert_eq!(udt_types.len(), 1);
        assert_eq!(udt_types[0].type_name, "point");
        assert_eq!(udt_types[0].instances[0].name, "p");
        assert_eq!(udt_types[0].fields.len(), 2);
        assert_eq!(udt_types[0].fields[0].name, "x");
        assert_eq!(udt_types[0].fields[1].name, "caption");
    }

    #[test]
    fn header_magic_is_correct() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        assert_eq!(&bytes[0..4], b"OXVB");
    }

    #[test]
    fn header_version_is_9() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(version, 9);
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut bytes = sample_bundle().serialize_to_bytes().expect("serialize");
        bytes[0] = b'X';
        assert!(OxBundle::deserialize_from_bytes(&bytes).is_err());
    }

    #[test]
    fn rejects_truncated_data() {
        let bytes = sample_bundle().serialize_to_bytes().expect("serialize");
        assert!(OxBundle::deserialize_from_bytes(&bytes[..8]).is_err());
    }

    #[test]
    fn rejects_wrong_version() {
        let mut bytes = sample_bundle().serialize_to_bytes().expect("serialize");
        bytes[4] = 99; // invalid version
        assert!(OxBundle::deserialize_from_bytes(&bytes).is_err());
    }

    #[test]
    fn v1_backward_compat() {
        // Construct a v1 bundle: serialize LegacyOxBundleV1, write with v1 header
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: vec![],
            slot_count: 0,
            user_slot_count: 0,
        };
        let metadata = BTreeMap::new();
        let legacy = LegacyOxBundleV1 {
            bytecode,
            procedure_metadata: metadata,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&1u32.to_le_bytes()); // version 1
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v1");
        assert_eq!(restored.bytecode.instructions.len(), 1);
        assert!(restored.manifest_snapshot.is_none());
        assert!(restored.export_inventory.is_none());
    }

    #[test]
    fn v2_backward_compat() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: vec![],
            slot_count: 0,
            user_slot_count: 0,
        };
        let legacy = LegacyOxBundleV2 {
            bytecode,
            procedure_metadata: BTreeMap::new(),
            manifest_snapshot: Some(ManifestSnapshot {
                project_name: "LegacyV2".to_string(),
                project_kind: "compiled".to_string(),
                module_names: vec!["Main".to_string()],
                reference_names: vec![],
            }),
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v2");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v2");
        assert_eq!(
            restored
                .manifest_snapshot
                .as_ref()
                .map(|snapshot| snapshot.project_name.as_str()),
            Some("LegacyV2")
        );
        assert!(restored.descriptor_inventory.is_none());
    }

    #[test]
    fn v3_backward_compat_upgrades_slot_descriptors() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: vec![],
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut procedure_metadata = BTreeMap::new();
        procedure_metadata.insert(
            "Main".to_string(),
            LegacyProcedureRuntimeMetadata {
                module_name: "Main".to_string(),
                procedure_name: "Main".to_string(),
                entry_pc: 0,
                source_line_start: 1,
                source_line_end: 1,
                statement_line_numbers: vec![1],
                statement_entry_pcs: vec![1],
                slots: vec![LegacyProcedureRuntimeSlotMetadata {
                    name: "arg".to_string(),
                    slot: 0,
                    kind: ProcedureRuntimeSlotKind::Parameter,
                }],
                param_slots: vec![0],
                return_slot: None,
                param_types: vec![DeclareParamType::Long],
                return_type: None,
            },
        );
        let legacy = LegacyOxBundleV3 {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v3");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v3");
        let slot = &restored.procedure_metadata["Main"].slots[0];
        assert_eq!(slot.declared_type, VbaTypeId::Long);
        assert_eq!(slot.initial_state, SlotInitialState::CallerProvided);
        assert_eq!(slot.carrier, RuntimeCarrierKind::I32);
    }

    #[test]
    fn v4_backward_compat_synthesizes_signature_descriptors() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: vec![],
            slot_count: 3,
            user_slot_count: 3,
        };
        let mut procedure_metadata = BTreeMap::new();
        procedure_metadata.insert(
            "Add".to_string(),
            LegacyProcedureRuntimeMetadataV4 {
                module_name: "Math".to_string(),
                procedure_name: "Add".to_string(),
                entry_pc: 0,
                source_line_start: 1,
                source_line_end: 3,
                statement_line_numbers: vec![2],
                statement_entry_pcs: vec![1],
                slots: vec![
                    ProcedureRuntimeSlotMetadata::new(
                        "a".to_string(),
                        0,
                        ProcedureRuntimeSlotKind::Parameter,
                        VbaTypeId::Long,
                    ),
                    ProcedureRuntimeSlotMetadata::new(
                        "b".to_string(),
                        1,
                        ProcedureRuntimeSlotKind::Parameter,
                        VbaTypeId::Double,
                    ),
                    ProcedureRuntimeSlotMetadata::new(
                        "Add".to_string(),
                        2,
                        ProcedureRuntimeSlotKind::ReturnValue,
                        VbaTypeId::Double,
                    ),
                ],
                param_slots: vec![0, 1],
                return_slot: Some(2),
                param_types: vec![DeclareParamType::Long, DeclareParamType::Double],
                return_type: Some(DeclareParamType::Double),
            },
        );
        let legacy = LegacyOxBundleV4 {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v4");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v4");
        let signature = &restored.procedure_metadata["Add"].signature;
        assert_eq!(signature.kind, ProcedureKindDescriptor::Function);
        assert_eq!(signature.return_type, Some(VbaTypeId::Double));
        assert_eq!(signature.return_slot, Some(2));
        assert_eq!(signature.parameters.len(), 2);
        assert_eq!(signature.parameters[0].name, "a");
        assert_eq!(signature.parameters[0].declared_type, VbaTypeId::Long);
        assert_eq!(
            signature.parameters[0].passing_mode,
            ParameterPassingMode::Unknown,
            "v4 did not serialize ByRef/ByVal, so upgraded signature keeps that gap explicit"
        );
        assert_eq!(signature.parameters[1].name, "b");
        assert_eq!(signature.parameters[1].declared_type, VbaTypeId::Double);
    }

    #[test]
    fn v5_backward_compat_upgrades_call_relevant_signature_shape() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: vec![],
            slot_count: 1,
            user_slot_count: 1,
        };
        let mut procedure_metadata = BTreeMap::new();
        procedure_metadata.insert(
            "property_let_value".to_string(),
            LegacyProcedureRuntimeMetadataV5 {
                module_name: "Widget".to_string(),
                procedure_name: "property_let_value".to_string(),
                entry_pc: 0,
                source_line_start: 1,
                source_line_end: 3,
                statement_line_numbers: vec![2],
                statement_entry_pcs: vec![1],
                slots: vec![ProcedureRuntimeSlotMetadata::new(
                    "newValue".to_string(),
                    0,
                    ProcedureRuntimeSlotKind::Parameter,
                    VbaTypeId::Long,
                )],
                param_slots: vec![0],
                return_slot: None,
                param_types: vec![DeclareParamType::Long],
                return_type: None,
                signature: LegacyProcedureSignatureDescriptorV5 {
                    procedure_name: "property_let_value".to_string(),
                    kind: ProcedureKindDescriptor::PropertyLet,
                    parameters: vec![LegacyParameterDescriptorV5 {
                        index: 0,
                        name: "newValue".to_string(),
                        slot: Some(0),
                        role: ParameterRole::PropertyValue,
                        passing_mode: ParameterPassingMode::ByRef,
                        declared_type: VbaTypeId::Long,
                        optional: false,
                        param_array: false,
                        default_value: None,
                    }],
                    return_type: None,
                    return_slot: None,
                    property_group: Some("value".to_string()),
                },
            },
        );
        let legacy = LegacyOxBundleV5 {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v5");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&5u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v5");
        let signature = &restored.procedure_metadata["property_let_value"].signature;
        let param = &signature.parameters[0];
        assert_eq!(param.source_mechanism, SourceParameterMechanism::Unknown);
        assert_eq!(
            param.resolved_mechanism,
            ResolvedParameterMechanism::PropertyValueByVal
        );
        assert_eq!(
            param.optional_descriptor,
            OptionalParameterDescriptor::Required
        );
        assert!(signature.implicit_current_object.is_none());
    }

    #[test]
    fn v6_backward_compat_adds_empty_call_site_descriptor_rows() {
        let bytecode = Bytecode {
            instructions: vec![Instruction::Halt],
            external_call_descriptors: vec![],
            slot_count: 1,
            user_slot_count: 1,
        };
        let slot = ProcedureRuntimeSlotMetadata::new(
            "value".to_string(),
            0,
            ProcedureRuntimeSlotKind::Parameter,
            VbaTypeId::Long,
        );
        let mut procedure_metadata = BTreeMap::new();
        procedure_metadata.insert(
            "use_value".to_string(),
            LegacyProcedureRuntimeMetadataV6 {
                module_name: "Main".to_string(),
                procedure_name: "use_value".to_string(),
                entry_pc: 0,
                source_line_start: 1,
                source_line_end: 2,
                statement_line_numbers: vec![1],
                statement_entry_pcs: vec![1],
                slots: vec![slot.clone()],
                param_slots: vec![0],
                return_slot: None,
                param_types: vec![DeclareParamType::Long],
                return_type: None,
                signature: legacy_procedure_signature_descriptor(
                    "use_value",
                    &[0],
                    &[DeclareParamType::Long],
                    None,
                    None,
                    &[slot],
                ),
            },
        );
        let legacy = LegacyOxBundleV6 {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v6");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&6u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v6");
        let metadata = &restored.procedure_metadata["use_value"];
        assert_eq!(metadata.signature.parameters.len(), 1);
        assert!(
            metadata.call_sites.is_empty(),
            "v6 bundles predate call-site descriptor rows and must upgrade with an explicit empty set"
        );
    }

    #[test]
    fn v7_backward_compat_adds_empty_array_shape_descriptor_rows() {
        let source = "Sub Main()\n\
                      Dim x As Long\n\
                      x = 1\n\
                      Call Touch(x)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub";
        let (bytecode, metadata) =
            crate::compile_with_runtime_metadata(source).expect("compile should succeed");
        let procedure_metadata = metadata
            .into_iter()
            .map(|(name, metadata)| {
                (
                    name,
                    LegacyProcedureRuntimeMetadataV7 {
                        module_name: metadata.module_name,
                        procedure_name: metadata.procedure_name,
                        entry_pc: metadata.entry_pc,
                        source_line_start: metadata.source_line_start,
                        source_line_end: metadata.source_line_end,
                        statement_line_numbers: metadata.statement_line_numbers,
                        statement_entry_pcs: metadata.statement_entry_pcs,
                        slots: metadata.slots,
                        param_slots: metadata.param_slots,
                        return_slot: metadata.return_slot,
                        param_types: metadata.param_types,
                        return_type: metadata.return_type,
                        signature: metadata.signature,
                        call_sites: metadata.call_sites,
                    },
                )
            })
            .collect();
        let legacy = LegacyOxBundleV7 {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v7");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&7u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v7");
        let metadata = &restored.procedure_metadata["main"];
        assert_eq!(metadata.call_sites.len(), 1);
        assert!(
            metadata.array_shapes.is_empty(),
            "v7 bundles predate array-shape descriptor rows and must upgrade with an explicit empty set"
        );
    }

    #[test]
    fn v8_backward_compat_adds_empty_udt_descriptor_rows() {
        let source = "Type Point\n\
                      X As Long\n\
                      End Type\n\
                      Sub Main()\n\
                      Dim p As Point\n\
                      p.X = 1\n\
                      End Sub";
        let (bytecode, metadata) =
            crate::compile_with_runtime_metadata(source).expect("compile should succeed");
        let procedure_metadata = metadata
            .into_iter()
            .map(|(name, metadata)| {
                (
                    name,
                    LegacyProcedureRuntimeMetadataV8 {
                        module_name: metadata.module_name,
                        procedure_name: metadata.procedure_name,
                        entry_pc: metadata.entry_pc,
                        source_line_start: metadata.source_line_start,
                        source_line_end: metadata.source_line_end,
                        statement_line_numbers: metadata.statement_line_numbers,
                        statement_entry_pcs: metadata.statement_entry_pcs,
                        slots: metadata.slots,
                        param_slots: metadata.param_slots,
                        return_slot: metadata.return_slot,
                        param_types: metadata.param_types,
                        return_type: metadata.return_type,
                        signature: metadata.signature,
                        call_sites: metadata.call_sites,
                        array_shapes: metadata.array_shapes,
                    },
                )
            })
            .collect();
        let legacy = LegacyOxBundleV8 {
            bytecode,
            procedure_metadata,
            manifest_snapshot: None,
            export_inventory: None,
            source_hashes: None,
            toolchain_fingerprint: None,
            event_dispatch_bindings: None,
            com_withevents_routes: None,
            dynamic_object_routes: None,
            descriptor_inventory: None,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v8");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&8u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        let restored = OxBundle::deserialize_from_bytes(&data).expect("deserialize v8");
        let metadata = &restored.procedure_metadata["main"];
        assert!(
            metadata.udt_types.is_empty(),
            "v8 bundles predate UDT descriptor rows and must upgrade with an explicit empty set"
        );
    }

    #[test]
    fn v2_roundtrip_with_populated_fields() {
        let mut bundle = sample_bundle();
        bundle.manifest_snapshot = Some(ManifestSnapshot {
            project_name: "TestProj".to_string(),
            project_kind: "Library".to_string(),
            module_names: vec!["Mod1".to_string(), "Mod2".to_string()],
            reference_names: vec!["Scripting".to_string()],
        });
        bundle.export_inventory = Some(ExportInventory {
            host_exports: vec![HostProcedureExport {
                project_name: "TestProj".to_string(),
                module_name: "Mod1".to_string(),
                procedure_name: "DoWork".to_string(),
                kind: ExportKind::Sub,
            }],
            com_class_exports: vec![ComClassExportEntry {
                class_name: "Widget".to_string(),
                prog_id: Some("TestProj.Widget".to_string()),
                instancing: Some("MultiUse".to_string()),
                clsid: None,
                description: Some("A widget".to_string()),
            }],
        });
        bundle.source_hashes = Some({
            let mut m = BTreeMap::new();
            m.insert("Mod1".to_string(), [0xABu8; 32]);
            m
        });

        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");

        let snap = restored.manifest_snapshot.as_ref().unwrap();
        assert_eq!(snap.project_name, "TestProj");
        assert_eq!(snap.module_names.len(), 2);

        let inv = restored.export_inventory.as_ref().unwrap();
        assert_eq!(inv.host_exports.len(), 1);
        assert_eq!(inv.host_exports[0].procedure_name, "DoWork");
        assert_eq!(inv.com_class_exports.len(), 1);
        assert_eq!(inv.com_class_exports[0].class_name, "Widget");

        let hashes = restored.source_hashes.as_ref().unwrap();
        assert_eq!(hashes["Mod1"], [0xABu8; 32]);
    }

    #[test]
    fn compile_and_bundle_roundtrip() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub";
        let (bytecode, metadata) = crate::compile_with_runtime_metadata(source).expect("compile");
        let bundle = OxBundle::new(bytecode, metadata);
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");

        // Verify bytecode structure is preserved.
        assert_eq!(
            bundle.bytecode.instructions.len(),
            restored.bytecode.instructions.len()
        );
        assert_eq!(bundle.bytecode.slot_count, restored.bytecode.slot_count);
        assert_eq!(
            bundle.bytecode.user_slot_count,
            restored.bytecode.user_slot_count
        );
        assert_eq!(
            bundle.procedure_metadata.len(),
            restored.procedure_metadata.len()
        );
        for (name, meta) in &bundle.procedure_metadata {
            let restored_meta = restored
                .procedure_metadata
                .get(name)
                .expect("procedure metadata present");
            assert_eq!(meta, restored_meta);
        }
    }

    #[test]
    fn from_compiled_project_populates_metadata() {
        let source = "Public Sub Hello()\nEnd Sub\nPublic Function Add(a, b) As Long\nAdd = a + b\nEnd Function";
        let manifest = crate::project::ProjectManifest {
            project_name: "TestBundle".to_string(),
            project_kind: crate::project::ProjectKind::Library,
            modules: vec![crate::project::ModuleUnit {
                module_name: "Mod1".to_string(),
                module_kind: crate::project::ModuleKind::Procedural,
                attributes: crate::project::ModuleAttributes {
                    vb_name: "Mod1".to_string(),
                    ..Default::default()
                },
                source: source.to_string(),
            }],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: BTreeMap::new(),
        };

        let compiled = crate::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project(&compiled, "TestBundle");

        let snap = bundle.manifest_snapshot.as_ref().unwrap();
        assert_eq!(snap.project_name, "TestBundle");

        let inv = bundle.export_inventory.as_ref().unwrap();
        assert!(!inv.host_exports.is_empty());

        // Should round-trip
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");
        assert!(restored.manifest_snapshot.is_some());
    }

    #[test]
    fn legacy_bundle_reports_callable_inventory_unavailable() {
        let bundle = sample_bundle();
        assert_eq!(
            bundle
                .callable_descriptors()
                .expect_err("no descriptor inventory"),
            BundleDescriptorInventoryError::Unavailable
        );
    }

    #[test]
    fn source_reflection_and_bundle_callable_inventory_match() {
        let manifest = crate::project::ProjectManifest {
            project_name: "MatchProj".to_string(),
            project_kind: crate::project::ProjectKind::Library,
            modules: vec![crate::project::ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: crate::project::ModuleKind::Procedural,
                attributes: crate::project::ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..Default::default()
                },
                source: "Public Function Add(ByVal a As Long, ByVal b As Double) As Double\nAdd = a + b\nEnd Function\nPrivate Sub Hidden()\nEnd Sub".to_string(),
            }],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: BTreeMap::new(),
        };

        let compiled = crate::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project(&compiled, "MatchProj");
        let callables = bundle.callable_descriptors().expect("callables");

        assert_eq!(
            callables.len(),
            compiled.project_reflection.procedures.len()
        );
        for procedure in &compiled.project_reflection.procedures {
            let bundled = callables
                .iter()
                .find(|candidate| candidate.callable_id == procedure.callable_id)
                .expect("matching bundled callable");
            assert_eq!(bundled.module_name, procedure.module_name);
            assert_eq!(bundled.procedure_name, procedure.procedure_name);
            assert_eq!(
                bundled.descriptor_fingerprint,
                procedure.descriptor_fingerprint
            );
            assert_eq!(
                bundled.signature.parameters.len(),
                procedure.signature.parameters.len()
            );
            assert_eq!(
                bundled
                    .signature
                    .return_type
                    .as_ref()
                    .map(|ty| ty.normalized.as_str()),
                procedure
                    .signature
                    .return_type
                    .as_ref()
                    .map(|ty| vba_type_name(&ty.normalized))
                    .as_deref()
            );
        }
    }

    #[test]
    fn descriptor_fingerprint_changes_when_signature_changes() {
        fn add_fingerprint(source: &str) -> String {
            let manifest = crate::project::ProjectManifest {
                project_name: "FingerprintProj".to_string(),
                project_kind: crate::project::ProjectKind::Library,
                modules: vec![crate::project::ModuleUnit {
                    module_name: "Main".to_string(),
                    module_kind: crate::project::ModuleKind::Procedural,
                    attributes: crate::project::ModuleAttributes {
                        vb_name: "Main".to_string(),
                        ..Default::default()
                    },
                    source: source.to_string(),
                }],
                references: vec![],
                reference_projects: vec![],
                conditional_constants: BTreeMap::new(),
            };
            let compiled = crate::compile_project(&manifest).expect("compile");
            let bundle = OxBundle::from_compiled_project(&compiled, "FingerprintProj");
            bundle
                .callable_descriptors()
                .expect("callables")
                .iter()
                .find(|callable| callable.procedure_name == "add")
                .expect("add callable")
                .descriptor_fingerprint
                .clone()
        }

        let first =
            add_fingerprint("Public Function Add(a As Long) As Long\nAdd = a\nEnd Function");
        let second =
            add_fingerprint("Public Function Add(a As Double) As Double\nAdd = a\nEnd Function");
        assert_ne!(first, second);
    }

    #[test]
    fn from_compiled_project_persists_descriptor_inventory() {
        let manifest = crate::project::ProjectManifest {
            project_name: "DescriptorProj".to_string(),
            project_kind: crate::project::ProjectKind::Library,
            modules: vec![
                crate::project::ModuleUnit {
                    module_name: "Main".to_string(),
                    module_kind: crate::project::ModuleKind::Procedural,
                    attributes: crate::project::ModuleAttributes {
                        vb_name: "Main".to_string(),
                        ..Default::default()
                    },
                    source: "Public Function HostAdd(a As Long, b As Long) As Long\nHostAdd = a + b\nEnd Function".to_string(),
                },
                crate::project::ModuleUnit {
                    module_name: "Widget".to_string(),
                    module_kind: crate::project::ModuleKind::Class,
                    attributes: crate::project::ModuleAttributes {
                        vb_name: "Widget".to_string(),
                        vb_creatable: true,
                        vb_exposed: true,
                        ..Default::default()
                    },
                    source: "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function\nPublic Property Get Value() As Long\nValue = 42\nEnd Property".to_string(),
                },
            ],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: BTreeMap::new(),
        };

        let compiled = crate::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project(&compiled, "DescriptorProj");
        let inventory = bundle
            .descriptor_inventory
            .as_ref()
            .expect("descriptor inventory");
        assert_eq!(inventory.com_classes.len(), 1);
        assert_eq!(inventory.com_classes[0].class_name, "widget");
        assert!(
            inventory.com_classes[0]
                .members
                .iter()
                .any(|member| member.member_name.eq_ignore_ascii_case("Add")
                    && !member.stable_member_id.is_empty())
        );
        assert!(
            inventory
                .callables
                .iter()
                .any(|call| call.procedure_name.eq_ignore_ascii_case("HostAdd")
                    && call.param_slots.len() == 2)
        );
        let host_add = inventory
            .callables
            .iter()
            .find(|call| call.procedure_name.eq_ignore_ascii_case("HostAdd"))
            .expect("HostAdd callable descriptor");
        assert_eq!(host_add.kind, "Function");
        assert!(!host_add.callable_id.is_empty());
        assert!(host_add.is_public);
        assert!(!host_add.is_class_member);
        assert_eq!(host_add.signature.parameters.len(), 2);
        assert_eq!(host_add.signature.parameters[0].name.as_deref(), Some("a"));
        assert_eq!(
            host_add.signature.parameters[0]
                .value_type
                .as_ref()
                .map(|ty| ty.normalized.as_str()),
            Some("Long")
        );
        assert_eq!(
            host_add
                .signature
                .return_type
                .as_ref()
                .map(|ty| ty.normalized.as_str()),
            Some("Long")
        );
        let descriptor_debug = format!("{host_add:#?}").to_ascii_lowercase();
        for forbidden in [
            "selection_policy",
            "volatile",
            "worksheet",
            "thread_safety",
            "formula",
        ] {
            assert!(!descriptor_debug.contains(forbidden));
        }

        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");
        let restored_inventory = restored
            .descriptor_inventory
            .as_ref()
            .expect("restored descriptor inventory");
        assert_eq!(
            restored_inventory.com_classes[0].stable_class_id,
            inventory.com_classes[0].stable_class_id
        );
        assert_eq!(
            restored_inventory.callables.len(),
            inventory.callables.len()
        );
        let restored_host_add = restored_inventory
            .callables
            .iter()
            .find(|call| call.procedure_name.eq_ignore_ascii_case("HostAdd"))
            .expect("restored HostAdd callable descriptor");
        assert_eq!(restored_host_add.callable_id, host_add.callable_id);
        assert_eq!(
            restored_host_add.descriptor_fingerprint,
            host_add.descriptor_fingerprint
        );
        assert_eq!(restored_host_add.signature.parameters.len(), 2);
    }
}
