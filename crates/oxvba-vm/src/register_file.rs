use oxvba_runtime::{BindingHandle, VarType, Variant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSlot {
    Variant(Variant),
    BindingHandle(BindingHandle),
}

impl Default for RuntimeSlot {
    fn default() -> Self {
        Self::Variant(Variant::empty())
    }
}

impl RuntimeSlot {
    pub(crate) fn variant_cell_pointer(&self) -> Result<i64, String> {
        match self {
            Self::Variant(value) => Ok(value.as_variant_cell_ptr() as usize as i64),
            Self::BindingHandle(handle) => Err(format!(
                "VarPtr(Variant) cannot expose internal binding handle {} as a VARIANT cell",
                handle.raw()
            )),
        }
    }

    pub(crate) fn as_i32_lossy(&self) -> Option<i32> {
        match self {
            Self::Variant(value) => {
                if let Some(value) = value.as_i32() {
                    Some(value)
                } else if let Some(value) = value.as_i16() {
                    Some(i32::from(value))
                } else if let Some(value) = value.as_u8() {
                    Some(i32::from(value))
                } else if let Some(value) = value.as_bool() {
                    Some(i32::from(value))
                } else {
                    value.as_object_ref().map(|value| value.raw())
                }
            }
            Self::BindingHandle(handle) => Some(handle.raw()),
        }
    }

    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Variant(value) if value.vtype() == VarType::Null)
    }
}

#[derive(Debug, Default)]
pub struct RegisterFile {
    pub registers: Vec<RuntimeSlot>,
}

impl RegisterFile {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            registers: vec![RuntimeSlot::default(); capacity],
        }
    }
}
