use crate::{
    Decimal96, RuntimeValue, VarType, Variant,
    bstr::BStr,
    object_ref::{ObjectRef, RawRuntimeIUnknown},
};
use core::ptr::NonNull;

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

pub const FADF_HAVEVARTYPE_VALUE: u16 = 0x0080;
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

unsafe impl Send for SafeArray {}
unsafe impl Sync for SafeArray {}

fn alloc_raw_bstr_from_bstr(text: &BStr) -> Result<*mut u16, String> {
    text.clone_raw_bstr()
}

unsafe fn free_raw_bstr(ptr: *mut u16) {
    let _ = unsafe { BStr::from_raw_bstr(ptr) };
}

unsafe fn raw_bstr_to_bstr(ptr: *mut u16) -> BStr {
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
    let prefix_ref = unsafe { &*prefix };
    if prefix_ref.magic == OXVBA_SAFEARRAY_OWNER_MAGIC
        && prefix_ref.version == OXVBA_SAFEARRAY_OWNER_VERSION
    {
        Some(prefix)
    } else {
        None
    }
}

fn payload_offset(kind: SafeArrayElementKind, index: usize) -> usize {
    kind.element_size() * index
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
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    Ok(match kind {
        SafeArrayElementKind::Variant => unsafe { &*ptr.cast::<Variant>() }.clone(),
        SafeArrayElementKind::I1 => Variant::from_i16(i16::from(unsafe { *ptr.cast::<i8>() })),
        SafeArrayElementKind::Ui1 => Variant::from_u8(unsafe { *ptr.cast::<u8>() }),
        SafeArrayElementKind::I2 => Variant::from_i16(unsafe { *ptr.cast::<i16>() }),
        SafeArrayElementKind::Ui2 => Variant::from_i32(i32::from(unsafe { *ptr.cast::<u16>() })),
        SafeArrayElementKind::I4 | SafeArrayElementKind::Int => {
            Variant::from_i32(unsafe { *ptr.cast::<i32>() })
        }
        SafeArrayElementKind::Ui4 | SafeArrayElementKind::UInt => {
            Variant::from_i64(i64::from(unsafe { *ptr.cast::<u32>() }))
        }
        SafeArrayElementKind::I8 => Variant::from_i64(unsafe { *ptr.cast::<i64>() }),
        SafeArrayElementKind::Ui8 => {
            let value = unsafe { *ptr.cast::<u64>() };
            Variant::from_i64(i64::try_from(value).map_err(|_| {
                format!("VT_UI8 SAFEARRAY element {value} exceeds i64 carrier range")
            })?)
        }
        SafeArrayElementKind::R4 => Variant::from_f32(unsafe { *ptr.cast::<f32>() }),
        SafeArrayElementKind::R8 => Variant::from_f64(unsafe { *ptr.cast::<f64>() }),
        SafeArrayElementKind::Currency => {
            Variant::from_currency_scaled_i64(unsafe { *ptr.cast::<i64>() })
        }
        SafeArrayElementKind::Date => Variant::from_date_f64(unsafe { *ptr.cast::<f64>() }),
        SafeArrayElementKind::Bool => Variant::from_bool(unsafe { *ptr.cast::<i16>() } != 0),
        SafeArrayElementKind::BStr => {
            Variant::from_string(unsafe { raw_bstr_to_bstr(*ptr.cast::<*mut u16>()) })
        }
        SafeArrayElementKind::Dispatch | SafeArrayElementKind::Unknown => {
            let raw = bytes_to_raw_iunknown(unsafe { *ptr.cast::<[u8; 8]>() });
            let Some(object) = (unsafe { ObjectRef::from_raw_iunknown_addref(raw) }) else {
                return Err("SAFEARRAY object element carried null IUnknown pointer".to_string());
            };
            Variant::from_object_ref(object)
        }
        SafeArrayElementKind::Decimal => {
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
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    if kind == SafeArrayElementKind::Variant {
        unsafe { ptr.cast::<Variant>().write(value.clone()) };
        return Ok(());
    }
    match kind {
        SafeArrayElementKind::Variant => unreachable!("handled above"),
        SafeArrayElementKind::I1 => unsafe {
            ptr.cast::<i8>()
                .write(i8::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_I1 SAFEARRAY element")
                })?);
        },
        SafeArrayElementKind::Ui1 => unsafe {
            ptr.cast::<u8>().write(
                u8::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI1 SAFEARRAY element")
                })?,
            );
        },
        SafeArrayElementKind::I2 => unsafe {
            ptr.cast::<i16>()
                .write(i16::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_I2 SAFEARRAY element")
                })?);
        },
        SafeArrayElementKind::Ui2 => unsafe {
            ptr.cast::<u16>().write(
                u16::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI2 SAFEARRAY element")
                })?,
            );
        },
        SafeArrayElementKind::I4 | SafeArrayElementKind::Int => unsafe {
            ptr.cast::<i32>()
                .write(i32::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_I4 SAFEARRAY element")
                })?);
        },
        SafeArrayElementKind::Ui4 | SafeArrayElementKind::UInt => unsafe {
            ptr.cast::<u32>().write(
                u32::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI4 SAFEARRAY element")
                })?,
            );
        },
        SafeArrayElementKind::I8 => unsafe {
            ptr.cast::<i64>().write(variant_i64(value)?);
        },
        SafeArrayElementKind::Ui8 => unsafe {
            ptr.cast::<u64>().write(
                u64::try_from(variant_i64(value)?).map_err(|_| {
                    format!("value {value:?} does not fit VT_UI8 SAFEARRAY element")
                })?,
            );
        },
        SafeArrayElementKind::R4 => unsafe {
            ptr.cast::<f32>().write(variant_f64(value)? as f32);
        },
        SafeArrayElementKind::R8 => unsafe {
            ptr.cast::<f64>().write(variant_f64(value)?);
        },
        SafeArrayElementKind::Currency => unsafe {
            ptr.cast::<i64>().write(
                value
                    .as_currency_scaled_i64()
                    .ok_or_else(|| format!("expected Currency SAFEARRAY element, got {value:?}"))?,
            );
        },
        SafeArrayElementKind::Date => unsafe {
            ptr.cast::<f64>().write(
                value
                    .as_date_f64()
                    .or_else(|| value.as_f64())
                    .ok_or_else(|| format!("expected Date SAFEARRAY element, got {value:?}"))?,
            );
        },
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
        SafeArrayElementKind::BStr => unsafe {
            ptr.cast::<*mut u16>().write(alloc_raw_bstr_from_bstr(
                &value
                    .as_bstr()
                    .ok_or_else(|| format!("expected String SAFEARRAY element, got {value:?}"))?,
            )?);
        },
        SafeArrayElementKind::Dispatch | SafeArrayElementKind::Unknown => unsafe {
            let object = value
                .as_object_ref()
                .ok_or_else(|| format!("expected Object SAFEARRAY element, got {value:?}"))?;
            let raw = object.raw_iunknown();
            core::mem::forget(object);
            ptr.cast::<[u8; 8]>().write(raw_iunknown_ptr_to_bytes(raw));
        },
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

unsafe fn drop_element(kind: SafeArrayElementKind, payload: *mut u8, index: usize) {
    let ptr = unsafe { payload.add(payload_offset(kind, index)) };
    match kind {
        SafeArrayElementKind::Variant => unsafe { core::ptr::drop_in_place(ptr.cast::<Variant>()) },
        SafeArrayElementKind::BStr => unsafe { free_raw_bstr(*ptr.cast::<*mut u16>()) },
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
        unsafe { drop_element(kind, raw, index) };
        index += 1;
    }
    if let Ok(layout) = payload_layout(kind, count) {
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
    let raw = unsafe { std::alloc::alloc_zeroed(layout) };
    if raw.is_null() {
        return Err("failed to allocate SAFEARRAY payload".to_string());
    }
    let mut initialized = 0usize;
    while initialized < values.len() {
        if let Err(err) =
            unsafe { encode_element_variant(kind, raw, initialized, &values[initialized]) }
        {
            let mut index = 0usize;
            while index < initialized {
                unsafe { drop_element(kind, raw, index) };
                index += 1;
            }
            unsafe { std::alloc::dealloc(raw, layout) };
            return Err(err);
        }
        initialized += 1;
    }
    Ok(raw.cast())
}

fn alloc_header(
    bounds: &[SafeArrayBound],
    element_vt: u16,
    cb_elements: usize,
    pv_data: *mut core::ffi::c_void,
) -> Result<NonNull<RawSafeArray>, String> {
    let layout = owner_layout(bounds.len())?;
    let raw_owner = unsafe { std::alloc::alloc_zeroed(layout) };
    let Some(raw_owner) = NonNull::new(raw_owner) else {
        return Err("failed to allocate SAFEARRAY header".to_string());
    };
    unsafe {
        raw_owner
            .cast::<RawSafeArrayOwnerPrefix>()
            .as_ptr()
            .write(RawSafeArrayOwnerPrefix {
                magic: OXVBA_SAFEARRAY_OWNER_MAGIC,
                version: OXVBA_SAFEARRAY_OWNER_VERSION,
                element_vt,
            });
        let header = raw_owner
            .as_ptr()
            .add(core::mem::size_of::<RawSafeArrayOwnerPrefix>())
            .cast::<RawSafeArray>();
        (*header).c_dims = u16::try_from(bounds.len())
            .map_err(|_| "SAFEARRAY dimension count exceeds u16 capacity".to_string())?;
        (*header).f_features = SafeArrayElementKind::from_vartype(element_vt)
            .map(SafeArrayElementKind::feature_flags)
            .unwrap_or(FADF_HAVEVARTYPE_VALUE);
        (*header).cb_elements = u32::try_from(cb_elements)
            .map_err(|_| "SAFEARRAY cbElements exceeds u32 capacity".to_string())?;
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
        let header = match alloc_header(&bounds, element_vt, kind.element_size(), pv_data) {
            Ok(header) => header,
            Err(err) => {
                unsafe { free_payload(kind, pv_data, expected_len) };
                return Err(err);
            }
        };
        Ok(Self(header))
    }

    fn from_bounds_and_runtime_values(
        bounds: Vec<SafeArrayBound>,
        element_vt: u16,
        values: Option<Vec<RuntimeValue>>,
    ) -> Result<Self, String> {
        let values = values
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| Variant::try_from_runtime_value(&value))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Self::from_bounds_and_variants(bounds, element_vt, values)
    }

    pub fn vector(len: usize) -> Self {
        Self::from_bounds_and_variants(
            default_bounds_for_len(len).expect("vector bounds should fit SAFEARRAY capacity"),
            VT_VARIANT_VALUE,
            None,
        )
        .expect("shape-only SAFEARRAY allocation should succeed")
    }

    /// Compatibility constructor that projects [`RuntimeValue`] inputs into
    /// retained `Variant` payload elements.
    ///
    /// New value-model call sites should prefer [`Self::from_variants`].
    pub fn from_values(values: Vec<RuntimeValue>) -> Self {
        let len = values.len();
        Self::from_bounds_and_runtime_values(
            default_bounds_for_len(len).expect("value bounds should fit SAFEARRAY capacity"),
            VT_VARIANT_VALUE,
            Some(values),
        )
        .expect("SAFEARRAY payload allocation should succeed for supported canonical values")
    }

    /// Compatibility constructor that projects [`RuntimeValue`] inputs into a
    /// retained multi-dimensional `Variant` payload.
    ///
    /// New value-model call sites should prefer [`Self::from_variants_nd`].
    pub fn from_values_nd(bounds: Vec<SafeArrayBound>, values: Vec<RuntimeValue>) -> Self {
        Self::from_bounds_and_runtime_values(bounds, VT_VARIANT_VALUE, Some(values))
            .expect("SAFEARRAY nd payload allocation should succeed for supported canonical values")
    }

    /// Compatibility constructor that projects [`RuntimeValue`] inputs into a
    /// retained typed SAFEARRAY payload.
    ///
    /// New value-model call sites should prefer [`Self::from_typed_variants`].
    pub fn from_typed_values(element_vt: u16, values: Vec<RuntimeValue>) -> Result<Self, String> {
        let len = values.len();
        Self::from_bounds_and_runtime_values(default_bounds_for_len(len)?, element_vt, Some(values))
    }

    /// Compatibility constructor that projects [`RuntimeValue`] inputs into a
    /// retained multi-dimensional typed SAFEARRAY payload.
    ///
    /// New value-model call sites should prefer [`Self::from_typed_variants_nd`].
    pub fn from_typed_values_nd(
        bounds: Vec<SafeArrayBound>,
        element_vt: u16,
        values: Vec<RuntimeValue>,
    ) -> Result<Self, String> {
        Self::from_bounds_and_runtime_values(bounds, element_vt, Some(values))
    }

    pub fn from_shape(bounds: Vec<SafeArrayBound>) -> Result<Self, String> {
        Self::from_bounds_and_variants(bounds, VT_VARIANT_VALUE, None)
    }

    pub fn from_shape_typed(bounds: Vec<SafeArrayBound>, element_vt: u16) -> Result<Self, String> {
        Self::from_bounds_and_variants(bounds, element_vt, None)
    }

    /// Compatibility constructor that projects [`RuntimeValue`] inputs into an
    /// explicitly shaped retained `Variant` payload.
    ///
    /// New value-model call sites should prefer [`Self::from_shape_and_variants`].
    pub fn from_shape_and_values(
        bounds: Vec<SafeArrayBound>,
        values: Vec<RuntimeValue>,
    ) -> Result<Self, String> {
        Self::from_bounds_and_runtime_values(bounds, VT_VARIANT_VALUE, Some(values))
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
        unsafe { (*self.0.as_ptr()).c_dims as u8 }
    }

    pub fn element_vartype(&self) -> u16 {
        let prefix = unsafe { validated_header_prefix(self.0.as_ptr()) }
            .expect("SAFEARRAY descriptor is not owned by OxVba");
        unsafe { (*prefix).element_vt }
    }

    pub fn feature_flags(&self) -> u16 {
        unsafe { (*self.0.as_ptr()).f_features }
    }

    fn element_kind(&self) -> SafeArrayElementKind {
        SafeArrayElementKind::from_vartype(self.element_vartype())
            .expect("internal SAFEARRAY element vartype should remain supported")
    }

    fn raw_bounds(&self) -> Vec<SafeArrayBound> {
        let dims = self.dimensions() as usize;
        if dims == 0 {
            return Vec::new();
        }
        let ptr =
            unsafe { core::ptr::addr_of!((*self.0.as_ptr()).rgsabound).cast::<SafeArrayBound>() };
        unsafe { core::slice::from_raw_parts(ptr, dims) }.to_vec()
    }

    pub fn len(&self) -> usize {
        bounds_total_len(&self.raw_bounds()).unwrap_or(0)
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

    /// Compatibility accessor that projects retained `Variant` elements into
    /// [`RuntimeValue`] values for legacy callers.
    ///
    /// New value-model call sites should prefer [`Self::variant_elements`].
    pub fn elements(&self) -> Option<Vec<RuntimeValue>> {
        self.variant_elements().map(|values| {
            values
                .into_iter()
                .map(|value| {
                    value
                        .to_runtime_value()
                        .expect("SAFEARRAY Variant element should project to RuntimeValue")
                })
                .collect()
        })
    }

    pub fn variant_elements(&self) -> Option<Vec<Variant>> {
        let data = unsafe { (*self.0.as_ptr()).pv_data.cast::<u8>() };
        if data.is_null() {
            return None;
        }
        let kind = self.element_kind();
        let mut values = Vec::with_capacity(self.len());
        let mut index = 0usize;
        while index < self.len() {
            values.push(
                unsafe { decode_element_variant(kind, data, index) }
                    .expect("SAFEARRAY intrinsic payload should decode into Variant"),
            );
            index += 1;
        }
        Some(values)
    }

    /// Compatibility replacement API that projects [`RuntimeValue`] inputs into
    /// retained payload elements while preserving the current shape and element
    /// vartype.
    ///
    /// New value-model call sites should prefer [`Self::replace_variant_elements`].
    pub fn replace_elements(&self, values: Vec<RuntimeValue>) -> Result<Self, String> {
        Self::from_bounds_and_runtime_values(
            self.bounds_for_shape(),
            self.element_vartype(),
            Some(values),
        )
    }

    pub fn replace_variant_elements(&self, values: Vec<Variant>) -> Result<Self, String> {
        Self::from_bounds_and_variants(
            self.bounds_for_shape(),
            self.element_vartype(),
            Some(values),
        )
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
        unsafe { validated_header_prefix(header.as_ptr()) }?;
        let borrowed = Self(header);
        let cloned = borrowed.clone();
        core::mem::forget(borrowed);
        Some(cloned)
    }
}

impl Clone for SafeArray {
    fn clone(&self) -> Self {
        let bounds = self.bounds_for_shape();
        match self.variant_elements() {
            Some(values) => {
                Self::from_bounds_and_variants(bounds, self.element_vartype(), Some(values))
                    .expect("cloning canonical SAFEARRAY with values should succeed")
            }
            None => Self::from_bounds_and_variants(bounds, self.element_vartype(), None)
                .expect("cloning shape-only SAFEARRAY should succeed"),
        }
    }
}

impl Drop for SafeArray {
    fn drop(&mut self) {
        let len = self.len();
        let kind = self.element_kind();
        let data = unsafe { (*self.0.as_ptr()).pv_data };
        unsafe { free_payload(kind, data, len) };
        if let Ok(layout) = owner_layout(self.dimensions() as usize) {
            let owner = unsafe {
                self.0
                    .as_ptr()
                    .cast::<u8>()
                    .sub(core::mem::size_of::<RawSafeArrayOwnerPrefix>())
            };
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
mod tests {
    use super::{
        ARRAY_TAG_BASE, FADF_BSTR_VALUE, FADF_DISPATCH_VALUE, FADF_HAVEVARTYPE_VALUE,
        FADF_UNKNOWN_VALUE, FADF_VARIANT_VALUE, SafeArray, SafeArrayBound, VT_BSTR_VALUE,
        VT_DISPATCH_VALUE, VT_I2_VALUE, VT_UNKNOWN_VALUE, VT_VARIANT_VALUE, array_len_from_tag,
        array_tag_from_safe_array, header_prefix_ptr, marshal_dispatch_argument,
        safe_array_from_tag,
    };
    use crate::{ObjectRef, RuntimeValue, Variant, bstr::BStr};

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
        let array = SafeArray::from_values(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)]);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(array.len(), 2);
        assert_eq!(array.effective_len(), 2);
        assert_eq!(array.element_vartype(), VT_VARIANT_VALUE);
        assert_eq!(
            array.elements(),
            Some(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)])
        );
        assert_eq!(array_tag_from_safe_array(&array), Some(ARRAY_TAG_BASE + 2));
    }

    #[test]
    fn safe_array_variant_api_preserves_canonical_payload_shape() {
        let array = SafeArray::from_variants(vec![
            Variant::try_from_runtime_value(&RuntimeValue::I32(4)).expect("variant"),
            Variant::try_from_runtime_value(&RuntimeValue::String(BStr::from("A")))
                .expect("variant"),
        ]);
        let elements = array
            .variant_elements()
            .expect("variant SAFEARRAY should expose variants");
        assert_eq!(
            elements[0].to_runtime_value().unwrap(),
            RuntimeValue::I32(4)
        );
        assert_eq!(
            elements[1].to_runtime_value().unwrap(),
            RuntimeValue::String(BStr::from("A"))
        );
        let replaced = array
            .replace_variant_elements(vec![
                Variant::try_from_runtime_value(&RuntimeValue::I32(9)).expect("variant"),
                Variant::try_from_runtime_value(&RuntimeValue::String(BStr::from("B")))
                    .expect("variant"),
            ])
            .expect("replace variant elements");
        assert_eq!(
            replaced.elements(),
            Some(vec![
                RuntimeValue::I32(9),
                RuntimeValue::String(BStr::from("B"))
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
    fn safe_array_from_values_nd_preserves_multi_dimensional_shape() {
        let bounds = vec![
            SafeArrayBound { lower: 1, count: 3 },
            SafeArrayBound { lower: 1, count: 2 },
        ];
        let values = vec![
            RuntimeValue::I32(1),
            RuntimeValue::I32(2),
            RuntimeValue::I32(3),
            RuntimeValue::I32(4),
            RuntimeValue::I32(5),
            RuntimeValue::I32(6),
        ];
        let array = SafeArray::from_values_nd(bounds.clone(), values.clone());
        assert_eq!(array.dimensions(), 2);
        assert_eq!(array.len(), 6);
        assert_eq!(array.effective_len(), 6);
        assert_eq!(array.bounds().as_ref(), Some(&bounds));
        assert_eq!(array.elements().as_ref(), Some(&values));
    }

    #[test]
    fn safe_array_vector_exposes_descriptor_bounds() {
        let array = SafeArray::vector(5);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(
            array.bounds(),
            Some(vec![SafeArrayBound { lower: 0, count: 5 }])
        );
        assert_eq!(array.elements(), None);
    }

    #[test]
    fn safe_array_replace_elements_preserves_shape() {
        let shape = SafeArray::from_shape(vec![
            SafeArrayBound { lower: 1, count: 2 },
            SafeArrayBound { lower: 4, count: 2 },
        ])
        .expect("shape");
        let replaced = shape
            .replace_elements(vec![
                RuntimeValue::I32(1),
                RuntimeValue::I32(2),
                RuntimeValue::I32(3),
                RuntimeValue::I32(4),
            ])
            .expect("replace");
        assert_eq!(replaced.bounds(), shape.bounds());
        assert_eq!(replaced.elements().expect("elements").len(), 4);
    }

    #[test]
    fn typed_i2_safearray_preserves_intrinsic_element_vartype() {
        let array = SafeArray::from_typed_values(
            VT_I2_VALUE,
            vec![RuntimeValue::I32(4), RuntimeValue::I32(9)],
        )
        .expect("typed array");
        assert_eq!(array.element_vartype(), VT_I2_VALUE);
        assert_eq!(array.feature_flags(), FADF_HAVEVARTYPE_VALUE);
        assert_eq!(
            array.elements(),
            Some(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)])
        );
    }

    #[test]
    fn safearray_descriptor_advertises_special_typed_element_metadata() {
        let bstr = SafeArray::from_typed_values(
            VT_BSTR_VALUE,
            vec![RuntimeValue::String(BStr::from("Alpha"))],
        )
        .expect("typed bstr array");
        assert_eq!(
            bstr.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_BSTR_VALUE
        );

        let dispatch = SafeArray::from_typed_values(
            VT_DISPATCH_VALUE,
            vec![RuntimeValue::Object(ObjectRef::from_compat_identity(41))],
        )
        .expect("typed dispatch array");
        assert_eq!(
            dispatch.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_DISPATCH_VALUE
        );

        let unknown = SafeArray::from_typed_values(
            VT_UNKNOWN_VALUE,
            vec![RuntimeValue::Object(ObjectRef::from_compat_identity(77))],
        )
        .expect("typed unknown array");
        assert_eq!(
            unknown.feature_flags(),
            FADF_HAVEVARTYPE_VALUE | FADF_UNKNOWN_VALUE
        );
    }

    #[test]
    fn typed_safearray_variant_elements_preserve_intrinsic_carriers_before_projection() {
        let array = SafeArray::from_typed_values(
            VT_I2_VALUE,
            vec![RuntimeValue::I32(4), RuntimeValue::I32(9)],
        )
        .expect("typed array");

        assert_eq!(
            array.variant_elements().expect("variant elements"),
            vec![Variant::from_i16(4), Variant::from_i16(9)]
        );
        assert_eq!(
            array.elements(),
            Some(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)])
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
        assert_eq!(
            array.elements(),
            Some(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)])
        );
    }

    #[test]
    fn typed_bstr_safearray_roundtrips_strings_without_variant_normalization() {
        let array = SafeArray::from_typed_values(
            VT_BSTR_VALUE,
            vec![
                RuntimeValue::String(BStr::from("Alpha")),
                RuntimeValue::String(BStr::from("Beta")),
            ],
        )
        .expect("typed bstr array");
        assert_eq!(array.element_vartype(), VT_BSTR_VALUE);
        assert_eq!(
            array.elements(),
            Some(vec![
                RuntimeValue::String(BStr::from("Alpha")),
                RuntimeValue::String(BStr::from("Beta")),
            ])
        );
    }

    #[test]
    fn typed_dispatch_safearray_preserves_intrinsic_object_payloads() {
        let object = ObjectRef::from_compat_identity(41);
        let array = SafeArray::from_typed_values(
            VT_DISPATCH_VALUE,
            vec![RuntimeValue::Object(object.clone())],
        )
        .expect("typed dispatch array");
        assert_eq!(array.element_vartype(), VT_DISPATCH_VALUE);
        let elements = array.elements().expect("typed dispatch elements");
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0], RuntimeValue::Object(object));
    }

    #[test]
    fn typed_unknown_safearray_preserves_intrinsic_object_payloads() {
        let object = ObjectRef::from_compat_identity(77);
        let array = SafeArray::from_typed_values(
            VT_UNKNOWN_VALUE,
            vec![RuntimeValue::Object(object.clone())],
        )
        .expect("typed unknown array");
        assert_eq!(array.element_vartype(), VT_UNKNOWN_VALUE);
        let elements = array.elements().expect("typed unknown elements");
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0], RuntimeValue::Object(object));
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
