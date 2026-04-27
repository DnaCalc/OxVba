use core::ptr::NonNull;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{SysAllocStringLen, SysFreeString, SysStringLen};

#[cfg(not(target_os = "windows"))]
const BSTR_PREFIX_BYTES: usize = core::mem::size_of::<u32>();
const BSTR_UNIT_BYTES: usize = core::mem::size_of::<u16>();

#[repr(transparent)]
pub struct BStr {
    raw: Option<NonNull<u16>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedBStrCore {
    len_bytes: u32,
    units_with_nul: Box<[u16]>,
}

impl OwnedBStrCore {
    pub fn from_utf8(text: &str) -> Self {
        BStr::from(text).owned_core()
    }

    pub fn from_utf16_lossy(units: &[u16]) -> Self {
        BStr::from_utf16_lossy(units).owned_core()
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
        Self::from_utf16_units(&[]).expect("zero-length BSTR allocation should succeed")
    }

    pub fn from_utf16_lossy(units: &[u16]) -> Self {
        let text = String::from_utf16_lossy(units);
        let units = text.encode_utf16().collect::<Vec<_>>();
        Self::from_utf16_units(&units).expect("BSTR payload length should fit in u32 byte count")
    }

    pub fn from_utf16_units(units: &[u16]) -> Result<Self, String> {
        Ok(Self {
            raw: NonNull::new(alloc_raw_bstr_from_units(units)?),
        })
    }

    /// Construct a BSTR wrapper from an owned raw BSTR pointer.
    ///
    /// # Safety
    ///
    /// `raw` must either be null or point to a valid BSTR allocation owned by
    /// the caller. After this call, the returned `BStr` owns the pointer and
    /// will free it on drop.
    pub unsafe fn from_raw_bstr(raw: *mut u16) -> Self {
        Self {
            raw: NonNull::new(raw),
        }
    }

    pub fn raw_bstr(&self) -> *mut u16 {
        self.raw
            .map(NonNull::as_ptr)
            .unwrap_or(core::ptr::null_mut())
    }

    pub fn clone_raw_bstr(&self) -> Result<*mut u16, String> {
        unsafe { clone_raw_bstr(self.raw_bstr()) }
    }

    pub fn as_str(&self) -> String {
        String::from_utf16_lossy(self.payload_units())
    }

    pub fn into_string(self) -> String {
        self.as_str()
    }

    pub fn len(&self) -> usize {
        self.as_str().len()
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_none() || self.utf16_len() == 0
    }

    pub fn to_utf16_units(&self) -> Vec<u16> {
        self.payload_units().to_vec()
    }

    pub fn utf16_len(&self) -> usize {
        unsafe { raw_bstr_len_units(self.raw_bstr()) }
    }

    pub fn byte_len(&self) -> u32 {
        unsafe { raw_bstr_len_bytes(self.raw_bstr()) }
    }

    pub fn payload_units(&self) -> &[u16] {
        let raw = self.raw_bstr();
        if raw.is_null() {
            return &[];
        }
        unsafe { core::slice::from_raw_parts(raw.cast_const(), raw_bstr_len_units(raw)) }
    }

    pub fn payload_units_with_nul(&self) -> Vec<u16> {
        let mut units = self.to_utf16_units();
        units.push(0);
        units
    }

    pub fn payload_ptr(&self) -> *const u16 {
        self.raw_bstr().cast_const()
    }

    pub fn owned_core(&self) -> OwnedBStrCore {
        let mut units = self.to_utf16_units();
        units.push(0);
        OwnedBStrCore {
            len_bytes: self.byte_len(),
            units_with_nul: units.into_boxed_slice(),
        }
    }
}

unsafe impl Send for BStr {}
unsafe impl Sync for BStr {}

impl Clone for BStr {
    fn clone(&self) -> Self {
        let raw = self
            .clone_raw_bstr()
            .expect("cloning BSTR payload should succeed");
        unsafe { Self::from_raw_bstr(raw) }
    }
}

impl Drop for BStr {
    fn drop(&mut self) {
        if let Some(raw) = self.raw.take() {
            unsafe { free_raw_bstr(raw.as_ptr()) };
        }
    }
}

impl core::fmt::Debug for BStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("BStr").field(&self.as_str()).finish()
    }
}

impl PartialEq for BStr {
    fn eq(&self, other: &Self) -> bool {
        self.payload_units() == other.payload_units()
    }
}

impl Eq for BStr {}

impl From<String> for BStr {
    fn from(value: String) -> Self {
        let units = value.encode_utf16().collect::<Vec<_>>();
        Self::from_utf16_units(&units).expect("BSTR payload length should fit in u32 byte count")
    }
}

impl From<&str> for BStr {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl core::fmt::Display for BStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.as_str())
    }
}

#[cfg(not(target_os = "windows"))]
fn raw_bstr_layout(len_units: usize) -> Result<std::alloc::Layout, String> {
    let payload_bytes = len_units
        .checked_add(1)
        .and_then(|count| count.checked_mul(BSTR_UNIT_BYTES))
        .ok_or_else(|| "BSTR payload size overflow".to_string())?;
    let total = BSTR_PREFIX_BYTES
        .checked_add(payload_bytes)
        .ok_or_else(|| "BSTR allocation size overflow".to_string())?;
    std::alloc::Layout::from_size_align(total, core::mem::align_of::<u32>())
        .map_err(|_| "invalid BSTR allocation layout".to_string())
}

fn alloc_raw_bstr_from_units(units: &[u16]) -> Result<*mut u16, String> {
    #[cfg(target_os = "windows")]
    {
        let len = u32::try_from(units.len())
            .map_err(|_| "BSTR payload length should fit in u32 code-unit count".to_string())?;
        let raw = unsafe { SysAllocStringLen(units.as_ptr(), len) };
        if raw.is_null() {
            return Err("failed to allocate BSTR payload".to_string());
        }
        Ok(raw.cast_mut())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let len_bytes = units
            .len()
            .checked_mul(BSTR_UNIT_BYTES)
            .and_then(|len| u32::try_from(len).ok())
            .ok_or_else(|| "BSTR payload length should fit in u32 byte count".to_string())?;
        let layout = raw_bstr_layout(units.len())?;
        let raw = unsafe { std::alloc::alloc(layout) };
        if raw.is_null() {
            return Err("failed to allocate BSTR payload".to_string());
        }
        unsafe {
            raw.cast::<u32>().write(len_bytes);
            let payload = raw.add(BSTR_PREFIX_BYTES).cast::<u16>();
            core::ptr::copy_nonoverlapping(units.as_ptr(), payload, units.len());
            payload.add(units.len()).write(0);
            Ok(payload)
        }
    }
}

unsafe fn raw_bstr_len_bytes(ptr: *mut u16) -> u32 {
    if ptr.is_null() {
        return 0;
    }
    #[cfg(target_os = "windows")]
    {
        unsafe { SysStringLen(ptr) }.saturating_mul(BSTR_UNIT_BYTES as u32)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let prefix = unsafe { ptr.cast::<u8>().sub(BSTR_PREFIX_BYTES).cast::<u32>() };
        unsafe { *prefix }
    }
}

unsafe fn raw_bstr_len_units(ptr: *mut u16) -> usize {
    usize::try_from(unsafe { raw_bstr_len_bytes(ptr) } / BSTR_UNIT_BYTES as u32).unwrap_or(0)
}

unsafe fn clone_raw_bstr(ptr: *mut u16) -> Result<*mut u16, String> {
    if ptr.is_null() {
        return Ok(core::ptr::null_mut());
    }
    let len = unsafe { raw_bstr_len_units(ptr) };
    let slice = unsafe { core::slice::from_raw_parts(ptr.cast_const(), len) };
    alloc_raw_bstr_from_units(slice)
}

unsafe fn free_raw_bstr(ptr: *mut u16) {
    if ptr.is_null() {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        unsafe { SysFreeString(ptr) };
    }
    #[cfg(not(target_os = "windows"))]
    {
        let len = unsafe { raw_bstr_len_units(ptr) };
        if let Ok(layout) = raw_bstr_layout(len) {
            let raw = unsafe { ptr.cast::<u8>().sub(BSTR_PREFIX_BYTES) };
            unsafe { std::alloc::dealloc(raw, layout) };
        }
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
    fn bstr_is_pointer_sized_native_bstr_owner() {
        assert_eq!(
            core::mem::size_of::<BStr>(),
            core::mem::size_of::<*mut u16>()
        );
        let value = BStr::from("Cafe");
        assert_ne!(value.raw_bstr(), core::ptr::null_mut());
        assert_eq!(value.byte_len(), 8);
        assert_eq!(value.payload_units(), &[67, 97, 102, 101]);
        assert_eq!(value.payload_units_with_nul(), &[67, 97, 102, 101, 0]);
        let prefix = unsafe { value.raw_bstr().cast::<u8>().sub(4).cast::<u32>() };
        assert_eq!(unsafe { *prefix }, 8);
    }

    #[test]
    fn bstr_clone_deep_copies_the_raw_bstr_payload() {
        let value = BStr::from("Cafe");
        let clone = value.clone();
        assert_eq!(value, clone);
        assert_ne!(value.raw_bstr(), clone.raw_bstr());
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
    fn bstr_from_utf16_lossy_uses_native_bstr_path() {
        let value = BStr::from_utf16_lossy(&[0x0041, 0xD83D, 0xDE00]);
        assert_eq!(value.as_str(), "A😀");
        assert!(!value.is_empty());
    }
}
