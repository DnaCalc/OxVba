#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decimal96 {
    pub lo: u32,
    pub mid: u32,
    pub hi: u32,
    pub scale_sign: u16,
}
