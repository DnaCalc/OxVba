use crate::{
    Decimal96, VarType, Variant, VbaRecord, VbaRecordLayout,
    bstr::BStr,
    object_ref::{ObjectRef, RawRuntimeIUnknown},
};
use core::ptr::NonNull;
use std::sync::Arc;

pub const ARRAY_TAG_BASE: i32 = -1_000_000_000;
pub const ARRAY_TAG_LIMIT: i32 = ARRAY_TAG_BASE + 1_000_000;
const DISPATCH_ARRAY_PAYLOAD_BASE: i32 = 20_000;

pub const VT_I2_VALUE: u16 = 0x0002;
pub const VT_I4_VALUE: u16 = 0x0003;
pub const VT_R4_VALUE: u16 = 0x0004;
pub const VT_R8_VALUE: u16 = 0x0005;
pub const VT_CY_VALUE: u16 = 0x0006;
pub const VT_DATE_VALUE: u16 = 0x0007;
pub const VT_BSTR_VALUE: u16 = 0x0008;
pub const VT_DISPATCH_VALUE: u16 = 0x0009;
pub const VT_BOOL_VALUE: u16 = 0x000B;
pub const VT_VARIANT_VALUE: u16 = 0x000C;
pub const VT_UNKNOWN_VALUE: u16 = 0x000D;
pub const VT_DECIMAL_VALUE: u16 = 0x000E;
pub const VT_I1_VALUE: u16 = 0x0010;
pub const VT_UI1_VALUE: u16 = 0x0011;
pub const VT_UI2_VALUE: u16 = 0x0012;
pub const VT_UI4_VALUE: u16 = 0x0013;
pub const VT_I8_VALUE: u16 = 0x0014;
pub const VT_UI8_VALUE: u16 = 0x0015;
pub const VT_INT_VALUE: u16 = 0x0016;
pub const VT_UINT_VALUE: u16 = 0x0017;
pub const VT_RECORD_VALUE: u16 = 0x0024;

pub const FADF_HAVEVARTYPE_VALUE: u16 = 0x0080;
pub const FADF_RECORD_VALUE: u16 = 0x0020;
/// `FADF_FIXEDSIZE` (0x0010): the SAFEARRAY may not be resized or reallocated.
/// VBA sets this on a fixed-size array (`Dim a(1 To 3)`), where `Erase`
/// reinitializes each element to its type default and keeps the array, in
/// contrast to a dynamic array (`Dim a()` + `ReDim`) whose `Erase` deallocates.
pub const FADF_FIXEDSIZE_VALUE: u16 = 0x0010;
pub const FADF_BSTR_VALUE: u16 = 0x0100;
pub const FADF_UNKNOWN_VALUE: u16 = 0x0200;
pub const FADF_DISPATCH_VALUE: u16 = 0x0400;
pub const FADF_VARIANT_VALUE: u16 = 0x0800;
const OXVBA_SAFEARRAY_OWNER_MAGIC: u32 = u32::from_le_bytes(*b"OVSA");
const OXVBA_SAFEARRAY_OWNER_VERSION: u16 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeArrayBound {
    pub count: u32,
    pub lower: i32,
}

#[repr(C, align(8))]
struct RawSafeArrayOwnerPrefix {
    // COM Automation stores HAVEVARTYPE metadata adjacent to the descriptor.
    // The public descriptor pointer still starts at RawSafeArray.
    magic: u32,
    version: u16,
    element_vt: u16,
    // Non-null only for OxVba-owned native record SAFEARRAYs. This is not an
    // IRecordInfo pointer and must not be projected as one at COM boundaries.
    record_layout: *const VbaRecordLayout,
}

#[repr(C)]
struct RawSafeArray {
    c_dims: u16,
    f_features: u16,
    cb_elements: u32,
    c_locks: u32,
    pv_data: *mut core::ffi::c_void,
    rgsabound: [SafeArrayBound; 1],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawDecimalArrayElement {
    reserved: u16,
    scale: u8,
    sign: u8,
    hi32: u32,
    lo32: u32,
    mid32: u32,
}

impl RawDecimalArrayElement {
    fn from_decimal96(value: Decimal96) -> Self {
        Self {
            reserved: 0,
            scale: value.scale(),
            sign: if value.is_negative() { 0x80 } else { 0 },
            hi32: value.hi,
            lo32: value.lo,
            mid32: value.mid,
        }
    }

    fn to_decimal96(self) -> Decimal96 {
        Decimal96::from_scale_sign(
            self.lo32,
            self.mid32,
            self.hi32,
            (u16::from(self.sign) << 8) | u16::from(self.scale),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeArrayElementKind {
    Variant,
    I1,
    Ui1,
    I2,
    Ui2,
    I4,
    Ui4,
    I8,
    Ui8,
    Int,
    UInt,
    R4,
    R8,
    Currency,
    Date,
    Bool,
    BStr,
    Dispatch,
    Unknown,
    Decimal,
}

impl SafeArrayElementKind {
    fn from_vartype(value: u16) -> Option<Self> {
        match value {
            VT_VARIANT_VALUE => Some(Self::Variant),
            VT_I1_VALUE => Some(Self::I1),
            VT_UI1_VALUE => Some(Self::Ui1),
            VT_I2_VALUE => Some(Self::I2),
            VT_UI2_VALUE => Some(Self::Ui2),
            VT_I4_VALUE => Some(Self::I4),
            VT_UI4_VALUE => Some(Self::Ui4),
            VT_I8_VALUE => Some(Self::I8),
            VT_UI8_VALUE => Some(Self::Ui8),
            VT_INT_VALUE => Some(Self::Int),
            VT_UINT_VALUE => Some(Self::UInt),
            VT_R4_VALUE => Some(Self::R4),
            VT_R8_VALUE => Some(Self::R8),
            VT_CY_VALUE => Some(Self::Currency),
            VT_DATE_VALUE => Some(Self::Date),
            VT_BOOL_VALUE => Some(Self::Bool),
            VT_BSTR_VALUE => Some(Self::BStr),
            VT_DISPATCH_VALUE => Some(Self::Dispatch),
            VT_UNKNOWN_VALUE => Some(Self::Unknown),
            VT_DECIMAL_VALUE => Some(Self::Decimal),
            _ => None,
        }
    }

    fn element_size(self) -> usize {
        match self {
            Self::Variant => core::mem::size_of::<Variant>(),
            Self::I1 | Self::Ui1 => 1,
            Self::I2 | Self::Ui2 | Self::Bool => 2,
            Self::I4 | Self::Ui4 | Self::Int | Self::UInt | Self::R4 => 4,
            Self::I8
            | Self::Ui8
            | Self::R8
            | Self::Currency
            | Self::Date
            | Self::BStr
            | Self::Dispatch
            | Self::Unknown => 8,
            Self::Decimal => core::mem::size_of::<RawDecimalArrayElement>(),
        }
    }

    fn alignment(self) -> usize {
        match self {
            Self::Variant => core::mem::align_of::<Variant>(),
            Self::I1 | Self::Ui1 => core::mem::align_of::<u8>(),
            Self::I2 | Self::Ui2 | Self::Bool => core::mem::align_of::<u16>(),
            Self::I4 | Self::Ui4 | Self::Int | Self::UInt | Self::R4 => {
                core::mem::align_of::<u32>()
            }
            Self::I8
            | Self::Ui8
            | Self::R8
            | Self::Currency
            | Self::Date
            | Self::BStr
            | Self::Dispatch
            | Self::Unknown => core::mem::align_of::<u64>(),
            Self::Decimal => core::mem::align_of::<RawDecimalArrayElement>(),
        }
    }

    fn feature_flags(self) -> u16 {
        FADF_HAVEVARTYPE_VALUE
            | match self {
                Self::Variant => FADF_VARIANT_VALUE,
                Self::BStr => FADF_BSTR_VALUE,
                Self::Dispatch => FADF_DISPATCH_VALUE,
                Self::Unknown => FADF_UNKNOWN_VALUE,
                _ => 0,
            }
    }
}

#[repr(transparent)]
pub struct SafeArray(NonNull<RawSafeArray>);

// SAFETY: ASSUMPTION — `SafeArray` exclusively owns its descriptor and payload
// allocations (created in `alloc_header`/`alloc_payload_from_variants`, freed
// only in `Drop`), and scalar/BStr elements are plain data or independently
// owned allocations, so those move freely between threads. Object
// (VT_DISPATCH/VT_UNKNOWN) elements hold a retained reference whose
// AddRef/Release counter is an `AtomicU32`, but the FINAL release parks the
// object in a `thread_local!` termination queue and touches `Cell` state
// (`compat_release` in object_ref.rs), so soundness additionally requires that
// arrays holding object elements are dropped on the VM thread that manages
// those objects' lifecycle.
unsafe impl Send for SafeArray {}
// SAFETY: no `&self` method mutates the descriptor or payload (this module has
// no interior mutability; `c_locks` is never toggled after construction), so
// shared access performs only reads plus atomic AddRefs when object elements
// are cloned out — data-race-free.
unsafe impl Sync for SafeArray {}

fn alloc_raw_bstr_from_bstr(text: &BStr) -> Result<*mut u16, String> {
    text.clone_raw_bstr()
}

unsafe fn free_raw_bstr(ptr: *mut u16) {
    // SAFETY: the caller transfers ownership of `ptr` (null or the raw BSTR a
    // SAFEARRAY slot owned, written there by `encode_element_variant`), which
    // is exactly `BStr::from_raw_bstr`'s contract; dropping the wrapper frees
    // the allocation exactly once.
    let _ = unsafe { BStr::from_raw_bstr(ptr) };
}

unsafe fn raw_bstr_to_bstr(ptr: *mut u16) -> BStr {
    // SAFETY: the caller passes a live raw BSTR whose ownership stays with the
    // SAFEARRAY slot; the wrapper adopts it only long enough to deep-copy the
    // UTF-16 payload and is `mem::forget`-ten below, so the slot's allocation
    // is never freed here.
    let text = unsafe { BStr::from_raw_bstr(ptr) };
    let cloned = text.clone();
    core::mem::forget(text);
    cloned
}

fn bounds_layout(dimensions: usize) -> Result<std::alloc::Layout, String> {
    if dimensions == 0 {
        return Err("SAFEARRAY must have at least one dimension".to_string());
    }
    let header = std::alloc::Layout::new::<RawSafeArray>();
    let extra = dimensions
        .checked_sub(1)
        .ok_or_else(|| "SAFEARRAY dimension underflow".to_string())?;
    let extra_bounds = std::alloc::Layout::array::<SafeArrayBound>(extra)
        .map_err(|_| "SAFEARRAY bounds layout overflow".to_string())?;
    header
        .extend(extra_bounds)
        .map(|(layout, _)| layout.pad_to_align())
        .map_err(|_| "SAFEARRAY header layout overflow".to_string())
}

fn owner_layout(dimensions: usize) -> Result<std::alloc::Layout, String> {
    let header = bounds_layout(dimensions)?;
    let total = core::mem::size_of::<RawSafeArrayOwnerPrefix>()
        .checked_add(header.size())
        .ok_or_else(|| "SAFEARRAY owner layout overflow".to_string())?;
    std::alloc::Layout::from_size_align(total, core::mem::align_of::<RawSafeArray>())
        .map_err(|_| "SAFEARRAY owner layout invalid".to_string())
}

fn payload_layout(kind: SafeArrayElementKind, count: usize) -> Result<std::alloc::Layout, String> {
    let size = kind
        .element_size()
        .checked_mul(count)
        .ok_or_else(|| "SAFEARRAY payload layout overflow".to_string())?;
    std::alloc::Layout::from_size_align(size.max(1), kind.alignment())
        .map_err(|_| "SAFEARRAY payload layout invalid".to_string())
}

fn record_payload_layout(
    layout: &VbaRecordLayout,
    count: usize,
) -> Result<std::alloc::Layout, String> {
    let size = layout
        .size()
        .checked_mul(count)
        .ok_or_else(|| "SAFEARRAY record payload layout overflow".to_string())?;
    std::alloc::Layout::from_size_align(size.max(1), layout.align())
        .map_err(|_| "SAFEARRAY record payload layout invalid".to_string())
}

fn default_bounds_for_len(len: usize) -> Result<Vec<SafeArrayBound>, String> {
    Ok(vec![SafeArrayBound {
        count: u32::try_from(len).map_err(|_| {
            format!("SAFEARRAY length {len} exceeds supported u32 element capacity")
        })?,
        lower: 0,
    }])
}

fn bounds_total_len(bounds: &[SafeArrayBound]) -> Result<usize, String> {
    let mut total = 1usize;
    for bound in bounds {
        total = total
            .checked_mul(bound.count as usize)
            .ok_or_else(|| "SAFEARRAY total element count overflowed".to_string())?;
    }
    Ok(total)
}

fn header_prefix_ptr(header: *const RawSafeArray) -> *const RawSafeArrayOwnerPrefix {
    // SAFETY: every descriptor reaching this helper was allocated by
    // `alloc_header` with `owner_layout`, which places a
    // `RawSafeArrayOwnerPrefix` immediately before the `RawSafeArray` inside
    // the same allocation (callers adopting raw pointers promise OxVba
    // provenance per the `from_raw_*` contracts), so the `sub` stays in-bounds.
    unsafe {
        header
            .cast::<u8>()
            .sub(core::mem::size_of::<RawSafeArrayOwnerPrefix>())
            .cast::<RawSafeArrayOwnerPrefix>()
    }
}

unsafe fn validated_header_prefix(
    header: *const RawSafeArray,
) -> Option<*const RawSafeArrayOwnerPrefix> {
    let prefix = header_prefix_ptr(header);
    // SAFETY: callers guarantee `header` points at a live OxVba descriptor
    // allocated by `alloc_header`, whose owner allocation initializes the
    // prefix before the header pointer is published, so this read is in-bounds
    // and initialized for the duration of the borrow.
    let prefix_ref = unsafe { &*prefix };
    if prefix_ref.magic == OXVBA_SAFEARRAY_OWNER_MAGIC
        && prefix_ref.version == OXVBA_SAFEARRAY_OWNER_VERSION
    {
        Some(prefix)
    } else {
        None
    }
}

unsafe fn header_record_layout(header: *const RawSafeArray) -> Option<Arc<VbaRecordLayout>> {
    let prefix = unsafe { validated_header_prefix(header)? };
    let raw = unsafe { (*prefix).record_layout };
    if raw.is_null() {
        return None;
    }
    // SAFETY: non-null record_layout pointers are stored from Arc::into_raw in
    // alloc_header and released when the SAFEARRAY descriptor is dropped.
    unsafe { Arc::increment_strong_count(raw) };
    Some(unsafe { Arc::from_raw(raw) })
}

fn payload_offset(kind: SafeArrayElementKind, index: usize) -> usize {
    kind.element_size() * index
}

fn record_payload_offset(layout: &VbaRecordLayout, index: usize) -> usize {
    layout.size() * index
}

fn raw_iunknown_ptr_to_bytes(ptr: *mut RawRuntimeIUnknown) -> [u8; 8] {
    (ptr as usize as u64).to_le_bytes()
}

fn bytes_to_raw_iunknown(bytes: [u8; 8]) -> *mut RawRuntimeIUnknown {
    u64::from_le_bytes(bytes) as usize as *mut RawRuntimeIUnknown
}

fn variant_i64(value: &Variant) -> Result<i64, String> {
    if matches!(value.vtype(), VarType::Empty) {
        return Ok(0);
    }
    value
        .as_i64()
        .or_else(|| value.as_i32().map(i64::from))
        .or_else(|| value.as_i16().map(i64::from))
        .or_else(|| value.as_u8().map(i64::from))
        .ok_or_else(|| format!("expected integer-compatible SAFEARRAY element, got {value:?}"))
}

fn variant_f64(value: &Variant) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| value.as_f32().map(f64::from))
        .or_else(|| value.as_date_f64())
        .ok_or_else(|| format!("expected floating-point SAFEARRAY element, got {value:?}"))
}

unsafe fn decode_element_variant(
    kind: SafeArrayElementKind,
    payload: *const u8,
    index: usize,
) -> Result<Variant, String> {
    // SAFETY: the caller guarantees `payload` is the live element buffer built
    // by `alloc_payload_from_variants` for this same `kind`, holding more than
    // `index` fully initialized elements, so the offset stays inside that
    // allocation and `ptr` is aligned for the element type matched below.
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    Ok(match kind {
        // SAFETY: kind == Variant, so this slot was initialized as a `Variant`
        // at encode time; it is only borrowed here long enough to clone.
        SafeArrayElementKind::Variant => unsafe { &*ptr.cast::<Variant>() }.clone(),
        // SAFETY: kind == I1, so encode initialized this slot as an i8.
        SafeArrayElementKind::I1 => Variant::from_i16(i16::from(unsafe { *ptr.cast::<i8>() })),
        // SAFETY: kind == Ui1, so encode initialized this slot as a u8.
        SafeArrayElementKind::Ui1 => Variant::from_u8(unsafe { *ptr.cast::<u8>() }),
        // SAFETY: kind == I2, so encode initialized this slot as an i16.
        SafeArrayElementKind::I2 => Variant::from_i16(unsafe { *ptr.cast::<i16>() }),
        // SAFETY: kind == Ui2, so encode initialized this slot as a u16.
        SafeArrayElementKind::Ui2 => Variant::from_i32(i32::from(unsafe { *ptr.cast::<u16>() })),
        SafeArrayElementKind::I4 | SafeArrayElementKind::Int => {
            // SAFETY: kind == I4/Int, so encode initialized this slot as an i32.
            Variant::from_i32(unsafe { *ptr.cast::<i32>() })
        }
        SafeArrayElementKind::Ui4 | SafeArrayElementKind::UInt => {
            // SAFETY: kind == Ui4/UInt, so encode initialized this slot as a u32.
            Variant::from_i64(i64::from(unsafe { *ptr.cast::<u32>() }))
        }
        // SAFETY: kind == I8, so encode initialized this slot as an i64.
        SafeArrayElementKind::I8 => Variant::from_i64(unsafe { *ptr.cast::<i64>() }),
        SafeArrayElementKind::Ui8 => {
            // SAFETY: kind == Ui8, so encode initialized this slot as a u64.
            let value = unsafe { *ptr.cast::<u64>() };
            Variant::from_i64(i64::try_from(value).map_err(|_| {
                format!("VT_UI8 SAFEARRAY element {value} exceeds i64 carrier range")
            })?)
        }
        // SAFETY: kind == R4, so encode initialized this slot as an f32.
        SafeArrayElementKind::R4 => Variant::from_f32(unsafe { *ptr.cast::<f32>() }),
        // SAFETY: kind == R8, so encode initialized this slot as an f64.
        SafeArrayElementKind::R8 => Variant::from_f64(unsafe { *ptr.cast::<f64>() }),
        SafeArrayElementKind::Currency => {
            // SAFETY: kind == Currency, so encode initialized this slot as the
            // scaled-i64 currency carrier.
            Variant::from_currency_scaled_i64(unsafe { *ptr.cast::<i64>() })
        }
        // SAFETY: kind == Date, so encode initialized this slot as an f64 date serial.
        SafeArrayElementKind::Date => Variant::from_date_f64(unsafe { *ptr.cast::<f64>() }),
        // SAFETY: kind == Bool, so encode initialized this slot as a VARIANT_BOOL i16.
        SafeArrayElementKind::Bool => Variant::from_bool(unsafe { *ptr.cast::<i16>() } != 0),
        SafeArrayElementKind::BStr => {
            // SAFETY: kind == BStr, so this slot holds the raw BSTR pointer the
            // array owns (written at encode time); `raw_bstr_to_bstr` borrows it
            // only long enough to deep-copy, leaving ownership in the slot.
            Variant::from_string(unsafe { raw_bstr_to_bstr(*ptr.cast::<*mut u16>()) })
        }
        SafeArrayElementKind::Dispatch | SafeArrayElementKind::Unknown => {
            // SAFETY: kind == Dispatch/Unknown, so encode stored the 8
            // little-endian pointer bytes of a retained runtime IUnknown here.
            let raw = bytes_to_raw_iunknown(unsafe { *ptr.cast::<[u8; 8]>() });
            // SAFETY: `raw` is null or the runtime IUnknown the array still
            // holds its own retained reference on, so it is live here;
            // `from_raw_iunknown_addref` takes an additional independent
            // reference per its contract.
            let Some(object) = (unsafe { ObjectRef::from_raw_iunknown_addref(raw) }) else {
                return Err("SAFEARRAY object element carried null IUnknown pointer".to_string());
            };
            Variant::from_object_ref(object)
        }
        SafeArrayElementKind::Decimal => {
            // SAFETY: kind == Decimal, so encode initialized this slot as a
            // plain-data RawDecimalArrayElement.
            Variant::from_decimal96(unsafe { *ptr.cast::<RawDecimalArrayElement>() }.to_decimal96())
        }
    })
}

unsafe fn encode_element_variant(
    kind: SafeArrayElementKind,
    payload: *mut u8,
    index: usize,
    value: &Variant,
) -> Result<(), String> {
    // SAFETY: the caller guarantees `payload` is an allocation laid out by
    // `payload_layout(kind, len)` with `index < len`, so the offset stays
    // in-bounds and `ptr` is aligned for `kind`'s element type.
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    if kind == SafeArrayElementKind::Variant {
        // SAFETY: kind == Variant, so the in-bounds slot computed above has
        // `Variant` size/alignment; the slot is written exactly once into the
        // fresh zeroed buffer, so `write` correctly skips dropping old contents.
        unsafe { ptr.cast::<Variant>().write(value.clone()) };
        return Ok(());
    }
    match kind {
        SafeArrayElementKind::Variant => unreachable!("handled above"),
        // SAFETY: kind == I1 sized/aligned the slot for i8; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::I1 => unsafe {
            ptr.cast::<i8>()
                .write(i8::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_I1 SAFEARRAY element")
                })?);
        },
        // SAFETY: kind == Ui1 sized/aligned the slot for u8; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::Ui1 => unsafe {
            ptr.cast::<u8>().write(
                u8::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI1 SAFEARRAY element")
                })?,
            );
        },
        // SAFETY: kind == I2 sized/aligned the slot for i16; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::I2 => unsafe {
            ptr.cast::<i16>()
                .write(i16::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_I2 SAFEARRAY element")
                })?);
        },
        // SAFETY: kind == Ui2 sized/aligned the slot for u16; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::Ui2 => unsafe {
            ptr.cast::<u16>().write(
                u16::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI2 SAFEARRAY element")
                })?,
            );
        },
        // SAFETY: kind == I4/Int sized/aligned the slot for i32; plain-data
        // write, exactly once into the zeroed buffer.
        SafeArrayElementKind::I4 | SafeArrayElementKind::Int => unsafe {
            ptr.cast::<i32>()
                .write(i32::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_I4 SAFEARRAY element")
                })?);
        },
        // SAFETY: kind == Ui4/UInt sized/aligned the slot for u32; plain-data
        // write, exactly once into the zeroed buffer.
        SafeArrayElementKind::Ui4 | SafeArrayElementKind::UInt => unsafe {
            ptr.cast::<u32>().write(
                u32::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI4 SAFEARRAY element")
                })?,
            );
        },
        // SAFETY: kind == I8 sized/aligned the slot for i64; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::I8 => unsafe {
            ptr.cast::<i64>().write(variant_i64(value)?);
        },
        // SAFETY: kind == Ui8 sized/aligned the slot for u64; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::Ui8 => unsafe {
            ptr.cast::<u64>().write(
                u64::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI8 SAFEARRAY element")
                })?,
            );
        },
        // SAFETY: kind == R4 sized/aligned the slot for f32; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::R4 => unsafe {
            ptr.cast::<f32>().write(variant_f64(value)? as f32);
        },
        // SAFETY: kind == R8 sized/aligned the slot for f64; plain-data write,
        // exactly once into the zeroed buffer.
        SafeArrayElementKind::R8 => unsafe {
            ptr.cast::<f64>().write(variant_f64(value)?);
        },
        // SAFETY: kind == Currency sized/aligned the slot for the scaled-i64
        // currency carrier; plain-data write, exactly once into the zeroed buffer.
        SafeArrayElementKind::Currency => unsafe {
            ptr.cast::<i64>().write(
                value
                    .as_currency_scaled_i64()
                    .ok_or_else(|| format!("expected Currency SAFEARRAY element, got {value:?}"))?,
            );
        },
        // SAFETY: kind == Date sized/aligned the slot for the f64 date serial;
        // plain-data write, exactly once into the zeroed buffer.
        SafeArrayElementKind::Date => unsafe {
            ptr.cast::<f64>().write(
                value
                    .as_date_f64()
                    .or_else(|| value.as_f64())
                    .ok_or_else(|| format!("expected Date SAFEARRAY element, got {value:?}"))?,
            );
        },
        // SAFETY: kind == Bool sized/aligned the slot for the VARIANT_BOOL i16;
        // plain-data write, exactly once into the zeroed buffer.
        SafeArrayElementKind::Bool => unsafe {
            ptr.cast::<i16>().write(
                if value
                    .as_bool()
                    .ok_or_else(|| format!("expected Bool SAFEARRAY element, got {value:?}"))?
                {
                    -1
                } else {
                    0
                },
            );
        },
        // SAFETY: kind == BStr sized/aligned the slot for a pointer; ownership
        // of the freshly allocated raw BSTR transfers into the slot, to be
        // freed exactly once by `drop_element`.
        SafeArrayElementKind::BStr => unsafe {
            ptr.cast::<*mut u16>().write(alloc_raw_bstr_from_bstr(
                &value
                    .as_bstr()
                    .ok_or_else(|| format!("expected String SAFEARRAY element, got {value:?}"))?,
            )?);
        },
        // SAFETY: kind == Dispatch/Unknown sized/aligned the slot for the
        // 8 pointer bytes; `mem::forget` transfers the ObjectRef's retained
        // reference into the slot, so the array owns exactly one IUnknown
        // reference, released by `drop_element`.
        SafeArrayElementKind::Dispatch | SafeArrayElementKind::Unknown => unsafe {
            let object = value
                .as_object_ref()
                .ok_or_else(|| format!("expected Object SAFEARRAY element, got {value:?}"))?;
            let raw = object.raw_iunknown();
            core::mem::forget(object);
            ptr.cast::<[u8; 8]>().write(raw_iunknown_ptr_to_bytes(raw));
        },
        // SAFETY: kind == Decimal sized/aligned the slot for the plain-data
        // RawDecimalArrayElement; written exactly once into the zeroed buffer.
        SafeArrayElementKind::Decimal => unsafe {
            ptr.cast::<RawDecimalArrayElement>()
                .write(RawDecimalArrayElement::from_decimal96(
                    value.as_decimal96().ok_or_else(|| {
                        format!("expected Decimal SAFEARRAY element, got {value:?}")
                    })?,
                ));
        },
    }
    Ok(())
}

unsafe fn replace_element_variant(
    kind: SafeArrayElementKind,
    payload: *mut u8,
    index: usize,
    value: &Variant,
) -> Result<(), String> {
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    match kind {
        SafeArrayElementKind::Variant => {
            let old = unsafe { ptr.cast::<Variant>().replace(value.clone()) };
            drop(old);
        }
        SafeArrayElementKind::I1 => unsafe {
            *ptr.cast::<i8>() = i8::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_I1 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::Ui1 => unsafe {
            *ptr.cast::<u8>() = u8::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI1 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::I2 => unsafe {
            *ptr.cast::<i16>() = i16::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_I2 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::Ui2 => unsafe {
            *ptr.cast::<u16>() = u16::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI2 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::I4 | SafeArrayElementKind::Int => unsafe {
            *ptr.cast::<i32>() = i32::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_I4 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::Ui4 | SafeArrayElementKind::UInt => unsafe {
            *ptr.cast::<u32>() = u32::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI4 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::I8 => unsafe {
            *ptr.cast::<i64>() = variant_i64(value)?;
        },
        SafeArrayElementKind::Ui8 => unsafe {
            *ptr.cast::<u64>() = u64::try_from(variant_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI8 SAFEARRAY element"))?;
        },
        SafeArrayElementKind::R4 => unsafe {
            *ptr.cast::<f32>() = variant_f64(value)? as f32;
        },
        SafeArrayElementKind::R8 => unsafe {
            *ptr.cast::<f64>() = variant_f64(value)?;
        },
        SafeArrayElementKind::Currency => unsafe {
            *ptr.cast::<i64>() = value
                .as_currency_scaled_i64()
                .ok_or_else(|| format!("expected Currency SAFEARRAY element, got {value:?}"))?;
        },
        SafeArrayElementKind::Date => unsafe {
            *ptr.cast::<f64>() = value
                .as_date_f64()
                .or_else(|| value.as_f64())
                .ok_or_else(|| format!("expected Date SAFEARRAY element, got {value:?}"))?;
        },
        SafeArrayElementKind::Bool => unsafe {
            *ptr.cast::<i16>() = if value
                .as_bool()
                .ok_or_else(|| format!("expected Bool SAFEARRAY element, got {value:?}"))?
            {
                -1
            } else {
                0
            };
        },
        SafeArrayElementKind::BStr => unsafe {
            let new = alloc_raw_bstr_from_bstr(
                &value
                    .as_bstr()
                    .ok_or_else(|| format!("expected String SAFEARRAY element, got {value:?}"))?,
            )?;
            let old = ptr.cast::<*mut u16>().replace(new);
            free_raw_bstr(old);
        },
        SafeArrayElementKind::Dispatch | SafeArrayElementKind::Unknown => unsafe {
            let object = value
                .as_object_ref()
                .ok_or_else(|| format!("expected Object SAFEARRAY element, got {value:?}"))?;
            let raw = object.raw_iunknown();
            core::mem::forget(object);
            let old = ptr
                .cast::<[u8; 8]>()
                .replace(raw_iunknown_ptr_to_bytes(raw));
            if let Some(object) = ObjectRef::from_raw_iunknown_owned(bytes_to_raw_iunknown(old)) {
                drop(object);
            }
        },
        SafeArrayElementKind::Decimal => unsafe {
            *ptr.cast::<RawDecimalArrayElement>() = RawDecimalArrayElement::from_decimal96(
                value
                    .as_decimal96()
                    .ok_or_else(|| format!("expected Decimal SAFEARRAY element, got {value:?}"))?,
            );
        },
    }
    Ok(())
}

unsafe fn drop_element(kind: SafeArrayElementKind, payload: *mut u8, index: usize) {
    // SAFETY: the caller guarantees `payload` holds more than `index`
    // initialized elements of `kind` and visits each index at most once, so
    // the offset is in-bounds and the slot still owns its resource.
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    match kind {
        // SAFETY: kind == Variant, so the slot holds an initialized, not yet
        // dropped `Variant` (callers drop each index exactly once).
        SafeArrayElementKind::Variant => unsafe { core::ptr::drop_in_place(ptr.cast::<Variant>()) },
        // SAFETY: kind == BStr, so the slot holds the array-owned raw BSTR
        // pointer written at encode time; ownership transfers to
        // `free_raw_bstr`, which frees it exactly once.
        SafeArrayElementKind::BStr => unsafe { free_raw_bstr(*ptr.cast::<*mut u16>()) },
        // SAFETY: kind == Dispatch/Unknown, so the slot holds the pointer bytes
        // of the array's single retained IUnknown reference (taken at encode
        // time); `from_raw_iunknown_owned` adopts and releases exactly that
        // reference, per its ownership-transfer contract.
        SafeArrayElementKind::Dispatch | SafeArrayElementKind::Unknown => unsafe {
            let raw = bytes_to_raw_iunknown(*ptr.cast::<[u8; 8]>());
            if let Some(object) = ObjectRef::from_raw_iunknown_owned(raw) {
                drop(object);
            }
        },
        _ => {}
    }
}

unsafe fn free_payload(kind: SafeArrayElementKind, payload: *mut core::ffi::c_void, count: usize) {
    if payload.is_null() || count == 0 {
        return;
    }
    let raw = payload.cast::<u8>();
    let mut index = 0usize;
    while index < count {
        // SAFETY: `payload` was checked non-null above and the caller passes
        // the same `kind`/`count` the buffer was built with, so every
        // `index < count` names an initialized element, dropped exactly once here.
        unsafe { drop_element(kind, raw, index) };
        index += 1;
    }
    if let Ok(layout) = payload_layout(kind, count) {
        // SAFETY: `raw` was returned by `alloc_zeroed` with this same
        // `payload_layout(kind, count)`; all elements were dropped above and
        // nothing reads the buffer after this point.
        unsafe { std::alloc::dealloc(raw, layout) };
    }
}

fn alloc_payload_from_variants(
    kind: SafeArrayElementKind,
    values: &[Variant],
) -> Result<*mut core::ffi::c_void, String> {
    if values.is_empty() {
        return Ok(core::ptr::null_mut());
    }
    let layout = payload_layout(kind, values.len())?;
    // SAFETY: `layout` has non-zero size (`payload_layout` clamps the size to
    // at least 1 byte), satisfying `alloc_zeroed`; the null failure case is
    // handled immediately below.
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    if raw.is_null() {
        return Err("failed to allocate SAFEARRAY payload".to_string());
    }
    let mut initialized = 0usize;
    while initialized < values.len() {
        // SAFETY: `raw` is a live zeroed allocation laid out by
        // `payload_layout(kind, values.len())` and `initialized < values.len()`,
        // so the target slot is in-bounds, aligned, and written exactly once.
        let encoded =
            unsafe { encode_element_variant(kind, raw, initialized, &values[initialized]) };
        if let Err(err) = encoded {
            let mut index = 0usize;
            while index < initialized {
                // SAFETY: indices below `initialized` were successfully
                // encoded, so each slot holds an initialized element dropped
                // exactly once on this error path.
                unsafe { drop_element(kind, raw, index) };
                index += 1;
            }
            // SAFETY: `raw` came from `alloc_zeroed` with this same `layout`,
            // the initialized prefix was dropped above, and the pointer never
            // escaped this function, so this is the sole deallocation.
            unsafe { std::alloc::dealloc(raw, layout) };
            return Err(err);
        }
        initialized += 1;
    }
    Ok(raw.cast())
}

fn alloc_record_payload_from_records(
    layout: &Arc<VbaRecordLayout>,
    values: &[VbaRecord],
) -> Result<*mut core::ffi::c_void, String> {
    if values.is_empty() {
        return Ok(core::ptr::null_mut());
    }
    let payload_layout = record_payload_layout(layout, values.len())?;
    let raw = unsafe { std::alloc::alloc_zeroed(payload_layout) };
    if raw.is_null() {
        return Err("failed to allocate SAFEARRAY record payload".to_string());
    }
    let mut initialized = 0usize;
    while initialized < values.len() {
        if values[initialized].layout().as_ref() != layout.as_ref() {
            let mut index = 0usize;
            while index < initialized {
                unsafe {
                    VbaRecord::drop_raw(raw.add(record_payload_offset(layout, index)), layout)
                };
                index += 1;
            }
            unsafe { std::alloc::dealloc(raw, payload_layout) };
            return Err("SAFEARRAY record element layout mismatch".to_string());
        }
        let dst = unsafe { raw.add(record_payload_offset(layout, initialized)) };
        if let Err(err) = unsafe { values[initialized].clone_into_raw(dst) } {
            let mut index = 0usize;
            while index < initialized {
                unsafe {
                    VbaRecord::drop_raw(raw.add(record_payload_offset(layout, index)), layout)
                };
                index += 1;
            }
            unsafe { std::alloc::dealloc(raw, payload_layout) };
            return Err(err);
        }
        initialized += 1;
    }
    Ok(raw.cast())
}

unsafe fn free_record_payload(
    layout: &VbaRecordLayout,
    payload: *mut core::ffi::c_void,
    count: usize,
) {
    let raw = payload.cast::<u8>();
    if raw.is_null() {
        return;
    }
    let mut index = 0usize;
    while index < count {
        unsafe { VbaRecord::drop_raw(raw.add(record_payload_offset(layout, index)), layout) };
        index += 1;
    }
    if let Ok(payload_layout) = record_payload_layout(layout, count) {
        unsafe { std::alloc::dealloc(raw, payload_layout) };
    }
}

fn alloc_header(
    bounds: &[SafeArrayBound],
    element_vt: u16,
    cb_elements: usize,
    pv_data: *mut core::ffi::c_void,
    record_layout: Option<Arc<VbaRecordLayout>>,
) -> Result<NonNull<RawSafeArray>, String> {
    let c_dims = u16::try_from(bounds.len())
        .map_err(|_| "SAFEARRAY dimension count exceeds u16 capacity".to_string())?;
    let cb_elements = u32::try_from(cb_elements)
        .map_err(|_| "SAFEARRAY cbElements exceeds u32 capacity".to_string())?;
    let f_features = if element_vt == VT_RECORD_VALUE {
        FADF_HAVEVARTYPE_VALUE | FADF_RECORD_VALUE
    } else {
        SafeArrayElementKind::from_vartype(element_vt)
            .map(SafeArrayElementKind::feature_flags)
            .unwrap_or(FADF_HAVEVARTYPE_VALUE)
    };
    let layout = owner_layout(bounds.len())?;
    // SAFETY: `layout` has non-zero size (owner prefix plus at least the
    // RawSafeArray header), satisfying `alloc_zeroed`; allocation failure is
    // handled by the NonNull check below.
    let raw_owner = unsafe { std::alloc::alloc_zeroed(layout) };
    let Some(raw_owner) = NonNull::new(raw_owner) else {
        return Err("failed to allocate SAFEARRAY header".to_string());
    };
    // SAFETY: `raw_owner` is a fresh zeroed allocation of
    // `owner_layout(bounds.len())`: a RawSafeArrayOwnerPrefix followed by a
    // RawSafeArray header with `bounds.len()` bounds entries (one inline plus
    // `bounds.len() - 1` trailing). All writes below stay inside that
    // allocation, the 8-byte prefix offset keeps the header aligned, `bounds`
    // provides exactly `bounds.len()` entries for the non-overlapping copy
    // into the fresh allocation, and the header pointer offsets a NonNull base
    // by a constant within the allocation — justifying `new_unchecked`.
    unsafe {
        raw_owner
            .cast::<RawSafeArrayOwnerPrefix>()
            .as_ptr()
            .write(RawSafeArrayOwnerPrefix {
                magic: OXVBA_SAFEARRAY_OWNER_MAGIC,
                version: OXVBA_SAFEARRAY_OWNER_VERSION,
                element_vt,
                record_layout: record_layout
                    .map(Arc::into_raw)
                    .unwrap_or(core::ptr::null()),
            });
        let header = raw_owner
            .as_ptr()
            .add(core::mem::size_of::<RawSafeArrayOwnerPrefix>())
            .cast::<RawSafeArray>();
        (*header).c_dims = c_dims;
        (*header).f_features = f_features;
        (*header).cb_elements = cb_elements;
        (*header).c_locks = 0;
        (*header).pv_data = pv_data;
        let dst = core::ptr::addr_of_mut!((*header).rgsabound).cast::<SafeArrayBound>();
        core::ptr::copy_nonoverlapping(bounds.as_ptr(), dst, bounds.len());
        Ok(NonNull::new_unchecked(header))
    }
}

impl SafeArray {
    pub fn supports_intrinsic_element_vartype(element_vt: u16) -> bool {
        SafeArrayElementKind::from_vartype(element_vt).is_some()
    }

    fn from_bounds_and_variants(
        bounds: Vec<SafeArrayBound>,
        element_vt: u16,
        values: Option<Vec<Variant>>,
    ) -> Result<Self, String> {
        let kind = SafeArrayElementKind::from_vartype(element_vt).ok_or_else(|| {
            format!("unsupported intrinsic SAFEARRAY element vartype 0x{element_vt:04X}")
        })?;
        let expected_len = bounds_total_len(&bounds)?;
        let pv_data = match values {
            Some(values) => {
                if values.len() != expected_len {
                    return Err(format!(
                        "SAFEARRAY payload length {} does not match shape length {}",
                        values.len(),
                        expected_len
                    ));
                }
                alloc_payload_from_variants(kind, &values)?
            }
            None => core::ptr::null_mut(),
        };
        let header = match alloc_header(&bounds, element_vt, kind.element_size(), pv_data, None) {
            Ok(header) => header,
            Err(err) => {
                // SAFETY: `pv_data` is null (shape-only) or the payload just
                // built by `alloc_payload_from_variants` with this `kind` and
                // exactly `expected_len` initialized elements; it never escaped,
                // so this is its sole release.
                unsafe { free_payload(kind, pv_data, expected_len) };
                return Err(err);
            }
        };
        Ok(Self(header))
    }

    pub fn from_vba_records_nd(
        bounds: Vec<SafeArrayBound>,
        layout: Arc<VbaRecordLayout>,
        values: Vec<VbaRecord>,
    ) -> Result<Self, String> {
        let expected_len = bounds_total_len(&bounds)?;
        if values.len() != expected_len {
            return Err(format!(
                "SAFEARRAY payload length {} does not match shape length {}",
                values.len(),
                expected_len
            ));
        }
        let pv_data = alloc_record_payload_from_records(&layout, &values)?;
        let header = match alloc_header(
            &bounds,
            VT_RECORD_VALUE,
            layout.size(),
            pv_data,
            Some(layout.clone()),
        ) {
            Ok(header) => header,
            Err(err) => {
                unsafe { free_record_payload(&layout, pv_data, expected_len) };
                return Err(err);
            }
        };
        Ok(Self(header))
    }

    pub fn vector(len: usize) -> Self {
        Self::from_bounds_and_variants(
            default_bounds_for_len(len).expect("vector bounds should fit SAFEARRAY capacity"),
            VT_VARIANT_VALUE,
            None,
        )
        .expect("shape-only SAFEARRAY allocation should succeed")
    }

    pub fn from_shape(bounds: Vec<SafeArrayBound>) -> Result<Self, String> {
        Self::from_bounds_and_variants(bounds, VT_VARIANT_VALUE, None)
    }

    pub fn from_shape_typed(bounds: Vec<SafeArrayBound>, element_vt: u16) -> Result<Self, String> {
        Self::from_bounds_and_variants(bounds, element_vt, None)
    }

    pub fn from_variants(values: Vec<Variant>) -> Self {
        let len = values.len();
        Self::from_bounds_and_variants(
            default_bounds_for_len(len).expect("value bounds should fit SAFEARRAY capacity"),
            VT_VARIANT_VALUE,
            Some(values),
        )
        .expect("SAFEARRAY payload allocation should succeed for supported canonical variants")
    }

    pub fn from_variants_nd(bounds: Vec<SafeArrayBound>, values: Vec<Variant>) -> Self {
        Self::from_bounds_and_variants(bounds, VT_VARIANT_VALUE, Some(values)).expect(
            "SAFEARRAY nd payload allocation should succeed for supported canonical variants",
        )
    }

    pub fn from_typed_variants(element_vt: u16, values: Vec<Variant>) -> Result<Self, String> {
        let len = values.len();
        Self::from_bounds_and_variants(default_bounds_for_len(len)?, element_vt, Some(values))
    }

    pub fn from_typed_variants_nd(
        bounds: Vec<SafeArrayBound>,
        element_vt: u16,
        values: Vec<Variant>,
    ) -> Result<Self, String> {
        Self::from_bounds_and_variants(bounds, element_vt, Some(values))
    }

    pub fn from_shape_and_variants(
        bounds: Vec<SafeArrayBound>,
        values: Vec<Variant>,
    ) -> Result<Self, String> {
        Self::from_bounds_and_variants(bounds, VT_VARIANT_VALUE, Some(values))
    }

    pub fn dimensions(&self) -> u8 {
        // SAFETY: `self.0` is the descriptor `alloc_header` produced and this
        // value owns until `Drop`, so reading its `c_dims` field is valid.
        unsafe { (*self.0.as_ptr()).c_dims as u8 }
    }

    pub fn element_vartype(&self) -> u16 {
        // SAFETY: `self.0` was produced by `alloc_header` (the only
        // construction path), which placed and initialized the owner prefix
        // immediately before the header in the same allocation, exactly what
        // `validated_header_prefix` requires of its caller.
        let prefix = unsafe { validated_header_prefix(self.0.as_ptr()) }
            .expect("SAFEARRAY descriptor is not owned by OxVba");
        // SAFETY: `prefix` was just validated against this value's live owner
        // allocation, so reading `element_vt` is in-bounds and initialized.
        unsafe { (*prefix).element_vt }
    }

    pub fn feature_flags(&self) -> u16 {
        // SAFETY: `self.0` is this value's live descriptor (allocated by
        // `alloc_header`, freed only in `Drop`), so reading `f_features` is valid.
        unsafe { (*self.0.as_ptr()).f_features }
    }

    /// Whether this SAFEARRAY carries `FADF_FIXEDSIZE` — i.e. it is a fixed-size
    /// array (`Dim a(1 To 3)`) whose `Erase` resets elements rather than freeing
    /// the storage. The bit lives in the descriptor's `fFeatures`, so it travels
    /// with [`Clone`] and across the COM boundary exactly like the other FADF
    /// flags.
    pub fn is_fixed_size(&self) -> bool {
        self.feature_flags() & FADF_FIXEDSIZE_VALUE != 0
    }

    /// Set or clear the descriptor's `FADF_FIXEDSIZE` bit in place.
    pub fn set_fixed_size(&mut self, fixed: bool) {
        // SAFETY: `self.0` is this value's live descriptor and `&mut self`
        // guarantees exclusive access, so toggling the `f_features` bit is a
        // sound in-bounds write.
        unsafe {
            let header = self.0.as_ptr();
            if fixed {
                (*header).f_features |= FADF_FIXEDSIZE_VALUE;
            } else {
                (*header).f_features &= !FADF_FIXEDSIZE_VALUE;
            }
        }
    }

    /// Consume `self`, setting/clearing `FADF_FIXEDSIZE`, and return it — a
    /// builder-style convenience over [`Self::set_fixed_size`].
    #[must_use]
    pub fn with_fixed_size(mut self, fixed: bool) -> Self {
        self.set_fixed_size(fixed);
        self
    }

    fn element_kind(&self) -> SafeArrayElementKind {
        SafeArrayElementKind::from_vartype(self.element_vartype())
            .expect("internal SAFEARRAY element vartype should remain supported")
    }

    pub fn vba_record_layout(&self) -> Option<Arc<VbaRecordLayout>> {
        if self.element_vartype() != VT_RECORD_VALUE {
            return None;
        }
        unsafe { header_record_layout(self.0.as_ptr()) }
    }

    fn raw_bounds(&self) -> Vec<SafeArrayBound> {
        let dims = self.dimensions() as usize;
        if dims == 0 {
            return Vec::new();
        }
        // SAFETY: `self.0` is this value's live descriptor; `addr_of!` only
        // computes the address of its `rgsabound` field without reading past
        // the header.
        let ptr =
            unsafe { core::ptr::addr_of!((*self.0.as_ptr()).rgsabound).cast::<SafeArrayBound>() };
        // SAFETY: `alloc_header` allocated and initialized `c_dims` contiguous
        // SafeArrayBound entries starting at `rgsabound` (declared-dimension
        // order), and `dims` was read from this same descriptor, so the slice
        // is in-bounds and initialized for the duration of this borrow.
        unsafe { core::slice::from_raw_parts(ptr, dims) }.to_vec()
    }

    pub fn len(&self) -> usize {
        bounds_total_len(&self.raw_bounds()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn effective_len(&self) -> usize {
        self.len()
    }

    pub fn bounds(&self) -> Option<Vec<SafeArrayBound>> {
        let bounds = self.raw_bounds();
        if bounds.is_empty() {
            return None;
        }
        Some(bounds)
    }

    fn bounds_for_shape(&self) -> Vec<SafeArrayBound> {
        self.bounds()
            .unwrap_or_else(|| default_bounds_for_len(self.len()).unwrap_or_default())
    }

    pub fn variant_elements(&self) -> Option<Vec<Variant>> {
        // SAFETY: `self.0` is this value's live descriptor, so reading
        // `pv_data` is valid; the null (shape-only) case is handled just below.
        let data = unsafe { (*self.0.as_ptr()).pv_data.cast::<u8>() };
        if data.is_null() {
            return None;
        }
        if let Some(layout) = self.vba_record_layout() {
            let mut values = Vec::with_capacity(self.len());
            let mut index = 0usize;
            while index < self.len() {
                let ptr = unsafe { data.add(record_payload_offset(&layout, index)) };
                values.push(Variant::from_vba_record(
                    unsafe { VbaRecord::clone_from_raw(ptr, layout.clone()) }
                        .expect("SAFEARRAY record payload should clone into VbaRecord"),
                ));
                index += 1;
            }
            return Some(values);
        }
        let kind = self.element_kind();
        let mut values = Vec::with_capacity(self.len());
        let mut index = 0usize;
        while index < self.len() {
            // SAFETY: `data` is the non-null payload built by
            // `alloc_payload_from_variants` for exactly `self.len()` initialized
            // elements of this descriptor's `kind`, and `index < self.len()`.
            values.push(
                unsafe { decode_element_variant(kind, data, index) }
                    .expect("SAFEARRAY intrinsic payload should decode into Variant"),
            );
            index += 1;
        }
        Some(values)
    }

    pub fn variant_element(&self, index: usize) -> Result<Variant, String> {
        if index >= self.len() {
            return Err(format!(
                "SAFEARRAY index {index} out of range for length {}",
                self.len()
            ));
        }
        let data = unsafe { (*self.0.as_ptr()).pv_data.cast::<u8>() };
        if data.is_null() {
            return Err("SAFEARRAY has no materialized element payload".to_string());
        }
        if let Some(layout) = self.vba_record_layout() {
            let ptr = unsafe { data.add(record_payload_offset(&layout, index)) };
            return Ok(Variant::from_vba_record(unsafe {
                VbaRecord::clone_from_raw(ptr, layout)
            }?));
        }
        unsafe { decode_element_variant(self.element_kind(), data, index) }
    }

    pub fn replace_variant_elements(&self, values: Vec<Variant>) -> Result<Self, String> {
        Self::from_bounds_and_variants(
            self.bounds_for_shape(),
            self.element_vartype(),
            Some(values),
        )
    }

    pub fn set_variant_element(&mut self, index: usize, value: &Variant) -> Result<(), String> {
        if index >= self.len() {
            return Err(format!(
                "SAFEARRAY index {index} out of range for length {}",
                self.len()
            ));
        }
        let data = unsafe { (*self.0.as_ptr()).pv_data.cast::<u8>() };
        if data.is_null() {
            return Err("SAFEARRAY has no materialized element payload".to_string());
        }
        if let Some(layout) = self.vba_record_layout() {
            let Some(record) = value.as_vba_record() else {
                return Err(format!(
                    "expected VBA record SAFEARRAY element, got {:?}",
                    value.vtype()
                ));
            };
            if record.layout().as_ref() != layout.as_ref() {
                return Err("VBA record SAFEARRAY element layout mismatch".to_string());
            }
            let ptr = unsafe { data.add(record_payload_offset(&layout, index)) };
            unsafe {
                VbaRecord::drop_raw(ptr, &layout);
                record.clone_into_raw(ptr)?;
            }
            return Ok(());
        }
        unsafe { replace_element_variant(self.element_kind(), data, index, value) }
    }

    pub fn raw_safearray_ptr(&self) -> *mut core::ffi::c_void {
        self.0.as_ptr().cast()
    }

    pub fn clone_raw_safearray_ptr(&self) -> *mut core::ffi::c_void {
        let cloned = self.clone();
        let raw = cloned.raw_safearray_ptr();
        core::mem::forget(cloned);
        raw
    }

    /// Takes ownership of a raw SAFEARRAY descriptor produced by this runtime.
    ///
    /// # Safety
    ///
    /// `raw` must be a descriptor pointer previously returned by
    /// [`Self::clone_raw_safearray_ptr`] or by moving a [`SafeArray`] into a
    /// [`Variant`](crate::Variant). This is not an external COM
    /// `SafeArrayDestroy` adapter; non-OxVba descriptors are rejected when the
    /// local owner prefix provenance marker is absent.
    pub unsafe fn from_raw_safearray_owned(raw: *mut core::ffi::c_void) -> Option<Self> {
        let header = NonNull::new(raw.cast::<RawSafeArray>())?;
        // SAFETY: this function's contract requires `raw` to be a descriptor
        // produced by this runtime, so an owner prefix precedes the header in
        // the same allocation; the magic/version check then rejects anything
        // that lost OxVba provenance before ownership is adopted.
        unsafe { validated_header_prefix(header.as_ptr()) }?;
        Some(Self(header))
    }

    /// Clones a raw SAFEARRAY descriptor produced by this runtime.
    ///
    /// # Safety
    ///
    /// `raw` must point at a live OxVba-owned descriptor. The clone receives
    /// independent descriptor/payload storage; ownership of `raw` is unchanged.
    pub unsafe fn clone_from_raw_safearray(raw: *mut core::ffi::c_void) -> Option<Self> {
        let header = NonNull::new(raw.cast::<RawSafeArray>())?;
        // SAFETY: this function's contract requires `raw` to point at a live
        // OxVba-owned descriptor, so the owner prefix preceding the header is
        // present, initialized, and readable; the temporary `Self` wrapper
        // below is forgotten, leaving ownership of `raw` untouched.
        unsafe { validated_header_prefix(header.as_ptr()) }?;
        let borrowed = Self(header);
        let cloned = borrowed.clone();
        core::mem::forget(borrowed);
        Some(cloned)
    }

    /// Mutates one element of a raw SAFEARRAY descriptor produced by this runtime.
    ///
    /// # Safety
    ///
    /// `raw` must point at a live OxVba-owned descriptor, and the caller must
    /// have exclusive access to that descriptor for the duration of this call.
    pub unsafe fn set_raw_safearray_variant_element(
        raw: *mut core::ffi::c_void,
        index: usize,
        value: &Variant,
    ) -> Result<(), String> {
        let header = NonNull::new(raw.cast::<RawSafeArray>())
            .ok_or_else(|| "SAFEARRAY raw pointer is null".to_string())?;
        // SAFETY: contract requires `raw` to be a live OxVba-owned descriptor, so the
        // owner prefix precedes the header; the temporary `Self` wrapper is forgotten
        // below, leaving ownership of `raw` with the caller's Variant.
        unsafe { validated_header_prefix(header.as_ptr()) }
            .ok_or_else(|| "SAFEARRAY raw pointer is not OxVba-owned".to_string())?;
        let mut borrowed = Self(header);
        let result = borrowed.set_variant_element(index, value);
        core::mem::forget(borrowed);
        result
    }

    /// Reads one element of a raw SAFEARRAY descriptor produced by this runtime
    /// **without cloning the array** — O(1) in array length (mirrors
    /// [`Self::set_raw_safearray_variant_element`]). This is the read primitive
    /// that keeps `arr(i)` element access constant-time instead of deep-cloning
    /// the whole backing store per access.
    ///
    /// # Safety
    ///
    /// `raw` must point at a live OxVba-owned descriptor, and the caller must
    /// have shared access to that descriptor for the duration of this call.
    pub unsafe fn raw_safearray_variant_element(
        raw: *mut core::ffi::c_void,
        index: usize,
    ) -> Result<Variant, String> {
        let header = NonNull::new(raw.cast::<RawSafeArray>())
            .ok_or_else(|| "SAFEARRAY raw pointer is null".to_string())?;
        // SAFETY: contract requires `raw` to be a live OxVba-owned descriptor, so
        // the owner prefix precedes the header; the temporary `Self` wrapper is
        // forgotten below, leaving ownership of `raw` with the caller's Variant.
        unsafe { validated_header_prefix(header.as_ptr()) }
            .ok_or_else(|| "SAFEARRAY raw pointer is not OxVba-owned".to_string())?;
        let borrowed = Self(header);
        let result = borrowed.variant_element(index);
        core::mem::forget(borrowed);
        result
    }

    /// Reads the bounds and element count of a raw SAFEARRAY descriptor **without
    /// cloning the element payload** — O(rank). Used with
    /// [`Self::raw_safearray_variant_element`] to bounds-check and index an array
    /// in place.
    ///
    /// # Safety
    ///
    /// Same contract as [`Self::raw_safearray_variant_element`].
    pub unsafe fn raw_safearray_bounds_len(
        raw: *mut core::ffi::c_void,
    ) -> Option<(Vec<SafeArrayBound>, usize)> {
        let header = NonNull::new(raw.cast::<RawSafeArray>())?;
        // SAFETY: as above — borrow the descriptor without taking ownership.
        unsafe { validated_header_prefix(header.as_ptr()) }?;
        let borrowed = Self(header);
        let bounds = borrowed.bounds();
        let len = borrowed.len();
        core::mem::forget(borrowed);
        bounds.map(|b| (b, len))
    }
}

impl Clone for SafeArray {
    fn clone(&self) -> Self {
        // The clone is rebuilt from bounds + element vartype + values, which
        // derives `f_features` afresh from the element type — so the
        // `FADF_FIXEDSIZE` storage property must be re-applied explicitly to
        // travel with the copy (matching VBA, where the descriptor's fixed-ness
        // moves with the array value).
        let fixed = self.is_fixed_size();
        let bounds = self.bounds_for_shape();
        if let Some(layout) = self.vba_record_layout() {
            let values = self
                .variant_elements()
                .unwrap_or_default()
                .into_iter()
                .map(|value| {
                    value
                        .as_vba_record()
                        .expect("record SAFEARRAY elements should decode to VbaRecord")
                })
                .collect();
            return Self::from_vba_records_nd(bounds, layout, values)
                .expect("cloning native record SAFEARRAY should succeed")
                .with_fixed_size(fixed);
        }
        match self.variant_elements() {
            Some(values) => {
                Self::from_bounds_and_variants(bounds, self.element_vartype(), Some(values))
                    .expect("cloning canonical SAFEARRAY with values should succeed")
            }
            None => Self::from_bounds_and_variants(bounds, self.element_vartype(), None)
                .expect("cloning shape-only SAFEARRAY should succeed"),
        }
        .with_fixed_size(fixed)
    }
}

impl Drop for SafeArray {
    fn drop(&mut self) {
        let len = self.len();
        // SAFETY: `self.0` is the live descriptor this value owns; reading
        // `pv_data` is valid until the deallocation below.
        let data = unsafe { (*self.0.as_ptr()).pv_data };
        if let Some(layout) = self.vba_record_layout() {
            // SAFETY: `data` is null or the payload built by
            // `alloc_record_payload_from_records` with this descriptor's
            // layout and exactly `len` initialized elements.
            unsafe { free_record_payload(&layout, data, len) };
        } else {
            let kind = self.element_kind();
            // SAFETY: `data` is null or the payload built by
            // `alloc_payload_from_variants` with this descriptor's `kind` and
            // exactly `len` initialized elements; `drop` runs at most once, so
            // the elements and the buffer are released exactly once.
            unsafe { free_payload(kind, data, len) };
        }
        if let Ok(layout) = owner_layout(self.dimensions() as usize) {
            let prefix = unsafe { validated_header_prefix(self.0.as_ptr()) };
            if let Some(prefix) = prefix {
                let record_layout = unsafe { (*prefix).record_layout };
                if !record_layout.is_null() {
                    // SAFETY: non-null record_layout pointers are stored from
                    // Arc::into_raw in alloc_header; this releases the owner
                    // prefix's strong reference exactly once.
                    unsafe { drop(Arc::from_raw(record_layout)) };
                }
            }
            // SAFETY: `alloc_header` placed the descriptor exactly
            // `size_of::<RawSafeArrayOwnerPrefix>()` bytes into the owner
            // allocation, so this `sub` recovers the allocation's base pointer
            // without leaving the allocation.
            let owner = unsafe {
                self.0
                    .as_ptr()
                    .cast::<u8>()
                    .sub(core::mem::size_of::<RawSafeArrayOwnerPrefix>())
            };
            // SAFETY: `owner` is the pointer `alloc_zeroed` returned in
            // `alloc_header`, and `owner_layout(self.dimensions())` recomputes
            // the layout used there (`c_dims` was written from `bounds.len()`);
            // nothing touches the allocation after this point.
            unsafe { std::alloc::dealloc(owner, layout) };
        }
    }
}

impl core::fmt::Debug for SafeArray {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SafeArray")
            .field("dimensions", &self.dimensions())
            .field("len", &self.len())
            .field(
                "element_vartype",
                &format_args!("{:#06X}", self.element_vartype()),
            )
            .field("bounds", &self.bounds())
            .field("elements", &self.variant_elements())
            .finish()
    }
}

impl PartialEq for SafeArray {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions() == other.dimensions()
            && self.len() == other.len()
            && self.element_vartype() == other.element_vartype()
            && self.bounds() == other.bounds()
            && self.variant_elements() == other.variant_elements()
    }
}

impl Eq for SafeArray {}

pub fn is_array_tag(value: i32) -> bool {
    (ARRAY_TAG_BASE..=ARRAY_TAG_LIMIT).contains(&value)
}

pub fn array_len_from_tag(value: i32) -> Option<usize> {
    if !is_array_tag(value) {
        return None;
    }
    let count = value.checked_sub(ARRAY_TAG_BASE)?;
    usize::try_from(count).ok()
}

pub fn safe_array_from_tag(value: i32) -> Option<SafeArray> {
    array_len_from_tag(value).map(SafeArray::vector)
}

pub fn array_tag_from_safe_array(array: &SafeArray) -> Option<i32> {
    if array.dimensions() == 0 {
        return None;
    }
    let len_i32 = i32::try_from(array.effective_len()).ok()?;
    ARRAY_TAG_BASE
        .checked_add(len_i32)
        .filter(|v| *v <= ARRAY_TAG_LIMIT)
}

pub fn marshal_dispatch_argument(value: i32) -> i32 {
    let Some(array) = safe_array_from_tag(value) else {
        return value;
    };
    match i32::try_from(array.len()) {
        Ok(len) => DISPATCH_ARRAY_PAYLOAD_BASE.saturating_add(len),
        Err(_) => DISPATCH_ARRAY_PAYLOAD_BASE,
    }
}

#[cfg(test)]
// Test-support code exercising the documented production raw-pointer paths.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::{
        ARRAY_TAG_BASE, FADF_BSTR_VALUE, FADF_DISPATCH_VALUE, FADF_HAVEVARTYPE_VALUE,
        FADF_UNKNOWN_VALUE, FADF_VARIANT_VALUE, RawSafeArray, SafeArray, SafeArrayBound,
        VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE, VT_DECIMAL_VALUE,
        VT_DISPATCH_VALUE, VT_I2_VALUE, VT_RECORD_VALUE, VT_UNKNOWN_VALUE, VT_VARIANT_VALUE,
        array_len_from_tag, array_tag_from_safe_array, header_prefix_ptr,
        marshal_dispatch_argument, safe_array_from_tag,
    };
    use crate::{
        Decimal96, ObjectRef, Variant, VbaRecord, VbaRecordFieldKind, VbaRecordFieldSpec,
        VbaRecordLayout, bstr::BStr,
    };
    use std::sync::Arc;

    fn offset_of<T, U>(field: fn(*const T) -> *const U) -> usize {
        let value = core::mem::MaybeUninit::<T>::uninit();
        let base = value.as_ptr();
        field(base) as usize - base as usize
    }

    #[test]
    fn safearray_descriptor_layout_matches_com_header_shape() {
        let pointer_sized_offsets = core::mem::size_of::<usize>() == 8;
        assert_eq!(core::mem::size_of::<SafeArrayBound>(), 8);
        assert_eq!(core::mem::align_of::<SafeArrayBound>(), 4);
        assert_eq!(
            core::mem::size_of::<RawSafeArray>(),
            if pointer_sized_offsets { 32 } else { 24 }
        );
        assert_eq!(
            core::mem::align_of::<RawSafeArray>(),
            core::mem::align_of::<*mut core::ffi::c_void>()
        );
        assert_eq!(
            offset_of::<RawSafeArray, _>(|base| unsafe { core::ptr::addr_of!((*base).c_dims) }),
            0
        );
        assert_eq!(
            offset_of::<RawSafeArray, _>(|base| unsafe { core::ptr::addr_of!((*base).f_features) }),
            2
        );
        assert_eq!(
            offset_of::<RawSafeArray, _>(|base| unsafe {
                core::ptr::addr_of!((*base).cb_elements)
            }),
            4
        );
        assert_eq!(
            offset_of::<RawSafeArray, _>(|base| unsafe { core::ptr::addr_of!((*base).c_locks) }),
            8
        );
        assert_eq!(
            offset_of::<RawSafeArray, _>(|base| unsafe { core::ptr::addr_of!((*base).pv_data) }),
            if pointer_sized_offsets { 16 } else { 12 }
        );
        assert_eq!(
            offset_of::<RawSafeArray, _>(|base| unsafe { core::ptr::addr_of!((*base).rgsabound) }),
            if pointer_sized_offsets { 24 } else { 16 }
        );
    }

    #[test]
    fn safe_array_tag_roundtrip_for_vector_shape() {
        let tag = ARRAY_TAG_BASE + 3;
        let array = safe_array_from_tag(tag).expect("array tag should decode");
        assert_eq!(array.len(), 3);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(array.element_vartype(), VT_VARIANT_VALUE);
        assert_eq!(array_tag_from_safe_array(&array), Some(tag));
    }

    #[test]
    fn marshal_dispatch_argument_distinguishes_array_tags() {
        assert_eq!(marshal_dispatch_argument(9), 9);
        assert_eq!(marshal_dispatch_argument(ARRAY_TAG_BASE + 4), 20_004);
        assert_eq!(array_len_from_tag(ARRAY_TAG_BASE + 2), Some(2));
    }

    #[test]
    fn safe_array_from_values_preserves_owned_payload_shape() {
        let array = SafeArray::from_variants(vec![Variant::from_i32(4), Variant::from_i32(9)]);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(array.len(), 2);
        assert_eq!(array.effective_len(), 2);
        assert_eq!(array.element_vartype(), VT_VARIANT_VALUE);
        assert_eq!(
            array.variant_elements(),
            Some(vec![Variant::from_i32(4), Variant::from_i32(9)])
        );
        assert_eq!(array_tag_from_safe_array(&array), Some(ARRAY_TAG_BASE + 2));
    }

    #[test]
    fn safe_array_variant_api_preserves_canonical_payload_shape() {
        let array = SafeArray::from_variants(vec![
            Variant::from_i32(4),
            Variant::from_string(BStr::from("A")),
        ]);
        let elements = array
            .variant_elements()
            .expect("variant SAFEARRAY should expose variants");
        assert_eq!(elements[0], Variant::from_i32(4));
        assert_eq!(elements[1], Variant::from_string(BStr::from("A")));
        let replaced = array
            .replace_variant_elements(vec![
                Variant::from_i32(9),
                Variant::from_string(BStr::from("B")),
            ])
            .expect("replace variant elements");
        assert_eq!(
            replaced.variant_elements(),
            Some(vec![
                Variant::from_i32(9),
                Variant::from_string(BStr::from("B"))
            ])
        );
    }

    #[test]
    fn safearray_descriptor_advertises_variant_vartype_metadata() {
        let array = SafeArray::from_variants(vec![Variant::from_i32(4)]);
        assert_eq!(array.element_vartype(), VT_VARIANT_VALUE);
        assert_eq!(
            array.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_VARIANT_VALUE
        );
    }

    #[test]
    fn raw_safearray_adoption_requires_oxvba_owner_prefix() {
        let array = SafeArray::from_variants(vec![Variant::from_i32(4)]);
        let raw = array.clone_raw_safearray_ptr();
        let prefix = header_prefix_ptr(raw.cast());
        let original_magic = unsafe { (*prefix).magic };
        unsafe {
            (*prefix.cast_mut()).magic = 0;
        }
        assert!(unsafe { SafeArray::from_raw_safearray_owned(raw) }.is_none());
        unsafe {
            (*prefix.cast_mut()).magic = original_magic;
        }
        let adopted = unsafe { SafeArray::from_raw_safearray_owned(raw) }
            .expect("restored OxVba-owned SAFEARRAY should be adopted");
        assert_eq!(
            adopted.variant_elements().expect("elements"),
            vec![Variant::from_i32(4)]
        );
    }

    #[test]
    fn fixed_size_flag_surfaces_in_feature_flags_and_survives_clone() {
        use super::FADF_FIXEDSIZE_VALUE;

        let dynamic = SafeArray::from_variants(vec![Variant::from_i32(7)]);
        assert!(!dynamic.is_fixed_size());
        assert_eq!(dynamic.feature_flags() & FADF_FIXEDSIZE_VALUE, 0);

        let fixed = SafeArray::from_variants(vec![Variant::from_i32(7)]).with_fixed_size(true);
        assert!(fixed.is_fixed_size());
        assert_eq!(
            fixed.feature_flags() & FADF_FIXEDSIZE_VALUE,
            FADF_FIXEDSIZE_VALUE
        );
        // The non-fixed FADF metadata is untouched alongside the fixed bit.
        assert_eq!(
            fixed.feature_flags(),
            dynamic.feature_flags() | FADF_FIXEDSIZE_VALUE
        );

        // The descriptor flag travels with the value copy (Variant assignment,
        // ByVal/ByRef copies all clone the SafeArray).
        let cloned = fixed.clone();
        assert!(cloned.is_fixed_size());
        assert_eq!(cloned.variant_elements(), Some(vec![Variant::from_i32(7)]));

        // Clearing the bit is observable, and a cleared array clones as dynamic.
        let recleared = cloned.with_fixed_size(false);
        assert!(!recleared.is_fixed_size());
        assert!(!recleared.clone().is_fixed_size());
    }

    #[test]
    fn fixed_size_flag_travels_through_raw_descriptor_roundtrip() {
        let fixed = SafeArray::from_typed_variants(VT_I2_VALUE, vec![Variant::from_i16(3)])
            .expect("typed array")
            .with_fixed_size(true);
        let raw = fixed.clone_raw_safearray_ptr();
        let adopted = unsafe { SafeArray::from_raw_safearray_owned(raw) }
            .expect("OxVba-owned descriptor should adopt");
        assert!(adopted.is_fixed_size());
    }

    #[test]
    fn safe_array_from_values_nd_preserves_multi_dimensional_shape() {
        let bounds = vec![
            SafeArrayBound { lower: 1, count: 3 },
            SafeArrayBound { lower: 1, count: 2 },
        ];
        let values = vec![
            Variant::from_i32(1),
            Variant::from_i32(2),
            Variant::from_i32(3),
            Variant::from_i32(4),
            Variant::from_i32(5),
            Variant::from_i32(6),
        ];
        let array = SafeArray::from_variants_nd(bounds.clone(), values.clone());
        assert_eq!(array.dimensions(), 2);
        assert_eq!(array.len(), 6);
        assert_eq!(array.effective_len(), 6);
        assert_eq!(array.bounds().as_ref(), Some(&bounds));
        assert_eq!(array.variant_elements().as_ref(), Some(&values));
    }

    #[test]
    fn safe_array_vector_exposes_descriptor_bounds() {
        let array = SafeArray::vector(5);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(
            array.bounds(),
            Some(vec![SafeArrayBound { lower: 0, count: 5 }])
        );
        assert_eq!(array.variant_elements(), None);
    }

    #[test]
    fn safe_array_replace_elements_preserves_shape() {
        let shape = SafeArray::from_shape(vec![
            SafeArrayBound { lower: 1, count: 2 },
            SafeArrayBound { lower: 4, count: 2 },
        ])
        .expect("shape");
        let replaced = shape
            .replace_variant_elements(vec![
                Variant::from_i32(1),
                Variant::from_i32(2),
                Variant::from_i32(3),
                Variant::from_i32(4),
            ])
            .expect("replace");
        assert_eq!(replaced.bounds(), shape.bounds());
        assert_eq!(replaced.variant_elements().expect("elements").len(), 4);
    }

    #[test]
    fn safe_array_set_variant_element_mutates_payload_without_rebuilding_descriptor() {
        let mut array = SafeArray::from_typed_variants(
            VT_I2_VALUE,
            vec![Variant::from_i16(4), Variant::from_i16(9)],
        )
        .expect("typed array");
        let raw = array.raw_safearray_ptr();

        array
            .set_variant_element(1, &Variant::from_i16(42))
            .expect("set element");

        assert_eq!(array.raw_safearray_ptr(), raw);
        assert_eq!(
            array.variant_elements(),
            Some(vec![Variant::from_i16(4), Variant::from_i16(42)])
        );
    }

    #[test]
    fn safe_array_set_variant_element_replaces_owned_bstr_payload() {
        let mut array =
            SafeArray::from_typed_variants(VT_BSTR_VALUE, vec![Variant::from_string("before")])
                .expect("typed bstr array");
        let raw = array.raw_safearray_ptr();

        array
            .set_variant_element(0, &Variant::from_string("after"))
            .expect("set bstr element");

        assert_eq!(array.raw_safearray_ptr(), raw);
        assert_eq!(
            array.variant_elements(),
            Some(vec![Variant::from_string("after")])
        );
    }

    #[test]
    fn safe_array_set_variant_element_replaces_variant_payload() {
        let mut array = SafeArray::from_variants(vec![Variant::from_i32(1)]);
        let raw = array.raw_safearray_ptr();

        array
            .set_variant_element(0, &Variant::from_string("variant"))
            .expect("set variant element");

        assert_eq!(array.raw_safearray_ptr(), raw);
        assert_eq!(
            array.variant_elements(),
            Some(vec![Variant::from_string("variant")])
        );
    }

    #[test]
    fn native_record_safearray_stores_contiguous_record_payloads() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![VbaRecordFieldSpec::named(
                "Value",
                VbaRecordFieldKind::Long,
            )])
            .expect("layout"),
        );
        let make_record = |value: i32| {
            let mut record = VbaRecord::new_default(layout.clone()).expect("record");
            let field = record.layout().fields()[0].clone();
            unsafe {
                record.field_mut_ptr(&field).cast::<i32>().write(value);
            }
            record
        };
        let mut array = SafeArray::from_vba_records_nd(
            vec![SafeArrayBound { count: 2, lower: 0 }],
            layout.clone(),
            vec![make_record(10), make_record(20)],
        )
        .expect("record array");

        assert_eq!(array.element_vartype(), VT_RECORD_VALUE);
        assert_eq!(
            array.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | super::FADF_RECORD_VALUE
        );
        assert!(array.vba_record_layout().is_some());
        assert_eq!(
            array
                .variant_element(1)
                .expect("element")
                .as_vba_record()
                .expect("record")
                .read_field_variant(0)
                .expect("field")
                .as_i32(),
            Some(20)
        );

        array
            .set_variant_element(1, &Variant::from_vba_record(make_record(99)))
            .expect("replace record element");
        assert_eq!(
            array.variant_elements().expect("elements")[1]
                .as_vba_record()
                .expect("record")
                .read_field_variant(0)
                .expect("field")
                .as_i32(),
            Some(99)
        );
    }

    #[test]
    fn safe_array_variant_element_reads_one_typed_payload_slot() {
        let array = SafeArray::from_typed_variants(
            VT_I2_VALUE,
            vec![
                Variant::from_i16(4),
                Variant::from_i16(9),
                Variant::from_i16(16),
            ],
        )
        .expect("typed array");

        assert_eq!(
            array.variant_element(1).expect("element"),
            Variant::from_i16(9)
        );
        assert_eq!(
            array.variant_element(3).expect_err("out of range"),
            "SAFEARRAY index 3 out of range for length 3"
        );
    }

    #[test]
    fn safe_array_variant_element_reads_owned_bstr_payload_slot() {
        let array = SafeArray::from_typed_variants(
            VT_BSTR_VALUE,
            vec![
                Variant::from_string("first"),
                Variant::from_string("second"),
            ],
        )
        .expect("typed bstr array");

        assert_eq!(
            array.variant_element(1).expect("element"),
            Variant::from_string("second")
        );
    }

    #[test]
    fn typed_i2_safearray_preserves_intrinsic_element_vartype() {
        let array = SafeArray::from_typed_variants(
            VT_I2_VALUE,
            vec![Variant::from_i16(4), Variant::from_i16(9)],
        )
        .expect("typed array");
        assert_eq!(array.element_vartype(), VT_I2_VALUE);
        assert_eq!(array.feature_flags(), FADF_HAVEVARTYPE_VALUE);
        assert_eq!(
            array.variant_elements(),
            Some(vec![Variant::from_i16(4), Variant::from_i16(9)])
        );
    }

    #[test]
    fn safearray_descriptor_advertises_special_typed_element_metadata() {
        let bstr = SafeArray::from_typed_variants(
            VT_BSTR_VALUE,
            vec![Variant::from_string(BStr::from("Alpha"))],
        )
        .expect("typed bstr array");
        assert_eq!(
            bstr.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_BSTR_VALUE
        );

        let dispatch = SafeArray::from_typed_variants(
            VT_DISPATCH_VALUE,
            vec![Variant::from_object_ref(ObjectRef::from_compat_identity(
                41,
            ))],
        )
        .expect("typed dispatch array");
        assert_eq!(
            dispatch.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_DISPATCH_VALUE
        );

        let unknown = SafeArray::from_typed_variants(
            VT_UNKNOWN_VALUE,
            vec![Variant::from_object_ref(ObjectRef::from_compat_identity(
                77,
            ))],
        )
        .expect("typed unknown array");
        assert_eq!(
            unknown.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_UNKNOWN_VALUE
        );
    }

    #[test]
    fn typed_safearray_variant_elements_preserve_intrinsic_carriers_before_projection() {
        let array = SafeArray::from_typed_variants(
            VT_I2_VALUE,
            vec![Variant::from_i16(4), Variant::from_i16(9)],
        )
        .expect("typed array");

        assert_eq!(
            array.variant_elements().expect("variant elements"),
            vec![Variant::from_i16(4), Variant::from_i16(9)]
        );
    }

    #[test]
    fn typed_safearray_variant_construction_encodes_intrinsic_carriers_directly() {
        let array = SafeArray::from_typed_variants(
            VT_I2_VALUE,
            vec![Variant::from_i16(4), Variant::from_i16(9)],
        )
        .expect("typed array from Variants");

        assert_eq!(array.element_vartype(), VT_I2_VALUE);
        assert_eq!(
            array.variant_elements().expect("variant elements"),
            vec![Variant::from_i16(4), Variant::from_i16(9)]
        );
    }

    #[test]
    fn typed_exact_safearray_carriers_preserve_intrinsic_payloads() {
        let currency = SafeArray::from_typed_variants(
            VT_CY_VALUE,
            vec![Variant::from_currency_scaled_i64(-42_500)],
        )
        .expect("typed currency array");
        assert_eq!(
            currency.variant_elements().expect("currency elements")[0].as_currency_scaled_i64(),
            Some(-42_500)
        );

        let decimal_value = Decimal96::from_parts(123_450, 0, 0, 3, true);
        let decimal = SafeArray::from_typed_variants(
            VT_DECIMAL_VALUE,
            vec![Variant::from_decimal96(decimal_value)],
        )
        .expect("typed decimal array");
        assert_eq!(
            decimal.variant_elements().expect("decimal elements")[0].as_decimal96(),
            Some(decimal_value)
        );

        let date =
            SafeArray::from_typed_variants(VT_DATE_VALUE, vec![Variant::from_date_f64(46_081.25)])
                .expect("typed date array");
        assert_eq!(
            date.variant_elements().expect("date elements")[0].as_date_f64(),
            Some(46_081.25)
        );

        let boolean = SafeArray::from_typed_variants(VT_BOOL_VALUE, vec![Variant::from_bool(true)])
            .expect("typed bool array");
        assert_eq!(
            boolean.variant_elements().expect("bool elements")[0].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn typed_bstr_safearray_roundtrips_strings_without_variant_normalization() {
        let array = SafeArray::from_typed_variants(
            VT_BSTR_VALUE,
            vec![
                Variant::from_string(BStr::from("Alpha")),
                Variant::from_string(BStr::from("Beta")),
            ],
        )
        .expect("typed bstr array");
        assert_eq!(array.element_vartype(), VT_BSTR_VALUE);
        assert_eq!(
            array.variant_elements(),
            Some(vec![
                Variant::from_string(BStr::from("Alpha")),
                Variant::from_string(BStr::from("Beta")),
            ])
        );
    }

    #[test]
    fn typed_dispatch_safearray_preserves_intrinsic_object_payloads() {
        let object = ObjectRef::from_compat_identity(41);
        let array = SafeArray::from_typed_variants(
            VT_DISPATCH_VALUE,
            vec![Variant::from_object_ref(object.clone())],
        )
        .expect("typed dispatch array");
        assert_eq!(array.element_vartype(), VT_DISPATCH_VALUE);
        let elements = array.variant_elements().expect("typed dispatch elements");
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0], Variant::from_object_ref(object));
    }

    #[test]
    fn typed_unknown_safearray_preserves_intrinsic_object_payloads() {
        let object = ObjectRef::from_compat_identity(77);
        let array = SafeArray::from_typed_variants(
            VT_UNKNOWN_VALUE,
            vec![Variant::from_object_ref(object.clone())],
        )
        .expect("typed unknown array");
        assert_eq!(array.element_vartype(), VT_UNKNOWN_VALUE);
        let elements = array.variant_elements().expect("typed unknown elements");
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0], Variant::from_object_ref(object));
    }
}

#[cfg(test)]
mod proptests {
    use super::{ARRAY_TAG_BASE, ARRAY_TAG_LIMIT, array_tag_from_safe_array, safe_array_from_tag};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_safe_array_tag_roundtrip(len in 0..=1_000_000usize) {
            let tag = ARRAY_TAG_BASE + len as i32;
            prop_assert!((ARRAY_TAG_BASE..=ARRAY_TAG_LIMIT).contains(&tag));

            let array = safe_array_from_tag(tag)
                .expect("tag in valid range should decode to SafeArray");
            prop_assert_eq!(array.len(), len);
            prop_assert_eq!(array.dimensions(), 1);

            let recovered_tag = array_tag_from_safe_array(&array)
                .expect("decoded SafeArray should encode back to a tag");
            prop_assert_eq!(recovered_tag, tag);
        }
    }
}
