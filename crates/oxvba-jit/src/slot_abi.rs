//! RtSlot: 16-byte tagged slot for JIT-compiled VBA code.
//!
//! Layout: `[tag: u32][_pad: u32][payload: u64]` — 16 bytes total.
//! Cranelift accesses `tag` as i32 at offset 0, `payload` as i64 at offset 8.

use oxvba_runtime::{
    BindingHandle, CurrencyValue, Decimal96, F64Subtype, F64Value, ObjectHandle, RuntimeValue,
    bstr::BStr, safe_array::SafeArray,
};

// ── Tag constants ─────────────────────────────────────────────────────

pub const TAG_EMPTY: u32 = 0;
pub const TAG_NULL: u32 = 1;
pub const TAG_I32: u32 = 2;
pub const TAG_F64: u32 = 3;
pub const TAG_STRING: u32 = 4;
pub const TAG_ERROR: u32 = 5;
pub const TAG_BOOL: u32 = 6;
pub const TAG_OBJECT: u32 = 7;
pub const TAG_ARRAY: u32 = 8;
pub const TAG_I64: u32 = 9;
pub const TAG_CURRENCY: u32 = 10;
pub const TAG_DECIMAL: u32 = 11;
pub const TAG_BINDING: u32 = 12;

/// Offset (in bytes) of the tag field within an RtSlot.
pub const SLOT_TAG_OFFSET: i32 = 0;
/// Offset (in bytes) of the payload field within an RtSlot.
pub const SLOT_PAYLOAD_OFFSET: i32 = 8;
/// Total size of one RtSlot in bytes.
pub const SLOT_SIZE: i32 = 16;

// ── RtSlot struct ─────────────────────────────────────────────────────

/// 16-byte C-ABI slot used by JIT-compiled code.
///
/// For simple types (Empty, Null, I32, F64, Bool, ErrorCode, I64, Currency),
/// the slot is self-contained. For heap types (String, Array, Decimal),
/// the payload holds a raw pointer to a boxed value owned by the JitContext.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RtSlot {
    pub tag: u32,
    pub _pad: u32,
    pub payload: u64,
}

impl Default for RtSlot {
    fn default() -> Self {
        Self {
            tag: TAG_EMPTY,
            _pad: 0,
            payload: 0,
        }
    }
}

impl RtSlot {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn null() -> Self {
        Self {
            tag: TAG_NULL,
            _pad: 0,
            payload: 0,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        Self {
            tag: TAG_I32,
            _pad: 0,
            payload: value as i64 as u64,
        }
    }

    pub fn from_f64(value: f64) -> Self {
        Self {
            tag: TAG_F64,
            _pad: 0, // Double subtype (default)
            payload: value.to_bits(),
        }
    }

    pub fn from_f64_value(value: &F64Value) -> Self {
        let subtype_code = match value.subtype() {
            F64Subtype::Single => 1u32,
            F64Subtype::Double => 0u32,
            F64Subtype::Date => 2u32,
        };
        Self {
            tag: TAG_F64,
            _pad: subtype_code,
            payload: value.as_f64().to_bits(),
        }
    }

    pub fn from_bool(value: bool) -> Self {
        Self {
            tag: TAG_BOOL,
            _pad: 0,
            payload: u64::from(value),
        }
    }

    pub fn from_error(code: i32) -> Self {
        Self {
            tag: TAG_ERROR,
            _pad: 0,
            payload: code as i64 as u64,
        }
    }

    pub fn from_object(handle: i32) -> Self {
        Self {
            tag: TAG_OBJECT,
            _pad: 0,
            payload: handle as i64 as u64,
        }
    }

    pub fn from_binding(handle: i32) -> Self {
        Self {
            tag: TAG_BINDING,
            _pad: 0,
            payload: handle as i64 as u64,
        }
    }

    pub fn from_i64(value: i64) -> Self {
        Self {
            tag: TAG_I64,
            _pad: 0,
            payload: value as u64,
        }
    }

    pub fn from_currency(scaled: i64) -> Self {
        Self {
            tag: TAG_CURRENCY,
            _pad: 0,
            payload: scaled as u64,
        }
    }

    /// Create an RtSlot from a heap-allocated string.
    /// The caller is responsible for tracking the Box for cleanup.
    pub fn from_string_box(boxed: Box<BStr>) -> Self {
        let ptr = Box::into_raw(boxed);
        Self {
            tag: TAG_STRING,
            _pad: 0,
            payload: ptr as u64,
        }
    }

    /// Convert this slot to a RuntimeValue.
    ///
    /// For heap types (String), this clones the data — the slot retains ownership.
    /// # Safety
    /// For TAG_STRING, the payload must be a valid pointer to a Box<BStr>.
    pub unsafe fn to_runtime_value(&self) -> RuntimeValue {
        match self.tag {
            TAG_EMPTY => RuntimeValue::Empty,
            TAG_NULL => RuntimeValue::Null,
            TAG_I32 => RuntimeValue::I32(self.payload as i64 as i32),
            TAG_F64 => {
                let subtype = match self._pad {
                    1 => F64Subtype::Single,
                    2 => F64Subtype::Date,
                    _ => F64Subtype::Double,
                };
                RuntimeValue::F64(F64Value::from_bits_with_subtype(self.payload, subtype))
            }
            TAG_BOOL => RuntimeValue::Bool(self.payload != 0),
            TAG_ERROR => RuntimeValue::ErrorCode(self.payload as i64 as i32),
            TAG_OBJECT => RuntimeValue::ObjectHandle(ObjectHandle::new(self.payload as i64 as i32)),
            TAG_BINDING => {
                RuntimeValue::BindingHandle(BindingHandle::new(self.payload as i64 as i32))
            }
            TAG_I64 => RuntimeValue::I64(self.payload as i64),
            TAG_CURRENCY => RuntimeValue::Currency(CurrencyValue::from_scaled_i64(self.payload as i64)),
            TAG_STRING => {
                let ptr = self.payload as *const BStr;
                if ptr.is_null() {
                    RuntimeValue::String(BStr(String::new()))
                } else {
                    RuntimeValue::String(unsafe { &*ptr }.clone())
                }
            }
            TAG_DECIMAL => {
                let ptr = self.payload as *const Decimal96;
                if ptr.is_null() {
                    RuntimeValue::Empty
                } else {
                    RuntimeValue::Decimal(unsafe { &*ptr }.clone())
                }
            }
            TAG_ARRAY => {
                let ptr = self.payload as *const SafeArray;
                if ptr.is_null() {
                    RuntimeValue::Empty
                } else {
                    RuntimeValue::ArrayIntent(unsafe { &*ptr }.clone())
                }
            }
            _ => RuntimeValue::Empty,
        }
    }

    /// Drop any heap-allocated data owned by this slot.
    /// # Safety
    /// Must only be called once per slot, and only when the slot owns heap data.
    pub unsafe fn drop_heap(&mut self) {
        match self.tag {
            TAG_STRING => {
                let ptr = self.payload as *mut BStr;
                if !ptr.is_null() {
                    drop(unsafe { Box::from_raw(ptr) });
                    self.payload = 0;
                }
            }
            TAG_DECIMAL => {
                let ptr = self.payload as *mut Decimal96;
                if !ptr.is_null() {
                    drop(unsafe { Box::from_raw(ptr) });
                    self.payload = 0;
                }
            }
            TAG_ARRAY => {
                let ptr = self.payload as *mut SafeArray;
                if !ptr.is_null() {
                    drop(unsafe { Box::from_raw(ptr) });
                    self.payload = 0;
                }
            }
            _ => {}
        }
    }

    /// Check if this slot tag represents a heap-allocated type.
    pub fn is_heap_type(&self) -> bool {
        matches!(self.tag, TAG_STRING | TAG_DECIMAL | TAG_ARRAY)
    }
}

/// Convert a RuntimeValue to an RtSlot.
///
/// For heap types (String), allocates a Box. The caller must ensure
/// the slot is properly cleaned up (via JitContext).
pub fn rtslot_from_runtime_value(value: &RuntimeValue) -> RtSlot {
    match value {
        RuntimeValue::Empty => RtSlot::empty(),
        RuntimeValue::Null => RtSlot::null(),
        RuntimeValue::I32(v) => RtSlot::from_i32(*v),
        RuntimeValue::F64(v) => RtSlot::from_f64_value(v),
        RuntimeValue::Bool(v) => RtSlot::from_bool(*v),
        RuntimeValue::ErrorCode(code) => RtSlot::from_error(*code),
        RuntimeValue::ObjectHandle(h) => RtSlot::from_object(h.raw()),
        RuntimeValue::BindingHandle(h) => RtSlot::from_binding(h.raw()),
        RuntimeValue::I64(v) => RtSlot::from_i64(*v),
        RuntimeValue::Currency(c) => RtSlot::from_currency(c.scaled_i64()),
        RuntimeValue::String(s) => RtSlot::from_string_box(Box::new(s.clone())),
        RuntimeValue::Decimal(d) => {
            let ptr = Box::into_raw(Box::new(d.clone()));
            RtSlot {
                tag: TAG_DECIMAL,
                _pad: 0,
                payload: ptr as u64,
            }
        }
        RuntimeValue::ArrayIntent(a) => {
            let ptr = Box::into_raw(Box::new(a.clone()));
            RtSlot {
                tag: TAG_ARRAY,
                _pad: 0,
                payload: ptr as u64,
            }
        }
    }
}
