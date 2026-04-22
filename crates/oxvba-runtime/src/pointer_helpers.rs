use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use crate::{RuntimeValue, bstr::BStr};

#[cfg(target_os = "windows")]
use windows_sys::{
    Win32::Foundation::{SysAllocString, SysFreeString},
    Win32::System::Com::SAFEARRAYBOUND,
    Win32::System::Ole::{
        SafeArrayCreate, SafeArrayCreateVector, SafeArrayDestroy, SafeArrayPutElement,
    },
    Win32::System::Variant::{
        VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_CY, VT_DATE, VT_EMPTY, VT_ERROR, VT_I4, VT_I8,
        VT_NULL, VT_UNKNOWN, VT_VARIANT, VariantClear,
    },
    core::BSTR,
};

#[cfg(target_os = "windows")]
const VT_R4: u16 = 4;
#[cfg(target_os = "windows")]
const VT_R8: u16 = 5;

#[cfg(target_os = "windows")]
#[derive(Debug)]
// Pointer helpers expose Windows-observable cells even though the current
// canonical runtime string carrier is still semantic-first `BStr`, not a raw
// process-wide BSTR allocation.
struct OwnedBstr(BSTR);

#[cfg(target_os = "windows")]
impl OwnedBstr {
    fn from_bstr(text: &BStr) -> Result<Self, String> {
        let core = text.owned_core();
        let bstr = unsafe { SysAllocString(core.payload_ptr()) };
        if bstr.is_null() {
            return Err("failed to allocate BSTR backing storage for pointer helper".to_string());
        }
        Ok(Self(bstr))
    }

    fn from_text(text: &str) -> Result<Self, String> {
        Self::from_bstr(&BStr::from(text))
    }

    fn as_ptr(&self) -> *mut c_void {
        self.0.cast_mut().cast()
    }

    fn into_raw(self) -> BSTR {
        let raw = self.0;
        std::mem::forget(self);
        raw
    }

    fn to_runtime_value(&self) -> RuntimeValue {
        let len = unsafe { windows_sys::Win32::Foundation::SysStringLen(self.0) } as usize;
        let slice = unsafe { std::slice::from_raw_parts(self.0, len) };
        RuntimeValue::String(BStr::from_utf16_lossy(slice))
    }
}

#[cfg(target_os = "windows")]
unsafe impl Send for OwnedBstr {}

#[cfg(target_os = "windows")]
impl Drop for OwnedBstr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { SysFreeString(self.0) };
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
// `VarPtr(String)` exposes a pointer to a BSTR cell. The helper therefore owns
// the cell itself and whichever BSTR pointer a native call leaves in that cell.
struct OwnedBstrCell {
    cell: Box<BSTR>,
}

#[cfg(target_os = "windows")]
impl OwnedBstrCell {
    fn from_bstr(text: &BStr) -> Result<Self, String> {
        let cell = Box::new(OwnedBstr::from_bstr(text)?.into_raw());
        Ok(Self { cell })
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&mut *self.cell as *mut BSTR).cast()
    }

    fn to_runtime_value(&self) -> RuntimeValue {
        if (*self.cell).is_null() {
            return RuntimeValue::String(BStr::empty());
        }
        let len = unsafe { windows_sys::Win32::Foundation::SysStringLen(*self.cell) } as usize;
        let slice = unsafe { std::slice::from_raw_parts(*self.cell, len) };
        RuntimeValue::String(BStr::from_utf16_lossy(slice))
    }
}

#[cfg(target_os = "windows")]
impl Drop for OwnedBstrCell {
    fn drop(&mut self) {
        if !(*self.cell).is_null() {
            unsafe { SysFreeString(*self.cell) };
            *self.cell = std::ptr::null_mut();
        }
    }
}

#[cfg(target_os = "windows")]
unsafe impl Send for OwnedBstrCell {}

#[cfg(target_os = "windows")]
// `VarPtr(Variant)` materializes a Windows-observable VARIANT cell from the
// canonical semantic Variant carrier; the raw VARIANT is still a boundary
// projection rather than the canonical runtime container itself.
struct OwnedVariant(VARIANT);

#[cfg(target_os = "windows")]
impl std::fmt::Debug for OwnedVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedVariant").finish_non_exhaustive()
    }
}

#[cfg(target_os = "windows")]
impl OwnedVariant {
    fn from_runtime_value(value: &RuntimeValue) -> Result<Self, String> {
        let canonical = crate::Variant::from_runtime_value(value);
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe { set_windows_variant_from_runtime_value(&mut variant, value, &canonical)? };
        Ok(Self(variant))
    }

    fn as_ptr(&mut self) -> *mut c_void {
        (&mut self.0 as *mut VARIANT).cast()
    }
}

#[cfg(target_os = "windows")]
unsafe impl Send for OwnedVariant {}

#[cfg(target_os = "windows")]
fn retained_iunknown_pointer(object: &crate::ObjectRef) -> *mut c_void {
    let retained = object.query_iunknown();
    let raw = retained.raw_iunknown().cast::<c_void>();
    std::mem::forget(retained);
    raw
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_windows_variant_array_arg(
    variant: *mut VARIANT,
    array: &crate::safe_array::SafeArray,
) -> Result<(), String> {
    let Some(values) = array.elements.as_ref() else {
        return Err("VarPtr over Variant containing an array shape without element payload is not yet supported".to_string());
    };

    if let Some(bounds) = array.bounds.as_ref()
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
        for runtime_value in values {
            let mut element: VARIANT = std::mem::zeroed();
            if let Err(detail) = set_windows_variant_from_runtime_value(
                &mut element,
                runtime_value,
                &crate::Variant::from_runtime_value(runtime_value),
            ) {
                let _ = VariantClear(&mut element);
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(detail);
            }
            let hr = SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&element as *const VARIANT).cast(),
            );
            let _ = VariantClear(&mut element);
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
    for (offset, runtime_value) in values.iter().enumerate() {
        let mut element: VARIANT = std::mem::zeroed();
        if let Err(detail) = set_windows_variant_from_runtime_value(
            &mut element,
            runtime_value,
            &crate::Variant::from_runtime_value(runtime_value),
        ) {
            let _ = VariantClear(&mut element);
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(detail);
        }
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(
            psa.cast_const(),
            &index,
            (&element as *const VARIANT).cast(),
        );
        let _ = VariantClear(&mut element);
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
unsafe fn set_windows_variant_from_runtime_value(
    variant: *mut VARIANT,
    value: &RuntimeValue,
    canonical: &crate::Variant,
) -> Result<(), String> {
    match value {
        RuntimeValue::ArrayIntent(array) => {
            set_windows_variant_array_arg(variant, array)?;
        }
        RuntimeValue::Object(object) => {
            (*variant).Anonymous.Anonymous.vt = VT_UNKNOWN;
            (*variant).Anonymous.Anonymous.Anonymous.punkVal = if object.raw() == 0 {
                std::ptr::null_mut()
            } else {
                retained_iunknown_pointer(object)
            };
        }
        RuntimeValue::BindingHandle(_) => {
            return Err(
                "VarPtr over Variant containing a binding handle is not yet supported".to_string(),
            );
        }
        _ => match canonical.vtype() {
            crate::VarType::Empty => {
                (*variant).Anonymous.Anonymous.vt = VT_EMPTY;
            }
            crate::VarType::Null => {
                (*variant).Anonymous.Anonymous.vt = VT_NULL;
            }
            crate::VarType::Error => {
                (*variant).Anonymous.Anonymous.vt = VT_ERROR;
                (*variant).Anonymous.Anonymous.Anonymous.scode =
                    canonical.as_error_code().expect("error payload");
            }
            crate::VarType::Integer | crate::VarType::Long => {
                (*variant).Anonymous.Anonymous.vt = VT_I4;
                (*variant).Anonymous.Anonymous.Anonymous.lVal = canonical
                    .to_runtime_value()?
                    .as_i32_lossy()
                    .expect("integer payload should project into i32");
            }
            crate::VarType::LongLong => {
                (*variant).Anonymous.Anonymous.vt = VT_I8;
                (*variant).Anonymous.Anonymous.Anonymous.llVal =
                    canonical.as_i64().expect("i64 payload");
            }
            crate::VarType::Boolean => {
                (*variant).Anonymous.Anonymous.vt = VT_BOOL;
                (*variant).Anonymous.Anonymous.Anonymous.boolVal =
                    if canonical.as_bool().expect("bool payload") {
                        -1
                    } else {
                        0
                    };
            }
            crate::VarType::Single | crate::VarType::Double | crate::VarType::Date => {
                let raw = match canonical.to_runtime_value()? {
                    RuntimeValue::F64(value) => value,
                    other => {
                        return Err(format!(
                            "floating canonical Variant should bridge back as RuntimeValue::F64, got {other:?}"
                        ));
                    }
                };
                match raw.subtype() {
                    crate::F64Subtype::Single => {
                        (*variant).Anonymous.Anonymous.vt = VT_R4;
                        (*variant).Anonymous.Anonymous.Anonymous.fltVal = raw.as_f64() as f32;
                    }
                    crate::F64Subtype::Double => {
                        (*variant).Anonymous.Anonymous.vt = VT_R8;
                        (*variant).Anonymous.Anonymous.Anonymous.dblVal = raw.as_f64();
                    }
                    crate::F64Subtype::Date => {
                        (*variant).Anonymous.Anonymous.vt = VT_DATE;
                        (*variant).Anonymous.Anonymous.Anonymous.dblVal = raw.as_f64();
                    }
                }
            }
            crate::VarType::Currency => {
                (*variant).Anonymous.Anonymous.vt = VT_CY;
                (*variant).Anonymous.Anonymous.Anonymous.cyVal.int64 = canonical
                    .as_currency_scaled_i64()
                    .expect("currency payload");
            }
            crate::VarType::String => {
                (*variant).Anonymous.Anonymous.vt = VT_BSTR;
                let text = canonical.as_bstr().ok_or_else(|| {
                    "canonical string Variant lost owned BSTR payload".to_string()
                })?;
                (*variant).Anonymous.Anonymous.Anonymous.bstrVal =
                    OwnedBstr::from_bstr(text)?.into_raw();
            }
            crate::VarType::Decimal => {
                let bytes = canonical.to_wire_bytes();
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), variant.cast::<u8>(), bytes.len());
            }
            crate::VarType::Object => {
                return Err(
                    "VarPtr over Variant containing unsupported object carrier is not yet supported"
                        .to_string(),
                );
            }
            other => {
                return Err(format!(
                    "VarPtr over Variant containing unsupported canonical type {:?} is not yet supported",
                    other
                ));
            }
        },
    }
    Ok(())
}

#[cfg(target_os = "windows")]
impl Drop for OwnedVariant {
    fn drop(&mut self) {
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
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
    I32(Box<i32>),
    I64(Box<i64>),
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
            Self::I32(value) => (&mut **value as *mut i32).cast(),
            Self::I64(value) => (&mut **value as *mut i64).cast(),
            Self::F64(value) => (&mut **value as *mut f64).cast(),
            Self::Bool(value) => (&mut **value as *mut i16).cast(),
            Self::ObjectIdentity(value) => (&mut **value as *mut i64).cast(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ObjectIdentityKey {
    Binding(i32),
}

#[derive(Debug, Default)]
struct PointerRegistry {
    entries: HashMap<usize, PointerEntry>,
    object_identities: HashMap<ObjectIdentityKey, usize>,
}

impl PointerRegistry {
    fn insert(&mut self, mut entry: PointerEntry) -> i64 {
        let addr = entry.as_ptr() as usize;
        self.entries.insert(addr, entry);
        addr as i64
    }

    fn insert_object_identity(&mut self, key: ObjectIdentityKey, raw: i64) -> i64 {
        if let Some(existing) = self.object_identities.get(&key) {
            return *existing as i64;
        }
        let pointer = self.insert(PointerEntry::ObjectIdentity(Box::new(raw)));
        self.object_identities.insert(key, pointer as usize);
        pointer
    }

    fn read_back_string_payload(&self, pointer: i64) -> Result<RuntimeValue, String> {
        if pointer == 0 {
            return Ok(RuntimeValue::String(BStr::empty()));
        }
        let Some(entry) = self.entries.get(&(pointer as usize)) else {
            return Err(format!(
                "pointer helper registry does not contain string payload pointer {pointer}"
            ));
        };
        match entry {
            #[cfg(target_os = "windows")]
            PointerEntry::Bstr(value) => Ok(value.to_runtime_value()),
            #[cfg(target_os = "windows")]
            PointerEntry::BstrCell(value) => Ok(value.to_runtime_value()),
            #[cfg(not(target_os = "windows"))]
            PointerEntry::Utf16(value) => {
                let end = value
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(value.len());
                Ok(RuntimeValue::String(BStr::from_utf16_lossy(&value[..end])))
            }
            other => Err(format!(
                "pointer helper entry {other:?} cannot be read back as a string payload"
            )),
        }
    }

    fn read_back_byte_array_payload(&self, pointer: i64) -> Result<RuntimeValue, String> {
        if pointer == 0 {
            return Ok(RuntimeValue::ArrayIntent(
                crate::safe_array::SafeArray::from_values(Vec::new()),
            ));
        }
        let Some(entry) = self.entries.get(&(pointer as usize)) else {
            return Err(format!(
                "pointer helper registry does not contain byte-array payload pointer {pointer}"
            ));
        };
        match entry {
            PointerEntry::Bytes(bytes) => Ok(RuntimeValue::ArrayIntent(
                crate::safe_array::SafeArray::from_values(
                    bytes
                        .iter()
                        .map(|byte| RuntimeValue::I32(i32::from(*byte)))
                        .collect(),
                ),
            )),
            other => Err(format!(
                "pointer helper entry {other:?} cannot be read back as a byte-array payload"
            )),
        }
    }
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

pub fn register_runtime_value_pointer(value: &RuntimeValue) -> Result<i64, String> {
    let entry = match value {
        RuntimeValue::Empty | RuntimeValue::Null => return Ok(0),
        RuntimeValue::I32(value) => PointerEntry::I32(Box::new(*value)),
        RuntimeValue::I64(value) => PointerEntry::I64(Box::new(*value)),
        RuntimeValue::F64(value) => PointerEntry::F64(Box::new(value.as_f64())),
        RuntimeValue::Currency(value) => PointerEntry::I64(Box::new(value.scaled_i64())),
        RuntimeValue::Bool(value) => PointerEntry::Bool(Box::new(if *value { -1 } else { 0 })),
        RuntimeValue::String(value) => {
            #[cfg(target_os = "windows")]
            {
                PointerEntry::Bstr(OwnedBstr::from_bstr(value)?)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let core = value.owned_core();
                let data: Box<[u16]> = core.payload_units_with_nul().to_vec().into_boxed_slice();
                PointerEntry::Utf16(data)
            }
        }
        RuntimeValue::ArrayIntent(array) => {
            let Some(elements) = &array.elements else {
                return Err(
                    "VarPtr over array shape without element payload is not yet supported"
                        .to_string(),
                );
            };
            let mut bytes = Vec::with_capacity(elements.len());
            for element in elements {
                match element {
                    RuntimeValue::Empty | RuntimeValue::Null => bytes.push(0),
                    RuntimeValue::I32(value) if (0..=255).contains(value) => {
                        bytes.push(*value as u8)
                    }
                    RuntimeValue::Bool(value) => bytes.push(if *value { 1 } else { 0 }),
                    other => {
                        return Err(format!(
                            "VarPtr over array payload currently requires byte-compatible elements, got {other:?}"
                        ));
                    }
                }
            }
            PointerEntry::Bytes(bytes.into_boxed_slice())
        }
        RuntimeValue::Object(handle) => {
            PointerEntry::ObjectIdentity(Box::new(handle.raw_iunknown() as usize as i64))
        }
        RuntimeValue::BindingHandle(handle) => {
            PointerEntry::ObjectIdentity(Box::new(i64::from(handle.raw())))
        }
        RuntimeValue::ErrorCode(value) => PointerEntry::I32(Box::new(*value)),
        RuntimeValue::Decimal(_) => {
            return Err("VarPtr/ObjPtr over Decimal is not yet supported".to_string());
        }
    };

    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(entry))
}

pub fn register_string_var_pointer(value: &RuntimeValue) -> Result<i64, String> {
    #[cfg(target_os = "windows")]
    {
        let entry = match value {
            RuntimeValue::String(text) => PointerEntry::BstrCell(OwnedBstrCell::from_bstr(text)?),
            RuntimeValue::Empty | RuntimeValue::Null => return Ok(0),
            _ => return Err("VarPtr over String requires a string variable".to_string()),
        };

        let mut guard = registry()
            .lock()
            .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
        return Ok(guard.insert(entry));
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_runtime_value_pointer(value)
    }
}

pub fn register_variant_var_pointer(value: &RuntimeValue) -> Result<i64, String> {
    #[cfg(target_os = "windows")]
    {
        let entry = PointerEntry::VariantCell(OwnedVariant::from_runtime_value(value)?);
        let mut guard = registry()
            .lock()
            .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
        return Ok(guard.insert(entry));
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_runtime_value_pointer(value)
    }
}

pub fn register_object_pointer(value: &RuntimeValue) -> Result<i64, String> {
    match value {
        RuntimeValue::Empty | RuntimeValue::Null => Ok(0),
        RuntimeValue::Object(handle) if handle.raw() == 0 => Ok(0),
        RuntimeValue::Object(handle) => Ok(handle.raw_iunknown() as usize as i64),
        RuntimeValue::BindingHandle(handle) => {
            let mut guard = registry()
                .lock()
                .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
            Ok(guard.insert_object_identity(
                ObjectIdentityKey::Binding(handle.raw()),
                i64::from(handle.raw()),
            ))
        }
        _ => Err("ObjPtr requires an object reference".to_string()),
    }
}

pub fn lookup_pointer(pointer: i64) -> Option<*mut c_void> {
    if pointer == 0 {
        return Some(std::ptr::null_mut());
    }
    let mut guard = registry().lock().ok()?;
    let entry = guard.entries.get_mut(&(pointer as usize))?;
    Some(entry.as_ptr())
}

pub fn read_back_string_payload(pointer: i64) -> Result<RuntimeValue, String> {
    let guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    guard.read_back_string_payload(pointer)
}

pub fn read_back_byte_array_payload(pointer: i64) -> Result<RuntimeValue, String> {
    let guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    guard.read_back_byte_array_payload(pointer)
}

pub fn register_byte_buffer(bytes: Vec<u8>) -> Result<i64, String> {
    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(PointerEntry::Bytes(bytes.into_boxed_slice())))
}

#[cfg(test)]
mod tests {
    use super::{
        lookup_pointer, register_object_pointer, register_runtime_value_pointer,
        register_string_var_pointer, register_utf16_string, register_variant_var_pointer,
    };
    use crate::{BindingHandle, Decimal96, ObjectRef, RuntimeValue, VarType, Variant, bstr::BStr};
    #[cfg(target_os = "windows")]
    use windows_sys::{
        Win32::Foundation::{SysAllocString, SysFreeString, SysStringLen},
        Win32::System::Ole::{SafeArrayGetDim, SafeArrayGetElement},
        Win32::System::Variant::{
            VARIANT, VT_ARRAY, VT_BSTR, VT_I4, VT_UNKNOWN, VT_VARIANT, VariantClear,
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
    fn runtime_value_pointer_handles_scalars_and_strings() {
        let string_ptr = register_runtime_value_pointer(&RuntimeValue::String(BStr::from("xyz")))
            .expect("register string runtime value");
        assert_ne!(string_ptr, 0);
        let scalar_ptr =
            register_runtime_value_pointer(&RuntimeValue::I64(42)).expect("register i64");
        assert_ne!(scalar_ptr, 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn string_var_pointer_exposes_bstr_cell_not_payload() {
        let ptr = register_string_var_pointer(&RuntimeValue::String(BStr::from("abc")))
            .expect("register string var");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr)
            .expect("lookup string var")
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
        let ptr = register_string_var_pointer(&RuntimeValue::String(BStr::from("abc")))
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
        unsafe {
            *raw = SysAllocString(replacement.payload_ptr());
        }
        let value = super::read_back_string_payload(ptr).expect("read back updated string var");
        assert_eq!(value, RuntimeValue::String(BStr::from("alpha")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn variant_var_pointer_materializes_variant_container() {
        let ptr = register_variant_var_pointer(&RuntimeValue::String(BStr::from("abc")))
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

        let int_ptr =
            register_variant_var_pointer(&RuntimeValue::I32(42)).expect("register i32 variant");
        let int_raw = lookup_pointer(int_ptr)
            .expect("lookup i32 variant")
            .cast::<VARIANT>();
        let int_variant = unsafe { &*int_raw };
        assert_eq!(unsafe { int_variant.Anonymous.Anonymous.vt }, VT_I4);
        assert_eq!(
            unsafe { int_variant.Anonymous.Anonymous.Anonymous.lVal },
            42
        );

        let decimal_ptr = register_variant_var_pointer(&RuntimeValue::Decimal(
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

    #[test]
    fn object_pointer_requires_object_like_value() {
        assert_eq!(
            register_object_pointer(&RuntimeValue::Object(ObjectRef::from_compat_identity(0)))
                .expect("nothing"),
            0
        );
        assert!(register_object_pointer(&RuntimeValue::I32(5)).is_err());
    }

    #[test]
    fn object_pointer_distinguishes_fresh_runtime_objects_from_binding_tokens() {
        let object_ptr =
            register_object_pointer(&RuntimeValue::Object(ObjectRef::from_compat_identity(42)))
                .expect("object identity");
        let same_object_ptr =
            register_object_pointer(&RuntimeValue::Object(ObjectRef::from_compat_identity(42)))
                .expect("same object identity");
        let binding_ptr =
            register_object_pointer(&RuntimeValue::BindingHandle(BindingHandle::new(42)))
                .expect("binding identity");
        assert_ne!(object_ptr, same_object_ptr);
        assert_ne!(object_ptr, binding_ptr);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn object_pointer_returns_raw_iunknown_address() {
        let object = ObjectRef::from_compat_identity(42);
        let pointer =
            register_object_pointer(&RuntimeValue::Object(object.clone())).expect("object pointer");
        assert_eq!(pointer, object.raw_iunknown() as usize as i64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn runtime_value_pointer_over_object_exposes_interface_pointer_cell() {
        let object = ObjectRef::from_compat_identity(42);
        let pointer = register_runtime_value_pointer(&RuntimeValue::Object(object.clone()))
            .expect("runtime value object pointer");
        let raw = lookup_pointer(pointer)
            .expect("lookup object cell")
            .cast::<i64>();
        assert!(!raw.is_null());
        assert_eq!(unsafe { *raw }, object.raw_iunknown() as usize as i64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn variant_var_pointer_supports_object_payload() {
        let object = ObjectRef::from_compat_identity(42);
        let ptr = register_variant_var_pointer(&RuntimeValue::Object(object.clone()))
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
        let ptr = register_variant_var_pointer(&RuntimeValue::ArrayIntent(
            crate::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(4),
                RuntimeValue::String(BStr::from("abc")),
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
