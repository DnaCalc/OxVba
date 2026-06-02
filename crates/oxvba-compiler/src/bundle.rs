//! OxBundle: portable compiled bytecode container.
//!
//! Bundles a compiled `Bytecode` together with `ProcedureRuntimeMetadata`
//! into a single serializable unit that can be persisted to disk (.oxb files)
//! and later deserialized for execution.

use std::collections::BTreeMap;

use rkyv::{Archive, Deserialize, Serialize};

use crate::bytecode::{Bytecode, ExternalCallDescriptor, Instruction};
use crate::emit::{
    ArgumentBindingDescriptor, ArrayShapeDescriptor, CallInvocationSyntaxDescriptor,
    CallReturnDescriptor, CallSiteDescriptor, CallTargetKindDescriptor, CarrierLayoutDescriptor,
    DefaultMemberPolicyDescriptor, ObjectTypeDescriptor, OptionalDefaultValue,
    OptionalMissingStatePolicy, OptionalParameterDescriptor, ParamArrayDescriptor,
    ParameterDescriptor, ParameterPassingMode, ParameterRole, ProcedureKindDescriptor,
    ProcedureRuntimeMetadata, ProcedureRuntimeSlotKind, ProcedureRuntimeSlotMetadata,
    ProcedureSignatureDescriptor, ResolvedParameterMechanism, SourceParameterMechanism,
    UdtTypeDescriptor, ValueStateDescriptor, VbaTypeId, legacy_procedure_signature_descriptor,
};
use crate::frontend_hir_lowering::lower_typed_hir_to_bound_module;
use crate::frontend_type_hooks::collect_type_hooks_from_source;
use crate::project::{
    CallableCapability, CallingShape, HostProcedureExport, InvocationLane, ModuleDescriptor,
    ModuleKind, ModuleUnit, ModuleVisibility, PassingMode, ProcedureAnnotation,
    ProcedureDescriptor, ProcedureKind, ProcedureParameterDescriptor, ProcedureSignature,
    ProcedureVisibility, ProjectComWithEventsRoute, ProjectDynamicMemberKind,
    ProjectDynamicObjectRoute, ProjectEventDispatchBinding, ProjectIdentity, ProjectKind,
    ProjectManifest, ProjectReference, ProjectReflection, ReferenceKind, ReferencedProjectManifest,
    RuntimeProcedureRoute, VbaType, VbaTypeDescriptor,
};
use crate::resolve::{
    BoundCompareMode, BoundExternalDecl, BoundModule, BoundType, builtin_pp_constants,
    collect_option_base, normalize_source_lines, resolve_symbols,
};

/// Magic header bytes for the OxBundle binary format.
const MAGIC: [u8; 4] = *b"OXVB";
/// Current strict executable semantic package bundle format version.
const FORMAT_VERSION: u32 = 15;
/// Header size in bytes (padded to 16 for rkyv alignment).
const HEADER_SIZE: usize = 16;

fn unsupported_bundle_version_message(version: u32) -> String {
    format!(
        "unsupported legacy bundle version {version}; strict executable semantic packages require current bundle format version {FORMAT_VERSION}"
    )
}

/// FNV-1a 64-bit offset basis (matches the dependency-free digest approach used
/// for descriptor identity in `descriptor_identity.rs`).
const BUNDLE_FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime.
const BUNDLE_FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Deterministic FNV-1a 64-bit hash over arbitrary bytes.
fn bundle_content_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = BUNDLE_FNV1A64_OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(BUNDLE_FNV1A64_PRIME);
    }
    hash
}

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

/// Project/import/source/host context facts carried by executable packages.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleProjectContext {
    pub project_name: String,
    pub project_kind: String,
    pub project_id: String,
    pub source_fingerprint: String,
    pub compile_context: BundleCompileContext,
    pub modules: Vec<BundleProjectModuleFact>,
    pub references: Vec<BundleProjectReferenceFact>,
    pub referenced_projects: Vec<BundleReferencedProjectFact>,
    pub source_maps: Vec<BundleModuleSourceMap>,
    pub native_libraries: Vec<BundleNativeLibraryFact>,
    pub host_capabilities: Vec<BundleHostCapabilityRequirement>,
    pub package_diagnostics: Vec<BundlePackageDiagnostic>,
    pub gap_classifications: Vec<BundlePackageGapClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleProjectModuleFact {
    pub module_id: String,
    pub name: String,
    pub kind: String,
    pub option_private_module: bool,
    pub option_explicit: bool,
    pub option_compare: String,
    pub option_base: i32,
    pub vb_exposed: bool,
    pub vb_creatable: bool,
    pub vb_predeclared_id: bool,
    pub vb_global_namespace: bool,
    pub source_fingerprint: String,
    pub source_line_count: usize,
    pub default_type_families: Vec<BundleDefaultTypeFamilyFact>,
    pub external_declare_count: usize,
    pub ptrsafe_declare_count: usize,
    pub uses_long_ptr: bool,
    pub uses_long_long: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleCompileContext {
    pub manifest_conditional_constants: Vec<BundleConditionalConstantFact>,
    pub builtin_conditional_constants: Vec<BundleConditionalConstantFact>,
    pub target_pointer_width_bits: u32,
    pub vba7: bool,
    pub win64: bool,
    pub long_ptr_carrier: String,
    pub long_long_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleConditionalConstantFact {
    pub name: String,
    pub value: i32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleDefaultTypeFamilyFact {
    pub start_letter: char,
    pub end_letter: char,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleProjectReferenceFact {
    pub reference_id: String,
    pub ordinal: usize,
    pub name: String,
    pub kind: String,
    pub resolution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleReferencedProjectFact {
    pub project_name: String,
    pub module_names: Vec<String>,
    pub module_count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleModuleSourceMap {
    pub module_name: String,
    pub lines: Vec<BundleSourceLineMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleSourceLineMapping {
    pub file_line: u32,
    pub runtime_line: Option<u32>,
    pub kind: String,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleNativeLibraryFact {
    pub descriptor_id: u32,
    pub declared_name: String,
    pub library: String,
    pub alias: String,
    pub ordinal_alias: bool,
    pub calling_convention: String,
    pub marshal_lane: String,
    pub selection_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundleHostCapabilityRequirement {
    pub capability_id: String,
    pub capability: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundlePackageDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub fact_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub struct BundlePackageGapClassification {
    pub gap_id: String,
    pub area: String,
    pub status: String,
    pub detail: String,
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
    pub project_context: Option<BundleProjectContext>,
}

/// v1 bundle layout retained only for rejection fixtures.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV1 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadata>,
}

/// v2 bundle layout retained only for rejection fixtures.
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

/// v3 bundle layout retained only for rejection fixtures.
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

/// v9 bundle layout before procedure object descriptors were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV9 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV9>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v10 bundle layout before project/import/source/host context facts were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV10 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV10>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
}

/// v11 bundle layout before carrier-layout and value-state descriptors were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV11 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV10>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
    project_context: Option<BundleProjectContext>,
}

/// v12 bundle layout before call invocation syntax and diagnostic policies were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV12 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV12>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
    project_context: Option<BundleProjectContext>,
}

/// v13 bundle layout before expression/operator/coercion/name/member descriptors were added.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV13 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, LegacyProcedureRuntimeMetadataV13>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
    descriptor_inventory: Option<DescriptorInventory>,
    project_context: Option<BundleProjectContext>,
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyCallSiteDescriptorV12 {
    call_site_id: String,
    caller_procedure_name: String,
    call_pc: usize,
    target_name: String,
    target_kind: CallTargetKindDescriptor,
    target_entry_pc: Option<usize>,
    default_member_policy: DefaultMemberPolicyDescriptor,
    arguments: Vec<ArgumentBindingDescriptor>,
    return_value: Option<CallReturnDescriptor>,
}

impl From<LegacyCallSiteDescriptorV12> for CallSiteDescriptor {
    fn from(legacy: LegacyCallSiteDescriptorV12) -> Self {
        let argument_evaluation_order = legacy_argument_evaluation_order(&legacy.arguments);
        CallSiteDescriptor {
            call_site_id: legacy.call_site_id,
            caller_procedure_name: legacy.caller_procedure_name,
            call_pc: legacy.call_pc,
            target_name: legacy.target_name,
            target_kind: legacy.target_kind,
            target_entry_pc: legacy.target_entry_pc,
            default_member_policy: legacy.default_member_policy,
            invocation_syntax: CallInvocationSyntaxDescriptor::Unknown,
            argument_evaluation_order,
            arguments: legacy.arguments,
            return_value: legacy.return_value,
            diagnostic_policies: Vec::new(),
        }
    }
}

fn legacy_argument_evaluation_order(arguments: &[ArgumentBindingDescriptor]) -> Vec<usize> {
    let mut order = arguments
        .iter()
        .filter_map(|argument| argument.source_index)
        .collect::<Vec<_>>();
    order.sort_unstable();
    order.dedup();
    order
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
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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
    call_sites: Vec<LegacyCallSiteDescriptorV12>,
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
            call_sites: legacy.call_sites.into_iter().map(Into::into).collect(),
            array_shapes: Vec::new(),
            udt_types: Vec::new(),
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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
    call_sites: Vec<LegacyCallSiteDescriptorV12>,
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
            call_sites: legacy.call_sites.into_iter().map(Into::into).collect(),
            array_shapes: legacy.array_shapes,
            udt_types: Vec::new(),
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV9 {
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
    call_sites: Vec<LegacyCallSiteDescriptorV12>,
    array_shapes: Vec<ArrayShapeDescriptor>,
    udt_types: Vec<UdtTypeDescriptor>,
}

impl From<LegacyProcedureRuntimeMetadataV9> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV9) -> Self {
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
            call_sites: legacy.call_sites.into_iter().map(Into::into).collect(),
            array_shapes: legacy.array_shapes,
            udt_types: legacy.udt_types,
            object_types: Vec::new(),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV10 {
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
    call_sites: Vec<LegacyCallSiteDescriptorV12>,
    array_shapes: Vec<ArrayShapeDescriptor>,
    udt_types: Vec<UdtTypeDescriptor>,
    object_types: Vec<ObjectTypeDescriptor>,
}

impl From<LegacyProcedureRuntimeMetadataV10> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV10) -> Self {
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
            call_sites: legacy.call_sites.into_iter().map(Into::into).collect(),
            array_shapes: legacy.array_shapes,
            udt_types: legacy.udt_types,
            object_types: legacy.object_types,
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV12 {
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
    call_sites: Vec<LegacyCallSiteDescriptorV12>,
    array_shapes: Vec<ArrayShapeDescriptor>,
    udt_types: Vec<UdtTypeDescriptor>,
    object_types: Vec<ObjectTypeDescriptor>,
    carrier_layouts: Vec<CarrierLayoutDescriptor>,
    value_states: Vec<ValueStateDescriptor>,
}

impl From<LegacyProcedureRuntimeMetadataV12> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV12) -> Self {
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
            call_sites: legacy.call_sites.into_iter().map(Into::into).collect(),
            array_shapes: legacy.array_shapes,
            udt_types: legacy.udt_types,
            object_types: legacy.object_types,
            carrier_layouts: legacy.carrier_layouts,
            value_states: legacy.value_states,
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyProcedureRuntimeMetadataV13 {
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
    udt_types: Vec<UdtTypeDescriptor>,
    object_types: Vec<ObjectTypeDescriptor>,
    carrier_layouts: Vec<CarrierLayoutDescriptor>,
    value_states: Vec<ValueStateDescriptor>,
}

impl From<LegacyProcedureRuntimeMetadataV13> for ProcedureRuntimeMetadata {
    fn from(legacy: LegacyProcedureRuntimeMetadataV13) -> Self {
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
            udt_types: legacy.udt_types,
            object_types: legacy.object_types,
            carrier_layouts: legacy.carrier_layouts,
            value_states: legacy.value_states,
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
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

impl OxBundle {
    /// Create a new bundle from a compiled bytecode and its procedure metadata.
    ///
    /// Optional package metadata fields default to `None`.
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
            project_context: None,
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

    /// Create a bundle from a `CompiledProject`, preserving reflection-derived
    /// package metadata when a source manifest is unavailable.
    pub fn from_compiled_project(
        compiled: &crate::project::CompiledProject,
        project_name: &str,
    ) -> Self {
        Self::from_compiled_project_parts(compiled, project_name, None)
    }

    /// Create a bundle from a compiled project and its source manifest.
    ///
    /// This preserves project/module/reference/import/source-map and host
    /// capability context that cannot be recovered from bytecode alone.
    pub fn from_compiled_project_with_manifest(
        compiled: &crate::project::CompiledProject,
        manifest: &ProjectManifest,
    ) -> Self {
        Self::from_compiled_project_parts(compiled, &manifest.project_name, Some(manifest))
    }

    fn from_compiled_project_parts(
        compiled: &crate::project::CompiledProject,
        project_name: &str,
        manifest: Option<&ProjectManifest>,
    ) -> Self {
        let manifest_snapshot = ManifestSnapshot {
            project_name: project_name.to_string(),
            project_kind: manifest
                .map(|manifest| project_kind_name(manifest.project_kind).to_string())
                .unwrap_or_else(|| "compiled".to_string()),
            module_names: manifest
                .map(|manifest| {
                    manifest
                        .modules
                        .iter()
                        .map(|module| module.module_name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| {
                    compiled
                        .project_reflection
                        .modules
                        .iter()
                        .map(|module| module.name.clone())
                        .collect()
                }),
            reference_names: manifest
                .map(|manifest| {
                    manifest
                        .references
                        .iter()
                        .map(|reference| reference.referenced_project_name.clone())
                        .collect()
                })
                .unwrap_or_default(),
        };

        // The bundle is the forward-execution package, not a source-reconstruction
        // artifact: `CompiledProject::rewritten_source` and
        // `CompiledProject::reference_visible_exports` are intentionally not serialized.
        // Executable COM/export facts are NOT in that category and are populated here.
        // Precise CLSID/instancing/description for registration are owned by the
        // COM-server build layer; the bundle records the exposed/creatable classes and
        // their default ProgID.
        let host_exports = compiled.host_exports.clone();
        let export_inventory = ExportInventory {
            host_exports,
            com_class_exports: com_class_exports_from_compiled_project(compiled, project_name),
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
            project_context: Some(project_context_from_compiled_project(
                compiled,
                project_name,
                manifest,
            )),
        }
    }

    /// Stable content digest over the full serialized package payload.
    ///
    /// Suitable as a package/JIT cache key. Because it hashes the complete rkyv
    /// payload, it covers every serialized section (bytecode, procedure
    /// metadata, descriptor/export inventories, routes, project context) — two
    /// packages that differ in any serialized fact produce different digests.
    pub fn content_digest(&self) -> Result<String, String> {
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map_err(|e| format!("content_digest serialize: {e}"))?;
        Ok(format!(
            "oxb-fnv1a64:{:016x}",
            bundle_content_fnv1a64(&payload)
        ))
    }

    /// Serialize the bundle to bytes with a header.
    ///
    /// Wire format (16-byte header, aligned for rkyv):
    /// ```text
    /// [4 bytes: magic "OXVB"]
    /// [4 bytes: format version, little-endian u32]
    /// [4 bytes: payload length, little-endian u32]
    /// [4 bytes: low 32 bits of the payload content digest (integrity check)]
    /// [N bytes: rkyv-serialized OxBundle payload]
    /// ```
    pub fn serialize_to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_strict_package_sections()?;
        let payload =
            rkyv::to_bytes::<rkyv::rancor::Error>(self).map_err(|e| format!("serialize: {e}"))?;

        let payload_len =
            u32::try_from(payload.len()).map_err(|_| "bundle payload exceeds 4 GiB".to_string())?;
        let digest_low = (bundle_content_fnv1a64(&payload) & 0xffff_ffff) as u32;

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&digest_low.to_le_bytes()); // payload integrity digest
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Deserialize a strict current-version bundle from bytes produced by
    /// `serialize_to_bytes`.
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
        if version != FORMAT_VERSION {
            return Err(unsupported_bundle_version_message(version));
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

        // Validate the payload integrity digest stored in the header.
        let stored_digest_low = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let actual_digest_low = (bundle_content_fnv1a64(payload) & 0xffff_ffff) as u32;
        if actual_digest_low != stored_digest_low {
            return Err(format!(
                "bundle content digest mismatch (corrupt or tampered payload): header {stored_digest_low:#010x}, computed {actual_digest_low:#010x}"
            ));
        }

        // rkyv requires aligned data. Copy to an aligned buffer (16-byte alignment).
        let mut aligned: rkyv::util::AlignedVec<16> =
            rkyv::util::AlignedVec::with_capacity(payload.len());
        aligned.extend_from_slice(payload);

        let bundle: OxBundle = rkyv::from_bytes::<OxBundle, rkyv::rancor::Error>(&aligned)
            .map_err(|e| format!("deserialize: {e}"))?;
        bundle.validate_strict_package_sections()?;
        Ok(bundle)
    }

    fn validate_strict_package_sections(&self) -> Result<(), String> {
        let mut missing = Vec::new();
        if self.bytecode.instructions.is_empty() {
            missing.push("bytecode");
        }
        if self.procedure_metadata.is_empty() {
            missing.push("procedure_metadata");
        }
        if self.manifest_snapshot.is_none() {
            missing.push("manifest_snapshot");
        }
        if self.export_inventory.is_none() {
            missing.push("export_inventory");
        }
        if self.descriptor_inventory.is_none() {
            missing.push("descriptor_inventory");
        }
        if self.project_context.is_none() {
            missing.push("project_context");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "BUNDLE-STRICT-MISSING-SECTIONS: current bundle format version {FORMAT_VERSION} requires {}",
                missing.join(", ")
            ))
        }
    }
}

/// Record COM-exposed creatable class modules as export-inventory entries.
///
/// Matches `oxvba_project::validate::validate_com_class_exports`'s filter
/// (exposed AND creatable). Precise CLSID/instancing/description are assigned by
/// the COM-server build layer; here the default ProgID (`Project.Class`) is
/// recorded and the rest left unset.
fn com_class_exports_from_compiled_project(
    compiled: &crate::project::CompiledProject,
    project_name: &str,
) -> Vec<ComClassExportEntry> {
    compiled
        .project_reflection
        .modules
        .iter()
        .filter(|module| {
            module.kind == ModuleKind::Class
                && module.visibility.vb_exposed
                && module.visibility.vb_creatable
        })
        .map(|module| ComClassExportEntry {
            class_name: module.name.clone(),
            prog_id: Some(format!("{project_name}.{}", module.name)),
            instancing: None,
            clsid: None,
            description: None,
        })
        .collect()
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

fn project_context_from_compiled_project(
    compiled: &crate::project::CompiledProject,
    project_name: &str,
    manifest: Option<&ProjectManifest>,
) -> BundleProjectContext {
    let modules = manifest
        .map(|manifest| {
            manifest
                .modules
                .iter()
                .map(|module| {
                    module_fact_from_manifest_module(module, &compiled.project_reflection)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            compiled
                .project_reflection
                .modules
                .iter()
                .map(module_fact_from_reflection_module)
                .collect::<Vec<_>>()
        });
    let references = manifest.map(project_reference_facts).unwrap_or_default();
    let referenced_projects = manifest
        .map(|manifest| {
            manifest
                .reference_projects
                .iter()
                .map(referenced_project_fact)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let source_maps = compiled
        .source_maps
        .modules
        .values()
        .map(|module| BundleModuleSourceMap {
            module_name: module.module_name.clone(),
            lines: module
                .lines
                .iter()
                .map(|line| BundleSourceLineMapping {
                    file_line: line.file_line,
                    runtime_line: line.runtime_line,
                    kind: format!("{:?}", line.kind),
                    executable: line.executable,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let native_libraries = native_library_facts(&compiled.bytecode.external_call_descriptors);
    let host_capabilities = host_capability_requirements(&compiled.bytecode);
    let package_diagnostics = package_context_diagnostics(&references);
    let gap_classifications = package_context_gaps(
        &references,
        &source_maps,
        &native_libraries,
        &host_capabilities,
    );
    BundleProjectContext {
        project_name: manifest
            .map(|manifest| manifest.project_name.clone())
            .unwrap_or_else(|| project_name.to_string()),
        project_kind: manifest
            .map(|manifest| project_kind_name(manifest.project_kind).to_string())
            .unwrap_or_else(|| "compiled".to_string()),
        project_id: compiled.project_reflection.identity.project_id.clone(),
        source_fingerprint: compiled
            .project_reflection
            .identity
            .source_fingerprint
            .clone(),
        compile_context: bundle_compile_context(manifest),
        modules,
        references,
        referenced_projects,
        source_maps,
        native_libraries,
        host_capabilities,
        package_diagnostics,
        gap_classifications,
    }
}

fn module_fact_from_manifest_module(
    module: &ModuleUnit,
    reflection: &ProjectReflection,
) -> BundleProjectModuleFact {
    let reflected = reflection
        .modules
        .iter()
        .find(|candidate| candidate.name.eq_ignore_ascii_case(&module.module_name));
    let bound = bound_module_for_bundle_facts(module);
    let external_declare_count = bound.external_declarations.len();
    let ptrsafe_declare_count = bound
        .external_declarations
        .values()
        .filter(|external| external.ptr_safe)
        .count();
    let uses_long_ptr = bound_module_uses_type(&bound, BoundType::LongPtr);
    let uses_long_long = bound_module_uses_type(&bound, BoundType::LongLong);
    BundleProjectModuleFact {
        module_id: reflected
            .map(|module| module.module_id.clone())
            .unwrap_or_else(|| stable_id(["module", &module.module_name])),
        name: module.module_name.clone(),
        kind: module_kind_name(module.module_kind).to_string(),
        option_private_module: module.attributes.option_private_module,
        option_explicit: bound.option_explicit,
        option_compare: compare_mode_name(bound.compare_mode).to_string(),
        option_base: bound
            .array_descriptors
            .values()
            .next()
            .map(|descriptor| descriptor.option_base)
            .unwrap_or_else(|| collect_module_option_base(&module.source)),
        vb_exposed: module.attributes.vb_exposed,
        vb_creatable: module.attributes.vb_creatable,
        vb_predeclared_id: module.attributes.vb_predeclared_id,
        vb_global_namespace: module.attributes.vb_global_namespace,
        source_fingerprint: reflected
            .map(|module| module.source_fingerprint.clone())
            .unwrap_or_default(),
        source_line_count: module.source.lines().count().max(1),
        default_type_families: default_type_families(&bound.default_type_table),
        external_declare_count,
        ptrsafe_declare_count,
        uses_long_ptr,
        uses_long_long,
    }
}

fn bound_module_for_bundle_facts(module: &ModuleUnit) -> BoundModule {
    collect_type_hooks_from_source(&module.module_name, &module.source)
        .ok()
        .and_then(|typed_hir| lower_typed_hir_to_bound_module(&module.source, &typed_hir).ok())
        .unwrap_or_else(|| resolve_symbols(&module.source))
}

fn module_fact_from_reflection_module(module: &ModuleDescriptor) -> BundleProjectModuleFact {
    BundleProjectModuleFact {
        module_id: module.module_id.clone(),
        name: module.name.clone(),
        kind: module_kind_name(module.kind).to_string(),
        option_private_module: module.visibility.option_private_module,
        option_explicit: false,
        option_compare: "Unknown".to_string(),
        option_base: 0,
        vb_exposed: module.visibility.vb_exposed,
        vb_creatable: module.visibility.vb_creatable,
        vb_predeclared_id: false,
        vb_global_namespace: false,
        source_fingerprint: module.source_fingerprint.clone(),
        source_line_count: module
            .source_span
            .as_ref()
            .map(|span| {
                span.end_line
                    .saturating_sub(span.start_line)
                    .saturating_add(1)
            })
            .unwrap_or_default(),
        default_type_families: Vec::new(),
        external_declare_count: 0,
        ptrsafe_declare_count: 0,
        uses_long_ptr: false,
        uses_long_long: false,
    }
}

fn bundle_compile_context(manifest: Option<&ProjectManifest>) -> BundleCompileContext {
    let manifest_conditional_constants = manifest
        .map(|manifest| {
            manifest
                .conditional_constants
                .iter()
                .map(|(name, value)| BundleConditionalConstantFact {
                    name: name.clone(),
                    value: *value,
                    source: "manifest".to_string(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut builtin_conditional_constants = builtin_pp_constants()
        .into_iter()
        .map(|(name, value)| BundleConditionalConstantFact {
            name,
            value,
            source: "builtin".to_string(),
        })
        .collect::<Vec<_>>();
    builtin_conditional_constants.sort_by(|left, right| left.name.cmp(&right.name));
    BundleCompileContext {
        manifest_conditional_constants,
        builtin_conditional_constants,
        target_pointer_width_bits: if cfg!(target_pointer_width = "64") {
            64
        } else {
            32
        },
        vba7: true,
        win64: cfg!(all(windows, target_pointer_width = "64")),
        long_ptr_carrier: if cfg!(target_pointer_width = "64") {
            "LongLong"
        } else {
            "Long"
        }
        .to_string(),
        long_long_supported: true,
    }
}

fn collect_module_option_base(source: &str) -> i32 {
    collect_option_base(&normalize_source_lines(source))
}

fn compare_mode_name(mode: BoundCompareMode) -> &'static str {
    match mode {
        BoundCompareMode::Binary => "Binary",
        BoundCompareMode::Text => "Text",
        BoundCompareMode::Database => "Database",
    }
}

fn default_type_families(table: &[BoundType; 26]) -> Vec<BundleDefaultTypeFamilyFact> {
    let mut families = Vec::new();
    let mut index = 0;
    while index < table.len() {
        let ty = table[index];
        let start = index;
        let mut end = index;
        while end + 1 < table.len() && table[end + 1] == ty {
            end += 1;
        }
        if ty != BoundType::Variant {
            families.push(BundleDefaultTypeFamilyFact {
                start_letter: letter_for_default_type_index(start),
                end_letter: letter_for_default_type_index(end),
                type_name: bound_type_name(ty).to_string(),
            });
        }
        index = end + 1;
    }
    families
}

fn letter_for_default_type_index(index: usize) -> char {
    char::from(b'A' + u8::try_from(index).unwrap_or(0))
}

fn external_uses_long_ptr(external: &BoundExternalDecl) -> bool {
    external.return_type == BoundType::LongPtr
        || external
            .params
            .iter()
            .any(|param| param.ty == BoundType::LongPtr)
}

fn external_uses_long_long(external: &BoundExternalDecl) -> bool {
    external.return_type == BoundType::LongLong
        || external
            .params
            .iter()
            .any(|param| param.ty == BoundType::LongLong)
}

fn bound_module_uses_type(module: &BoundModule, ty: BoundType) -> bool {
    module.default_type_table.contains(&ty)
        || module
            .declaration_types
            .values()
            .any(|candidate| *candidate == ty)
        || module.external_declarations.values().any(|external| {
            if ty == BoundType::LongPtr {
                external_uses_long_ptr(external)
            } else if ty == BoundType::LongLong {
                external_uses_long_long(external)
            } else {
                external.return_type == ty || external.params.iter().any(|param| param.ty == ty)
            }
        })
        || module.procedures.iter().any(|procedure| {
            procedure.return_type == ty
                || procedure
                    .declaration_types
                    .values()
                    .any(|candidate| *candidate == ty)
                || procedure.params.iter().any(|param| param.ty == ty)
        })
}

fn bound_type_name(ty: BoundType) -> &'static str {
    match ty {
        BoundType::Variant => "Variant",
        BoundType::Integer => "Integer",
        BoundType::Long => "Long",
        BoundType::LongLong => "LongLong",
        BoundType::LongPtr => "LongPtr",
        BoundType::Byte => "Byte",
        BoundType::Single => "Single",
        BoundType::Double => "Double",
        BoundType::Currency => "Currency",
        BoundType::Decimal => "Decimal",
        BoundType::Date => "Date",
        BoundType::String => "String",
        BoundType::Boolean => "Boolean",
        BoundType::Object => "Object",
        BoundType::Array => "Array",
    }
}

fn project_reference_facts(manifest: &ProjectManifest) -> Vec<BundleProjectReferenceFact> {
    manifest
        .references
        .iter()
        .enumerate()
        .map(|(index, reference)| {
            let kind = reference_kind_name(reference.reference_kind).to_string();
            BundleProjectReferenceFact {
                reference_id: stable_id([
                    "reference",
                    &index.to_string(),
                    &kind,
                    &reference.referenced_project_name,
                ]),
                ordinal: index,
                name: reference.referenced_project_name.clone(),
                kind,
                resolution: reference_resolution(reference, manifest),
            }
        })
        .collect()
}

fn reference_resolution(reference: &ProjectReference, manifest: &ProjectManifest) -> String {
    if manifest.reference_projects.iter().any(|project| {
        project
            .project_name
            .eq_ignore_ascii_case(&reference.referenced_project_name)
    }) {
        return "project-source-present".to_string();
    }
    if reference.reference_kind == ReferenceKind::TypeLibrary {
        if manifest.reference_projects.iter().any(|project| {
            projected_typelib_reference_matches(&reference.referenced_project_name, project)
        }) {
            "typelib-projection-present".to_string()
        } else {
            "typelib-projection-missing".to_string()
        }
    } else {
        "project-source-missing".to_string()
    }
}

fn projected_typelib_reference_matches(
    reference_name: &str,
    project: &ReferencedProjectManifest,
) -> bool {
    let reference_key = reference_name.to_ascii_lowercase();
    project.modules.iter().any(|module| {
        let source = module.source.to_ascii_lowercase();
        source.contains("oxvbaprojectedtypelibreference")
            && (source.contains(&format!("reference-name={reference_key}"))
                || source.contains(&format!("requested-coclass={reference_key}"))
                || project.project_name.eq_ignore_ascii_case(reference_name))
    })
}

fn referenced_project_fact(project: &ReferencedProjectManifest) -> BundleReferencedProjectFact {
    BundleReferencedProjectFact {
        project_name: project.project_name.clone(),
        module_names: project
            .modules
            .iter()
            .map(|module| module.module_name.clone())
            .collect(),
        module_count: project.modules.len(),
        source: if project.modules.iter().any(|module| {
            module
                .source
                .to_ascii_lowercase()
                .contains("oxvbaprojectedtypelibreference")
        }) {
            "projected-typelib".to_string()
        } else {
            "project-reference".to_string()
        },
    }
}

fn native_library_facts(descriptors: &[ExternalCallDescriptor]) -> Vec<BundleNativeLibraryFact> {
    let mut facts = descriptors
        .iter()
        .map(|descriptor| BundleNativeLibraryFact {
            descriptor_id: descriptor.descriptor_id,
            declared_name: descriptor.declared_name.clone(),
            library: descriptor.library.clone(),
            alias: descriptor.alias.clone(),
            ordinal_alias: descriptor.ordinal_alias,
            calling_convention: descriptor.calling_convention.clone(),
            marshal_lane: descriptor.marshal_lane.clone(),
            selection_policy: descriptor.selection_policy.clone(),
        })
        .collect::<Vec<_>>();
    facts.sort_by_key(|fact| fact.descriptor_id);
    facts
}

fn host_capability_requirements(bytecode: &Bytecode) -> Vec<BundleHostCapabilityRequirement> {
    let mut requirements = bytecode
        .instructions
        .iter()
        .filter_map(instruction_host_capability)
        .map(|(source, capability)| BundleHostCapabilityRequirement {
            capability_id: stable_id(["host-capability", capability, source]),
            capability: capability.to_string(),
            source: source.to_string(),
        })
        .collect::<Vec<_>>();
    requirements.extend(bytecode.external_call_descriptors.iter().map(|descriptor| {
        BundleHostCapabilityRequirement {
            capability_id: stable_id([
                "host-capability",
                "dynamic-linking",
                &descriptor.declared_name,
            ]),
            capability: "dynamic-linking".to_string(),
            source: descriptor.declared_name.clone(),
        }
    }));
    requirements.sort_by(|left, right| {
        left.capability_id
            .cmp(&right.capability_id)
            .then(left.source.cmp(&right.source))
    });
    requirements.dedup_by(|left, right| left.capability_id == right.capability_id);
    requirements
}

fn instruction_host_capability(instruction: &Instruction) -> Option<(&'static str, &'static str)> {
    match instruction {
        Instruction::IntrinsicShellHost { .. } => Some(("Shell", "process-env")),
        Instruction::IntrinsicEnvironHost { .. } => Some(("Environ", "process-env")),
        Instruction::IntrinsicDirHost { .. } => Some(("Dir", "process-env")),
        Instruction::IntrinsicDateNowHost { .. } => Some(("Date", "time-locale")),
        Instruction::IntrinsicTimeNowHost { .. } => Some(("Time", "time-locale")),
        Instruction::IntrinsicNowHost { .. } => Some(("Now", "time-locale")),
        Instruction::IntrinsicTimerHost { .. } => Some(("Timer", "time-locale")),
        Instruction::IntrinsicFreeFileHost { .. } => Some(("FreeFile", "file-system-io")),
        Instruction::IntrinsicConsolePrintHost { .. } => Some(("Print", "console-io")),
        Instruction::IntrinsicConsoleInputHost { .. } => Some(("Input", "console-io")),
        Instruction::IntrinsicConsoleLineInputHost { .. } => Some(("Line Input", "console-io")),
        Instruction::IntrinsicMsgBoxHost { .. } => Some(("MsgBox", "ui-interaction")),
        Instruction::IntrinsicInputBoxHost { .. } => Some(("InputBox", "ui-interaction")),
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

fn package_context_diagnostics(
    references: &[BundleProjectReferenceFact],
) -> Vec<BundlePackageDiagnostic> {
    references
        .iter()
        .filter_map(|reference| {
            let code = match reference.resolution.as_str() {
                "project-source-missing" => "PKG-IMPORT-PROJECT-SOURCE-MISSING",
                "typelib-projection-missing" => "PKG-IMPORT-TYPELIB-PROJECTION-MISSING",
                _ => return None,
            };
            Some(BundlePackageDiagnostic {
                code: code.to_string(),
                severity: "unsupported".to_string(),
                message: format!(
                    "{} reference `{}` is present in the package manifest but has no executable reference projection",
                    reference.kind, reference.name
                ),
                fact_id: reference.reference_id.clone(),
            })
        })
        .collect()
}

fn package_context_gaps(
    references: &[BundleProjectReferenceFact],
    source_maps: &[BundleModuleSourceMap],
    native_libraries: &[BundleNativeLibraryFact],
    host_capabilities: &[BundleHostCapabilityRequirement],
) -> Vec<BundlePackageGapClassification> {
    let mut gaps = Vec::new();
    gaps.extend(
        references
            .iter()
            .map(|reference| BundlePackageGapClassification {
                gap_id: stable_id(["gap", "reference", &reference.reference_id]),
                area: "reference-import".to_string(),
                status: reference.resolution.clone(),
                detail: format!("{} reference `{}`", reference.kind, reference.name),
            }),
    );
    gaps.push(BundlePackageGapClassification {
        gap_id: "gap:source-map".to_string(),
        area: "source-map".to_string(),
        status: if source_maps.is_empty() {
            "missing".to_string()
        } else {
            "present".to_string()
        },
        detail: format!("source map modules={}", source_maps.len()),
    });
    if !native_libraries.is_empty() {
        gaps.push(BundlePackageGapClassification {
            gap_id: "gap:native-declare".to_string(),
            area: "native-library".to_string(),
            status: "descriptor-present".to_string(),
            detail: format!("native declare descriptors={}", native_libraries.len()),
        });
    }
    if !host_capabilities.is_empty() {
        gaps.push(BundlePackageGapClassification {
            gap_id: "gap:host-capability-policy".to_string(),
            area: "host-capability".to_string(),
            status: "policy-required".to_string(),
            detail: format!("host capabilities={}", host_capabilities.len()),
        });
    }
    gaps
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

fn project_kind_name(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Source => "Source",
        ProjectKind::Host => "Host",
        ProjectKind::Library => "Library",
    }
}

fn module_kind_name(kind: ModuleKind) -> &'static str {
    match kind {
        ModuleKind::Procedural => "Procedural",
        ModuleKind::Class => "Class",
        ModuleKind::Document => "Document",
        ModuleKind::Form => "Form",
        ModuleKind::Extension => "Extension",
    }
}

fn reference_kind_name(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Project => "Project",
        ReferenceKind::TypeLibrary => "TypeLibrary",
        ReferenceKind::HostInjected => "HostInjected",
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
    use crate::emit::{ParameterPassingMode, ParameterRole, ProcedureKindDescriptor, VbaTypeId};
    use crate::project::{
        ExportKind, ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
        ReferencedProjectManifest, module_unit_from_source,
    };

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
                object_types: Vec::new(),
                carrier_layouts: Vec::new(),
                value_states: Vec::new(),
                expression_semantics: Vec::new(),
                operator_semantics: Vec::new(),
                coercions: Vec::new(),
                name_bindings: Vec::new(),
                object_member_bindings: Vec::new(),
            },
        );
        OxBundle::new(bytecode, metadata)
    }

    fn strict_sample_bundle() -> OxBundle {
        let manifest = ProjectManifest {
            project_name: "StrictBundle".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source(
                    "Main",
                    ModuleKind::Procedural,
                    "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nDim x As Long\nx = 42\nEnd Sub",
                )
                .expect("module"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = crate::project::compile_project(&manifest).expect("compile");
        OxBundle::from_compiled_project_with_manifest(&compiled, &manifest)
    }

    fn assert_legacy_bundle_rejected(version: u32, data: &[u8]) {
        let err = OxBundle::deserialize_from_bytes(data)
            .expect_err("legacy bundle version should be rejected");
        assert_eq!(err, unsupported_bundle_version_message(version));
    }

    fn legacy_v10_metadata(
        metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) -> BTreeMap<String, LegacyProcedureRuntimeMetadataV10> {
        metadata
            .into_iter()
            .map(|(name, metadata)| {
                (
                    name,
                    LegacyProcedureRuntimeMetadataV10 {
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
                        call_sites: metadata
                            .call_sites
                            .into_iter()
                            .map(legacy_call_site_v12)
                            .collect(),
                        array_shapes: metadata.array_shapes,
                        udt_types: metadata.udt_types,
                        object_types: metadata.object_types,
                    },
                )
            })
            .collect()
    }

    fn legacy_call_site_v12(call_site: CallSiteDescriptor) -> LegacyCallSiteDescriptorV12 {
        LegacyCallSiteDescriptorV12 {
            call_site_id: call_site.call_site_id,
            caller_procedure_name: call_site.caller_procedure_name,
            call_pc: call_site.call_pc,
            target_name: call_site.target_name,
            target_kind: call_site.target_kind,
            target_entry_pc: call_site.target_entry_pc,
            default_member_policy: call_site.default_member_policy,
            arguments: call_site.arguments,
            return_value: call_site.return_value,
        }
    }

    fn legacy_v12_metadata(
        metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) -> BTreeMap<String, LegacyProcedureRuntimeMetadataV12> {
        metadata
            .into_iter()
            .map(|(name, metadata)| {
                (
                    name,
                    LegacyProcedureRuntimeMetadataV12 {
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
                        call_sites: metadata
                            .call_sites
                            .into_iter()
                            .map(legacy_call_site_v12)
                            .collect(),
                        array_shapes: metadata.array_shapes,
                        udt_types: metadata.udt_types,
                        object_types: metadata.object_types,
                        carrier_layouts: metadata.carrier_layouts,
                        value_states: metadata.value_states,
                    },
                )
            })
            .collect()
    }

    fn legacy_v13_metadata(
        metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) -> BTreeMap<String, LegacyProcedureRuntimeMetadataV13> {
        metadata
            .into_iter()
            .map(|(name, metadata)| {
                (
                    name,
                    LegacyProcedureRuntimeMetadataV13 {
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
                        udt_types: metadata.udt_types,
                        object_types: metadata.object_types,
                        carrier_layouts: metadata.carrier_layouts,
                        value_states: metadata.value_states,
                    },
                )
            })
            .collect()
    }

    fn bundle_bytes_from_payload(version: u32, payload: &[u8]) -> Vec<u8> {
        let payload_len = payload.len() as u32;
        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&version.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let bundle = strict_sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");

        assert!(!restored.bytecode.instructions.is_empty());
        assert!(
            restored
                .procedure_metadata
                .values()
                .any(|metadata| { metadata.procedure_name.eq_ignore_ascii_case("Main") })
        );
        assert!(restored.manifest_snapshot.is_some());
        assert!(restored.export_inventory.is_some());
        assert!(restored.descriptor_inventory.is_some());
        assert!(restored.project_context.is_some());
    }

    #[test]
    fn content_digest_is_stable_and_section_sensitive() {
        let bundle = strict_sample_bundle();
        let digest = bundle.content_digest().expect("digest");
        // Deterministic: same bundle hashes identically.
        assert_eq!(digest, bundle.content_digest().expect("digest"));
        assert!(digest.starts_with("oxb-fnv1a64:"));

        // Changing a serialized fact changes the digest.
        let mut mutated = bundle.clone();
        if let Some(metadata) = mutated.procedure_metadata.values_mut().next() {
            metadata.entry_pc += 1;
        }
        assert_ne!(
            digest,
            mutated.content_digest().expect("digest"),
            "content digest must change when a serialized section changes"
        );
    }

    #[test]
    fn tampered_payload_byte_is_rejected() {
        let bundle = strict_sample_bundle();
        let mut bytes = bundle.serialize_to_bytes().expect("serialize");
        // Flip a byte inside the payload (past the 16-byte header).
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        let err = OxBundle::deserialize_from_bytes(&bytes)
            .expect_err("tampered payload must be rejected");
        assert!(
            err.contains("content digest mismatch"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn bytecode_empty_bundle_rejects_strict_sections() {
        let mut bundle = strict_sample_bundle();
        bundle.bytecode.instructions.clear();
        let err = bundle
            .serialize_to_bytes()
            .expect_err("empty bytecode must reject");
        assert!(err.contains("BUNDLE-STRICT-MISSING-SECTIONS"), "got: {err}");
        assert!(err.contains("bytecode"), "got: {err}");
    }

    #[test]
    fn current_roundtrip_preserves_call_site_array_udt_and_object_descriptors() {
        let source = "Type Point\n\
                      X As Long\n\
                      Caption As String\n\
                      End Type\n\
                      Sub Main()\n\
                      Dim x As Long\n\
                      Dim a(1 To 2) As Long\n\
                      Dim p As Point\n\
                      Dim o As Object\n\
                      x = 1\n\
                      a(1) = x\n\
                      p.X = x\n\
                      p.Caption = \"pt\"\n\
                      Call Touch(x)\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub";
        let manifest = ProjectManifest {
            project_name: "DescriptorRoundtrip".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Main", ModuleKind::Procedural, source).expect("module"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = crate::project::compile_project(&manifest).expect("compile should succeed");
        let bundle = OxBundle::from_compiled_project_with_manifest(&compiled, &manifest);
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");

        let main = restored
            .procedure_metadata
            .values()
            .find(|metadata| metadata.procedure_name.eq_ignore_ascii_case("Main"))
            .expect("main metadata");
        let call_sites = &main.call_sites;
        assert!(!call_sites.is_empty());
        let call_site = call_sites
            .iter()
            .find(|call| call.target_name.to_ascii_lowercase().ends_with("_touch"))
            .unwrap_or_else(|| panic!("Touch call site should be present: {call_sites:#?}"));
        assert_eq!(
            call_site.target_entry_pc,
            restored
                .procedure_metadata
                .values()
                .find(|metadata| metadata.procedure_name.eq_ignore_ascii_case("Touch"))
                .map(|metadata| metadata.entry_pc)
        );
        let array_shapes = &main.array_shapes;
        assert_eq!(array_shapes.len(), 1);
        assert_eq!(array_shapes[0].name, "a");
        assert_eq!(array_shapes[0].rank, 1);
        assert_eq!(array_shapes[0].bounds[0].lower_bound, 1);
        assert_eq!(array_shapes[0].bounds[0].upper_bound, 2);
        let udt_types = &main.udt_types;
        assert_eq!(udt_types.len(), 1);
        assert_eq!(udt_types[0].type_name, "point");
        assert_eq!(udt_types[0].instances[0].name, "p");
        assert_eq!(udt_types[0].fields.len(), 2);
        assert_eq!(udt_types[0].fields[0].name, "x");
        assert_eq!(udt_types[0].fields[1].name, "caption");
        let object_types = &main.object_types;
        assert_eq!(object_types.len(), 1);
        assert_eq!(object_types[0].descriptor_id, "object:object");
        assert_eq!(object_types[0].instances[0].name, "o");
    }

    #[test]
    fn v10_legacy_bundle_rejects_without_project_context() {
        let bundle = sample_bundle();
        let legacy = LegacyOxBundleV10 {
            bytecode: bundle.bytecode,
            procedure_metadata: legacy_v10_metadata(bundle.procedure_metadata),
            manifest_snapshot: bundle.manifest_snapshot,
            export_inventory: bundle.export_inventory,
            source_hashes: bundle.source_hashes,
            toolchain_fingerprint: bundle.toolchain_fingerprint,
            event_dispatch_bindings: bundle.event_dispatch_bindings,
            com_withevents_routes: bundle.com_withevents_routes,
            dynamic_object_routes: bundle.dynamic_object_routes,
            descriptor_inventory: bundle.descriptor_inventory,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v10");
        let data = bundle_bytes_from_payload(10, &payload);

        assert_legacy_bundle_rejected(10, &data);
    }

    #[test]
    fn v11_legacy_bundle_rejects_without_typed_value_descriptors() {
        let manifest = ProjectManifest {
            project_name: "CompatV11".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source(
                    "Main",
                    ModuleKind::Procedural,
                    "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nEnd Sub",
                )
                .expect("module"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = crate::project::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project_with_manifest(&compiled, &manifest);
        let legacy = LegacyOxBundleV11 {
            bytecode: bundle.bytecode,
            procedure_metadata: legacy_v10_metadata(bundle.procedure_metadata),
            manifest_snapshot: bundle.manifest_snapshot,
            export_inventory: bundle.export_inventory,
            source_hashes: bundle.source_hashes,
            toolchain_fingerprint: bundle.toolchain_fingerprint,
            event_dispatch_bindings: bundle.event_dispatch_bindings,
            com_withevents_routes: bundle.com_withevents_routes,
            dynamic_object_routes: bundle.dynamic_object_routes,
            descriptor_inventory: bundle.descriptor_inventory,
            project_context: bundle.project_context,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v11");
        let data = bundle_bytes_from_payload(11, &payload);

        assert_legacy_bundle_rejected(11, &data);
    }

    #[test]
    fn v12_legacy_bundle_rejects_without_call_policy_fields() {
        let source = "Sub Main()\n\
                      Dim value As Long\n\
                      Touch value\n\
                      Debug.Print Empty\n\
                      End Sub\n\
                      Sub Touch(ByRef value As Long)\n\
                      value = value + 1\n\
                      End Sub";
        let (bytecode, metadata) = crate::compile_with_runtime_metadata(source).expect("compile");
        let bundle = OxBundle::new(bytecode, metadata);
        let legacy = LegacyOxBundleV12 {
            bytecode: bundle.bytecode,
            procedure_metadata: legacy_v12_metadata(bundle.procedure_metadata),
            manifest_snapshot: bundle.manifest_snapshot,
            export_inventory: bundle.export_inventory,
            source_hashes: bundle.source_hashes,
            toolchain_fingerprint: bundle.toolchain_fingerprint,
            event_dispatch_bindings: bundle.event_dispatch_bindings,
            com_withevents_routes: bundle.com_withevents_routes,
            dynamic_object_routes: bundle.dynamic_object_routes,
            descriptor_inventory: bundle.descriptor_inventory,
            project_context: bundle.project_context,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v12");
        let data = bundle_bytes_from_payload(12, &payload);

        assert_legacy_bundle_rejected(12, &data);
    }

    #[test]
    fn v13_legacy_bundle_rejects_without_expression_descriptor_fields() {
        let source = "Sub Main()\n\
                      Dim value As Long\n\
                      value = 1\n\
                      Touch value\n\
                      End Sub\n\
                      Sub Touch(ByVal value As Double)\n\
                      End Sub";
        let (bytecode, metadata) = crate::compile_with_runtime_metadata(source).expect("compile");
        let bundle = OxBundle::new(bytecode, metadata);
        let legacy = LegacyOxBundleV13 {
            bytecode: bundle.bytecode,
            procedure_metadata: legacy_v13_metadata(bundle.procedure_metadata),
            manifest_snapshot: bundle.manifest_snapshot,
            export_inventory: bundle.export_inventory,
            source_hashes: bundle.source_hashes,
            toolchain_fingerprint: bundle.toolchain_fingerprint,
            event_dispatch_bindings: bundle.event_dispatch_bindings,
            com_withevents_routes: bundle.com_withevents_routes,
            dynamic_object_routes: bundle.dynamic_object_routes,
            descriptor_inventory: bundle.descriptor_inventory,
            project_context: bundle.project_context,
        };
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v13");
        let data = bundle_bytes_from_payload(13, &payload);

        assert_legacy_bundle_rejected(13, &data);
    }

    #[test]
    fn header_magic_is_correct() {
        let bundle = strict_sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        assert_eq!(&bytes[0..4], b"OXVB");
    }

    #[test]
    fn header_version_is_current_strict_version() {
        let bundle = strict_sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(version, FORMAT_VERSION);
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut bytes = strict_sample_bundle()
            .serialize_to_bytes()
            .expect("serialize");
        bytes[0] = b'X';
        assert!(OxBundle::deserialize_from_bytes(&bytes).is_err());
    }

    #[test]
    fn rejects_truncated_data() {
        let bytes = strict_sample_bundle()
            .serialize_to_bytes()
            .expect("serialize");
        assert!(OxBundle::deserialize_from_bytes(&bytes[..8]).is_err());
    }

    #[test]
    fn rejects_wrong_version() {
        let mut bytes = strict_sample_bundle()
            .serialize_to_bytes()
            .expect("serialize");
        bytes[4..8].copy_from_slice(&99u32.to_le_bytes());
        let err = OxBundle::deserialize_from_bytes(&bytes).expect_err("wrong version rejects");
        assert_eq!(err, unsupported_bundle_version_message(99));
    }

    #[test]
    fn previous_current_v14_bundle_rejects_deterministically() {
        let mut bytes = strict_sample_bundle()
            .serialize_to_bytes()
            .expect("serialize");
        bytes[4..8].copy_from_slice(&14u32.to_le_bytes());
        assert_legacy_bundle_rejected(14, &bytes);
    }

    #[test]
    fn v1_legacy_bundle_rejects() {
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

        assert_legacy_bundle_rejected(1, &data);
    }

    #[test]
    fn v2_legacy_bundle_rejects() {
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

        assert_legacy_bundle_rejected(2, &data);
    }

    #[test]
    fn v3_legacy_bundle_rejects_instead_of_upgrading_slot_descriptors() {
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

        assert_legacy_bundle_rejected(3, &data);
    }

    #[test]
    fn v4_legacy_bundle_rejects_instead_of_synthesizing_signature_descriptors() {
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

        assert_legacy_bundle_rejected(4, &data);
    }

    #[test]
    fn v5_legacy_bundle_rejects_instead_of_upgrading_signature_shape() {
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

        assert_legacy_bundle_rejected(5, &data);
    }

    #[test]
    fn v6_legacy_bundle_rejects_instead_of_adding_empty_call_site_rows() {
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

        assert_legacy_bundle_rejected(6, &data);
    }

    #[test]
    fn v7_legacy_bundle_rejects_instead_of_adding_empty_array_shape_rows() {
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
                        call_sites: metadata
                            .call_sites
                            .into_iter()
                            .map(legacy_call_site_v12)
                            .collect(),
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

        assert_legacy_bundle_rejected(7, &data);
    }

    #[test]
    fn v8_legacy_bundle_rejects_instead_of_adding_empty_udt_rows() {
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
                        call_sites: metadata
                            .call_sites
                            .into_iter()
                            .map(legacy_call_site_v12)
                            .collect(),
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

        assert_legacy_bundle_rejected(8, &data);
    }

    #[test]
    fn v9_legacy_bundle_rejects_instead_of_adding_empty_object_rows() {
        let source = "Sub Main()\n\
                      Dim o As Object\n\
                      End Sub";
        let (bytecode, metadata) =
            crate::compile_with_runtime_metadata(source).expect("compile should succeed");
        let procedure_metadata = metadata
            .into_iter()
            .map(|(name, metadata)| {
                (
                    name,
                    LegacyProcedureRuntimeMetadataV9 {
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
                        call_sites: metadata
                            .call_sites
                            .into_iter()
                            .map(legacy_call_site_v12)
                            .collect(),
                        array_shapes: metadata.array_shapes,
                        udt_types: metadata.udt_types,
                    },
                )
            })
            .collect();
        let legacy = LegacyOxBundleV9 {
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
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy).expect("serialize legacy v9");
        let payload_len = payload.len() as u32;

        let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
        data.extend_from_slice(&MAGIC);
        data.extend_from_slice(&9u32.to_le_bytes());
        data.extend_from_slice(&payload_len.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&payload);

        assert_legacy_bundle_rejected(9, &data);
    }

    #[test]
    fn current_roundtrip_with_populated_optional_fields() {
        let mut bundle = strict_sample_bundle();
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
        let manifest = ProjectManifest {
            project_name: "Roundtrip".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Main", ModuleKind::Procedural, source).expect("module"),
            ],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let compiled = crate::project::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project_with_manifest(&compiled, &manifest);
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
    fn from_compiled_project_with_manifest_persists_project_import_source_and_host_context() {
        let main_source = "Attribute VB_Name = \"Main\"\n\
Option Explicit\n\
Option Compare Text\n\
Option Base 1\n\
DefLng A-Z\n\
Private Declare PtrSafe Function GetTickCount Lib \"kernel32\" () As Long\n\
Public Sub Main()\n\
Dim p As LongPtr\n\
Dim q As LongLong\n\
Debug.Print GetTickCount()\n\
End Sub";
        let lib_source = "Attribute VB_Name = \"LibMod\"\nPublic Sub Helper()\nEnd Sub";
        let mut conditional_constants = BTreeMap::new();
        conditional_constants.insert("FeatureX".to_string(), -1);
        let manifest = ProjectManifest {
            project_name: "PkgCtx".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![
                module_unit_from_source("Main", ModuleKind::Procedural, main_source)
                    .expect("main module"),
            ],
            references: vec![
                ProjectReference {
                    referenced_project_name: "Lib".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
                ProjectReference {
                    referenced_project_name: "MissingLib".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
                ProjectReference {
                    referenced_project_name: "MissingTypeLib".to_string(),
                    reference_kind: ReferenceKind::TypeLibrary,
                },
            ],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Lib".to_string(),
                modules: vec![
                    module_unit_from_source("LibMod", ModuleKind::Procedural, lib_source)
                        .expect("lib module"),
                ],
            }],
            conditional_constants,
        };
        let compiled = crate::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project_with_manifest(&compiled, &manifest);
        let context = bundle.project_context.as_ref().expect("project context");

        assert_eq!(context.project_name, "PkgCtx");
        assert_eq!(context.project_kind, "Source");
        assert_eq!(context.modules.len(), 1);
        assert_eq!(context.modules[0].name, "Main");
        assert_eq!(context.modules[0].source_line_count, 11);
        assert!(context.modules[0].option_explicit);
        assert_eq!(context.modules[0].option_compare, "Text");
        assert_eq!(context.modules[0].option_base, 1);
        assert!(
            context.modules[0]
                .default_type_families
                .iter()
                .any(|family| family.start_letter == 'A'
                    && family.end_letter == 'Z'
                    && family.type_name == "Long")
        );
        assert_eq!(context.modules[0].external_declare_count, 1);
        assert_eq!(context.modules[0].ptrsafe_declare_count, 1);
        assert!(context.modules[0].uses_long_ptr);
        assert!(context.modules[0].uses_long_long);
        assert!(
            context
                .compile_context
                .manifest_conditional_constants
                .iter()
                .any(|constant| constant.name == "FeatureX" && constant.value == -1)
        );
        assert!(
            context
                .compile_context
                .builtin_conditional_constants
                .iter()
                .any(|constant| constant.name == "vba7" && constant.value == -1)
        );
        assert!(matches!(
            context.compile_context.target_pointer_width_bits,
            32 | 64
        ));
        assert_eq!(context.references.len(), 3);
        assert!(context.references.iter().any(|reference| {
            reference.name == "Lib" && reference.resolution == "project-source-present"
        }));
        assert!(context.references.iter().any(|reference| {
            reference.name == "MissingLib" && reference.resolution == "project-source-missing"
        }));
        assert!(context.references.iter().any(|reference| {
            reference.name == "MissingTypeLib"
                && reference.resolution == "typelib-projection-missing"
        }));
        assert_eq!(context.referenced_projects[0].project_name, "Lib");
        assert!(
            context
                .source_maps
                .iter()
                .any(|map| map.module_name == "Main")
        );
        assert!(context.native_libraries.iter().any(|native| {
            native.declared_name.eq_ignore_ascii_case("GetTickCount")
                && native.library.eq_ignore_ascii_case("kernel32")
        }));
        assert!(context.host_capabilities.iter().any(|capability| {
            capability.capability == "dynamic-linking"
                && capability.source.eq_ignore_ascii_case("GetTickCount")
        }));
        assert!(context.host_capabilities.iter().any(|capability| {
            capability.capability == "diagnostics-telemetry" && capability.source == "Debug.Print"
        }));
        assert!(
            context
                .package_diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "PKG-IMPORT-PROJECT-SOURCE-MISSING" })
        );
        assert!(
            context
                .package_diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "PKG-IMPORT-TYPELIB-PROJECTION-MISSING" })
        );

        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");
        let restored_context = restored.project_context.as_ref().expect("restored context");
        assert_eq!(restored_context.references, context.references);
        assert_eq!(restored_context.modules, context.modules);
        assert_eq!(restored_context.compile_context, context.compile_context);
        assert_eq!(restored_context.native_libraries, context.native_libraries);
        assert_eq!(
            restored_context.host_capabilities,
            context.host_capabilities
        );
    }

    #[test]
    fn incomplete_in_memory_bundle_reports_callable_inventory_unavailable() {
        let bundle = sample_bundle();
        assert_eq!(
            bundle
                .callable_descriptors()
                .expect_err("no descriptor inventory"),
            BundleDescriptorInventoryError::Unavailable
        );
    }

    #[test]
    fn incomplete_in_memory_bundle_cannot_serialize_as_current_strict_format() {
        let err = sample_bundle()
            .serialize_to_bytes()
            .expect_err("incomplete bundle must not serialize as current strict format");
        assert!(err.contains("BUNDLE-STRICT-MISSING-SECTIONS"));
        assert!(err.contains("manifest_snapshot"));
        assert!(err.contains("descriptor_inventory"));
        assert!(err.contains("project_context"));
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

    #[test]
    fn from_compiled_project_populates_com_class_exports() {
        let manifest = crate::project::ProjectManifest {
            project_name: "ExportProj".to_string(),
            project_kind: crate::project::ProjectKind::Library,
            modules: vec![
                crate::project::ModuleUnit {
                    module_name: "Main".to_string(),
                    module_kind: crate::project::ModuleKind::Procedural,
                    attributes: crate::project::ModuleAttributes {
                        vb_name: "Main".to_string(),
                        ..Default::default()
                    },
                    source: "Public Sub Go()\nEnd Sub".to_string(),
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
                    source: "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function".to_string(),
                },
                crate::project::ModuleUnit {
                    module_name: "Helper".to_string(),
                    module_kind: crate::project::ModuleKind::Class,
                    attributes: crate::project::ModuleAttributes {
                        vb_name: "Helper".to_string(),
                        vb_creatable: false,
                        vb_exposed: true,
                        ..Default::default()
                    },
                    source: "Public Sub Tick()\nEnd Sub".to_string(),
                },
            ],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: BTreeMap::new(),
        };

        let compiled = crate::compile_project(&manifest).expect("compile");
        let bundle = OxBundle::from_compiled_project(&compiled, "ExportProj");
        let exports = bundle
            .export_inventory
            .as_ref()
            .expect("export inventory")
            .com_class_exports
            .clone();
        // Only the exposed AND creatable class is exported.
        assert_eq!(exports.len(), 1, "got: {exports:#?}");
        assert_eq!(exports[0].class_name, "Widget");
        assert_eq!(exports[0].prog_id.as_deref(), Some("ExportProj.Widget"));

        // Round-trips through serialization.
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let restored = OxBundle::deserialize_from_bytes(&bytes).expect("deserialize");
        assert_eq!(
            restored
                .export_inventory
                .as_ref()
                .expect("restored export inventory")
                .com_class_exports
                .len(),
            1
        );
    }

    #[test]
    fn compile_source_to_bundle_produces_a_strict_complete_package() {
        let bundle = crate::compile_source_to_bundle("Sub Main()\nDim x As Long\nx = 1\nEnd Sub")
            .expect("snippet compiles to bundle");
        assert!(!bundle.bytecode.instructions.is_empty());
        assert!(bundle.manifest_snapshot.is_some());
        assert!(bundle.export_inventory.is_some());
        assert!(bundle.descriptor_inventory.is_some());
        assert!(bundle.project_context.is_some());
        // Strict completeness: serialize_to_bytes runs validate_strict_package_sections.
        bundle
            .serialize_to_bytes()
            .expect("complete in-memory package serializes as strict");
    }
}
