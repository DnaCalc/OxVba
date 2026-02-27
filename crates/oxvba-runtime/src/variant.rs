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
pub struct Variant {
    pub vtype: VarType,
    pub reserved1: u16,
    pub reserved2: u16,
    pub reserved3: u16,
    pub data: VariantData,
}

impl core::fmt::Debug for Variant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Variant")
            .field("vtype", &self.vtype)
            .field("reserved1", &self.reserved1)
            .field("reserved2", &self.reserved2)
            .field("reserved3", &self.reserved3)
            .field("data", &self.data_bytes())
            .finish()
    }
}

impl PartialEq for Variant {
    fn eq(&self, other: &Self) -> bool {
        self.vtype == other.vtype
            && self.reserved1 == other.reserved1
            && self.reserved2 == other.reserved2
            && self.reserved3 == other.reserved3
            && self.data_bytes() == other.data_bytes()
    }
}

impl Eq for Variant {}

impl Variant {
    fn from_bytes(vtype: VarType, bytes: [u8; 8]) -> Self {
        Self {
            vtype,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            data: VariantData { bytes },
        }
    }

    fn data_bytes(&self) -> [u8; 8] {
        // SAFETY: `VariantData` is a C union with a `[u8; 8]` member that spans
        // the full storage of the union.
        unsafe { self.data.bytes }
    }
}

impl Variant {
    pub fn empty() -> Self {
        Self::from_bytes(VarType::Empty, [0; 8])
    }

    pub fn from_i16(value: i16) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&value.to_le_bytes());
        Self::from_bytes(VarType::Integer, bytes)
    }

    pub fn as_i16(&self) -> Option<i16> {
        if self.vtype != VarType::Integer {
            return None;
        }
        let bytes = self.data_bytes();
        Some(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn from_i32(value: i32) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.to_le_bytes());
        Self::from_bytes(VarType::Long, bytes)
    }

    pub fn as_i32(&self) -> Option<i32> {
        if self.vtype != VarType::Long {
            return None;
        }
        let bytes = self.data_bytes();
        Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn from_f64(value: f64) -> Self {
        Self::from_bytes(VarType::Double, value.to_le_bytes())
    }

    pub fn as_f64(&self) -> Option<f64> {
        if self.vtype != VarType::Double {
            return None;
        }
        Some(f64::from_le_bytes(self.data_bytes()))
    }

    pub fn from_bool(value: bool) -> Self {
        let mut bytes = [0u8; 8];
        let vb_bool: i16 = if value { -1 } else { 0 };
        bytes[0..2].copy_from_slice(&vb_bool.to_le_bytes());
        Self::from_bytes(VarType::Boolean, bytes)
    }

    pub fn as_bool(&self) -> Option<bool> {
        if self.vtype != VarType::Boolean {
            return None;
        }
        let bytes = self.data_bytes();
        let v = i16::from_le_bytes([bytes[0], bytes[1]]);
        Some(v != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{VarType, Variant, VariantData};

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
    fn com_variant_layout_shape() {
        assert_eq!(core::mem::size_of::<Variant>(), 16);
        assert_eq!(core::mem::size_of::<VariantData>(), 8);
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::Variant;

    #[kani::proof]
    fn com_variant_layout_is_16_bytes() {
        assert_eq!(core::mem::size_of::<Variant>(), 16);
    }
}
