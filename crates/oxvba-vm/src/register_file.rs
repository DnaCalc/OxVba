use oxvba_runtime::{BindingHandle, RuntimeValue, VarType, Variant};

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
    pub fn from_runtime_value(value: RuntimeValue) -> Result<Self, String> {
        match value {
            RuntimeValue::BindingHandle(handle) => Ok(Self::BindingHandle(handle)),
            value => Variant::try_from_runtime_value(&value).map(Self::Variant),
        }
    }

    pub fn from_compat_slot_i32(value: i32) -> Result<Self, String> {
        Variant::try_from_compat_slot_i32(value).map(Self::Variant)
    }

    pub fn to_runtime_value(&self) -> Result<RuntimeValue, String> {
        match self {
            Self::Variant(value) => value.to_runtime_value(),
            Self::BindingHandle(handle) => Ok(RuntimeValue::BindingHandle(*handle)),
        }
    }

    pub fn project_compat_slot_i32(&self) -> Result<i32, String> {
        match self {
            Self::Variant(value) => value.project_compat_slot_i32(),
            Self::BindingHandle(handle) => Ok(handle.raw()),
        }
    }

    pub fn as_i32_lossy(&self) -> Option<i32> {
        self.project_compat_slot_i32().ok()
    }

    pub fn is_null(&self) -> bool {
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
