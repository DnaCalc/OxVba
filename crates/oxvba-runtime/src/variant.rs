use crate::{
    Decimal96,
    bstr::{BStr, OwnedBStrCore},
    com_record::ComRecord,
    object_ref::{ObjectRef, RawRuntimeIUnknown},
    safe_array::SafeArray,
    vba_record::VbaRecord,
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
    SignedByte = 0x0010,
    Byte = 0x0011,
    UnsignedInteger = 0x0012,
    UnsignedLong = 0x0013,
    LongLong = 0x0014,
    UnsignedLongLong = 0x0015,
    UnsignedInt = 0x0017,
    Record = 0x0024,
    ArrayVariant = 0x200C,
    ProcRef = 0xFFF0,
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
            0x0009 | 0x000D => Some(Self::Object),
            0x000A => Some(Self::Error),
            0x000B => Some(Self::Boolean),
            0x000E => Some(Self::Decimal),
            0x0010 => Some(Self::SignedByte),
            0x0011 => Some(Self::Byte),
            0x0012 => Some(Self::UnsignedInteger),
            0x0013 => Some(Self::UnsignedLong),
            0x0014 => Some(Self::LongLong),
            0x0015 => Some(Self::UnsignedLongLong),
            0x0017 => Some(Self::UnsignedInt),
            0x0024 => Some(Self::Record),
            0x200C => Some(Self::ArrayVariant),
            0xFFF0 => Some(Self::ProcRef),
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
        // SAFETY: every `VariantData` in the workspace is constructed through its full
        // 8-byte `bytes` field (`from_bytes`, `from_wire_bytes`, `from_decimal96`), so all
        // 8 bytes are initialized, and any bit pattern is a valid `[u8; 8]`.
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
        if vtype != VarType::Decimal && (reserved1 != 0 || reserved2 != 0 || reserved3 != 0) {
            return Err(format!(
                "malformed VARIANT wire bytes: non-zero reserved words for {vtype:?}"
            ));
        }
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

#[derive(Debug)]
enum RecordPayload {
    Com(ComRecord),
    Vba(VbaRecord),
}

impl RecordPayload {
    fn deep_clone(&self) -> Result<Self, String> {
        match self {
            Self::Com(record) => Ok(Self::Com(record.deep_clone()?)),
            Self::Vba(record) => Ok(Self::Vba(record.clone())),
        }
    }
}

fn raw_record_payload_ptr_to_bytes(ptr: *const RecordPayload) -> [u8; 8] {
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

fn bytes_to_raw_record_payload(bytes: [u8; 8]) -> *const RecordPayload {
    u64::from_le_bytes(bytes) as usize as *const RecordPayload
}

fn alloc_raw_bstr_from_bstr(text: &BStr) -> Result<*mut u16, String> {
    text.clone_raw_bstr()
}

unsafe fn raw_bstr_to_bstr(ptr: *mut u16) -> BStr {
    // SAFETY: per this fn's contract, `ptr` is null or a live BSTR; the wrapper exists
    // only to borrow it for the deep clone below and is then forgotten, so ownership
    // never actually leaves the caller.
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
            VarType::String | VarType::Object | VarType::ArrayVariant | VarType::Record => {
                Err(format!(
                    "pointer-carrying VARIANT wire bytes for {:?} require trusted in-process provenance",
                    core.vtype
                ))
            }
            _ => Ok(Self::from_core(core)),
        }
    }

    /// Rebuilds a Variant from wire bytes that may contain process-local pointer
    /// carriers (`BSTR`, runtime object identity, OxVba-owned SAFEARRAY, or
    /// runtime-owned record payload).
    ///
    /// # Safety
    /// For `String`, `Object`, `ArrayVariant`, and `Record` payloads, the pointer
    /// encoded in the wire bytes must have been produced by `Variant::to_wire_bytes`
    /// in this process and the source allocation/object must still be live for the
    /// whole call. Arbitrary or stale pointer bytes are undefined behavior.
    pub unsafe fn from_trusted_wire_bytes(bytes: [u8; 16]) -> Result<Self, String> {
        let core = VariantCore::from_wire_bytes(bytes)?;
        match core.vtype {
            VarType::String => {
                let ptr = bytes_to_raw_bstr(core.data_bytes());
                // SAFETY: guaranteed by this unsafe fn's caller.
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
                // SAFETY: guaranteed by this unsafe fn's caller.
                let object = unsafe { ObjectRef::from_raw_iunknown_addref(ptr) };
                Ok(match object {
                    Some(value) => Self::from_object_ref(value),
                    None => Self::from_core(VariantCore::from_bytes(VarType::Object, [0; 8])),
                })
            }
            VarType::ArrayVariant => {
                let ptr = bytes_to_raw_safearray(core.data_bytes());
                // SAFETY: guaranteed by this unsafe fn's caller.
                let Some(array) = (unsafe { SafeArray::clone_from_raw_safearray(ptr) }) else {
                    return Ok(Self::from_core(VariantCore::from_bytes(
                        VarType::ArrayVariant,
                        [0; 8],
                    )));
                };
                Ok(Self::from_safearray(array))
            }
            VarType::Record => {
                let ptr = bytes_to_raw_record_payload(core.data_bytes());
                if ptr.is_null() {
                    return Ok(Self::from_core(VariantCore::from_bytes(
                        VarType::Record,
                        [0; 8],
                    )));
                }
                // SAFETY: guaranteed by this unsafe fn's caller.
                let payload = unsafe { (*ptr).deep_clone()? };
                Ok(Self::from_record_payload(payload))
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

    pub fn from_proc_ref(proc: usize) -> Self {
        Self::from_core(VariantCore::from_bytes(
            VarType::ProcRef,
            (proc as u64).to_le_bytes(),
        ))
    }

    pub fn as_proc_ref(&self) -> Option<usize> {
        if self.vtype() != VarType::ProcRef {
            return None;
        }
        usize::try_from(u64::from_le_bytes(self.data_bytes())).ok()
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

    pub fn from_i8(value: i8) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0] = value as u8;
        Self::from_core(VariantCore::from_bytes(VarType::SignedByte, bytes))
    }

    pub fn as_i8(&self) -> Option<i8> {
        if self.vtype() != VarType::SignedByte {
            return None;
        }
        Some(self.data_bytes()[0] as i8)
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

    pub fn from_u16(value: u16) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&value.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::UnsignedInteger, bytes))
    }

    pub fn as_u16(&self) -> Option<u16> {
        if self.vtype() != VarType::UnsignedInteger {
            return None;
        }
        let bytes = self.data_bytes();
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn from_u32(value: u32) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::UnsignedLong, bytes))
    }

    pub fn from_uint(value: u32) -> Self {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&value.to_le_bytes());
        Self::from_core(VariantCore::from_bytes(VarType::UnsignedInt, bytes))
    }

    pub fn as_u32(&self) -> Option<u32> {
        if self.vtype() != VarType::UnsignedLong && self.vtype() != VarType::UnsignedInt {
            return None;
        }
        let bytes = self.data_bytes();
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn from_u64(value: u64) -> Self {
        Self::from_core(VariantCore::from_bytes(
            VarType::UnsignedLongLong,
            value.to_le_bytes(),
        ))
    }

    pub fn as_u64(&self) -> Option<u64> {
        if self.vtype() != VarType::UnsignedLongLong {
            return None;
        }
        Some(u64::from_le_bytes(self.data_bytes()))
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

    /// Build a `String` Variant DIRECTLY from raw UTF-16 code units, preserving any
    /// lone/paired surrogate halves verbatim. Unlike [`from_string`] (which goes
    /// through a Rust `&str`/`String` and so can only carry Unicode SCALAR values),
    /// this is the faithful path for `ChrW(&HD83D)` and code-unit-level string
    /// concatenation — VBA strings are UTF-16 sequences that may hold unpaired
    /// surrogates, which a UTF-8 round-trip would replace with U+FFFD.
    pub fn from_utf16_units(units: &[u16]) -> Self {
        Self::from_string(BStr::from_utf16_units(units).expect("BSTR allocation should succeed"))
    }

    /// The String Variant's raw UTF-16 code units (none for a non-string), preserving
    /// surrogate halves verbatim — the faithful counterpart to a lossy `as_str`.
    pub fn string_units(&self) -> Option<Vec<u16>> {
        self.string_core().map(|core| core.payload_units().to_vec())
    }

    pub fn as_bstr(&self) -> Option<BStr> {
        if self.vtype() != VarType::String {
            return None;
        }
        // SAFETY: vtype was checked to be String above, so the payload is null or the BSTR
        // pointer this Variant owns until drop (a VARIANT owns its BSTR payload until
        // cleared); `raw_bstr_to_bstr` deep-clones without taking that ownership.
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
        // SAFETY: vtype was checked to be Object above, so the payload is null or the
        // IUnknown pointer on which this Variant holds one retained reference until drop,
        // keeping the object alive for the AddRef taken here on behalf of the returned
        // ObjectRef.
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
        // SAFETY: vtype was checked to be ArrayVariant above, so the payload is null or
        // the OxVba SAFEARRAY descriptor this Variant owns until drop (set by
        // `from_safearray`/clone); `clone_from_raw_safearray` verifies the provenance
        // prefix and deep-clones, leaving ownership with this Variant.
        unsafe { SafeArray::clone_from_raw_safearray(bytes_to_raw_safearray(self.data_bytes())) }
    }

    pub fn set_safearray_element(&mut self, index: usize, value: &Variant) -> Result<(), String> {
        if self.vtype() != VarType::ArrayVariant {
            return Err(format!(
                "expected Array Variant for SAFEARRAY element write, got {:?}",
                self.vtype()
            ));
        }
        // SAFETY: `&mut self` proves exclusive access to this Variant. An
        // ArrayVariant owns its raw SAFEARRAY descriptor until drop, so mutating
        // one element through the descriptor preserves ownership and aliases no
        // other safe reference to the same descriptor.
        unsafe {
            SafeArray::set_raw_safearray_variant_element(
                bytes_to_raw_safearray(self.data_bytes()),
                index,
                value,
            )
        }
    }

    pub fn from_com_record(value: ComRecord) -> Self {
        Self::from_record_payload(RecordPayload::Com(value))
    }

    pub fn from_vba_record(value: VbaRecord) -> Self {
        Self::from_record_payload(RecordPayload::Vba(value))
    }

    fn from_record_payload(value: RecordPayload) -> Self {
        let raw = Box::into_raw(Box::new(value));
        Self::from_core(VariantCore::from_bytes(
            VarType::Record,
            raw_record_payload_ptr_to_bytes(raw),
        ))
    }

    fn as_record_payload(&self) -> Option<&RecordPayload> {
        if self.vtype() != VarType::Record {
            return None;
        }
        let raw = bytes_to_raw_record_payload(self.data_bytes());
        if raw.is_null() {
            return None;
        }
        // SAFETY: vtype was checked to be Record, so the payload is a
        // Box<RecordPayload> pointer produced by `from_record_payload` and owned by
        // this Variant until drop.
        Some(unsafe { &*raw })
    }

    pub fn as_com_record(&self) -> Option<ComRecord> {
        match self.as_record_payload()? {
            RecordPayload::Com(record) => Some(record.clone()),
            RecordPayload::Vba(_) => None,
        }
    }

    pub fn as_vba_record(&self) -> Option<VbaRecord> {
        match self.as_record_payload()? {
            RecordPayload::Com(_) => None,
            RecordPayload::Vba(record) => Some(record.clone()),
        }
    }
}

impl Clone for Variant {
    fn clone(&self) -> Self {
        match self.vtype() {
            VarType::String => {
                // SAFETY: this arm is reached only when vtype is String, so the payload is
                // null or the live BSTR this Variant owns until drop; `raw_bstr_to_bstr`
                // deep-clones it without disturbing that ownership.
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
            VarType::Record => match self.as_record_payload() {
                Some(payload) => Self::from_record_payload(
                    payload
                        .deep_clone()
                        .expect("record Variant clone should deep-copy record payload"),
                ),
                None => Self::from_core(self.core),
            },
            _ => Self::from_core(self.core),
        }
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        match self.vtype() {
            // SAFETY: a String Variant owns its BSTR payload until drop (VARIANT owns its
            // BSTR until cleared), so reconstituting the `BStr` here takes that ownership
            // and its drop frees the allocation exactly once.
            VarType::String => unsafe {
                let _ = BStr::from_raw_bstr(bytes_to_raw_bstr(self.data_bytes()));
            },
            VarType::Object => {
                let raw = bytes_to_raw_iunknown(self.data_bytes());
                if !raw.is_null() {
                    // SAFETY: `raw` was checked non-null above and an Object Variant holds
                    // exactly one retained reference taken at construction
                    // (`from_object_ref` / `from_raw_iunknown_addref`), so the object is
                    // still alive — every COM object's first field is its vtbl — and this
                    // release balances that AddRef.
                    unsafe {
                        let vtbl = (*raw).vtbl;
                        ((*vtbl).release)(raw.cast());
                    }
                }
            }
            VarType::ArrayVariant => {
                let raw = bytes_to_raw_safearray(self.data_bytes());
                // SAFETY: an ArrayVariant payload was produced by moving an OxVba
                // SafeArray into this Variant (`from_safearray`/clone), satisfying
                // `from_raw_safearray_owned`'s provenance contract; ownership transfers
                // back here, so the descriptor is freed exactly once.
                if let Some(array) = unsafe { SafeArray::from_raw_safearray_owned(raw) } {
                    drop(array);
                }
            }
            VarType::Record => {
                let raw = bytes_to_raw_record_payload(self.data_bytes()).cast_mut();
                if !raw.is_null() {
                    // SAFETY: a Record Variant owns a Box<RecordPayload> created by
                    // `from_record_payload`; taking the box here drops that payload.
                    unsafe {
                        drop(Box::from_raw(raw));
                    }
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
            VarType::Record => {
                dbg.field("record", &self.as_record_payload());
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
            VarType::Record => match (self.as_record_payload(), other.as_record_payload()) {
                (Some(RecordPayload::Com(left)), Some(RecordPayload::Com(right))) => left == right,
                (Some(RecordPayload::Vba(left)), Some(RecordPayload::Vba(right))) => {
                    left.data_ptr() == right.data_ptr()
                }
                (None, None) => true,
                _ => false,
            },
            _ => self.data_bytes() == other.data_bytes(),
        }
    }
}

impl Eq for Variant {}

#[cfg(test)]
mod tests {
    use core::ffi::c_void;

    use crate::{
        Decimal96, VbaRecord, VbaRecordFieldKind, VbaRecordFieldSpec, VbaRecordLayout, bstr::BStr,
    };

    use super::{VarType, Variant, VariantCore, VariantData};

    fn offset_of<T, U>(field: fn(*const T) -> *const U) -> usize {
        let value = core::mem::MaybeUninit::<T>::uninit();
        let base = value.as_ptr();
        field(base) as usize - base as usize
    }

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
        assert_eq!(core::mem::align_of::<VariantCore>(), 8);
        assert_eq!(core::mem::size_of::<VariantData>(), 8);
        assert_eq!(core::mem::align_of::<VariantData>(), 8);
        assert_eq!(core::mem::size_of::<Variant>(), 16);
        assert_eq!(core::mem::align_of::<Variant>(), 8);
        assert_eq!(
            offset_of::<VariantCore, _>(|base| unsafe { core::ptr::addr_of!((*base).vtype) }),
            0
        );
        assert_eq!(
            offset_of::<VariantCore, _>(|base| unsafe { core::ptr::addr_of!((*base).reserved1) }),
            2
        );
        assert_eq!(
            offset_of::<VariantCore, _>(|base| unsafe { core::ptr::addr_of!((*base).reserved2) }),
            4
        );
        assert_eq!(
            offset_of::<VariantCore, _>(|base| unsafe { core::ptr::addr_of!((*base).reserved3) }),
            6
        );
        assert_eq!(
            offset_of::<VariantCore, _>(|base| unsafe { core::ptr::addr_of!((*base).data) }),
            8
        );
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
        // SAFETY: `wire` was just produced from `original`, which stays live for
        // the full call.
        let roundtrip = unsafe { Variant::from_trusted_wire_bytes(wire) }.expect("wire roundtrip");
        assert_eq!(roundtrip.as_bstr(), Some(BStr::from("A\0BC")));
    }

    #[test]
    fn array_variant_set_element_preserves_owned_safearray_pointer() {
        let array = crate::safe_array::SafeArray::from_variants(vec![Variant::from_i32(1)]);
        let mut value = Variant::from_safearray(array);
        let raw_before = u64::from_le_bytes(value.data_bytes());

        value
            .set_safearray_element(0, &Variant::from_i32(2))
            .expect("set element");

        assert_eq!(u64::from_le_bytes(value.data_bytes()), raw_before);
        assert_eq!(
            value
                .as_safearray()
                .and_then(|array| array.variant_elements()),
            Some(vec![Variant::from_i32(2)])
        );
    }

    unsafe fn clone_test_record(
        record_info: *mut c_void,
        record_data: *const c_void,
    ) -> Result<(*mut c_void, *mut c_void), String> {
        let value = unsafe { *record_data.cast::<i32>() };
        Ok((record_info, Box::into_raw(Box::new(value)).cast()))
    }

    unsafe fn destroy_test_record(_record_info: *mut c_void, record_data: *mut c_void) {
        unsafe {
            drop(Box::from_raw(record_data.cast::<i32>()));
        }
    }

    fn test_record_variant(value: i32) -> Variant {
        let record_info = core::ptr::NonNull::<u8>::dangling()
            .as_ptr()
            .cast::<c_void>();
        let record_data = Box::into_raw(Box::new(value)).cast::<c_void>();
        let record = unsafe {
            crate::ComRecord::from_raw_parts(
                record_info,
                record_data,
                clone_test_record,
                destroy_test_record,
            )
            .expect("test record")
        };
        Variant::from_com_record(record)
    }

    #[test]
    fn record_variant_clone_deep_copies_record_payload() {
        let original = test_record_variant(41);
        let cloned = original.clone();
        let original_record = original.as_com_record().expect("original record");
        let cloned_record = cloned.as_com_record().expect("cloned record");

        assert_ne!(
            original_record.record_data_ptr(),
            cloned_record.record_data_ptr()
        );
        assert_eq!(
            unsafe { *original_record.record_data_ptr().cast::<i32>() },
            41
        );
        assert_eq!(
            unsafe { *cloned_record.record_data_ptr().cast::<i32>() },
            41
        );
        unsafe {
            *original_record.record_data_ptr().cast::<i32>() = 99;
        }
        assert_eq!(
            unsafe { *original_record.record_data_ptr().cast::<i32>() },
            99
        );
        assert_eq!(
            unsafe { *cloned_record.record_data_ptr().cast::<i32>() },
            41
        );
    }

    #[test]
    fn vba_record_variant_clone_deep_copies_native_payload() {
        let layout = std::sync::Arc::new(
            VbaRecordLayout::new(vec![VbaRecordFieldSpec::named(
                "Value",
                VbaRecordFieldKind::Long,
            )])
            .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");
        let field = record.layout().fields()[0].clone();
        unsafe {
            record.field_mut_ptr(&field).cast::<i32>().write(41);
        }
        let original = Variant::from_vba_record(record);
        let cloned = original.clone();

        assert!(original.as_com_record().is_none());
        assert!(cloned.as_com_record().is_none());
        let original_record = original.as_vba_record().expect("original record");
        let cloned_record = cloned.as_vba_record().expect("cloned record");
        assert_ne!(original_record.data_ptr(), cloned_record.data_ptr());
        assert_eq!(
            original_record
                .read_field_variant(0)
                .expect("original field")
                .as_i32(),
            Some(41)
        );
        assert_eq!(
            cloned_record
                .read_field_variant(0)
                .expect("cloned field")
                .as_i32(),
            Some(41)
        );
    }

    #[test]
    fn safe_variant_wire_rejects_pointer_carriers() {
        let wire = Variant::from_string("A\0BC").to_wire_bytes();
        assert_eq!(
            Variant::from_wire_bytes(wire).expect_err("safe wire must reject pointers"),
            "pointer-carrying VARIANT wire bytes for String require trusted in-process provenance"
        );
    }

    #[test]
    fn variant_wire_rejects_non_decimal_reserved_words() {
        let mut wire = Variant::from_i32(42).to_wire_bytes();
        wire[2..4].copy_from_slice(&1u16.to_le_bytes());

        assert_eq!(
            Variant::from_wire_bytes(wire).expect_err("reserved words must be zero"),
            "malformed VARIANT wire bytes: non-zero reserved words for Long"
        );
    }

    #[test]
    fn decimal_variant_wire_accepts_reserved_payload_words() {
        let original =
            Variant::from_decimal96(Decimal96::from_parts(123_450, 7, 0x0002_0001, 3, true));
        let roundtrip = Variant::from_wire_bytes(original.to_wire_bytes()).expect("decimal wire");

        assert_eq!(
            roundtrip.as_decimal96(),
            Some(Decimal96::from_parts(123_450, 7, 0x0002_0001, 3, true))
        );
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
