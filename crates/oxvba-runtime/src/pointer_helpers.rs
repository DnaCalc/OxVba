use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use crate::{RuntimeValue, bstr::BStr};

#[derive(Debug)]
enum PointerEntry {
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
    Object(i32),
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
}

fn registry() -> &'static Mutex<PointerRegistry> {
    static REGISTRY: OnceLock<Mutex<PointerRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(PointerRegistry::default()))
}

pub fn register_utf16_string(text: &str) -> Result<i64, String> {
    let data: Box<[u16]> = text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(PointerEntry::Utf16(data)))
}

pub fn register_runtime_value_pointer(value: &RuntimeValue) -> Result<i64, String> {
    let entry = match value {
        RuntimeValue::Empty | RuntimeValue::Null => return Ok(0),
        RuntimeValue::I32(value) => PointerEntry::I32(Box::new(*value)),
        RuntimeValue::I64(value) => PointerEntry::I64(Box::new(*value)),
        RuntimeValue::F64(value) => PointerEntry::F64(Box::new(value.as_f64())),
        RuntimeValue::Currency(value) => PointerEntry::I64(Box::new(value.scaled_i64())),
        RuntimeValue::Bool(value) => PointerEntry::Bool(Box::new(if *value { -1 } else { 0 })),
        RuntimeValue::String(BStr(value)) => {
            let data: Box<[u16]> = value
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            PointerEntry::Utf16(data)
        }
        RuntimeValue::ArrayIntent(array) => {
            let Some(elements) = &array.elements else {
                return Err("VarPtr over array shape without element payload is not yet supported".to_string());
            };
            let mut bytes = Vec::with_capacity(elements.len());
            for element in elements {
                match element {
                    RuntimeValue::Empty | RuntimeValue::Null => bytes.push(0),
                    RuntimeValue::I32(value) if (0..=255).contains(value) => bytes.push(*value as u8),
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
        RuntimeValue::ObjectHandle(handle) => {
            PointerEntry::ObjectIdentity(Box::new(i64::from(handle.raw())))
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

pub fn register_object_pointer(value: &RuntimeValue) -> Result<i64, String> {
    match value {
        RuntimeValue::Empty | RuntimeValue::Null => Ok(0),
        RuntimeValue::ObjectHandle(handle) if handle.raw() == 0 => Ok(0),
        RuntimeValue::ObjectHandle(handle) => {
            let mut guard = registry()
                .lock()
                .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
            Ok(guard.insert_object_identity(
                ObjectIdentityKey::Object(handle.raw()),
                i64::from(handle.raw()),
            ))
        }
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

pub fn register_byte_buffer(bytes: Vec<u8>) -> Result<i64, String> {
    let mut guard = registry()
        .lock()
        .map_err(|_| "pointer helper registry lock poisoned".to_string())?;
    Ok(guard.insert(PointerEntry::Bytes(bytes.into_boxed_slice())))
}

#[cfg(test)]
mod tests {
    use super::{lookup_pointer, register_object_pointer, register_runtime_value_pointer, register_utf16_string};
    use crate::{BindingHandle, ObjectHandle, RuntimeValue, bstr::BStr};

    #[test]
    fn utf16_pointer_helper_allocates_terminated_text() {
        let ptr = register_utf16_string("abc").expect("register string");
        assert_ne!(ptr, 0);
        let raw = lookup_pointer(ptr).expect("lookup pointer") as *const u16;
        assert!(!raw.is_null());
        let slice = unsafe { std::slice::from_raw_parts(raw, 4) };
        assert_eq!(slice, &[97, 98, 99, 0]);
    }

    #[test]
    fn runtime_value_pointer_handles_scalars_and_strings() {
        let string_ptr = register_runtime_value_pointer(&RuntimeValue::String(BStr("xyz".to_string())))
            .expect("register string runtime value");
        assert_ne!(string_ptr, 0);
        let scalar_ptr = register_runtime_value_pointer(&RuntimeValue::I64(42)).expect("register i64");
        assert_ne!(scalar_ptr, 0);
    }

    #[test]
    fn object_pointer_requires_object_like_value() {
        assert_eq!(
            register_object_pointer(&RuntimeValue::ObjectHandle(ObjectHandle::new(0)))
                .expect("nothing"),
            0
        );
        assert!(register_object_pointer(&RuntimeValue::I32(5)).is_err());
    }

    #[test]
    fn object_pointer_is_stable_for_same_runtime_identity() {
        let object_ptr = register_object_pointer(&RuntimeValue::ObjectHandle(ObjectHandle::new(42)))
            .expect("object identity");
        let same_object_ptr =
            register_object_pointer(&RuntimeValue::ObjectHandle(ObjectHandle::new(42)))
                .expect("same object identity");
        let binding_ptr =
            register_object_pointer(&RuntimeValue::BindingHandle(BindingHandle::new(42)))
                .expect("binding identity");
        assert_eq!(object_ptr, same_object_ptr);
        assert_ne!(object_ptr, binding_ptr);
    }
}
