//! OxBundle: portable compiled bytecode container.
//!
//! Bundles a compiled `Bytecode` together with `ProcedureRuntimeMetadata`
//! into a single serializable unit that can be persisted to disk (.oxb files)
//! and later deserialized for execution.

use std::collections::BTreeMap;

use rkyv::{Archive, Deserialize, Serialize};

use crate::bytecode::Bytecode;
use crate::emit::ProcedureRuntimeMetadata;

/// Magic header bytes for the OxBundle binary format.
const MAGIC: [u8; 4] = *b"OXVB";
/// Current bundle format version.
const FORMAT_VERSION: u32 = 1;
/// Header size in bytes (padded to 16 for rkyv alignment).
const HEADER_SIZE: usize = 16;

/// Compiled bytecode bundle — the unit of persistence.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct OxBundle {
    /// Compiled bytecode (instructions, slot counts, external call descriptors).
    pub bytecode: Bytecode,
    /// Per-procedure metadata (entry points, parameter slots, return slots).
    pub procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
}

impl OxBundle {
    /// Create a new bundle from a compiled bytecode and its procedure metadata.
    pub fn new(
        bytecode: Bytecode,
        procedure_metadata: BTreeMap<String, ProcedureRuntimeMetadata>,
    ) -> Self {
        Self {
            bytecode,
            procedure_metadata,
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

        let payload_len = u32::try_from(payload.len())
            .map_err(|_| "bundle payload exceeds 4 GiB".to_string())?;

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved padding
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Deserialize a bundle from bytes produced by `serialize_to_bytes`.
    pub fn deserialize_from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < HEADER_SIZE {
            return Err("bundle too short for header".to_string());
        }

        // Validate magic.
        if &data[0..4] != &MAGIC {
            return Err("invalid bundle magic (expected OXVB)".to_string());
        }

        // Validate version.
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != FORMAT_VERSION {
            return Err(format!(
                "unsupported bundle version {version} (expected {FORMAT_VERSION})"
            ));
        }

        // Read payload length.
        let payload_len =
            u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
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

        let bundle: OxBundle = rkyv::from_bytes::<OxBundle, rkyv::rancor::Error>(&aligned)
            .map_err(|e| format!("deserialize: {e}"))?;
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{Bytecode, Instruction};

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
    }

    #[test]
    fn header_magic_is_correct() {
        let bundle = sample_bundle();
        let bytes = bundle.serialize_to_bytes().expect("serialize");
        assert_eq!(&bytes[0..4], b"OXVB");
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
    fn compile_and_bundle_roundtrip() {
        let source = "Sub Main()\nDim x\nx = 1\nx = x + 2\nEnd Sub";
        let (bytecode, metadata) = crate::compile_with_runtime_metadata(source)
            .expect("compile");
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
}
