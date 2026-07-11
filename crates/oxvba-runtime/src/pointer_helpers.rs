use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use crate::{Variant, bstr::BStr};

#[cfg(target_os = "windows")]
use windows_sys::{
    Win32::Foundation::SysFreeString,
    Win32::System::Com::SAFEARRAYBOUND,
    Win32::System::Ole::{
        SafeArrayCreate, SafeArrayCreateVector, SafeArrayDestroy, SafeArrayPutElement,
    },
    Win32::System::Variant::{
        VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_CY, VT_DATE, VT_EMPTY, VT_ERROR, VT_I4, VT_I8,
        VT_NULL, VT_UI1, VT_UNKNOWN, VT_VARIANT, VariantClear,
    },
    core::BSTR,
};

#[cfg(target_os = "windows")]
const VT_R4: u16 = 4;
#[cfg(target_os = "windows")]
const VT_R8: u16 = 5;
#[cfg(target_os = "windows")]
const VT_I1: u16 = 16;
#[cfg(target_os = "windows")]
const VT_UI2: u16 = 18;
#[cfg(target_os = "windows")]
const VT_UI4: u16 = 19;
#[cfg(target_os = "windows")]
const VT_UI8: u16 = 21;
#[cfg(target_os = "windows")]
const VT_UINT: u16 = 23;

#[cfg(target_os = "windows")]
#[derive(Debug)]
// Pointer helpers expose real Windows BSTR cells by cloning the canonical
// runtime BSTR payload.
struct OwnedBstr(Option<BStr>);

#[cfg(target_os = "windows")]
impl OwnedBstr {
    fn from_bstr(text: &BStr) -> Result<Self, String> {
        if text.raw_bstr().is_null() {
            return Ok(Self(None));
        }
        Ok(Self(Some(BStr::from_bytes(text.payload_bytes())?)))
    }

    fn from_text(text: &str) -> Result<Self, String> {
        Ok(Self(Some(BStr::from(text))))
    }

    fn as_ptr(&self) -> *mut c_void {
        self.raw_bstr().cast_mut().cast()
    }

    fn raw_bstr(&self) -> BSTR {
        self.0
            .as_ref()
            .map(BStr::raw_bstr)
            .unwrap_or(std::ptr::null_mut())
    }

    /// Transfer this canonical BSTR owner into a fresh Windows VARIANT and
    /// attach the independent accounting token before returning.
    ///
    /// # Safety
    /// `variant` must point to a writable, zero/VT_EMPTY-initialized VARIANT
    /// cell not currently owning a payload. `accounting` must belong to that
    /// same cell and contain no prior BSTR transfer token.
    unsafe fn transfer_into_variant(
        mut self,
        variant: *mut VARIANT,
        accounting: &mut Option<TrackedBstrAccountingToken>,
    ) -> Result<(), String> {
        if accounting.is_some() {
            return Err("Windows VARIANT already carries a tracked BSTR transfer".to_string());
        }
        let Some(value) = self.0.take() else {
            // SAFETY: guaranteed by this method's fresh writable-cell contract.
            unsafe {
                (*variant).Anonymous.Anonymous.Anonymous.bstrVal = std::ptr::null_mut();
                (*variant).Anonymous.Anonymous.vt = VT_BSTR;
            }
            return Ok(());
        };

        // No fallible operation follows this ownership transfer. The pointer
        // is installed in the destination before the accounting token is
        // attached; both writes target the caller's valid fresh cell.
        let raw = value.into_raw_bstr();
        // SAFETY: guaranteed by this method's fresh writable-cell contract.
        unsafe {
            (*variant).Anonymous.Anonymous.Anonymous.bstrVal = raw;
            (*variant).Anonymous.Anonymous.vt = VT_BSTR;
        }
        *accounting = Some(TrackedBstrAccountingToken);
        Ok(())
    }
}

/// Counter ownership for one canonical BSTR allocation transferred into a
/// Windows VARIANT. Windows owns and frees the native payload after transfer;
/// this token owns only the matching OxVba live-counter debit.
#[cfg(target_os = "windows")]
#[derive(Debug)]
struct TrackedBstrAccountingToken;

#[cfg(target_os = "windows")]
impl Drop for TrackedBstrAccountingToken {
    fn drop(&mut self) {
        crate::live_counters::bstr_freed();
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
// `VarPtr(String)` exposes a pointer to a BSTR cell. The helper therefore owns
// the cell itself and whichever BSTR pointer a native call leaves in that cell.
struct OwnedBstrCell {
    cell: Box<BSTR>,
    original: Option<BStr>,
}

#[cfg(target_os = "windows")]
impl OwnedBstrCell {
    fn from_bstr(text: &BStr) -> Result<Self, String> {
        let mut owner = OwnedBstr::from_bstr(text)?;
        let cell = Box::new(owner.raw_bstr());
        Ok(Self {
            cell,
            original: owner.0.take(),
        })
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&mut *self.cell as *mut BSTR).cast()
    }
}

#[cfg(target_os = "windows")]
impl Drop for OwnedBstrCell {
    fn drop(&mut self) {
        let current = std::mem::replace(&mut *self.cell, std::ptr::null_mut());
        let Some(original) = self.original.take() else {
            if !current.is_null() {
                // SAFETY: A non-null cell with no original OxVba owner can only
                // contain a native replacement whose ownership transferred to
                // this cell. The consuming call has completed before pin drop.
                unsafe { SysFreeString(current) };
            }
            return;
        };
        let original_raw = original.raw_bstr();
        if current == original_raw {
            // The cell still contains its original canonical owner. Dropping
            // through `BStr` performs both the OS free and live-counter debit.
            drop(original);
            return;
        }

        // The native LPBSTR write consumed/freed the original before replacing
        // the cell. Reconcile that original tracked allocation without freeing
        // it twice. The replacement was allocated by native code, so it must be
        // freed below without debiting the OxVba BSTR counter.
        crate::live_counters::bstr_freed();
        // Native code already consumed/freed this allocation. Taking its raw
        // address only disarms the stale Rust owner; the dangling value must
        // never be dereferenced or freed again.
        let _consumed_raw = original.into_raw_bstr();
        if !current.is_null() {
            // SAFETY: `current` is the native replacement now solely owned by
            // this cell, and the consuming native call has completed.
            unsafe { SysFreeString(current) };
        }
    }
}

#[cfg(target_os = "windows")]
// SAFETY: The `Box<BSTR>` cell and the BSTR allocation it points to are owned
// exclusively by this wrapper; BSTRs are thread-agnostic OLE-heap allocations
// and `SysFreeString` is callable from any thread, so transferring the sole
// owner across threads (required by the global `Mutex<PointerRegistry>`) is
// sound.
unsafe impl Send for OwnedBstrCell {}

#[cfg(target_os = "windows")]
// `VarPtr(Variant)` materializes a Windows-observable VARIANT cell from the
// canonical semantic Variant carrier; the raw VARIANT is still a boundary
// projection rather than the canonical runtime container itself.
struct OwnedVariant {
    cell: VARIANT,
    tracked_bstr: Option<TrackedBstrAccountingToken>,
}

#[cfg(target_os = "windows")]
impl std::fmt::Debug for OwnedVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedVariant")
            .field("tracked_bstr", &self.tracked_bstr.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(target_os = "windows")]
impl OwnedVariant {
    fn from_variant(value: &Variant) -> Result<Self, String> {
        // SAFETY: The all-zero bit pattern is a valid VARIANT — vt == VT_EMPTY
        // with no owned payload, the same state `VariantInit` produces; every
        // union field (integers, raw pointers) admits the zero bit pattern.
        let mut owner = Self {
            // SAFETY: the all-zero bit pattern is a valid VT_EMPTY VARIANT.
            cell: unsafe { std::mem::zeroed() },
            tracked_bstr: None,
        };
        // SAFETY: `&mut owner.cell` points to a writable VARIANT local that was
        // just zeroed to VT_EMPTY, so the callee overwrites no pre-owned
        // payload; it stores only payloads matching the vt it sets, which
        // `OwnedVariant`'s Drop later releases via VariantClear.
        unsafe {
            set_windows_variant_from_variant(&mut owner.cell, value, &mut owner.tracked_bstr)?
        };
        Ok(owner)
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&mut self.cell as *mut VARIANT).cast()
    }

    fn as_const_ptr(&self) -> *const VARIANT {
        &self.cell
    }
}

#[cfg(target_os = "windows")]
// SAFETY: ASSUMPTION — the VARIANT struct itself is plain data, and the BSTR /
// SAFEARRAY payloads `set_windows_variant_from_variant` writes are fresh,
// exclusively owned, thread-agnostic OLE-heap allocations; the punkVal payload
// is one retained `ObjectRef` reference whose runtime refcount is atomic
// (object_ref.rs `ref_count: AtomicU32`), so retain/release of runtime objects
// is thread-safe. What must additionally hold for foreign (CreateObject) COM
// payloads, which may be apartment-affine: the pin is created and dropped
// (VariantClear → Release) on the same VM thread within one statement, per
// docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md — Send is demanded only
// because the registry lives behind a process-global Mutex, not because
// entries actually migrate threads.
unsafe impl Send for OwnedVariant {}

#[cfg(target_os = "windows")]
fn retained_iunknown_pointer(object: &crate::ObjectRef) -> *mut c_void {
    object.query_iunknown().into_raw_iunknown().cast::<c_void>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_windows_variant_array_arg(
    variant: *mut VARIANT,
    array: &crate::safe_array::SafeArray,
) -> Result<(), String> {
    // A declared-but-unallocated array (`Dim a()` never `ReDim`'d, or a fixed
    // shape whose storage is null) has a descriptor shape but no element
    // payload. VBA still marshals an array of that shape, so synthesize
    // default (`Empty`) elements sized to the bounds — an empty array when the
    // descriptor carries no bounds either. These flow through the same
    // SAFEARRAY-creation paths below as a real payload.
    let values: Vec<Variant> = match array.try_variant_elements()? {
        Some(values) => values,
        None => {
            let count = array
                .bounds()
                .map(|dims| dims.iter().map(|d| d.count as usize).product::<usize>())
                .unwrap_or(0);
            vec![Variant::empty(); count]
        }
    };

    if let Some(bounds) = array.bounds()
        && bounds.len() > 1
    {
        let dims = u32::try_from(bounds.len())
            .map_err(|_| "SAFEARRAY dimension count exceeds supported u32 range".to_string())?;
        let sa_bounds: Vec<SAFEARRAYBOUND> = bounds
            .iter()
            .map(|b| SAFEARRAYBOUND {
                cElements: b.count,
                lLbound: b.lower,
            })
            .collect();
        let psa = SafeArrayCreate(VT_VARIANT, dims, sa_bounds.as_ptr());
        if psa.is_null() {
            return Err("SafeArrayCreate(VT_VARIANT) returned null".to_string());
        }
        let mut indices: Vec<i32> = bounds.iter().map(|b| b.lower).collect();
        for value in values {
            let element = match OwnedVariant::from_variant(&value) {
                Ok(element) => element,
                Err(detail) => {
                    let _ = SafeArrayDestroy(psa.cast_const());
                    return Err(detail);
                }
            };
            let hr = SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                element.as_const_ptr().cast(),
            );
            // SafeArrayPutElement copies the VARIANT payload; the temporary
            // still owns its source and accounting token and is cleared now.
            drop(element);
            if hr < 0 {
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(format!(
                    "SafeArrayPutElement failed with HRESULT {:#010X} at indices {indices:?}",
                    hr as u32
                ));
            }
            let mut carry = true;
            for (dim_idx, bound) in bounds.iter().enumerate() {
                if !carry {
                    break;
                }
                indices[dim_idx] += 1;
                if indices[dim_idx] >= bound.lower + bound.count as i32 {
                    indices[dim_idx] = bound.lower;
                } else {
                    carry = false;
                }
            }
        }
        (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
        (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
        return Ok(());
    }

    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_VARIANT, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_VARIANT) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let element = match OwnedVariant::from_variant(value) {
            Ok(element) => element,
            Err(detail) => {
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(detail);
            }
        };
        let index = match i32::try_from(offset) {
            Ok(index) => index,
            Err(_) => {
                drop(element);
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err("SAFEARRAY index exceeds supported i32 range".to_string());
            }
        };
        let hr = SafeArrayPutElement(psa.cast_const(), &index, element.as_const_ptr().cast());
        // SafeArrayPutElement copies the VARIANT payload; clear the temporary
        // and settle its transfer token independently of the SAFEARRAY copy.
        drop(element);
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_windows_variant_from_variant(
    variant: *mut VARIANT,
    value: &Variant,
    tracked_bstr: &mut Option<TrackedBstrAccountingToken>,
) -> Result<(), String> {
    // This projection is only valid for the fresh VT_EMPTY cell and matching
    // empty accounting slot constructed by OwnedVariant::from_variant.
    match value.vtype() {
        crate::VarType::Empty => {
            (*variant).Anonymous.Anonymous.vt = VT_EMPTY;
        }
        crate::VarType::Null => {
            (*variant).Anonymous.Anonymous.vt = VT_NULL;
        }
        crate::VarType::Error => {
            (*variant).Anonymous.Anonymous.vt = VT_ERROR;
            (*variant).Anonymous.Anonymous.Anonymous.scode =
                value.as_error_code().expect("error payload");
        }
        crate::VarType::Integer => {
            (*variant).Anonymous.Anonymous.vt = VT_I4;
            (*variant).Anonymous.Anonymous.Anonymous.lVal =
                i32::from(value.as_i16().expect("integer payload"));
        }
        crate::VarType::Long => {
            (*variant).Anonymous.Anonymous.vt = VT_I4;
            (*variant).Anonymous.Anonymous.Anonymous.lVal = value.as_i32().expect("long payload");
        }
        crate::VarType::SignedByte => {
            (*variant).Anonymous.Anonymous.vt = VT_I1;
            (*variant).Anonymous.Anonymous.Anonymous.cVal = value.as_i8().expect("i1 payload");
        }
        crate::VarType::Byte => {
            (*variant).Anonymous.Anonymous.vt = VT_UI1;
            (*variant).Anonymous.Anonymous.Anonymous.bVal = value.as_u8().expect("byte payload");
        }
        crate::VarType::UnsignedInteger => {
            (*variant).Anonymous.Anonymous.vt = VT_UI2;
            (*variant).Anonymous.Anonymous.Anonymous.uiVal = value.as_u16().expect("ui2 payload");
        }
        crate::VarType::UnsignedLong => {
            (*variant).Anonymous.Anonymous.vt = VT_UI4;
            (*variant).Anonymous.Anonymous.Anonymous.ulVal = value.as_u32().expect("ui4 payload");
        }
        crate::VarType::UnsignedInt => {
            (*variant).Anonymous.Anonymous.vt = VT_UINT;
            (*variant).Anonymous.Anonymous.Anonymous.uintVal =
                value.as_u32().expect("uint payload");
        }
        crate::VarType::LongLong => {
            (*variant).Anonymous.Anonymous.vt = VT_I8;
            (*variant).Anonymous.Anonymous.Anonymous.llVal = value.as_i64().expect("i64 payload");
        }
        crate::VarType::UnsignedLongLong => {
            (*variant).Anonymous.Anonymous.vt = VT_UI8;
            (*variant).Anonymous.Anonymous.Anonymous.ullVal = value.as_u64().expect("ui8 payload");
        }
        crate::VarType::Boolean => {
            (*variant).Anonymous.Anonymous.vt = VT_BOOL;
            (*variant).Anonymous.Anonymous.Anonymous.boolVal =
                if value.as_bool().expect("bool payload") {
                    -1
                } else {
                    0
                };
        }
        crate::VarType::Single => {
            (*variant).Anonymous.Anonymous.vt = VT_R4;
            (*variant).Anonymous.Anonymous.Anonymous.fltVal =
                value.as_f32().expect("single payload");
        }
        crate::VarType::Double => {
            (*variant).Anonymous.Anonymous.vt = VT_R8;
            (*variant).Anonymous.Anonymous.Anonymous.dblVal =
                value.as_f64().expect("double payload");
        }
        crate::VarType::Date => {
            (*variant).Anonymous.Anonymous.vt = VT_DATE;
            (*variant).Anonymous.Anonymous.Anonymous.dblVal =
                value.as_date_f64().expect("date payload");
        }
        crate::VarType::Currency => {
            (*variant).Anonymous.Anonymous.vt = VT_CY;
            (*variant).Anonymous.Anonymous.Anonymous.cyVal.int64 =
                value.as_currency_scaled_i64().expect("currency payload");
        }
        crate::VarType::String => {
            let text = value
                .try_as_bstr()?
                .ok_or_else(|| "canonical string Variant lost owned BSTR payload".to_string())?;
            // SAFETY: this function's caller supplies the fresh writable
            // VARIANT cell and the matching empty accounting slot.
            unsafe { OwnedBstr::from_bstr(&text)?.transfer_into_variant(variant, tracked_bstr)? };
        }
        crate::VarType::Decimal => {
            let bytes = value.to_wire_bytes();
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), variant.cast::<u8>(), bytes.len());
        }
        crate::VarType::Object => {
            let Some(object) = value.as_object_ref() else {
                (*variant).Anonymous.Anonymous.vt = VT_UNKNOWN;
                (*variant).Anonymous.Anonymous.Anonymous.punkVal = std::ptr::null_mut();
                return Ok(());
            };
            (*variant).Anonymous.Anonymous.vt = VT_UNKNOWN;
            (*variant).Anonymous.Anonymous.Anonymous.punkVal = if object.raw() == 0 {
                std::ptr::null_mut()
            } else {
                retained_iunknown_pointer(&object)
            };
        }
        crate::VarType::ArrayVariant => {
            let Some(array) = value.try_as_safearray()? else {
                return Err(
                    "VarPtr over Variant containing null SAFEARRAY payload is not yet supported"
                        .to_string(),
                );
            };
            set_windows_variant_array_arg(variant, &array)?;
        }
        crate::VarType::Record => {
            return Err(
                "COM records cannot be marshaled by the generic pointer helper".to_string(),
            );
        }
        crate::VarType::ProcRef => {
            return Err("procedure references cannot be marshaled as VARIANT values".to_string());
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
impl Drop for OwnedVariant {
    fn drop(&mut self) {
        // SAFETY: `self.cell` was fully initialized by `from_variant` (zeroed to
        // VT_EMPTY, then populated by `set_windows_variant_from_variant`). The
        // VARIANT owns its BSTR/SAFEARRAY/IUnknown payload until VariantClear,
        // and this cell is that payload's sole owner (the registry holds the
        // entry until `free_pins` removes it, after the consuming native call
        // per docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md), so this releases
        // the current payload exactly once; VariantClear on VT_EMPTY is a no-op.
        // The transfer token is independent of the cell's current pointer/type:
        // native code may already have cleared the original and installed null
        // or a replacement. Settle the original tracked allocation only after
        // VariantClear has released whichever valid native value remains.
        unsafe {
            let _ = VariantClear(&mut self.cell);
        }
        drop(self.tracked_bstr.take());
    }
}

#[derive(Debug)]
enum PointerEntry {
    #[cfg(target_os = "windows")]
    Bstr(OwnedBstr),
    #[cfg(target_os = "windows")]
    BstrCell(OwnedBstrCell),
    #[cfg(target_os = "windows")]
    VariantCell(OwnedVariant),
    #[cfg(not(target_os = "windows"))]
    Utf16(Box<[u16]>),
    Bytes(Box<[u8]>),
    I8(Box<i8>),
    U8(Box<u8>),
    I16(Box<i16>),
    U16(Box<u16>),
    I32(Box<i32>),
    U32(Box<u32>),
    I64(Box<i64>),
    U64(Box<u64>),
    F32(Box<f32>),
    F64(Box<f64>),
    Bool(Box<i16>),
    ObjectIdentity(Box<i64>),
}

impl PointerEntry {
    fn as_ptr(&mut self) -> *mut c_void {
        match self {
            #[cfg(target_os = "windows")]
            Self::Bstr(value) => value.as_ptr(),
            #[cfg(target_os = "windows")]
            Self::BstrCell(value) => value.as_ptr(),
            #[cfg(target_os = "windows")]
            Self::VariantCell(value) => value.as_ptr(),
            #[cfg(not(target_os = "windows"))]
            Self::Utf16(value) => value.as_mut_ptr().cast(),
            Self::Bytes(value) => value.as_mut_ptr().cast(),
            Self::I8(value) => (&mut **value as *mut i8).cast(),
            Self::U8(value) => (&mut **value as *mut u8).cast(),
            Self::I16(value) => (&mut **value as *mut i16).cast(),
            Self::U16(value) => (&mut **value as *mut u16).cast(),
            Self::I32(value) => (&mut **value as *mut i32).cast(),
            Self::U32(value) => (&mut **value as *mut u32).cast(),
            Self::I64(value) => (&mut **value as *mut i64).cast(),
            Self::U64(value) => (&mut **value as *mut u64).cast(),
            Self::F32(value) => (&mut **value as *mut f32).cast(),
            Self::F64(value) => (&mut **value as *mut f64).cast(),
            Self::Bool(value) => (&mut **value as *mut i16).cast(),
            Self::ObjectIdentity(value) => (&mut **value as *mut i64).cast(),
        }
    }
}

#[derive(Debug, Default)]
struct PointerRegistry {
    entries: HashMap<usize, PointerEntry>,
}

impl PointerRegistry {
    fn insert(&mut self, mut entry: PointerEntry) -> i64 {
        let addr = entry.as_ptr() as usize;
        self.entries.insert(addr, entry);
        addr as i64
    }

    fn read_back_string_payload_variant(&self, pointer: i64) -> Result<Variant, String> {
        if pointer == 0 {
            return Ok(Variant::from_string(BStr::empty()));
        }
        let Some(entry) = self.entries.get(&(pointer as usize)) else {
            return Err(format!(
                "pointer helper registry does not contain string payload pointer {pointer}"
            ));
        };
        match entry {
            #[cfg(target_os = "windows")]
            PointerEntry::Bstr(value) => {
                let Some(value) = value.0.as_ref() else {
                    return Ok(Variant::from_string(BStr::empty()));
                };
                Ok(Variant::from_string(BStr::from_bytes(
                    value.payload_bytes(),
                )?))
            }
            #[cfg(target_os = "windows")]
            PointerEntry::BstrCell(value) => {
                if (*value.cell).is_null() {
                    return Ok(Variant::from_string(BStr::empty()));
                }
                // SAFETY: `*value.cell` was checked non-null just above. The
                // cell holds either our original allocation or the BSTR a
                // native call wrote through the VarPtr(String) cell, whose
                // ownership transferred to the cell with that write; per
                // docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md the pin (and
                // thus the BSTR) stays live until `free_pins` runs after this
                // read-back, so SysStringByteLen reads a live allocation's 4-byte
                // byte-length prefix at ptr-4.
                let len = unsafe { windows_sys::Win32::Foundation::SysStringByteLen(*value.cell) }
                    as usize;
                // SAFETY: Same live, non-null BSTR as above; `len` is the
                // payload byte count from the length prefix, so the first
                // `len` bytes are within the allocation and readable. Embedded
                // NULs and odd byte lengths are legal because the length is
                // prefix-derived; the registry guard held by the caller keeps
                // the entry alive for the duration of this borrow.
                let bytes = unsafe { std::slice::from_raw_parts((*value.cell).cast::<u8>(), len) };
                Ok(Variant::from_string(BStr::from_bytes(bytes)?))
            }
            #[cfg(not(target_os = "windows"))]
            PointerEntry::Utf16(value) => {
                let end = value
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(value.len());
                Ok(Variant::from_string(BStr::from_utf16_units(&value[..end])?))
            }
            other => Err(format!(
                "pointer helper entry {other:?} cannot be read back as a string payload"
            )),
        }
    }

    fn read_back_byte_array_payload_variant(&self, pointer: i64) -> Result<Variant, String> {
        if pointer == 0 {
            return Ok(Variant::from_safearray(
                crate::safe_array::SafeArray::from_variants(Vec::new()),
            ));
        }
        let Some(entry) = self.entries.get(&(pointer as usize)) else {
            return Err(format!(
                "pointer helper registry does not contain byte-array payload pointer {pointer}"
            ));
        };
        match entry {
            PointerEntry::Bytes(bytes) => Ok(Variant::from_safearray(
                crate::safe_array::SafeArray::from_variants(
                    bytes
                        .iter()
                        .map(|byte| crate::Variant::from_u8(*byte))
                        .collect(),
                ),
            )),
            other => Err(format!(
                "pointer helper entry {other:?} cannot be read back as a byte-array payload"
            )),
        }
    }

    fn read_back_scalar_payload_variant(
        &self,
        pointer: i64,
        kind: ScalarPointerKind,
    ) -> Result<Variant, String> {
        let Some(entry) = self.entries.get(&(pointer as usize)) else {
            return Err(format!(
                "pointer helper registry does not contain scalar payload pointer {pointer}"
            ));
        };
        match (kind, entry) {
            (ScalarPointerKind::Boolean, PointerEntry::Bool(value)) => {
                Ok(Variant::from_bool(**value != 0))
            }
            (ScalarPointerKind::Byte, PointerEntry::U8(value)) => Ok(Variant::from_u8(**value)),
            (ScalarPointerKind::Integer, PointerEntry::I16(value)) => {
                Ok(Variant::from_i16(**value))
            }
            (ScalarPointerKind::Long, PointerEntry::I32(value)) => Ok(Variant::from_i32(**value)),
            (ScalarPointerKind::LongLong, PointerEntry::I64(value)) => {
                Ok(Variant::from_i64(**value))
            }
            (ScalarPointerKind::LongPtr, PointerEntry::I64(value)) => {
                Ok(Variant::from_i64(**value))
            }
            (ScalarPointerKind::Single, PointerEntry::F32(value)) => Ok(Variant::from_f32(**value)),
            (ScalarPointerKind::Double, PointerEntry::F64(value)) => Ok(Variant::from_f64(**value)),
            (ScalarPointerKind::Currency, PointerEntry::I64(value)) => {
                Ok(Variant::from_currency_scaled_i64(**value))
            }
            (ScalarPointerKind::Date, PointerEntry::F64(value)) => {
                Ok(Variant::from_date_f64(**value))
            }
            (_, other) => Err(format!(
                "pointer helper entry {other:?} cannot be read back as {kind:?}"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarPointerKind {
    Boolean,
    Byte,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Single,
    Double,
    Currency,
    Date,
}

fn registry() -> &'static Mutex<PointerRegistry> {
    static REGISTRY: OnceLock<Mutex<PointerRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(PointerRegistry::default()))
}

pub fn register_utf16_string(text: &str) -> Result<i64, String> {
    #[cfg(target_os = "windows")]
    let entry = PointerEntry::Bstr(OwnedBstr::from_text(text)?);

    #[cfg(not(target_os = "windows"))]
    let entry = {
        let core = BStr::from(text).owned_core();
        PointerEntry::Utf16(core.payload_units_with_nul().to_vec().into_boxed_slice())
    };

    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(entry))
}

pub fn register_variant_pointer(value: &Variant) -> Result<i64, String> {
    let entry = match value.vtype() {
        crate::VarType::Empty | crate::VarType::Null => return Ok(0),
        crate::VarType::Integer => {
            PointerEntry::I16(Box::new(value.as_i16().expect("Integer Variant payload")))
        }
        crate::VarType::Long => {
            PointerEntry::I32(Box::new(value.as_i32().expect("Long Variant payload")))
        }
        crate::VarType::LongLong => {
            PointerEntry::I64(Box::new(value.as_i64().expect("LongLong Variant payload")))
        }
        crate::VarType::SignedByte => {
            PointerEntry::I8(Box::new(value.as_i8().expect("SignedByte Variant payload")))
        }
        crate::VarType::Byte => {
            PointerEntry::U8(Box::new(value.as_u8().expect("Byte Variant payload")))
        }
        crate::VarType::UnsignedInteger => PointerEntry::U16(Box::new(
            value.as_u16().expect("UnsignedInteger Variant payload"),
        )),
        crate::VarType::UnsignedLong | crate::VarType::UnsignedInt => PointerEntry::U32(Box::new(
            value.as_u32().expect("unsigned 32-bit Variant payload"),
        )),
        crate::VarType::UnsignedLongLong => PointerEntry::U64(Box::new(
            value.as_u64().expect("UnsignedLongLong Variant payload"),
        )),
        crate::VarType::Single => {
            PointerEntry::F32(Box::new(value.as_f32().expect("Single Variant payload")))
        }
        crate::VarType::Double => {
            PointerEntry::F64(Box::new(value.as_f64().expect("Double Variant payload")))
        }
        crate::VarType::Date => {
            PointerEntry::F64(Box::new(value.as_date_f64().expect("Date Variant payload")))
        }
        crate::VarType::Currency => PointerEntry::I64(Box::new(
            value
                .as_currency_scaled_i64()
                .expect("Currency Variant payload"),
        )),
        crate::VarType::Boolean => PointerEntry::Bool(Box::new(
            if value.as_bool().expect("Boolean Variant payload") {
                -1
            } else {
                0
            },
        )),
        crate::VarType::String => {
            let text = value
                .as_bstr()
                .ok_or_else(|| "String Variant lost BSTR payload".to_string())?;
            #[cfg(target_os = "windows")]
            {
                PointerEntry::Bstr(OwnedBstr::from_bstr(&text)?)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let core = text.owned_core();
                let data: Box<[u16]> = core.payload_units_with_nul().to_vec().into_boxed_slice();
                PointerEntry::Utf16(data)
            }
        }
        crate::VarType::ArrayVariant => {
            let Some(array) = value.as_safearray() else {
                return Err("VarPtr over null SAFEARRAY payload is not yet supported".to_string());
            };
            byte_array_variant_pointer_entry(&array)?
        }
        crate::VarType::Object => {
            let Some(handle) = value.as_object_ref() else {
                return Ok(0);
            };
            PointerEntry::ObjectIdentity(Box::new(handle.raw_iunknown() as usize as i64))
        }
        crate::VarType::Error => PointerEntry::I32(Box::new(
            value.as_error_code().expect("Error Variant payload"),
        )),
        crate::VarType::Decimal => {
            return Err("VarPtr/ObjPtr over Decimal is not yet supported".to_string());
        }
        crate::VarType::Record => {
            return Err("VarPtr/ObjPtr over COM records is not supported".to_string());
        }
        crate::VarType::ProcRef => {
            return Err("VarPtr/ObjPtr over procedure references is not supported".to_string());
        }
    };

    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(entry))
}

fn byte_array_variant_pointer_entry(
    array: &crate::safe_array::SafeArray,
) -> Result<PointerEntry, String> {
    let Some(elements) = array.variant_elements() else {
        return Err(
            "VarPtr over array shape without element payload is not yet supported".to_string(),
        );
    };
    let mut bytes = Vec::with_capacity(elements.len());
    for element in elements {
        match element.vtype() {
            crate::VarType::Empty | crate::VarType::Null => bytes.push(0),
            crate::VarType::Byte => bytes.push(element.as_u8().expect("Byte Variant payload")),
            crate::VarType::Boolean => {
                bytes.push(if element.as_bool().expect("Boolean Variant payload") {
                    1
                } else {
                    0
                })
            }
            crate::VarType::Integer => {
                let value = i32::from(element.as_i16().expect("Integer Variant payload"));
                if !(0..=255).contains(&value) {
                    return Err(format!(
                        "VarPtr over array payload currently requires byte-compatible elements, got Integer({value})"
                    ));
                }
                bytes.push(value as u8);
            }
            crate::VarType::Long => {
                let value = element.as_i32().expect("Long Variant payload");
                if !(0..=255).contains(&value) {
                    return Err(format!(
                        "VarPtr over array payload currently requires byte-compatible elements, got Long({value})"
                    ));
                }
                bytes.push(value as u8);
            }
            other => {
                return Err(format!(
                    "VarPtr over array payload currently requires byte-compatible Variant elements, got {other:?}"
                ));
            }
        }
    }
    Ok(PointerEntry::Bytes(bytes.into_boxed_slice()))
}

pub fn register_string_variant_pointer(value: &Variant) -> Result<i64, String> {
    #[cfg(target_os = "windows")]
    {
        let entry = match value.vtype() {
            crate::VarType::String => {
                let text = value
                    .as_bstr()
                    .ok_or_else(|| "String Variant lost BSTR payload".to_string())?;
                PointerEntry::BstrCell(OwnedBstrCell::from_bstr(&text)?)
            }
            crate::VarType::Empty | crate::VarType::Null => return Ok(0),
            _ => return Err("VarPtr over String requires a string variable".to_string()),
        };

        let mut guard = registry()
            .lock()
            .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
        Ok(guard.insert(entry))
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_variant_pointer(value)
    }
}

pub fn register_variant_var_variant_pointer(value: &Variant) -> Result<i64, String> {
    #[cfg(target_os = "windows")]
    {
        let entry = PointerEntry::VariantCell(OwnedVariant::from_variant(value)?);
        let mut guard = registry()
            .lock()
            .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
        Ok(guard.insert(entry))
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_variant_pointer(value)
    }
}

pub fn register_object_variant_pointer(value: &Variant) -> Result<i64, String> {
    match value.vtype() {
        crate::VarType::Empty | crate::VarType::Null => Ok(0),
        crate::VarType::Object => match value.as_object_ref() {
            Some(handle) if handle.raw() == 0 => Ok(0),
            Some(handle) => Ok(handle.raw_iunknown() as usize as i64),
            None => Ok(0),
        },
        _ => Err("ObjPtr requires an object reference".to_string()),
    }
}

pub fn register_array_payload_pointer(array: &crate::safe_array::SafeArray) -> Result<i64, String> {
    let entry = byte_array_variant_pointer_entry(array)?;
    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(entry))
}

pub fn lookup_pointer(pointer: i64) -> Option<*mut c_void> {
    if pointer == 0 {
        return Some(std::ptr::null_mut());
    }
    let mut guard = registry().lock().ok()?;
    let entry = guard.entries.get_mut(&(pointer as usize))?;
    Some(entry.as_ptr())
}

pub fn read_back_string_payload_variant(pointer: i64) -> Result<Variant, String> {
    let guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    guard.read_back_string_payload_variant(pointer)
}

pub fn read_back_byte_array_payload_variant(pointer: i64) -> Result<Variant, String> {
    let guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    guard.read_back_byte_array_payload_variant(pointer)
}

pub fn read_back_scalar_payload_variant(
    pointer: i64,
    kind: ScalarPointerKind,
) -> Result<Variant, String> {
    let guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    guard.read_back_scalar_payload_variant(pointer, kind)
}

pub fn register_byte_buffer(bytes: Vec<u8>) -> Result<i64, String> {
    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(PointerEntry::Bytes(bytes.into_boxed_slice())))
}

/// Release the pinned cells at the given addresses, dropping each entry (which
/// frees its native BSTR/VARIANT/buffer). Addresses not in the registry — `0`,
/// already-freed, or never-pinned integers — are ignored. Pins are scoped to the
/// VBA statement that creates them: the VM drains the addresses it pinned at each
/// statement boundary (after any pointer write-back has read them), matching VBA's
/// "the pointer is valid for the duration of the call" contract and keeping the
/// registry bounded for long-running/looping code instead of leaking every pin.
pub fn free_pins(addrs: &[i64]) {
    if addrs.is_empty() {
        return;
    }
    let Ok(mut guard) = registry().lock() else {
        return;
    };
    for addr in addrs {
        if *addr != 0 {
            guard.entries.remove(&(*addr as usize));
        }
    }
}

/// Number of pinned cells currently live in the registry. Diagnostic — used by
/// tests to assert that pointer-helper pins do not accumulate across a loop.
pub fn live_pin_count() -> usize {
    registry()
        .lock()
        .map(|guard| guard.entries.len())
        .unwrap_or(0)
}

#[cfg(test)]
// Test-support code exercising the documented production pointer-helper paths.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::{
        ScalarPointerKind, free_pins, lookup_pointer, register_byte_buffer,
        register_object_variant_pointer, register_string_variant_pointer, register_utf16_string,
        register_variant_pointer, register_variant_var_variant_pointer,
    };
    use crate::{Decimal96, ObjectRef, VarType, Variant, bstr::BStr};
    #[cfg(target_os = "windows")]
    use windows_sys::{
        Win32::Foundation::{
            SysAllocStringByteLen, SysAllocStringLen, SysFreeString, SysStringLen,
        },
        Win32::System::Ole::{SafeArrayGetDim, SafeArrayGetElement},
        Win32::System::Variant::{
            VARIANT, VT_ARRAY, VT_BSTR, VT_DATE, VT_I4, VT_UNKNOWN, VT_VARIANT, VariantClear,
        },
        core::BSTR,
    };

    #[test]
    fn utf16_pointer_helper_allocates_terminated_text() {
        let ptr = register_utf16_string("abc").expect("register string");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr).expect("lookup pointer") as *const u16;
        assert!(!raw.is_null());
        let slice = unsafe { std::slice::from_raw_parts(raw, 4) };
        assert_eq!(slice, &[97, 98, 99, 0]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn string_pointer_helper_materializes_real_bstr_payload() {
        let ptr = register_utf16_string("abc").expect("register string");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr).expect("lookup pointer").cast::<u16>();
        assert!(!raw.is_null());
        let len = unsafe { SysStringLen(raw.cast()) };
        assert_eq!(len, 3);
    }

    #[test]
    fn free_pins_releases_only_the_named_addresses() {
        // `free_pins` removes exactly the listed pins (idempotent on absent
        // addresses) so the VM can scope a statement's pins without disturbing
        // any other live pin. Keyed by concrete addresses, so it is robust to
        // pins other tests leave in the shared global registry.
        let a = register_byte_buffer(vec![1, 2, 3]).expect("register a");
        let b = register_byte_buffer(vec![4, 5, 6]).expect("register b");
        assert!(lookup_pointer(a).is_some());
        assert!(lookup_pointer(b).is_some());

        free_pins(&[a, 0, a]); // includes a no-op `0` and a duplicate
        assert!(lookup_pointer(a).is_none(), "a should be released");
        assert!(lookup_pointer(b).is_some(), "b must be untouched");

        free_pins(&[b]);
        assert!(lookup_pointer(b).is_none(), "b should be released");
    }

    #[test]
    fn variant_pointer_handles_canonical_scalars_and_strings() {
        let string_ptr = register_variant_pointer(&Variant::from_string(BStr::from("xyz")))
            .expect("register string variant");
        assert_ne!(string_ptr, 0);
        let scalar_ptr =
            register_variant_pointer(&Variant::from_i64(42)).expect("register i64 variant");
        assert_ne!(scalar_ptr, 0);
    }

    #[test]
    fn variant_pointer_preserves_date_and_byte_array_payloads() {
        let date_ptr =
            register_variant_pointer(&Variant::from_date_f64(42.25)).expect("register date");
        let date_raw = lookup_pointer(date_ptr)
            .expect("lookup date pointer")
            .cast::<f64>();
        assert!(!date_raw.is_null());
        assert_eq!(unsafe { *date_raw }, 42.25);

        let array_ptr = register_variant_pointer(&Variant::from_safearray(
            crate::safe_array::SafeArray::from_variants(vec![
                Variant::from_u8(1),
                Variant::from_bool(false),
                Variant::from_i32(3),
            ]),
        ))
        .expect("register variant array");
        let array_raw = lookup_pointer(array_ptr)
            .expect("lookup array pointer")
            .cast::<u8>();
        assert!(!array_raw.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(array_raw, 3) };
        assert_eq!(bytes, &[1, 0, 3]);
    }

    #[test]
    fn scalar_pointer_readback_uses_declared_storage_widths() {
        let long_ptr = register_variant_pointer(&Variant::from_i32(10)).expect("register long");
        let long_raw = lookup_pointer(long_ptr)
            .expect("lookup long pointer")
            .cast::<i32>();
        unsafe {
            *long_raw = 0x11223344;
        }
        assert_eq!(
            super::read_back_scalar_payload_variant(long_ptr, ScalarPointerKind::Long)
                .expect("read back long"),
            Variant::from_i32(0x11223344)
        );

        let int_ptr = register_variant_pointer(&Variant::from_i16(7)).expect("register integer");
        let int_raw = lookup_pointer(int_ptr)
            .expect("lookup integer pointer")
            .cast::<i16>();
        unsafe {
            *int_raw = -1234;
        }
        assert_eq!(
            super::read_back_scalar_payload_variant(int_ptr, ScalarPointerKind::Integer)
                .expect("read back integer"),
            Variant::from_i16(-1234)
        );

        let single_ptr =
            register_variant_pointer(&Variant::from_f32(1.5)).expect("register single");
        let single_raw = lookup_pointer(single_ptr)
            .expect("lookup single pointer")
            .cast::<f32>();
        unsafe {
            *single_raw = 2.25;
        }
        assert_eq!(
            super::read_back_scalar_payload_variant(single_ptr, ScalarPointerKind::Single)
                .expect("read back single"),
            Variant::from_f32(2.25)
        );
        free_pins(&[long_ptr, int_ptr, single_ptr]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn string_variant_pointer_exposes_bstr_cell_not_payload() {
        let ptr = register_string_variant_pointer(&Variant::from_string(BStr::from("abc")))
            .expect("register string variant var");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr)
            .expect("lookup string variant var")
            .cast::<usize>();
        assert!(!raw.is_null());
        let payload = unsafe { *raw as *const u16 };
        assert!(!payload.is_null());
        let len = unsafe { SysStringLen(payload.cast_mut()) };
        assert_eq!(len, 3);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn string_var_pointer_readback_tracks_updated_bstr_cell() {
        let ptr = register_string_variant_pointer(&Variant::from_string(BStr::from("abc")))
            .expect("register string var");
        let raw = lookup_pointer(ptr)
            .expect("lookup string var")
            .cast::<BSTR>();
        assert!(!raw.is_null());
        let old_payload = unsafe { *raw };
        if !old_payload.is_null() {
            unsafe { SysFreeString(old_payload) };
        }
        let replacement = BStr::from("alpha").owned_core();
        let replacement_len = u32::try_from(replacement.len_code_units()).expect("u32 length");
        unsafe {
            *raw = SysAllocStringLen(replacement.payload_ptr(), replacement_len);
        }
        let variant =
            super::read_back_string_payload_variant(ptr).expect("read back updated string var");
        assert_eq!(variant, Variant::from_string(BStr::from("alpha")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn string_var_pointer_preserves_embedded_nul_bstr_payload() {
        let original = Variant::from_string(BStr::from("abc"));
        let ptr = register_string_variant_pointer(&original).expect("register string var");
        let raw = lookup_pointer(ptr)
            .expect("lookup string var")
            .cast::<BSTR>();
        assert!(!raw.is_null());
        let old_payload = unsafe { *raw };
        if !old_payload.is_null() {
            unsafe { SysFreeString(old_payload) };
        }
        let replacement = BStr::from("a\0bc").owned_core();
        let replacement_len = u32::try_from(replacement.len_code_units()).expect("u32 length");
        unsafe {
            *raw = SysAllocStringLen(replacement.payload_ptr(), replacement_len);
        }
        let payload = unsafe { *raw };
        let len = unsafe { SysStringLen(payload) };
        assert_eq!(len, 4);
        let variant =
            super::read_back_string_payload_variant(ptr).expect("read back embedded nul string");
        assert_eq!(variant, Variant::from_string(BStr::from("a\0bc")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn string_var_pointer_readback_preserves_odd_byte_bstr_payload() {
        let original = Variant::from_string(BStr::from("abc"));
        let ptr = register_string_variant_pointer(&original).expect("register string var");
        let raw = lookup_pointer(ptr)
            .expect("lookup string var")
            .cast::<BSTR>();
        assert!(!raw.is_null());
        let old_payload = unsafe { *raw };
        if !old_payload.is_null() {
            unsafe { SysFreeString(old_payload) };
        }
        let bytes = [0x00, 0x43, 0x00];
        unsafe {
            *raw = SysAllocStringByteLen(bytes.as_ptr(), bytes.len() as u32);
        }
        let variant =
            super::read_back_string_payload_variant(ptr).expect("read back odd-byte string");
        assert_eq!(variant.string_byte_len(), Some(3));
        assert_eq!(variant.string_bytes(), Some(bytes.to_vec()));
        assert_eq!(variant.string_units(), Some(vec![0x4300]));
    }

    #[test]
    fn byte_buffer_pointer_readback_exposes_variant_array_payload() {
        let ptr = super::register_byte_buffer(vec![1, 2, 3]).expect("register byte buffer");
        let variant =
            super::read_back_byte_array_payload_variant(ptr).expect("read back byte buffer");
        let elements = variant
            .as_safearray()
            .expect("variant should carry SAFEARRAY")
            .variant_elements()
            .expect("SAFEARRAY should expose variant elements");
        assert_eq!(
            elements,
            vec![
                Variant::from_u8(1),
                Variant::from_u8(2),
                Variant::from_u8(3)
            ]
        );
        assert_eq!(
            Variant::from_safearray(crate::safe_array::SafeArray::from_variants(elements)),
            variant
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn variant_var_pointer_materializes_variant_container() {
        let ptr = register_variant_var_variant_pointer(&Variant::from_string(BStr::from("abc")))
            .expect("register variant");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr)
            .expect("lookup variant")
            .cast::<VARIANT>();
        assert!(!raw.is_null());
        let variant = unsafe { &*raw };
        assert_eq!(unsafe { variant.Anonymous.Anonymous.vt }, VT_BSTR);
        let payload = unsafe { variant.Anonymous.Anonymous.Anonymous.bstrVal };
        let len = unsafe { SysStringLen(payload) };
        assert_eq!(len, 3);

        let int_ptr = register_variant_var_variant_pointer(&Variant::from_i32(42))
            .expect("register i32 variant");
        let int_raw = lookup_pointer(int_ptr)
            .expect("lookup i32 variant")
            .cast::<VARIANT>();
        let int_variant = unsafe { &*int_raw };
        assert_eq!(unsafe { int_variant.Anonymous.Anonymous.vt }, VT_I4);
        assert_eq!(
            unsafe { int_variant.Anonymous.Anonymous.Anonymous.lVal },
            42
        );

        let date_ptr = register_variant_var_variant_pointer(&Variant::from_date_f64(42.25))
            .expect("register date variant");
        let date_raw = lookup_pointer(date_ptr)
            .expect("lookup date variant")
            .cast::<VARIANT>();
        let date_variant = unsafe { &*date_raw };
        assert_eq!(unsafe { date_variant.Anonymous.Anonymous.vt }, VT_DATE);
        assert_eq!(
            unsafe { date_variant.Anonymous.Anonymous.Anonymous.dblVal },
            42.25
        );

        let decimal_ptr = register_variant_var_variant_pointer(&Variant::from_decimal96(
            Decimal96::from_parts(123_450, 0, 0, 3, false),
        ))
        .expect("register decimal variant");
        let decimal_raw = lookup_pointer(decimal_ptr)
            .expect("lookup decimal variant")
            .cast::<u8>();
        let bytes = unsafe { std::slice::from_raw_parts(decimal_raw, 16) };
        let mut wire = [0u8; 16];
        wire.copy_from_slice(bytes);
        let decimal_variant = Variant::from_wire_bytes(wire).expect("decimal compat-slot wire");
        assert_eq!(decimal_variant.vtype(), VarType::Decimal);
        assert_eq!(
            decimal_variant.as_decimal96(),
            Some(Decimal96::from_parts(123_450, 0, 0, 3, false))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn variant_var_pointer_accepts_canonical_variant_input() {
        let ptr = register_variant_var_variant_pointer(&Variant::from_string(BStr::from("abc")))
            .expect("register canonical variant");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr)
            .expect("lookup canonical variant")
            .cast::<VARIANT>();
        assert!(!raw.is_null());
        let variant = unsafe { &*raw };
        assert_eq!(unsafe { variant.Anonymous.Anonymous.vt }, VT_BSTR);
        let payload = unsafe { variant.Anonymous.Anonymous.Anonymous.bstrVal };
        let len = unsafe { SysStringLen(payload) };
        assert_eq!(len, 3);
    }

    #[test]
    fn object_pointer_requires_object_like_value() {
        assert_eq!(
            register_object_variant_pointer(&Variant::from_object_ref(
                ObjectRef::from_compat_identity(0)
            ))
            .expect("nothing"),
            0
        );
        assert!(register_object_variant_pointer(&Variant::from_i32(5)).is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn object_pointer_returns_raw_iunknown_address() {
        let object = ObjectRef::from_compat_identity(42);
        let pointer = register_object_variant_pointer(&Variant::from_object_ref(object.clone()))
            .expect("object pointer");
        assert_eq!(pointer, object.raw_iunknown() as usize as i64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn variant_var_pointer_supports_object_payload() {
        let object = ObjectRef::from_compat_identity(42);
        let ptr = register_variant_var_variant_pointer(&Variant::from_object_ref(object.clone()))
            .expect("register object-valued variant");
        let raw = lookup_pointer(ptr)
            .expect("lookup variant var")
            .cast::<VARIANT>();
        assert!(!raw.is_null());
        let variant = unsafe { &*raw };
        assert_eq!(unsafe { variant.Anonymous.Anonymous.vt }, VT_UNKNOWN);
        assert_eq!(
            unsafe { variant.Anonymous.Anonymous.Anonymous.punkVal },
            object.raw_iunknown().cast()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn variant_var_pointer_supports_array_payload() {
        let ptr = register_variant_var_variant_pointer(&Variant::from_safearray(
            crate::safe_array::SafeArray::from_variants(vec![
                Variant::from_i32(4),
                Variant::from_string(BStr::from("abc")),
            ]),
        ))
        .expect("register array-valued variant");
        let raw = lookup_pointer(ptr)
            .expect("lookup variant var")
            .cast::<VARIANT>();
        assert!(!raw.is_null());
        let variant = unsafe { &*raw };
        assert_eq!(
            unsafe { variant.Anonymous.Anonymous.vt },
            VT_ARRAY | VT_VARIANT
        );
        let psa = unsafe { variant.Anonymous.Anonymous.Anonymous.parray };
        assert!(!psa.is_null());
        assert_eq!(unsafe { SafeArrayGetDim(psa) }, 1);

        let mut first: VARIANT = unsafe { std::mem::zeroed() };
        let index = 0i32;
        let hr = unsafe {
            SafeArrayGetElement(
                psa.cast_const(),
                &index,
                (&mut first as *mut VARIANT).cast(),
            )
        };
        assert!(hr >= 0);
        assert_eq!(unsafe { first.Anonymous.Anonymous.vt }, VT_I4);
        assert_eq!(unsafe { first.Anonymous.Anonymous.Anonymous.lVal }, 4);
        unsafe {
            VariantClear(&mut first);
        }
    }
}
