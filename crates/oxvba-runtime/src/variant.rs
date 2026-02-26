#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum VarType {
    Empty = 0x0000,
    Null = 0x0001,
    Integer = 0x0002,
    Long = 0x0003,
    Single = 0x0004,
    Double = 0x0005,
    String = 0x0008,
    Object = 0x0009,
    Boolean = 0x000B,
    Decimal = 0x000E,
    Byte = 0x0011,
    LongLong = 0x0014,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C, align(16))]
pub struct Variant {
    pub vtype: VarType,
    pub payload: [u8; 14],
}

impl Variant {
    pub fn empty() -> Self {
        Self {
            vtype: VarType::Empty,
            payload: [0; 14],
        }
    }

    pub fn from_i16(value: i16) -> Self {
        let mut payload = [0u8; 14];
        payload[0..2].copy_from_slice(&value.to_le_bytes());
        Self {
            vtype: VarType::Integer,
            payload,
        }
    }

    pub fn as_i16(&self) -> Option<i16> {
        if self.vtype != VarType::Integer {
            return None;
        }
        Some(i16::from_le_bytes([self.payload[0], self.payload[1]]))
    }

    pub fn from_i32(value: i32) -> Self {
        let mut payload = [0u8; 14];
        payload[0..4].copy_from_slice(&value.to_le_bytes());
        Self {
            vtype: VarType::Long,
            payload,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        if self.vtype != VarType::Long {
            return None;
        }
        Some(i32::from_le_bytes([
            self.payload[0],
            self.payload[1],
            self.payload[2],
            self.payload[3],
        ]))
    }

    pub fn from_f64(value: f64) -> Self {
        let mut payload = [0u8; 14];
        payload[0..8].copy_from_slice(&value.to_le_bytes());
        Self {
            vtype: VarType::Double,
            payload,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if self.vtype != VarType::Double {
            return None;
        }
        Some(f64::from_le_bytes([
            self.payload[0],
            self.payload[1],
            self.payload[2],
            self.payload[3],
            self.payload[4],
            self.payload[5],
            self.payload[6],
            self.payload[7],
        ]))
    }

    pub fn from_bool(value: bool) -> Self {
        let mut payload = [0u8; 14];
        let vb_bool: i16 = if value { -1 } else { 0 };
        payload[0..2].copy_from_slice(&vb_bool.to_le_bytes());
        Self {
            vtype: VarType::Boolean,
            payload,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if self.vtype != VarType::Boolean {
            return None;
        }
        let v = i16::from_le_bytes([self.payload[0], self.payload[1]]);
        Some(v != 0)
    }

    pub fn from_inline_ascii(value: &str) -> Option<Self> {
        if value.len() > 14 || !value.is_ascii() {
            return None;
        }

        let mut payload = [0u8; 14];
        payload[..value.len()].copy_from_slice(value.as_bytes());
        Some(Self {
            vtype: VarType::String,
            payload,
        })
    }

    pub fn as_inline_ascii(&self) -> Option<String> {
        if self.vtype != VarType::String {
            return None;
        }

        let len = self.payload.iter().position(|b| *b == 0).unwrap_or(14);
        let bytes = &self.payload[..len];
        String::from_utf8(bytes.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{VarType, Variant};

    #[test]
    fn numeric_roundtrip() {
        let i16v = Variant::from_i16(-12);
        assert_eq!(i16v.as_i16(), Some(-12));

        let i32v = Variant::from_i32(1024);
        assert_eq!(i32v.as_i32(), Some(1024));

        let f64v = Variant::from_f64(3.5);
        assert_eq!(f64v.as_f64(), Some(3.5));
    }

    #[test]
    fn boolean_roundtrip_vba_encoding() {
        let t = Variant::from_bool(true);
        let f = Variant::from_bool(false);
        assert_eq!(t.vtype, VarType::Boolean);
        assert_eq!(f.vtype, VarType::Boolean);
        assert_eq!(t.as_bool(), Some(true));
        assert_eq!(f.as_bool(), Some(false));
    }

    #[test]
    fn inline_ascii_sso_roundtrip() {
        let v = Variant::from_inline_ascii("A1").expect("short ascii should fit");
        assert_eq!(v.vtype, VarType::String);
        assert_eq!(v.as_inline_ascii().as_deref(), Some("A1"));
    }
}
