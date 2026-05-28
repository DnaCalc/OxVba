//! Canonical descriptor identity helpers for the executable semantic package.
//!
//! The VM and future JIT consumers must reference package-owned descriptor
//! identities. These helpers intentionally live in the compiler/package crate
//! rather than the VM so execution engines consume identity facts instead of
//! inventing them locally.

use std::fmt::Debug;

use crate::emit::{RuntimeCarrierKind, VbaTypeId};

const DESCRIPTOR_IDENTITY_VERSION: &str = "oxvba-descriptor-identity-v1";
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorFamily {
    Package,
    Bytecode,
    Procedure,
    Slot,
    ProcedureSignature,
    CallSite,
    ArrayShape,
    UdtType,
    ObjectType,
    Interop,
    Lifecycle,
    ErrorRouting,
    DeoptSnapshot,
    HostPolicy,
    CarrierLayout,
    ValueState,
    ExpressionSemantics,
    OperatorSemantics,
    Coercion,
    NameBinding,
    ObjectMemberBinding,
    TypeRegistry,
    DescriptorSet,
}

impl DescriptorFamily {
    pub fn registry_key(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Bytecode => "bytecode",
            Self::Procedure => "procedure",
            Self::Slot => "slot",
            Self::ProcedureSignature => "procedure-signature",
            Self::CallSite => "call-site",
            Self::ArrayShape => "array-shape",
            Self::UdtType => "udt-type",
            Self::ObjectType => "object-type",
            Self::Interop => "interop",
            Self::Lifecycle => "lifecycle",
            Self::ErrorRouting => "error-routing",
            Self::DeoptSnapshot => "deopt-snapshot",
            Self::HostPolicy => "host-policy",
            Self::CarrierLayout => "carrier-layout",
            Self::ValueState => "value-state",
            Self::ExpressionSemantics => "expression-semantics",
            Self::OperatorSemantics => "operator-semantics",
            Self::Coercion => "coercion",
            Self::NameBinding => "name-binding",
            Self::ObjectMemberBinding => "object-member-binding",
            Self::TypeRegistry => "type-registry",
            Self::DescriptorSet => "descriptor-set",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorIdentity {
    pub family: DescriptorFamily,
    pub descriptor_id: String,
    pub descriptor_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaTypeRegistryEntry {
    pub type_id: VbaTypeId,
    pub registry_key: &'static str,
    pub descriptor_id: String,
    pub default_carrier: RuntimeCarrierKind,
}

impl VbaTypeId {
    pub fn registry_key(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Boolean => "boolean",
            Self::Byte => "byte",
            Self::Integer => "integer",
            Self::Long => "long",
            Self::LongLong => "longlong",
            Self::LongPtr => "longptr",
            Self::Single => "single",
            Self::Double => "double",
            Self::Currency => "currency",
            Self::Date => "date",
            Self::String => "string",
            Self::Variant => "variant",
            Self::Object => "object",
            Self::Array => "array",
            Self::InteropAny => "interop-any",
        }
    }

    pub fn descriptor_id(self) -> String {
        canonical_descriptor_id(DescriptorFamily::TypeRegistry, [self.registry_key()])
    }
}

impl RuntimeCarrierKind {
    pub fn registry_key(&self) -> String {
        match self {
            Self::Unknown => "unknown".to_string(),
            Self::Variant => "variant".to_string(),
            Self::Boolean => "boolean".to_string(),
            Self::I16 => "i16".to_string(),
            Self::U8 => "u8".to_string(),
            Self::I32 => "i32".to_string(),
            Self::I64 => "i64".to_string(),
            Self::PointerSizedInteger => "pointer-sized-integer".to_string(),
            Self::F32 => "f32".to_string(),
            Self::F64 => "f64".to_string(),
            Self::Currency => "currency".to_string(),
            Self::Date => "date".to_string(),
            Self::BStr => "bstr".to_string(),
            Self::Decimal96VariantSubtype => "decimal96-variant-subtype".to_string(),
            Self::ObjectRef => "object-ref".to_string(),
            Self::SafeArray => "safearray".to_string(),
            Self::UdtFields { descriptor } => {
                format!("udt-fields:{}", escape_descriptor_component(descriptor))
            }
            Self::BindingHandleInternal => "binding-handle-internal".to_string(),
        }
    }
}

pub fn vba_type_registry() -> Vec<VbaTypeRegistryEntry> {
    [
        VbaTypeId::Unknown,
        VbaTypeId::Boolean,
        VbaTypeId::Byte,
        VbaTypeId::Integer,
        VbaTypeId::Long,
        VbaTypeId::LongLong,
        VbaTypeId::LongPtr,
        VbaTypeId::Single,
        VbaTypeId::Double,
        VbaTypeId::Currency,
        VbaTypeId::Date,
        VbaTypeId::String,
        VbaTypeId::Variant,
        VbaTypeId::Object,
        VbaTypeId::Array,
        VbaTypeId::InteropAny,
    ]
    .iter()
    .copied()
    .map(|type_id| VbaTypeRegistryEntry {
        type_id,
        registry_key: type_id.registry_key(),
        descriptor_id: type_id.descriptor_id(),
        default_carrier: RuntimeCarrierKind::for_declared_type(type_id),
    })
    .collect()
}

pub fn canonical_descriptor_id<'a>(
    family: DescriptorFamily,
    parts: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut id = family.registry_key().to_string();
    for part in parts {
        id.push(':');
        id.push_str(&escape_descriptor_component(
            &part.trim().to_ascii_lowercase(),
        ));
    }
    id
}

pub fn descriptor_digest_from_fields<'a>(
    family: DescriptorFamily,
    descriptor_id: &str,
    fields: impl IntoIterator<Item = (&'a str, String)>,
) -> String {
    let mut hash = FNV1A64_OFFSET;
    update_fnv1a64(&mut hash, DESCRIPTOR_IDENTITY_VERSION.as_bytes());
    update_hash_field(&mut hash, "family", family.registry_key());
    update_hash_field(&mut hash, "descriptor_id", descriptor_id);
    for (name, value) in fields {
        update_hash_field(&mut hash, name, &value);
    }
    finish_fnv1a64(hash)
}

pub fn descriptor_digest_debug<T: Debug>(
    family: DescriptorFamily,
    descriptor_id: &str,
    value: &T,
) -> String {
    descriptor_digest_from_fields(family, descriptor_id, [("debug", format!("{value:#?}"))])
}

pub fn descriptor_identity_debug<T: Debug>(
    family: DescriptorFamily,
    descriptor_id: String,
    value: &T,
) -> DescriptorIdentity {
    let descriptor_digest = descriptor_digest_debug(family, &descriptor_id, value);
    DescriptorIdentity {
        family,
        descriptor_id,
        descriptor_digest,
    }
}

fn update_fnv1a64(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn update_hash_field(hash: &mut u64, name: &str, value: &str) {
    update_fnv1a64(hash, b"\0field\0");
    update_fnv1a64(hash, name.as_bytes());
    update_fnv1a64(hash, b"\0len\0");
    update_fnv1a64(hash, value.len().to_string().as_bytes());
    update_fnv1a64(hash, b"\0value\0");
    update_fnv1a64(hash, value.as_bytes());
}

fn finish_fnv1a64(hash: u64) -> String {
    format!("fnv1a64:{hash:016x}")
}

fn escape_descriptor_component(component: &str) -> String {
    let mut escaped = String::new();
    for byte in component.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => escaped.push(byte as char),
            b' ' => escaped.push('_'),
            other => escaped.push_str(&format!("%{other:02x}")),
        }
    }
    if escaped.is_empty() {
        "_".to_string()
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vba_type_registry_ids_are_stable_and_distinct() {
        let entries = vba_type_registry();
        assert_eq!(entries.len(), 16);
        assert_eq!(VbaTypeId::Long.descriptor_id(), "type-registry:long");
        assert_eq!(VbaTypeId::String.descriptor_id(), "type-registry:string");
        assert_ne!(
            VbaTypeId::Long.descriptor_id(),
            VbaTypeId::LongPtr.descriptor_id()
        );
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.type_id == VbaTypeId::Double)
                .expect("double registry row")
                .default_carrier,
            RuntimeCarrierKind::F64
        );
    }

    #[test]
    fn descriptor_digest_changes_when_field_changes() {
        let descriptor_id =
            canonical_descriptor_id(DescriptorFamily::Slot, ["proc:main", "slot:1"]);
        let first = descriptor_digest_from_fields(
            DescriptorFamily::Slot,
            &descriptor_id,
            [
                ("declared_type", "long".to_string()),
                ("carrier", "i32".to_string()),
            ],
        );
        let second = descriptor_digest_from_fields(
            DescriptorFamily::Slot,
            &descriptor_id,
            [
                ("declared_type", "double".to_string()),
                ("carrier", "f64".to_string()),
            ],
        );
        assert_eq!(descriptor_id, "slot:proc%3amain:slot%3a1");
        assert_ne!(first, second);
        assert!(first.starts_with("fnv1a64:"));
    }
}
