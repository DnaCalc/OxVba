//! Miri-compatible mock paths for VARIANT layout verification.
//!
//! This module provides pure-Rust mock structures that mirror the memory layout
//! of Windows COM types (VARIANT, SAFEARRAY, DECIMAL, CY, BSTR). These can be
//! exercised under Miri to detect undefined behavior in pointer arithmetic,
//! alignment, and type punning code — without calling actual Windows COM APIs.
//!
//! To run under Miri:
//!   cargo +nightly miri test -p oxvba-com -- miri_variant

use crate::ComValue;
use oxvba_runtime::{CurrencyValue, Decimal96, F64Subtype, F64Value};

// ── Mock VARIANT layout ──
//
// A Windows VARIANT is 24 bytes on x64:
//   offset 0:  vt (u16) — the VARENUM tag
//   offset 2:  wReserved1 (u16)
//   offset 4:  wReserved2 (u16)
//   offset 6:  wReserved3 (u16)
//   offset 8:  union payload (16 bytes, holds i32/i64/f64/BSTR ptr/etc.)

/// Mock VARIANT with the exact Windows x64 memory layout.
#[repr(C)]
pub struct MockVariant {
    pub vt: u16,
    pub reserved1: u16,
    pub reserved2: u16,
    pub reserved3: u16,
    pub payload: [u8; 16], // 16-byte union payload
}

impl MockVariant {
    pub fn empty() -> Self {
        Self {
            vt: VT_EMPTY,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            payload: [0u8; 16],
        }
    }

    /// Set the payload as an i32 value (VT_I4).
    pub fn set_i32(&mut self, value: i32) {
        self.vt = VT_I4;
        self.payload[..4].copy_from_slice(&value.to_le_bytes());
    }

    /// Read the payload as an i32.
    pub fn get_i32(&self) -> i32 {
        i32::from_le_bytes([
            self.payload[0],
            self.payload[1],
            self.payload[2],
            self.payload[3],
        ])
    }

    /// Set the payload as an i64 value (VT_I8).
    pub fn set_i64(&mut self, value: i64) {
        self.vt = VT_I8;
        self.payload[..8].copy_from_slice(&value.to_le_bytes());
    }

    /// Read the payload as an i64.
    pub fn get_i64(&self) -> i64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.payload[..8]);
        i64::from_le_bytes(buf)
    }

    /// Set the payload as an f64 value (VT_R8 / VT_DATE).
    pub fn set_f64(&mut self, value: f64, vt: u16) {
        self.vt = vt;
        self.payload[..8].copy_from_slice(&value.to_le_bytes());
    }

    /// Read the payload as an f64.
    pub fn get_f64(&self) -> f64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.payload[..8]);
        f64::from_le_bytes(buf)
    }

    /// Set the payload as a VARIANT_BOOL (VT_BOOL).
    /// VBA True = -1 (0xFFFF), False = 0.
    pub fn set_bool(&mut self, value: bool) {
        self.vt = VT_BOOL;
        let vb: i16 = if value { -1 } else { 0 };
        self.payload[..2].copy_from_slice(&vb.to_le_bytes());
    }

    /// Read the payload as a VARIANT_BOOL.
    pub fn get_bool(&self) -> bool {
        let vb = i16::from_le_bytes([self.payload[0], self.payload[1]]);
        vb != 0
    }

    /// Set the payload as a Currency value (VT_CY) — scaled i64.
    pub fn set_currency(&mut self, cy: CurrencyValue) {
        self.vt = VT_CY;
        let raw = cy.scaled_i64();
        self.payload[..8].copy_from_slice(&raw.to_le_bytes());
    }

    /// Read the payload as a CurrencyValue.
    pub fn get_currency(&self) -> CurrencyValue {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.payload[..8]);
        let raw = i64::from_le_bytes(buf);
        CurrencyValue::from_scaled_i64(raw)
    }

    /// Set the payload as an error code (VT_ERROR).
    pub fn set_error(&mut self, code: i32) {
        self.vt = VT_ERROR;
        self.payload[..4].copy_from_slice(&code.to_le_bytes());
    }

    /// Read the payload as an error code.
    pub fn get_error(&self) -> i32 {
        i32::from_le_bytes([
            self.payload[0],
            self.payload[1],
            self.payload[2],
            self.payload[3],
        ])
    }
}

// VARENUM constants (matching Windows VT_* values)
pub const VT_EMPTY: u16 = 0;
pub const VT_NULL: u16 = 1;
pub const VT_I2: u16 = 2;
pub const VT_I4: u16 = 3;
pub const VT_R4: u16 = 4;
pub const VT_R8: u16 = 5;
pub const VT_CY: u16 = 6;
pub const VT_DATE: u16 = 7;
pub const VT_BSTR: u16 = 8;
pub const VT_ERROR: u16 = 10;
pub const VT_BOOL: u16 = 11;
pub const VT_I8: u16 = 20;

/// Mock VARIANT → ComValue conversion.
///
/// Mirrors the logic in `windows_variant.rs::variant_to_com_value` but operates
/// on the mock layout without calling any Windows APIs.
pub fn mock_variant_to_com_value(v: &MockVariant) -> Result<ComValue, String> {
    match v.vt {
        VT_EMPTY => Ok(ComValue::Empty),
        VT_NULL => Ok(ComValue::Null),
        VT_I2 => {
            let val = i16::from_le_bytes([v.payload[0], v.payload[1]]);
            Ok(ComValue::I32(val as i32))
        }
        VT_I4 => Ok(ComValue::I32(v.get_i32())),
        VT_R4 => {
            let f = f32::from_le_bytes([v.payload[0], v.payload[1], v.payload[2], v.payload[3]]);
            Ok(ComValue::F64(F64Value::from_single_f64(f as f64)))
        }
        VT_R8 => Ok(ComValue::F64(F64Value::from_f64(v.get_f64()))),
        VT_CY => Ok(ComValue::Currency(v.get_currency())),
        VT_DATE => Ok(ComValue::F64(F64Value::from_date_f64(v.get_f64()))),
        VT_BOOL => Ok(ComValue::Bool(v.get_bool())),
        VT_ERROR => Ok(ComValue::ErrorCode(v.get_error())),
        VT_I8 => Ok(ComValue::I64(v.get_i64())),
        _ => Err(format!("Unsupported VT: {}", v.vt)),
    }
}

/// Mock ComValue → VARIANT conversion.
///
/// Mirrors the logic in `windows_variant.rs::set_variant_from_com_value`.
pub fn mock_com_value_to_variant(value: &ComValue) -> MockVariant {
    let mut v = MockVariant::empty();
    match value {
        ComValue::Empty => {}
        ComValue::Null => v.vt = VT_NULL,
        ComValue::Bool(b) => v.set_bool(*b),
        ComValue::I32(n) => v.set_i32(*n),
        ComValue::I64(n) => v.set_i64(*n),
        ComValue::F64(f) => {
            let vt = match f.subtype() {
                F64Subtype::Single => VT_R4,
                F64Subtype::Double => VT_R8,
                F64Subtype::Date => VT_DATE,
            };
            // For single: round-trip through f32
            if f.subtype() == F64Subtype::Single {
                let f32_val = f.as_f64() as f32;
                v.vt = VT_R4;
                v.payload[..4].copy_from_slice(&f32_val.to_le_bytes());
            } else {
                v.set_f64(f.as_f64(), vt);
            }
        }
        ComValue::Currency(cy) => v.set_currency(*cy),
        ComValue::ErrorCode(code) => v.set_error(*code),
        ComValue::String(s) => {
            // For Miri: represent BSTR as a length-prefixed string in the payload.
            // Real VARIANT stores a BSTR pointer; mock stores string length for verification.
            v.vt = VT_BSTR;
            let len = s.0.len() as u32;
            v.payload[..4].copy_from_slice(&len.to_le_bytes());
        }
        _ => {
            // Decimal, ObjectHandle, ArrayIntent — complex types, mock as VT_EMPTY
            v.vt = VT_EMPTY;
        }
    }
    v
}

// ── Mock DECIMAL layout ──
//
// Windows DECIMAL is 16 bytes:
//   offset 0:  wReserved (u16)
//   offset 2:  scale (u8)
//   offset 3:  sign (u8) — 0x80 for negative
//   offset 4:  Hi32 (u32)
//   offset 8:  Lo64 (u64) — actually Lo32 + Mid32

/// Mock DECIMAL structure matching Windows layout.
#[repr(C)]
pub struct MockDecimal {
    pub reserved: u16,
    pub scale: u8,
    pub sign: u8,
    pub hi32: u32,
    pub lo32: u32,
    pub mid32: u32,
}

impl MockDecimal {
    /// Convert from Decimal96 to mock DECIMAL layout.
    pub fn from_decimal96(d: Decimal96) -> Self {
        Self {
            reserved: 0,
            scale: d.scale(),
            sign: if d.is_negative() { 0x80 } else { 0 },
            hi32: d.hi,
            lo32: d.lo,
            mid32: d.mid,
        }
    }

    /// Convert from mock DECIMAL layout back to Decimal96.
    pub fn to_decimal96(&self) -> Decimal96 {
        let sign_scale = (u16::from(self.sign) << 8) | u16::from(self.scale);
        Decimal96::from_scale_sign(self.lo32, self.mid32, self.hi32, sign_scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miri_variant_i32_roundtrip() {
        let mut v = MockVariant::empty();
        v.set_i32(42);
        assert_eq!(v.vt, VT_I4);
        assert_eq!(v.get_i32(), 42);

        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::I32(42));
    }

    #[test]
    fn miri_variant_i32_negative() {
        let mut v = MockVariant::empty();
        v.set_i32(-1);
        assert_eq!(v.get_i32(), -1);

        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::I32(-1));
    }

    #[test]
    fn miri_variant_i64_roundtrip() {
        let mut v = MockVariant::empty();
        v.set_i64(i64::MAX);
        assert_eq!(v.get_i64(), i64::MAX);

        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::I64(i64::MAX));
    }

    #[test]
    fn miri_variant_f64_roundtrip() {
        let mut v = MockVariant::empty();
        v.set_f64(3.14, VT_R8);
        assert_eq!(v.vt, VT_R8);

        let cv = mock_variant_to_com_value(&v).unwrap();
        match cv {
            ComValue::F64(f) => {
                assert!((f.as_f64() - 3.14).abs() < 1e-10);
                assert_eq!(f.subtype(), F64Subtype::Double);
            }
            _ => panic!("Expected F64"),
        }
    }

    #[test]
    fn miri_variant_date_roundtrip() {
        let mut v = MockVariant::empty();
        let date_serial = 45000.5; // Some date
        v.set_f64(date_serial, VT_DATE);

        let cv = mock_variant_to_com_value(&v).unwrap();
        match cv {
            ComValue::F64(f) => {
                assert!((f.as_f64() - date_serial).abs() < 1e-10);
                assert_eq!(f.subtype(), F64Subtype::Date);
            }
            _ => panic!("Expected F64(Date)"),
        }
    }

    #[test]
    fn miri_variant_bool_roundtrip() {
        let mut v = MockVariant::empty();
        v.set_bool(true);
        assert!(v.get_bool());

        v.set_bool(false);
        assert!(!v.get_bool());

        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::Bool(false));
    }

    #[test]
    fn miri_variant_currency_roundtrip() {
        let cy = CurrencyValue::from_scaled_i64(12345678);
        let mut v = MockVariant::empty();
        v.set_currency(cy);

        assert_eq!(v.get_currency().scaled_i64(), 12345678);

        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::Currency(cy));
    }

    #[test]
    fn miri_variant_error_roundtrip() {
        let mut v = MockVariant::empty();
        v.set_error(0x800A0005_u32 as i32);

        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::ErrorCode(0x800A0005_u32 as i32));
    }

    #[test]
    fn miri_variant_empty_and_null() {
        let v_empty = MockVariant::empty();
        assert_eq!(
            mock_variant_to_com_value(&v_empty).unwrap(),
            ComValue::Empty
        );

        let mut v_null = MockVariant::empty();
        v_null.vt = VT_NULL;
        assert_eq!(mock_variant_to_com_value(&v_null).unwrap(), ComValue::Null);
    }

    #[test]
    fn miri_variant_com_value_roundtrip_i32() {
        let cv = ComValue::I32(999);
        let v = mock_com_value_to_variant(&cv);
        let back = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(back, cv);
    }

    #[test]
    fn miri_variant_com_value_roundtrip_bool() {
        let cv = ComValue::Bool(true);
        let v = mock_com_value_to_variant(&cv);
        let back = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(back, cv);
    }

    #[test]
    fn miri_variant_com_value_roundtrip_f64() {
        let cv = ComValue::F64(F64Value::from_f64(2.718));
        let v = mock_com_value_to_variant(&cv);
        let back = mock_variant_to_com_value(&v).unwrap();
        match back {
            ComValue::F64(f) => assert!((f.as_f64() - 2.718).abs() < 1e-10),
            _ => panic!("Expected F64"),
        }
    }

    #[test]
    fn miri_variant_com_value_roundtrip_currency() {
        let cy = CurrencyValue::from_scaled_i64(-50000);
        let cv = ComValue::Currency(cy);
        let v = mock_com_value_to_variant(&cv);
        let back = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(back, cv);
    }

    #[test]
    fn miri_variant_layout_size() {
        // Verify MockVariant is 24 bytes (same as Windows VARIANT on x64)
        assert_eq!(std::mem::size_of::<MockVariant>(), 24);
    }

    #[test]
    fn miri_decimal_roundtrip() {
        let d = Decimal96::from_scale_sign(100, 200, 300, 0x0002); // scale=2, positive
        let md = MockDecimal::from_decimal96(d);
        let back = md.to_decimal96();
        assert_eq!(back.lo, 100);
        assert_eq!(back.mid, 200);
        assert_eq!(back.hi, 300);
        assert_eq!(back.scale(), 2);
        assert!(!back.is_negative());
    }

    #[test]
    fn miri_decimal_negative() {
        let d = Decimal96::from_scale_sign(1, 0, 0, 0x8004); // scale=4, negative
        let md = MockDecimal::from_decimal96(d);
        assert_eq!(md.sign, 0x80);
        assert_eq!(md.scale, 4);
        let back = md.to_decimal96();
        assert!(back.is_negative());
        assert_eq!(back.scale(), 4);
    }

    #[test]
    fn miri_decimal_layout_size() {
        assert_eq!(std::mem::size_of::<MockDecimal>(), 16);
    }

    #[test]
    fn miri_variant_i16_promotion() {
        // VT_I2 should promote to ComValue::I32
        let mut v = MockVariant::empty();
        v.vt = VT_I2;
        v.payload[0] = 0xFF;
        v.payload[1] = 0x7F; // 32767
        let cv = mock_variant_to_com_value(&v).unwrap();
        assert_eq!(cv, ComValue::I32(32767));
    }

    #[test]
    fn miri_variant_f32_roundtrip() {
        let mut v = MockVariant::empty();
        v.vt = VT_R4;
        let f = 1.5f32;
        v.payload[..4].copy_from_slice(&f.to_le_bytes());
        let cv = mock_variant_to_com_value(&v).unwrap();
        match cv {
            ComValue::F64(fv) => {
                assert!((fv.as_f64() - 1.5).abs() < 1e-6);
                assert_eq!(fv.subtype(), F64Subtype::Single);
            }
            _ => panic!("Expected F64(Single)"),
        }
    }
}
