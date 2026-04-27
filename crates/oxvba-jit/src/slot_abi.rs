//! JIT slot ABI.
//!
//! JIT slots are the runtime's canonical `VARIANT` carrier. The layout is the
//! COM layout used by `oxvba_runtime::Variant`: `VARTYPE` at offset 0, reserved
//! words at offsets 2/4/6, and the 8-byte union payload at offset 8.

#[cfg(test)]
use oxvba_runtime::RuntimeValue;
use oxvba_runtime::{VarType, Variant};

pub const VT_EMPTY: u16 = VarType::Empty as u16;
pub const VT_NULL: u16 = VarType::Null as u16;
pub const VT_I4: u16 = VarType::Long as u16;

/// Offset (in bytes) of the VARTYPE field within an RtSlot/VARIANT.
pub const SLOT_VTYPE_OFFSET: i32 = 0;
/// Offset (in bytes) of the VARIANT union payload field.
pub const SLOT_PAYLOAD_OFFSET: i32 = 8;
/// Total size of one VARIANT slot in bytes.
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

    /// Build an ABI slot from the legacy semantic value API.
    ///
    /// The retained JIT carrier is still `Variant`; this accepts
    /// `RuntimeValue` only for compatibility callers that have not migrated to
    /// `from_variant`.
    #[cfg(test)]
    pub fn from_runtime_value(value: &RuntimeValue) -> Self {
        match value {
            RuntimeValue::BindingHandle(handle) => Self::from_i32(handle.raw()),
            value => Self {
                variant: Variant::from_runtime_value(value),
            },
        }
    }

    /// Project the retained ABI slot back to the legacy semantic value API.
    ///
    /// New value-model call sites should use `variant` instead.
    #[cfg(test)]
    pub fn to_runtime_value(&self) -> RuntimeValue {
        self.variant
            .to_runtime_value()
            .expect("JIT VARIANT slot should carry a runtime-supported value")
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

/// Convert a legacy semantic value to a VARIANT-backed JIT slot.
///
/// `BindingHandle` is an internal non-VBA token, so it is projected to the same
/// Long carrier accepted by the WithEvents semantics bridge instead of becoming
/// a custom JIT storage tag. This is a compatibility wrapper around the retained
/// `RtSlot`/`Variant` carrier.
#[cfg(test)]
pub fn rtslot_from_runtime_value(value: &RuntimeValue) -> RtSlot {
    RtSlot::from_runtime_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_runtime::{CurrencyValue, F64Value, bstr::BStr};

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
        assert_eq!(slot.to_runtime_value(), RuntimeValue::I32(42));
    }

    #[test]
    fn scalar_roundtrip_f64_uses_vt_r8() {
        let expected = std::f64::consts::PI;
        let slot = rtslot_from_runtime_value(&RuntimeValue::F64(F64Value::from_f64(expected)));
        assert_eq!(slot.vtype(), VarType::Double);
        match slot.to_runtime_value() {
            RuntimeValue::F64(value) => assert_eq!(value.as_f64().to_bits(), expected.to_bits()),
            other => panic!("expected F64, got {other:?}"),
        }
    }

    #[test]
    fn heap_roundtrip_string_uses_vt_bstr() {
        let original = BStr::from("hello");
        let slot = rtslot_from_runtime_value(&RuntimeValue::String(original.clone()));
        assert_eq!(slot.vtype(), VarType::String);
        assert_eq!(slot.to_runtime_value(), RuntimeValue::String(original));
    }

    #[test]
    fn rtslot_from_runtime_value_roundtrips_supported_scalars() {
        let cases = vec![
            RuntimeValue::Empty,
            RuntimeValue::Null,
            RuntimeValue::I32(-1),
            RuntimeValue::Bool(true),
            RuntimeValue::ErrorCode(13),
            RuntimeValue::I64(i64::MAX),
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(12345)),
        ];
        for original in cases {
            let slot = rtslot_from_runtime_value(&original);
            let recovered = slot.to_runtime_value();
            assert_eq!(recovered, original, "roundtrip failed for {original:?}");
        }
    }

    #[test]
    fn binding_handle_projects_to_long_not_custom_variant_tag() {
        let slot = rtslot_from_runtime_value(&RuntimeValue::BindingHandle(7.into()));
        assert_eq!(slot.vtype(), VarType::Long);
        assert_eq!(slot.to_runtime_value(), RuntimeValue::I32(7));
    }

    #[test]
    fn variant_cell_pointer_exposes_actual_slot_storage() {
        let slot = rtslot_from_runtime_value(&RuntimeValue::String(BStr::from("ABC")));
        let pointer = slot.variant_cell_pointer();
        assert_ne!(pointer, 0);
        assert_eq!(pointer, (&slot as *const RtSlot) as usize as i64);
    }
}
