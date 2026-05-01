//! JIT slot ABI.
//!
//! JIT slots retain the runtime `Variant` carrier. The slot ABI is intentionally
//! VARIANT-shaped for efficient boundary materialization: `VARTYPE` at offset 0,
//! reserved words at offsets 2/4/6, and the 8-byte union payload at offset 8.

use oxvba_runtime::{VarType, Variant};

pub const VT_EMPTY: u16 = VarType::Empty as u16;
pub const VT_NULL: u16 = VarType::Null as u16;
pub const VT_I4: u16 = VarType::Long as u16;

/// Offset (in bytes) of the VARTYPE field within an RtSlot.
pub const SLOT_VTYPE_OFFSET: i32 = 0;
/// Offset (in bytes) of the Variant payload field.
pub const SLOT_PAYLOAD_OFFSET: i32 = 8;
/// Total size of one retained Variant slot in bytes.
pub const SLOT_SIZE: i32 = 16;

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtSlot {
    variant: Variant,
}

impl Default for RtSlot {
    fn default() -> Self {
        Self {
            variant: Variant::empty(),
        }
    }
}

impl RtSlot {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn null() -> Self {
        Self {
            variant: Variant::null(),
        }
    }

    pub fn from_i32(value: i32) -> Self {
        Self {
            variant: Variant::from_i32(value),
        }
    }

    pub fn variant(&self) -> &Variant {
        &self.variant
    }

    pub fn from_variant(variant: Variant) -> Self {
        Self { variant }
    }

    pub fn vtype(&self) -> VarType {
        self.variant.vtype()
    }

    pub fn payload_u64(&self) -> u64 {
        u64::from_le_bytes(self.variant.data_bytes())
    }

    pub fn variant_cell_pointer(&self) -> i64 {
        self.variant.as_variant_cell_ptr() as usize as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_runtime::bstr::BStr;

    #[test]
    fn rtslot_layout_is_windows_variant_layout() {
        assert_eq!(std::mem::size_of::<RtSlot>(), 16);
        assert_eq!(std::mem::align_of::<RtSlot>(), 8);
        assert_eq!(SLOT_VTYPE_OFFSET, 0);
        assert_eq!(SLOT_PAYLOAD_OFFSET, 8);
        assert_eq!(SLOT_SIZE, 16);
    }

    #[test]
    fn scalar_roundtrip_i32_uses_vt_i4() {
        let slot = RtSlot::from_i32(42);
        assert_eq!(slot.vtype(), VarType::Long);
        assert_eq!(slot.payload_u64() as i32, 42);
        assert_eq!(slot.variant(), &Variant::from_i32(42));
    }

    #[test]
    fn scalar_roundtrip_f64_uses_vt_r8() {
        let expected = std::f64::consts::PI;
        let slot = RtSlot::from_variant(Variant::from_f64(expected));
        assert_eq!(slot.vtype(), VarType::Double);
        assert_eq!(
            slot.variant().as_f64().map(f64::to_bits),
            Some(expected.to_bits())
        );
    }

    #[test]
    fn heap_roundtrip_string_uses_vt_bstr() {
        let original = BStr::from("hello");
        let slot = RtSlot::from_variant(Variant::from_string(original.clone()));
        assert_eq!(slot.vtype(), VarType::String);
        assert_eq!(slot.variant().as_bstr(), Some(original));
    }

    #[test]
    fn rtslot_from_variant_roundtrips_supported_scalars() {
        let cases = vec![
            Variant::empty(),
            Variant::null(),
            Variant::from_i32(-1),
            Variant::from_bool(true),
            Variant::from_error_code(13),
            Variant::from_i64(i64::MAX),
            Variant::from_currency_scaled_i64(12345),
        ];
        for original in cases {
            let slot = RtSlot::from_variant(original.clone());
            assert_eq!(
                slot.variant(),
                &original,
                "roundtrip failed for {original:?}"
            );
        }
    }

    #[test]
    fn binding_handle_projects_to_long_not_custom_variant_tag() {
        let slot = RtSlot::from_i32(7);
        assert_eq!(slot.vtype(), VarType::Long);
        assert_eq!(slot.variant(), &Variant::from_i32(7));
    }

    #[test]
    fn variant_cell_pointer_exposes_actual_slot_storage() {
        let slot = RtSlot::from_variant(Variant::from_string(BStr::from("ABC")));
        let pointer = slot.variant_cell_pointer();
        assert_ne!(pointer, 0);
        assert_eq!(pointer, (&slot as *const RtSlot) as usize as i64);
    }

    #[test]
    fn malformed_pointer_slot_stays_in_variant_carrier() {
        let object_slot = RtSlot::from_variant(Variant::zeroed(VarType::Object));
        assert_eq!(object_slot.vtype(), VarType::Object);
        assert!(object_slot.variant().as_object_ref().is_none());

        let array_slot = RtSlot::from_variant(Variant::zeroed(VarType::ArrayVariant));
        assert_eq!(array_slot.vtype(), VarType::ArrayVariant);
        assert!(array_slot.variant().as_safearray().is_none());
    }
}
