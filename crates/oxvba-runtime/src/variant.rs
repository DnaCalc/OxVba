use crate::{
    Decimal96,
    bstr::{BStr, OwnedBStrCore},
    runtime_value::{BindingHandle, CurrencyValue, F64Subtype, F64Value, ObjectHandle, RuntimeValue},
    safe_array::SafeArray,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum VarType {
    Empty = 0x0000,
    Null = 0x0001,
    Integer = 0x0002,
    Long = 0x0003,
    Single = 0x0004,
    Double = 0x0005,
    Currency = 0x0006,
    Date = 0x0007,
    String = 0x0008,
    Object = 0x0009,
    Error = 0x000A,
    Boolean = 0x000B,
    Decimal = 0x000E,
    Byte = 0x0011,
    LongLong = 0x0014,
}

impl VarType {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0000 => Some(Self::Empty),
            0x0001 => Some(Self::Null),
            0x0002 => Some(Self::Integer),
            0x0003 => Some(Self::Long),
            0x0004 => Some(Self::Single),
            0x0005 => Some(Self::Double),
            0x0006 => Some(Self::Currency),
            0x0007 => Some(Self::Date),
            0x0008 => Some(Self::String),
            0x0009 => Some(Self::Object),
            0x000A => Some(Self::Error),
            0x000B => Some(Self::Boolean),
            0x000E => Some(Self::Decimal),
            0x0011 => Some(Self::Byte),
            0x0014 => Some(Self::LongLong),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VariantData {
    pub bytes: [u8; 8],
    pub i16_val: i16,
    pub i32_val: i32,
    pub i64_val: i64,
    pub f64_val: f64,
    pub ptr_val: *mut core::ffi::c_void,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VariantCore {
    pub vtype: VarType,
    pub reserved1: u16,
    pub reserved2: u16,
    pub reserved3: u16,
    pub data: VariantData,
}

impl core::fmt::Debug for VariantCore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VariantCore")
            .field("vtype", &self.vtype)
            .field("reserved1", &self.reserved1)
            .field("reserved2", &self.reserved2)
            .field("reserved3", &self.reserved3)
            .field("data", &self.data_bytes())
            .finish()
    }
}

impl PartialEq for VariantCore {
    fn eq(&self, other: &Self) -> bool {
        self.vtype == other.vtype
            && self.reserved1 == other.reserved1
            && self.reserved2 == other.reserved2
            && self.reserved3 == other.reserved3
            && self.data_bytes() == other.data_bytes()
    }
}

impl Eq for VariantCore {}

impl VariantCore {
    fn from_bytes(vtype: VarType, bytes: [u8; 8]) -> Self {
        Self {
            vtype,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            data: VariantData { bytes },
        }
    }

    pub fn data_bytes(&self) -> [u8; 8] {
        unsafe { self.data.bytes }
    }

    pub fn to_wire_bytes(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..2].copy_from_slice(&(self.vtype as u16).to_le_bytes());
        out[2..4].copy_from_slice(&self.reserved1.to_le_bytes());
        out[4..6].copy_from_slice(&self.reserved2.to_le_bytes());
        out[6..8].copy_from_slice(&self.reserved3.to_le_bytes());
        out[8..16].copy_from_slice(&self.data_bytes());
        out
    }

    pub fn from_wire_bytes(bytes: [u8; 16]) -> Result<Self, String> {
        let vtype_raw = u16::from_le_bytes([bytes[0], bytes[1]]);
        let Some(vtype) = VarType::from_u16(vtype_raw) else {
            return Err(format!("unsupported VARENUM value: 0x{vtype_raw:04X}"));
        };
        let reserved1 = u16::from_le_bytes([bytes[2], bytes[3]]);
        let reserved2 = u16::from_le_bytes([bytes[4], bytes[5]]);
        let reserved3 = u16::from_le_bytes([bytes[6], bytes[7]]);
        let mut payload = [0u8; 8];
        payload.copy_from_slice(&bytes[8..16]);
        Ok(Self {
            vtype,
            reserved1,
            reserved2,
            reserved3,
            data: VariantData { bytes: payload },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OwnedVariantData {
    String { text: BStr, core: OwnedBStrCore },
    ArrayIntent(SafeArray),
    ObjectHandle(ObjectHandle),
    BindingHandle(BindingHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    core: VariantCore,
    owned: Option<OwnedVariantData>,
}

impl Variant {
    fn from_core(core: VariantCore) -> Self {
        Self { core, owned: None }
    }

    pub fn zeroed(vtype: VarType) -> Self {
        Self::from_core(VariantCore::from_bytes(vtype, [0; 8]))
    }

    pub fn vtype(&self) -> VarType {
        self.core.vtype
    }

    pub fn core(&self) -> VariantCore {
        self.core
    }

    pub fn reserved1(&self) -> u16 {
        self.core.reserved1
    }

    pub fn reserved2(&self) -> u16 {
        self.core.reserved2
    }

    pub fn reserved3(&self) -> u16 {
        self.core.reserved3
    }

    pub fn data_bytes(&self) -> [u8; 8] {
        self.core.data_bytes()
    }

    pub fn to_wire_bytes(&self) -> [u8; 16] {
        self.core.to_wire_bytes()
    }

    pub fn from_wire_bytes(bytes: [u8; 16]) -> Result<Self, String> {
        Ok(Self::from_core(VariantCore::from_wire_bytes(bytes)?))
    }

    pub fn empty() -> Self {
        Self::from_core(VariantCore::from_bytes(VarType::Empty, [0; 8]))
    }

    pub fn null() -> Self {
        Self::from_core(VariantCore::from_bytes(VarType::Null, [0; 8]))
    }

    pub fn from_i16(value: i16) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&value.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::Integer, bytes))
    }

    pub fn as_i16(&self) -> Option<i16> {
        if self.vtype() != VarType::Integer {
            return None;
        }
        let bytes = self.data_bytes();
        Some(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn from_i32(value: i32) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::Long, bytes))
    }

    pub fn as_i32(&self) -> Option<i32> {
        if self.vtype() != VarType::Long {
            return None;
        }
        let bytes = self.data_bytes();
        Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn from_i64(value: i64) -> Self {
        Self::from_core(VariantCore::from_bytes(VarType::LongLong, value.to_le_bytes()))
    }

    pub fn as_i64(&self) -> Option<i64> {
        if self.vtype() != VarType::LongLong {
            return None;
        }
        Some(i64::from_le_bytes(self.data_bytes()))
    }

    pub fn from_f32(value: f32) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::Single, bytes))
    }

    pub fn as_f32(&self) -> Option<f32> {
        if self.vtype() != VarType::Single {
            return None;
        }
        let bytes = self.data_bytes();
        Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn from_f64(value: f64) -> Self {
        Self::from_core(VariantCore::from_bytes(VarType::Double, value.to_le_bytes()))
    }

    pub fn as_f64(&self) -> Option<f64> {
        if self.vtype() != VarType::Double {
            return None;
        }
        Some(f64::from_le_bytes(self.data_bytes()))
    }

    pub fn from_currency_scaled_i64(value: i64) -> Self {
        Self::from_core(VariantCore::from_bytes(VarType::Currency, value.to_le_bytes()))
    }

    pub fn as_currency_scaled_i64(&self) -> Option<i64> {
        if self.vtype() != VarType::Currency {
            return None;
        }
        Some(i64::from_le_bytes(self.data_bytes()))
    }

    pub fn from_decimal96(value: Decimal96) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.lo.to_le_bytes());
        bytes[4..8].copy_from_slice(&value.mid.to_le_bytes());
        Self::from_core(VariantCore {
            vtype: VarType::Decimal,
            reserved1: value.scale_sign,
            reserved2: (value.hi & 0xFFFF) as u16,
            reserved3: (value.hi >> 16) as u16,
            data: VariantData { bytes },
        })
    }

    pub fn as_decimal96(&self) -> Option<Decimal96> {
        if self.vtype() != VarType::Decimal {
            return None;
        }
        let bytes = self.data_bytes();
        let lo = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let mid = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let hi = u32::from(self.reserved2()) | (u32::from(self.reserved3()) << 16);
        Some(Decimal96::from_scale_sign(lo, mid, hi, self.reserved1()))
    }

    pub fn from_date_f64(value: f64) -> Self {
        Self::from_core(VariantCore::from_bytes(VarType::Date, value.to_le_bytes()))
    }

    pub fn as_date_f64(&self) -> Option<f64> {
        if self.vtype() != VarType::Date {
            return None;
        }
        Some(f64::from_le_bytes(self.data_bytes()))
    }

    pub fn from_bool(value: bool) -> Self {
        let mut bytes = [0u8; 8];
        let vb_bool: i16 = if value { -1 } else { 0 };
        bytes[0..2].copy_from_slice(&vb_bool.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::Boolean, bytes))
    }

    pub fn as_bool(&self) -> Option<bool> {
        if self.vtype() != VarType::Boolean {
            return None;
        }
        let bytes = self.data_bytes();
        let v = i16::from_le_bytes([bytes[0], bytes[1]]);
        Some(v != 0)
    }

    pub fn from_u8(value: u8) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0] = value;
        Self::from_core(VariantCore::from_bytes(VarType::Byte, bytes))
    }

    pub fn as_u8(&self) -> Option<u8> {
        if self.vtype() != VarType::Byte {
            return None;
        }
        Some(self.data_bytes()[0])
    }

    pub fn from_error_code(code: i32) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&code.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::Error, bytes))
    }

    pub fn as_error_code(&self) -> Option<i32> {
        if self.vtype() != VarType::Error {
            return None;
        }
        let bytes = self.data_bytes();
        Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn from_string(value: impl Into<BStr>) -> Self {
        let text = value.into();
        let core_view = text.owned_core();
        let mut bytes = [0u8; 8];
        let ptr = (core_view.payload_ptr() as usize as u64).to_le_bytes();
        bytes.copy_from_slice(&ptr);
        Self {
            core: VariantCore::from_bytes(VarType::String, bytes),
            owned: Some(OwnedVariantData::String {
                text,
                core: core_view,
            }),
        }
    }

    pub fn as_bstr(&self) -> Option<&BStr> {
        match &self.owned {
            Some(OwnedVariantData::String { text, .. }) if self.vtype() == VarType::String => {
                Some(text)
            }
            _ => None,
        }
    }

    pub fn string_core(&self) -> Option<&OwnedBStrCore> {
        match &self.owned {
            Some(OwnedVariantData::String { core, .. }) if self.vtype() == VarType::String => {
                Some(core)
            }
            _ => None,
        }
    }

    pub fn from_runtime_value(value: &RuntimeValue) -> Self {
        match value {
            RuntimeValue::Empty => Self::empty(),
            RuntimeValue::Null => Self::null(),
            RuntimeValue::ErrorCode(code) => Self::from_error_code(*code),
            RuntimeValue::I32(value) => Self::from_i32(*value),
            RuntimeValue::I64(value) => Self::from_i64(*value),
            RuntimeValue::F64(value) => match value.subtype() {
                F64Subtype::Single => Self::from_f32(value.as_f64() as f32),
                F64Subtype::Double => Self::from_f64(value.as_f64()),
                F64Subtype::Date => Self::from_date_f64(value.as_f64()),
            },
            RuntimeValue::Decimal(value) => Self::from_decimal96(*value),
            RuntimeValue::Currency(value) => Self::from_currency_scaled_i64(value.scaled_i64()),
            RuntimeValue::Bool(value) => Self::from_bool(*value),
            RuntimeValue::String(value) => Self::from_string(value.clone()),
            RuntimeValue::ArrayIntent(array) => Self {
                core: VariantCore::from_bytes(VarType::Empty, [0; 8]),
                owned: Some(OwnedVariantData::ArrayIntent(array.clone())),
            },
            RuntimeValue::ObjectHandle(handle) => Self {
                core: VariantCore::from_bytes(VarType::Object, [0; 8]),
                owned: Some(OwnedVariantData::ObjectHandle(*handle)),
            },
            RuntimeValue::BindingHandle(handle) => Self {
                core: VariantCore::from_bytes(VarType::Object, [0; 8]),
                owned: Some(OwnedVariantData::BindingHandle(*handle)),
            },
        }
    }

    pub fn from_compat_slot_i32(value: i32) -> Self {
        Self::from_runtime_value(&RuntimeValue::from_compat_slot_i32(value))
    }

    pub fn to_runtime_value(&self) -> Result<RuntimeValue, String> {
        match self.vtype() {
            VarType::Empty => {
                if let Some(OwnedVariantData::ArrayIntent(array)) = &self.owned {
                    return Ok(RuntimeValue::ArrayIntent(array.clone()));
                }
                Ok(RuntimeValue::Empty)
            }
            VarType::Null => Ok(RuntimeValue::Null),
            VarType::Integer => self
                .as_i16()
                .map(|value| RuntimeValue::I32(value as i32))
                .ok_or_else(|| "invalid Integer variant payload".to_string()),
            VarType::Long => self
                .as_i32()
                .map(RuntimeValue::I32)
                .ok_or_else(|| "invalid Long variant payload".to_string()),
            VarType::LongLong => self
                .as_i64()
                .map(RuntimeValue::I64)
                .ok_or_else(|| "invalid LongLong variant payload".to_string()),
            VarType::Single => self
                .as_f32()
                .map(|value| RuntimeValue::F64(F64Value::from_single_f64(value as f64)))
                .ok_or_else(|| "invalid Single variant payload".to_string()),
            VarType::Double => self
                .as_f64()
                .map(|value| RuntimeValue::F64(F64Value::from_f64(value)))
                .ok_or_else(|| "invalid Double variant payload".to_string()),
            VarType::Decimal => self
                .as_decimal96()
                .map(RuntimeValue::Decimal)
                .ok_or_else(|| "invalid Decimal variant payload".to_string()),
            VarType::Currency => self
                .as_currency_scaled_i64()
                .map(|value| RuntimeValue::Currency(CurrencyValue::from_scaled_i64(value)))
                .ok_or_else(|| "invalid Currency variant payload".to_string()),
            VarType::Date => self
                .as_date_f64()
                .map(|value| RuntimeValue::F64(F64Value::from_date_f64(value)))
                .ok_or_else(|| "invalid Date variant payload".to_string()),
            VarType::String => self
                .as_bstr()
                .cloned()
                .map(RuntimeValue::String)
                .ok_or_else(|| {
                    "string Variant reconstructed from wire bytes does not own string payload"
                        .to_string()
                }),
            VarType::Boolean => self
                .as_bool()
                .map(RuntimeValue::Bool)
                .ok_or_else(|| "invalid Boolean variant payload".to_string()),
            VarType::Error => self
                .as_error_code()
                .map(RuntimeValue::ErrorCode)
                .ok_or_else(|| "invalid Error variant payload".to_string()),
            VarType::Object => match &self.owned {
                Some(OwnedVariantData::ObjectHandle(handle)) => Ok(RuntimeValue::ObjectHandle(*handle)),
                Some(OwnedVariantData::BindingHandle(handle)) => {
                    Ok(RuntimeValue::BindingHandle(*handle))
                }
                _ => Err(
                    "object Variant reconstructed from wire bytes does not own interface identity"
                        .to_string(),
                ),
            },
            other => Err(format!(
                "runtime Variant -> RuntimeValue bridge does not yet support {:?}",
                other
            )),
        }
    }

    pub fn project_compat_slot_i32(&self) -> Result<i32, String> {
        self.to_runtime_value()?.project_compat_slot_i32()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CurrencyValue, Decimal96, F64Value, RuntimeValue, bstr::BStr, safe_array::SafeArray,
    };

    use super::{VarType, Variant, VariantCore, VariantData};

    #[test]
    fn numeric_roundtrip() {
        let i16v = Variant::from_i16(-12);
        assert_eq!(i16v.as_i16(), Some(-12));

        let i32v = Variant::from_i32(1024);
        assert_eq!(i32v.as_i32(), Some(1024));

        let i64v = Variant::from_i64(5_000_000_000);
        assert_eq!(i64v.as_i64(), Some(5_000_000_000));

        let f32v = Variant::from_f32(3.5);
        assert_eq!(f32v.as_f32(), Some(3.5));

        let f64v = Variant::from_f64(3.5);
        assert_eq!(f64v.as_f64(), Some(3.5));

        let cyv = Variant::from_currency_scaled_i64(125_000);
        assert_eq!(cyv.as_currency_scaled_i64(), Some(125_000));

        let decv = Variant::from_decimal96(Decimal96::from_parts(123_450, 0, 0, 3, true));
        assert_eq!(
            decv.as_decimal96(),
            Some(Decimal96::from_parts(123_450, 0, 0, 3, true))
        );

        let datev = Variant::from_date_f64(45200.25);
        assert_eq!(datev.as_date_f64(), Some(45200.25));
    }

    #[test]
    fn string_roundtrip_preserves_owned_payload_and_pointer_core() {
        let value = Variant::from_string("abc");
        assert_eq!(value.vtype(), VarType::String);
        assert_eq!(value.as_bstr(), Some(&BStr::from("abc")));
        assert!(value.string_core().is_some());
        assert_ne!(u64::from_le_bytes(value.data_bytes()), 0);
    }

    #[test]
    fn boolean_roundtrip_vba_encoding() {
        let t = Variant::from_bool(true);
        let f = Variant::from_bool(false);
        assert_eq!(t.vtype(), VarType::Boolean);
        assert_eq!(f.vtype(), VarType::Boolean);
        assert_eq!(t.as_bool(), Some(true));
        assert_eq!(f.as_bool(), Some(false));
    }

    #[test]
    fn com_variant_layout_shape() {
        assert_eq!(core::mem::size_of::<VariantCore>(), 16);
        assert_eq!(core::mem::size_of::<VariantData>(), 8);
        assert!(core::mem::size_of::<Variant>() >= 16);
    }

    #[test]
    fn com_variant_wire_roundtrip_for_numeric_value() {
        let original = Variant::from_i32(42);
        let wire = original.to_wire_bytes();
        let roundtrip = Variant::from_wire_bytes(wire).expect("wire roundtrip");
        assert_eq!(roundtrip.vtype(), VarType::Long);
        assert_eq!(roundtrip.as_i32(), Some(42));
    }

    #[test]
    fn single_variant_bridges_to_runtime_f64_lane() {
        let single_variant = Variant::from_f32(12.5);
        assert_eq!(single_variant.vtype(), VarType::Single);
        assert_eq!(
            single_variant
                .to_runtime_value()
                .expect("single Variant should bridge into RuntimeValue::F64"),
            RuntimeValue::F64(F64Value::from_single_f64(12.5))
        );
    }

    #[test]
    fn date_variant_bridges_to_runtime_f64_lane() {
        let date_variant = Variant::from_date_f64(45200.25);
        assert_eq!(date_variant.vtype(), VarType::Date);
        assert_eq!(
            date_variant
                .to_runtime_value()
                .expect("date Variant should bridge into RuntimeValue::F64"),
            RuntimeValue::F64(F64Value::from_date_f64(45200.25))
        );
    }

    #[test]
    fn currency_variant_bridges_to_runtime_currency_lane() {
        let currency_variant = Variant::from_currency_scaled_i64(125_000);
        assert_eq!(currency_variant.vtype(), VarType::Currency);
        assert_eq!(
            currency_variant
                .to_runtime_value()
                .expect("currency Variant should bridge into RuntimeValue::Currency"),
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(125_000))
        );
    }

    #[test]
    fn decimal_variant_bridges_to_runtime_decimal_lane() {
        let decimal_variant =
            Variant::from_decimal96(Decimal96::from_parts(123_450, 0, 0, 3, true));
        assert_eq!(decimal_variant.vtype(), VarType::Decimal);
        assert_eq!(
            decimal_variant
                .to_runtime_value()
                .expect("decimal Variant should bridge into RuntimeValue::Decimal"),
            RuntimeValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, true))
        );
    }

    #[test]
    fn variant_runtime_value_bridge_roundtrips_supported_subset() {
        let bool_variant = Variant::from_runtime_value(&RuntimeValue::Bool(true));
        assert_eq!(
            bool_variant
                .to_runtime_value()
                .expect("bool Variant should bridge back"),
            RuntimeValue::Bool(true)
        );

        let i64_variant = Variant::from_runtime_value(&RuntimeValue::I64(5_000_000_000));
        assert_eq!(
            i64_variant
                .to_runtime_value()
                .expect("i64 Variant should bridge back"),
            RuntimeValue::I64(5_000_000_000)
        );

        let string_variant = Variant::from_runtime_value(&RuntimeValue::String(BStr::from("hello")));
        assert_eq!(
            string_variant
                .to_runtime_value()
                .expect("string Variant should bridge back"),
            RuntimeValue::String(BStr::from("hello"))
        );

        let double_variant = Variant::from_runtime_value(&RuntimeValue::F64(F64Value::from_f64(3.5)));
        assert_eq!(
            double_variant
                .to_runtime_value()
                .expect("double Variant should bridge back"),
            RuntimeValue::F64(F64Value::from_f64(3.5))
        );

        let single_variant =
            Variant::from_runtime_value(&RuntimeValue::F64(F64Value::from_single_f64(3.5)));
        assert_eq!(
            single_variant
                .to_runtime_value()
                .expect("single Variant should bridge back"),
            RuntimeValue::F64(F64Value::from_single_f64(3.5))
        );

        let date_variant =
            Variant::from_runtime_value(&RuntimeValue::F64(F64Value::from_date_f64(45200.25)));
        assert_eq!(
            date_variant
                .to_runtime_value()
                .expect("date Variant should bridge back"),
            RuntimeValue::F64(F64Value::from_date_f64(45200.25))
        );

        let currency_variant =
            Variant::from_runtime_value(&RuntimeValue::Currency(CurrencyValue::from_scaled_i64(
                -42_500,
            )));
        assert_eq!(
            currency_variant
                .to_runtime_value()
                .expect("currency Variant should bridge back"),
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(-42_500))
        );

        let decimal_variant = Variant::from_runtime_value(&RuntimeValue::Decimal(
            Decimal96::from_parts(123_450, 0, 0, 3, false),
        ));
        assert_eq!(
            decimal_variant
                .to_runtime_value()
                .expect("decimal Variant should bridge back"),
            RuntimeValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, false))
        );

        let null_variant = Variant::from_runtime_value(&RuntimeValue::Null);
        assert_eq!(
            null_variant.to_runtime_value().expect("null roundtrip"),
            RuntimeValue::Null
        );

        let error_variant = Variant::from_runtime_value(&RuntimeValue::ErrorCode(17));
        assert_eq!(
            error_variant.to_runtime_value().expect("error roundtrip"),
            RuntimeValue::ErrorCode(17)
        );
    }

    #[test]
    fn variant_runtime_value_bridge_classifies_extended_owned_shapes() {
        let array_variant = Variant::from_runtime_value(&RuntimeValue::ArrayIntent(SafeArray::vector(3)));
        assert_eq!(
            array_variant
                .to_runtime_value()
                .expect("array Variant should bridge back"),
            RuntimeValue::ArrayIntent(SafeArray::vector(3))
        );

        let object_variant = Variant::from_runtime_value(&RuntimeValue::ObjectHandle(42.into()));
        assert_eq!(
            object_variant
                .to_runtime_value()
                .expect("object Variant should bridge back"),
            RuntimeValue::ObjectHandle(42.into())
        );

        let binding_variant = Variant::from_runtime_value(&RuntimeValue::BindingHandle(7.into()));
        assert_eq!(
            binding_variant
                .to_runtime_value()
                .expect("binding Variant should bridge back"),
            RuntimeValue::BindingHandle(7.into())
        );
    }

    #[test]
    fn variant_compat_slot_boundary_roundtrips_supported_subset() {
        let value = Variant::from_compat_slot_i32(42);
        assert_eq!(value.project_compat_slot_i32().expect("compat slot"), 42);
        let array = Variant::from_compat_slot_i32(crate::safe_array::ARRAY_TAG_BASE + 3);
        assert_eq!(
            array.to_runtime_value().expect("array runtime value"),
            RuntimeValue::ArrayIntent(SafeArray::vector(3))
        );
    }

    #[test]
    fn string_variant_from_wire_bytes_requires_owned_payload_for_runtime_bridge() {
        let mut wire = [0u8; 16];
        wire[0..2].copy_from_slice(&(VarType::String as u16).to_le_bytes());
        let variant = Variant::from_wire_bytes(wire).expect("raw string wire should decode");
        assert!(variant.to_runtime_value().is_err());
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::VariantCore;

    #[kani::proof]
    fn com_variant_layout_is_16_bytes() {
        assert_eq!(core::mem::size_of::<VariantCore>(), 16);
    }
}
