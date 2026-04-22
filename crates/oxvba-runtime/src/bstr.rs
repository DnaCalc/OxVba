#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BStr(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedBStrCore {
    len_bytes: u32,
    units_with_nul: Box<[u16]>,
}

impl OwnedBStrCore {
    pub fn from_utf8(text: &str) -> Self {
        let mut units = text.encode_utf16().collect::<Vec<u16>>();
        let len_bytes = units
            .len()
            .checked_mul(core::mem::size_of::<u16>())
            .and_then(|len| u32::try_from(len).ok())
            .expect("BSTR payload length should fit in u32 byte count");
        units.push(0);
        Self {
            len_bytes,
            units_with_nul: units.into_boxed_slice(),
        }
    }

    pub fn from_utf16_lossy(units: &[u16]) -> Self {
        let text = String::from_utf16_lossy(units);
        Self::from_utf8(&text)
    }

    pub fn len_bytes(&self) -> u32 {
        self.len_bytes
    }

    pub fn len_code_units(&self) -> usize {
        self.units_with_nul.len().saturating_sub(1)
    }

    pub fn payload_units(&self) -> &[u16] {
        &self.units_with_nul[..self.len_code_units()]
    }

    pub fn payload_units_with_nul(&self) -> &[u16] {
        &self.units_with_nul
    }

    pub fn payload_ptr(&self) -> *const u16 {
        self.units_with_nul.as_ptr()
    }

    pub fn to_utf8_lossy(&self) -> String {
        String::from_utf16_lossy(self.payload_units())
    }
}

impl BStr {
    pub fn empty() -> Self {
        Self(String::new())
    }

    pub fn from_utf16_lossy(units: &[u16]) -> Self {
        Self(OwnedBStrCore::from_utf16_lossy(units).to_utf8_lossy())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_utf16_units(&self) -> Vec<u16> {
        self.0.encode_utf16().collect()
    }

    pub fn utf16_len(&self) -> usize {
        self.0.encode_utf16().count()
    }

    pub fn byte_len(&self) -> u32 {
        self.owned_core().len_bytes()
    }

    pub fn owned_core(&self) -> OwnedBStrCore {
        OwnedBStrCore::from_utf8(&self.0)
    }
}

impl From<String> for BStr {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BStr {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl core::fmt::Display for BStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{BStr, OwnedBStrCore};

    #[test]
    fn owned_bstr_core_from_utf8_tracks_utf16_payload_and_nul() {
        let core = OwnedBStrCore::from_utf8("Hello");
        assert_eq!(core.len_code_units(), 5);
        assert_eq!(core.len_bytes(), 10);
        assert_eq!(core.payload_units(), &[72, 101, 108, 108, 111]);
        assert_eq!(core.payload_units_with_nul(), &[72, 101, 108, 108, 111, 0]);
        assert_ne!(core.payload_ptr(), core::ptr::null());
        assert_eq!(core.to_utf8_lossy(), "Hello");
    }

    #[test]
    fn owned_bstr_core_preserves_non_bmp_utf16_width() {
        let core = OwnedBStrCore::from_utf8("A😀");
        assert_eq!(core.len_code_units(), 3);
        assert_eq!(core.len_bytes(), 6);
        assert_eq!(core.payload_units_with_nul(), &[65, 0xD83D, 0xDE00, 0]);
        assert_eq!(core.to_utf8_lossy(), "A😀");
    }

    #[test]
    fn owned_bstr_core_from_utf16_lossy_roundtrips_lossy_text() {
        let core = OwnedBStrCore::from_utf16_lossy(&[0x0041, 0xD83D, 0xDE00]);
        assert_eq!(core.payload_units_with_nul(), &[0x0041, 0xD83D, 0xDE00, 0]);
        assert_eq!(core.to_utf8_lossy(), "A😀");
    }

    #[test]
    fn bstr_exposes_windows_style_owned_core_view() {
        let value = BStr::from("Cafe");
        let core = value.owned_core();
        assert_eq!(value.as_str(), "Cafe");
        assert_eq!(value.byte_len(), 8);
        assert_eq!(value.to_utf16_units(), &[67, 97, 102, 101]);
        assert_eq!(core.payload_units_with_nul(), &[67, 97, 102, 101, 0]);
    }

    #[test]
    fn bstr_from_utf16_lossy_uses_owned_core_path() {
        let value = BStr::from_utf16_lossy(&[0x0041, 0xD83D, 0xDE00]);
        assert_eq!(value.as_str(), "A😀");
        assert!(!value.is_empty());
    }
}
