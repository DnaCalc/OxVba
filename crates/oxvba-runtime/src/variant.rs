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
}
