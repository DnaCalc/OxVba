//! OxBundle: portable compiled bytecode container.
//!
//! Bundles a compiled `Bytecode` together with `ProcedureRuntimeMetadata`
//! into a single serializable unit that can be persisted to disk (.oxb files)
//! and later deserialized for execution.

use std::collections::BTreeMap;

use rkyv::{Archive, Deserialize, Serialize};

use crate::bytecode::Bytecode;
use crate::emit::ProcedureRuntimeMetadata;
use crate::project::{HostProcedureExport, ProjectDynamicObjectRoute, ProjectEventDispatchBinding};

/// Magic header bytes for the OxBundle binary format.
const MAGIC: [u8; 4] = *b"OXVB";
/// Current bundle format version.
const FORMAT_VERSION: u32 = 2;
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
    pub dynamic_object_routes: Option<Vec<ProjectDynamicObjectRoute>>,
}

/// v1 bundle layout for backward-compatible deserialization.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
struct LegacyOxBundleV1 {
    bytecode: Bytecode,
    procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
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
            dynamic_object_routes: None,
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
            dynamic_object_routes: if compiled.project_dynamic_objects.is_empty() {
                None
            } else {
                Some(compiled.project_dynamic_objects.clone())
            },
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
        if version != 1 && version != 2 {
            return Err(format!(
                "unsupported bundle version {version} (expected 1 or 2)"
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
        } else {
            let bundle: OxBundle = rkyv::from_bytes::<OxBundle, rkyv::rancor::Error>(&aligned)
                .map_err(|e| format!("deserialize: {e}"))?;
            Ok(bundle)
        }
    }
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
                entry_pc: 0,
                param_slots: vec![],
                return_slot: None,
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
    fn header_version_is_2() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(version, 2);
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
}
