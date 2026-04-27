use crate::{
    Decimal96,
    bstr::{BStr, OwnedBStrCore},
    object_ref::{ObjectRef, RawRuntimeIUnknown},
    runtime_value::{CurrencyValue, F64Subtype, F64Value, RuntimeValue},
    safe_array::{SafeArray, array_tag_from_safe_array, safe_array_from_tag},
    value_tags::{EMPTY_TAG, NULL_TAG, error_code_from_tag, error_tag_from_code, is_error_tag},
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
    Object = 0x000D,
    Error = 0x000A,
    Boolean = 0x000B,
    Decimal = 0x000E,
    Byte = 0x0011,
    LongLong = 0x0014,
    ArrayVariant = 0x200C,
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
            0x000D => Some(Self::Object),
            0x000A => Some(Self::Error),
            0x000B => Some(Self::Boolean),
            0x000E => Some(Self::Decimal),
            0x0011 => Some(Self::Byte),
            0x0014 => Some(Self::LongLong),
            0x200C => Some(Self::ArrayVariant),
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

fn raw_bstr_ptr_to_bytes(ptr: *mut u16) -> [u8; 8] {
    (ptr as usize as u64).to_le_bytes()
}

fn raw_iunknown_ptr_to_bytes(ptr: *mut RawRuntimeIUnknown) -> [u8; 8] {
    (ptr as usize as u64).to_le_bytes()
}

fn raw_safearray_ptr_to_bytes(ptr: *mut core::ffi::c_void) -> [u8; 8] {
    (ptr as usize as u64).to_le_bytes()
}

fn bytes_to_raw_bstr(bytes: [u8; 8]) -> *mut u16 {
    u64::from_le_bytes(bytes) as usize as *mut u16
}

fn bytes_to_raw_iunknown(bytes: [u8; 8]) -> *mut RawRuntimeIUnknown {
    u64::from_le_bytes(bytes) as usize as *mut RawRuntimeIUnknown
}

fn bytes_to_raw_safearray(bytes: [u8; 8]) -> *mut core::ffi::c_void {
    u64::from_le_bytes(bytes) as usize as *mut core::ffi::c_void
}

fn alloc_raw_bstr_from_bstr(text: &BStr) -> Result<*mut u16, String> {
    text.clone_raw_bstr()
}

unsafe fn raw_bstr_to_bstr(ptr: *mut u16) -> BStr {
    let text = unsafe { BStr::from_raw_bstr(ptr) };
    let cloned = text.clone();
    core::mem::forget(text);
    cloned
}

#[repr(transparent)]
pub struct Variant {
    core: VariantCore,
}

impl Variant {
    fn from_core(core: VariantCore) -> Self {
        Self { core }
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

    pub fn as_variant_cell_ptr(&self) -> *mut core::ffi::c_void {
        (self as *const Self).cast_mut().cast()
    }

    pub fn to_wire_bytes(&self) -> [u8; 16] {
        self.core.to_wire_bytes()
    }

    pub fn from_wire_bytes(bytes: [u8; 16]) -> Result<Self, String> {
        let core = VariantCore::from_wire_bytes(bytes)?;
        match core.vtype {
            VarType::String => {
                let ptr = bytes_to_raw_bstr(core.data_bytes());
                let text = unsafe { raw_bstr_to_bstr(ptr) };
                let cloned = text.raw_bstr();
                core::mem::forget(text);
                Ok(Self::from_core(VariantCore::from_bytes(
                    VarType::String,
                    raw_bstr_ptr_to_bytes(cloned),
                )))
            }
            VarType::Object => {
                let ptr = bytes_to_raw_iunknown(core.data_bytes());
                let object = unsafe { ObjectRef::from_raw_iunknown_addref(ptr) };
                Ok(match object {
                    Some(value) => Self::from_object_ref(value),
                    None => Self::from_core(VariantCore::from_bytes(VarType::Object, [0; 8])),
                })
            }
            VarType::ArrayVariant => {
                let ptr = bytes_to_raw_safearray(core.data_bytes());
                let Some(array) = (unsafe { SafeArray::clone_from_raw_safearray(ptr) }) else {
                    return Ok(Self::from_core(VariantCore::from_bytes(
                        VarType::ArrayVariant,
                        [0; 8],
                    )));
                };
                Ok(Self::from_safearray(array))
            }
            _ => Ok(Self::from_core(core)),
        }
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
        Self::from_core(VariantCore::from_bytes(
            VarType::LongLong,
            value.to_le_bytes(),
        ))
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
        Self::from_core(VariantCore::from_bytes(
            VarType::Double,
            value.to_le_bytes(),
        ))
    }

    pub fn as_f64(&self) -> Option<f64> {
        if self.vtype() != VarType::Double {
            return None;
        }
        Some(f64::from_le_bytes(self.data_bytes()))
    }

    pub fn from_currency_scaled_i64(value: i64) -> Self {
        Self::from_core(VariantCore::from_bytes(
            VarType::Currency,
            value.to_le_bytes(),
        ))
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
        let raw = alloc_raw_bstr_from_bstr(&text).expect("raw BSTR allocation should succeed");
        Self::from_core(VariantCore::from_bytes(
            VarType::String,
            raw_bstr_ptr_to_bytes(raw),
        ))
    }

    pub fn as_bstr(&self) -> Option<BStr> {
        if self.vtype() != VarType::String {
            return None;
        }
        Some(unsafe { raw_bstr_to_bstr(bytes_to_raw_bstr(self.data_bytes())) })
    }

    pub fn string_core(&self) -> Option<OwnedBStrCore> {
        self.as_bstr().map(|text| text.owned_core())
    }

    pub fn from_object_ref(value: ObjectRef) -> Self {
        let raw = value.raw_iunknown();
        core::mem::forget(value);
        Self::from_core(VariantCore::from_bytes(
            VarType::Object,
            raw_iunknown_ptr_to_bytes(raw),
        ))
    }

    pub fn as_object_ref(&self) -> Option<ObjectRef> {
        if self.vtype() != VarType::Object {
            return None;
        }
        unsafe { ObjectRef::from_raw_iunknown_addref(bytes_to_raw_iunknown(self.data_bytes())) }
    }

    pub fn from_safearray(value: SafeArray) -> Self {
        let raw = value.raw_safearray_ptr();
        core::mem::forget(value);
        Self::from_core(VariantCore::from_bytes(
            VarType::ArrayVariant,
            raw_safearray_ptr_to_bytes(raw),
        ))
    }

    pub fn as_safearray(&self) -> Option<SafeArray> {
        if self.vtype() != VarType::ArrayVariant {
            return None;
        }
        unsafe { SafeArray::clone_from_raw_safearray(bytes_to_raw_safearray(self.data_bytes())) }
    }

    /// Compatibility bridge from the legacy semantic [`RuntimeValue`] carrier
    /// into the retained runtime `Variant`.
    ///
    /// New value-model code should construct `Variant` values directly.
    pub fn try_from_runtime_value(value: &RuntimeValue) -> Result<Self, String> {
        Ok(match value {
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
            RuntimeValue::Object(object) => Self::from_object_ref(object.clone()),
            RuntimeValue::ArrayIntent(array) => Self::from_safearray(array.clone()),
            RuntimeValue::BindingHandle(handle) => {
                return Err(format!(
                    "binding handle {} is an internal non-VBA token and is intentionally excluded from the canonical Variant carrier",
                    handle.raw()
                ));
            }
        })
    }

    /// Panicking compatibility bridge from [`RuntimeValue`] into `Variant`.
    ///
    /// New value-model code should construct `Variant` values directly.
    pub fn from_runtime_value(value: &RuntimeValue) -> Self {
        Self::try_from_runtime_value(value)
            .expect("runtime Variant bridge should only be used for supported exact carriers")
    }

    /// Compatibility bridge from the legacy i32 slot-token lane into a retained
    /// `Variant`.
    pub fn try_from_compat_slot_i32(value: i32) -> Result<Self, String> {
        if value == EMPTY_TAG {
            return Ok(Self::empty());
        }
        if value == NULL_TAG {
            return Ok(Self::null());
        }
        if is_error_tag(value) {
            return Ok(Self::from_error_code(
                error_code_from_tag(value).unwrap_or(0),
            ));
        }
        if let Some(array) = safe_array_from_tag(value) {
            return Ok(Self::from_safearray(array));
        }
        Ok(Self::from_i32(value))
    }

    /// Panicking compatibility bridge from the legacy i32 slot-token lane into
    /// a retained `Variant`.
    pub fn from_compat_slot_i32(value: i32) -> Self {
        Self::try_from_compat_slot_i32(value)
            .expect("compat slot -> Variant bridge should stay on the supported exact subset")
    }

    /// Compatibility projection from the retained runtime `Variant` into the
    /// legacy semantic [`RuntimeValue`] carrier.
    pub fn to_runtime_value(&self) -> Result<RuntimeValue, String> {
        match self.vtype() {
            VarType::Empty => Ok(RuntimeValue::Empty),
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
            VarType::Byte => self
                .as_u8()
                .map(|value| RuntimeValue::I32(i32::from(value)))
                .ok_or_else(|| "invalid Byte variant payload".to_string()),
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
                .map(RuntimeValue::String)
                .ok_or_else(|| "invalid String variant payload".to_string()),
            VarType::Boolean => self
                .as_bool()
                .map(RuntimeValue::Bool)
                .ok_or_else(|| "invalid Boolean variant payload".to_string()),
            VarType::Error => self
                .as_error_code()
                .map(RuntimeValue::ErrorCode)
                .ok_or_else(|| "invalid Error variant payload".to_string()),
            VarType::Object => self
                .as_object_ref()
                .map(RuntimeValue::Object)
                .ok_or_else(|| "invalid Object variant payload".to_string()),
            VarType::ArrayVariant => self
                .as_safearray()
                .map(RuntimeValue::ArrayIntent)
                .ok_or_else(|| "invalid SAFEARRAY variant payload".to_string()),
        }
    }

    pub fn project_compat_slot_i32(&self) -> Result<i32, String> {
        match self.vtype() {
            VarType::Empty => Ok(EMPTY_TAG),
            VarType::Null => Ok(NULL_TAG),
            VarType::Integer => self
                .as_i16()
                .map(|value| value as i32)
                .ok_or_else(|| "invalid Integer variant payload".to_string()),
            VarType::Long => self
                .as_i32()
                .ok_or_else(|| "invalid Long variant payload".to_string()),
            VarType::LongLong => self
                .as_i64()
                .ok_or_else(|| "invalid LongLong variant payload".to_string())
                .and_then(|value| {
                    i32::try_from(value).map_err(|_| {
                        format!(
                            "i64 value {value} cannot be represented in current compat slot lane"
                        )
                    })
                }),
            VarType::Byte => self
                .as_u8()
                .map(i32::from)
                .ok_or_else(|| "invalid Byte variant payload".to_string()),
            VarType::Boolean => self
                .as_bool()
                .map(i32::from)
                .ok_or_else(|| "invalid Boolean variant payload".to_string()),
            VarType::Error => self
                .as_error_code()
                .map(error_tag_from_code)
                .ok_or_else(|| "invalid Error variant payload".to_string()),
            VarType::Object => self
                .as_object_ref()
                .map(|value| value.raw())
                .ok_or_else(|| "invalid Object variant payload".to_string()),
            VarType::ArrayVariant => self
                .as_safearray()
                .ok_or_else(|| "invalid SAFEARRAY variant payload".to_string())
                .and_then(|array| {
                    array_tag_from_safe_array(&array).ok_or_else(|| {
                        "array intent cannot be represented in current compat slot tag".to_string()
                    })
                }),
            VarType::Single | VarType::Double | VarType::Date => {
                Err("f64 cannot be represented in current compat slot lane".to_string())
            }
            VarType::Decimal => {
                Err("decimal cannot be represented in current compat slot lane".to_string())
            }
            VarType::Currency => {
                Err("currency cannot be represented in current compat slot lane".to_string())
            }
            VarType::String => {
                Err("string cannot be represented in current compat slot lane".to_string())
            }
        }
    }
}

impl Clone for Variant {
    fn clone(&self) -> Self {
        match self.vtype() {
            VarType::String => {
                let cloned = unsafe { raw_bstr_to_bstr(bytes_to_raw_bstr(self.data_bytes())) };
                let raw = cloned.raw_bstr();
                core::mem::forget(cloned);
                Self::from_core(VariantCore::from_bytes(
                    VarType::String,
                    raw_bstr_ptr_to_bytes(raw),
                ))
            }
            VarType::Object => match self.as_object_ref() {
                Some(object) => Self::from_object_ref(object),
                None => Self::from_core(self.core),
            },
            VarType::ArrayVariant => match self.as_safearray() {
                Some(array) => Self::from_safearray(array),
                None => Self::from_core(self.core),
            },
            _ => Self::from_core(self.core),
        }
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        match self.vtype() {
            VarType::String => unsafe {
                let _ = BStr::from_raw_bstr(bytes_to_raw_bstr(self.data_bytes()));
            },
            VarType::Object => {
                let raw = bytes_to_raw_iunknown(self.data_bytes());
                if !raw.is_null() {
                    unsafe {
                        let vtbl = (*raw).vtbl;
                        ((*vtbl).release)(raw.cast());
                    }
                }
            }
            VarType::ArrayVariant => {
                let raw = bytes_to_raw_safearray(self.data_bytes());
                if let Some(array) = unsafe { SafeArray::from_raw_safearray_owned(raw) } {
                    drop(array);
                }
            }
            _ => {}
        }
    }
}

impl core::fmt::Debug for Variant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut dbg = f.debug_struct("Variant");
        dbg.field("vtype", &self.vtype())
            .field("reserved1", &self.reserved1())
            .field("reserved2", &self.reserved2())
            .field("reserved3", &self.reserved3());
        match self.vtype() {
            VarType::String => {
                dbg.field("string", &self.as_bstr());
            }
            VarType::Object => {
                dbg.field("object", &self.as_object_ref().map(|value| value.raw()));
            }
            VarType::ArrayVariant => {
                dbg.field("array", &self.as_safearray());
            }
            _ => {
                dbg.field("data", &self.data_bytes());
            }
        }
        dbg.finish()
    }
}

impl PartialEq for Variant {
    fn eq(&self, other: &Self) -> bool {
        if self.vtype() != other.vtype()
            || self.reserved1() != other.reserved1()
            || self.reserved2() != other.reserved2()
            || self.reserved3() != other.reserved3()
        {
            return false;
        }
        match self.vtype() {
            VarType::String => self.as_bstr() == other.as_bstr(),
            VarType::Object => {
                bytes_to_raw_iunknown(self.data_bytes())
                    == bytes_to_raw_iunknown(other.data_bytes())
            }
            VarType::ArrayVariant => self.as_safearray() == other.as_safearray(),
            _ => self.data_bytes() == other.data_bytes(),
        }
    }
}

impl Eq for Variant {}

#[cfg(test)]
mod tests {
    use crate::{
        CurrencyValue, Decimal96, F64Value, ObjectRef, RuntimeValue, bstr::BStr,
        safe_array::SafeArray,
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
    fn string_roundtrip_preserves_owned_pointer_payload() {
        let value = Variant::from_string("abc");
        assert_eq!(value.vtype(), VarType::String);
        assert_eq!(value.as_bstr(), Some(BStr::from("abc")));
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
        assert_eq!(core::mem::size_of::<Variant>(), 16);
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
    fn string_variant_wire_roundtrip_clones_bstr_payload() {
        let original = Variant::from_string("A\0BC");
        let wire = original.to_wire_bytes();
        let roundtrip = Variant::from_wire_bytes(wire).expect("wire roundtrip");
        assert_eq!(roundtrip.as_bstr(), Some(BStr::from("A\0BC")));
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
    fn variant_runtime_value_bridge_roundtrips_supported_exact_subset() {
        let bool_variant = Variant::try_from_runtime_value(&RuntimeValue::Bool(true))
            .expect("bool bridge should be supported");
        assert_eq!(
            bool_variant
                .to_runtime_value()
                .expect("bool Variant should bridge back"),
            RuntimeValue::Bool(true)
        );

        let i64_variant = Variant::try_from_runtime_value(&RuntimeValue::I64(5_000_000_000))
            .expect("i64 bridge should be supported");
        assert_eq!(
            i64_variant
                .to_runtime_value()
                .expect("i64 Variant should bridge back"),
            RuntimeValue::I64(5_000_000_000)
        );

        let string_variant =
            Variant::try_from_runtime_value(&RuntimeValue::String(BStr::from("hello")))
                .expect("string bridge should be supported");
        assert_eq!(
            string_variant
                .to_runtime_value()
                .expect("string Variant should bridge back"),
            RuntimeValue::String(BStr::from("hello"))
        );

        let object_variant = Variant::try_from_runtime_value(&RuntimeValue::Object(
            ObjectRef::from_compat_identity(42),
        ))
        .expect("object bridge should be supported");
        assert_eq!(object_variant.vtype() as u16, 0x000D);
        let object_ref = object_variant
            .as_object_ref()
            .expect("object ref should be retained");
        assert_eq!(object_ref.raw(), 42);
        drop(object_ref);
        let roundtripped = object_variant
            .to_runtime_value()
            .expect("object Variant should bridge back");
        let RuntimeValue::Object(object_ref) = roundtripped else {
            panic!("expected object-ref runtime carrier");
        };
        assert_eq!(object_ref.raw(), 42);

        let array_value = RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
            RuntimeValue::I32(4),
            RuntimeValue::String(BStr::from("payload")),
        ]));
        let array_variant = Variant::try_from_runtime_value(&array_value)
            .expect("array bridge should be supported");
        assert_eq!(array_variant.vtype(), VarType::ArrayVariant);
        assert_ne!(u64::from_le_bytes(array_variant.data_bytes()), 0);
        assert_eq!(
            array_variant
                .to_runtime_value()
                .expect("array Variant should bridge back"),
            array_value
        );
    }

    #[test]
    fn variant_runtime_value_bridge_excludes_binding_tokens() {
        let binding = Variant::try_from_runtime_value(&RuntimeValue::BindingHandle(7.into()));
        assert_eq!(
            binding.expect_err("binding handles remain outside canonical Variant"),
            "binding handle 7 is an internal non-VBA token and is intentionally excluded from the canonical Variant carrier"
        );
    }

    #[test]
    fn variant_compat_slot_boundary_roundtrips_supported_subset() {
        let value = Variant::from_compat_slot_i32(42);
        assert_eq!(value.project_compat_slot_i32().expect("compat slot"), 42);

        let bool_value = Variant::from_bool(true);
        assert_eq!(bool_value.project_compat_slot_i32().expect("bool slot"), 1);

        let error_value = Variant::from_error_code(17);
        assert_eq!(
            error_value.project_compat_slot_i32().expect("error slot"),
            crate::value_tags::error_tag_from_code(17)
        );

        let array_value = Variant::from_safearray(SafeArray::vector(3));
        assert_eq!(
            array_value.project_compat_slot_i32().expect("array slot"),
            crate::safe_array::ARRAY_TAG_BASE + 3
        );
    }

    #[test]
    fn variant_compat_slot_boundary_rejects_non_legacy_carriers() {
        assert_eq!(
            Variant::from_string("ABC")
                .project_compat_slot_i32()
                .expect_err("string should stay outside compat slot lane"),
            "string cannot be represented in current compat slot lane"
        );
        assert_eq!(
            Variant::from_i64(5_000_000_000)
                .project_compat_slot_i32()
                .expect_err("overflow should stay outside compat slot lane"),
            "i64 value 5000000000 cannot be represented in current compat slot lane"
        );
    }

    #[test]
    fn compat_slot_variant_bridge_preserves_tagged_shapes_without_runtime_value_detour() {
        let empty = Variant::try_from_compat_slot_i32(crate::value_tags::EMPTY_TAG)
            .expect("empty compat slot");
        assert_eq!(empty.vtype(), VarType::Empty);

        let null = Variant::try_from_compat_slot_i32(crate::value_tags::NULL_TAG)
            .expect("null compat slot");
        assert_eq!(null.vtype(), VarType::Null);

        let error = Variant::try_from_compat_slot_i32(crate::value_tags::error_tag_from_code(17))
            .expect("error compat slot");
        assert_eq!(error.as_error_code(), Some(17));

        let array = Variant::try_from_compat_slot_i32(crate::safe_array::ARRAY_TAG_BASE + 2)
            .expect("array compat slot");
        assert_eq!(array.vtype(), VarType::ArrayVariant);
        assert_eq!(array.as_safearray().expect("array").len(), 2);
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::{Variant, VariantCore};

    #[kani::proof]
    fn com_variant_layout_is_16_bytes() {
        assert_eq!(core::mem::size_of::<VariantCore>(), 16);
        assert_eq!(core::mem::size_of::<Variant>(), 16);
    }
}
