//! OxBundle: portable compiled bytecode container.
//!
//! Bundles a compiled `Bytecode` together with `ProcedureRuntimeMetadata`
//! into a single serializable unit that can be persisted to disk (.oxb files)
//! and later deserialized for execution.

use std::collections::BTreeMap;

use rkyv::{Archive, Deserialize, Serialize};

use crate::bytecode::Bytecode;
use crate::emit::ProcedureRuntimeMetadata;
use crate::project::{
    HostProcedureExport, ProjectComWithEventsRoute, ProjectDynamicMemberKind,
    ProjectDynamicObjectRoute, ProjectEventDispatchBinding,
};

/// Magic header bytes for the OxBundle binary format.
const MAGIC: [u8; 4] = *b"OXVB";
/// Current bundle format version.
const FORMAT_VERSION: u32 = 3;
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
    pub host_calls: Vec<BundleHostCallDescriptor>,
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
pub struct BundleHostCallDescriptor {
    pub stable_host_call_id: String,
    pub project_name: String,
    pub module_name: String,
    pub procedure_name: String,
    pub kind: crate::project::ExportKind,
    pub selection_policy: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub argument_descriptions: Vec<Option<String>>,
    pub volatile: bool,
    pub dependency_policy: String,
    pub side_effect_policy: String,
    pub thread_safety_policy: String,
    pub allowed_contexts: Vec<String>,
    pub entry_pc: usize,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
    pub param_types: Vec<crate::bytecode::DeclareParamType>,
    pub return_type: Option<crate::bytecode::DeclareParamType>,
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

/// Compiled bytecode bundle — the unit of persistence (format v2).
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct OxBundle {
    /// Compiled bytecode (instructions, slot counts, external call descriptors).
    pub bytecode: Bytecode,
    /// Per-procedure metadata (entry points, parameter slots, return slots).
    pub procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    /// v2 fields — all optional for backward compat with v1 bundles.
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
    procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
}

/// v2 bundle layout for backward-compatible deserialization.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV2 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    manifest_snapshot: Option<ManifestSnapshot>,
    export_inventory: Option<ExportInventory>,
    source_hashes: Option<BTreeMap<String, [u8; 32]>>,
    toolchain_fingerprint: Option<ToolchainFingerprint>,
    event_dispatch_bindings: Option<Vec<ProjectEventDispatchBinding>>,
    com_withevents_routes: Option<Vec<ProjectComWithEventsRoute>>,
    dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
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
    /// Accepts format versions 1 and 2. Version 1 bundles are upgraded to v2
    /// with all new fields set to `None`.
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
        if version != 1 && version != 2 && version != 3 {
            return Err(format!(
                "unsupported bundle version {version} (expected 1, 2, or 3)"
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
            Ok(OxBundle::new(legacy.bytecode, legacy.procedure_metadata))
        } else if version == 2 {
            let legacy: LegacyOxBundleV2 =
                rkyv::from_bytes::<LegacyOxBundleV2, rkyv::rancor::Error>(&aligned)
                    .map_err(|e| format!("deserialize v2: {e}"))?;
            Ok(OxBundle {
                bytecode: legacy.bytecode,
                procedure_metadata: legacy.procedure_metadata,
                manifest_snapshot: legacy.manifest_snapshot,
                export_inventory: legacy.export_inventory,
                source_hashes: legacy.source_hashes,
                toolchain_fingerprint: legacy.toolchain_fingerprint,
                event_dispatch_bindings: legacy.event_dispatch_bindings,
                com_withevents_routes: legacy.com_withevents_routes,
                dynamic_object_routes: legacy.dynamic_object_routes,
                descriptor_inventory: None,
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

    let host_calls = compiled
        .host_exports
        .iter()
        .map(|export| {
            host_call_descriptor_from_export(export, &compiled.procedure_runtime_metadata)
        })
        .collect::<Vec<_>>();

    if com_classes.is_empty() && com_events.is_empty() && host_calls.is_empty() {
        None
    } else {
        Some(DescriptorInventory {
            com_classes,
            com_events,
            host_calls,
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

fn host_call_descriptor_from_export(
    export: &HostProcedureExport,
    metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> BundleHostCallDescriptor {
    let procedure_metadata = metadata
        .get(&format!(
            "{}.{}",
            export.module_name.to_ascii_lowercase(),
            export.procedure_name.to_ascii_lowercase()
        ))
        .or_else(|| {
            metadata.values().find(|candidate| {
                candidate
                    .module_name
                    .eq_ignore_ascii_case(&export.module_name)
                    && candidate
                        .procedure_name
                        .eq_ignore_ascii_case(&export.procedure_name)
            })
        });
    let argument_descriptions = procedure_metadata
        .map(argument_descriptions_from_metadata)
        .unwrap_or_default();
    BundleHostCallDescriptor {
        stable_host_call_id: stable_id([
            "host-call",
            &export.project_name,
            &export.module_name,
            &export.procedure_name,
            &format!("{:?}", export.kind),
        ]),
        project_name: export.project_name.clone(),
        module_name: export.module_name.clone(),
        procedure_name: export.procedure_name.clone(),
        kind: export.kind,
        selection_policy: "public-procedural-functions".to_string(),
        category: None,
        description: None,
        argument_descriptions,
        volatile: false,
        dependency_policy: "explicit-arguments-only".to_string(),
        side_effect_policy: "no-host-side-effects".to_string(),
        thread_safety_policy: "single-threaded-vba-compatible".to_string(),
        allowed_contexts: vec![
            "worksheet-cell".to_string(),
            "host-formula-evaluator".to_string(),
        ],
        entry_pc: procedure_metadata.map(|meta| meta.entry_pc).unwrap_or(0),
        param_slots: procedure_metadata
            .map(|meta| meta.param_slots.clone())
            .unwrap_or_default(),
        return_slot: procedure_metadata.and_then(|meta| meta.return_slot),
        param_types: procedure_metadata
            .map(|meta| meta.param_types.clone())
            .unwrap_or_default(),
        return_type: procedure_metadata.and_then(|meta| meta.return_type),
    }
}

fn argument_descriptions_from_metadata(meta: &ProcedureRuntimeMetadata) -> Vec<Option<String>> {
    meta.param_slots
        .iter()
        .map(|param_slot| {
            meta.slots
                .iter()
                .find(|slot| slot.slot == *param_slot)
                .and_then(|slot| {
                    let name = slot.name.trim();
                    (!name.is_empty()).then(|| name.to_string())
                })
        })
        .collect()
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
    use crate::bytecode::{Bytecode, Instruction};
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
                slots: vec![],
                param_slots: vec![],
                return_slot: None,
                param_types: vec![],
                return_type: None,
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
    fn header_magic_is_correct() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        assert_eq!(&bytes[0..4], b"OXVB");
    }

    #[test]
    fn header_version_is_3() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(version, 3);
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
                .host_calls
                .iter()
                .any(|call| call.procedure_name.eq_ignore_ascii_case("HostAdd")
                    && call.param_slots.len() == 2)
        );
        let host_add = inventory
            .host_calls
            .iter()
            .find(|call| call.procedure_name.eq_ignore_ascii_case("HostAdd"))
            .expect("HostAdd host-call descriptor");
        assert_eq!(host_add.kind, ExportKind::Function);
        assert!(!host_add.stable_host_call_id.is_empty());
        assert_eq!(host_add.selection_policy, "public-procedural-functions");
        assert_eq!(
            host_add.argument_descriptions,
            vec![Some("a".to_string()), Some("b".to_string())]
        );
        assert!(!host_add.volatile);
        assert_eq!(host_add.dependency_policy, "explicit-arguments-only");
        assert_eq!(host_add.side_effect_policy, "no-host-side-effects");
        assert_eq!(
            host_add.thread_safety_policy,
            "single-threaded-vba-compatible"
        );
        assert_eq!(
            host_add.allowed_contexts,
            vec![
                "worksheet-cell".to_string(),
                "host-formula-evaluator".to_string()
            ]
        );
        assert_eq!(host_add.param_types.len(), 2);
        assert!(host_add.return_type.is_some());

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
            restored_inventory.host_calls.len(),
            inventory.host_calls.len()
        );
        let restored_host_add = restored_inventory
            .host_calls
            .iter()
            .find(|call| call.procedure_name.eq_ignore_ascii_case("HostAdd"))
            .expect("restored HostAdd host-call descriptor");
        assert_eq!(
            restored_host_add.selection_policy,
            host_add.selection_policy
        );
        assert_eq!(
            restored_host_add.argument_descriptions,
            host_add.argument_descriptions
        );
        assert_eq!(
            restored_host_add.allowed_contexts,
            host_add.allowed_contexts
        );
    }
}
