#[cfg(test)]
use oxvba_runtime::RuntimeValue;
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
    /// Build a retained VM slot from a legacy semantic value.
    ///
    /// `RuntimeValue` is accepted only as a compatibility ingress surface. The
    /// slot stores VBA/COM values as `Variant`; `BindingHandle` remains a
    /// separate internal side-lane because it is not a VBA value.
    #[cfg(test)]
    pub(crate) fn from_runtime_value(value: RuntimeValue) -> Result<Self, String> {
        match value {
            RuntimeValue::BindingHandle(handle) => Ok(Self::BindingHandle(handle)),
            value => Variant::try_from_runtime_value(&value).map(Self::Variant),
        }
    }

    /// Build a retained VM slot from the legacy 4-byte compatibility token.
    ///
    /// This preserves historical slot-token meanings while materializing the
    /// value as a `Variant` carrier.
    pub(crate) fn from_compat_slot_i32(value: i32) -> Result<Self, String> {
        Variant::try_from_compat_slot_i32(value).map(Self::Variant)
    }

    /// Project a retained slot back to the legacy semantic value API.
    ///
    /// New value-model call sites should read the `Variant` slot directly.
    #[cfg(test)]
    pub(crate) fn to_runtime_value(&self) -> Result<RuntimeValue, String> {
        match self {
            Self::Variant(value) => value.to_runtime_value(),
            Self::BindingHandle(handle) => Ok(RuntimeValue::BindingHandle(*handle)),
        }
    }

    pub(crate) fn variant_cell_pointer(&self) -> Result<i64, String> {
        match self {
            Self::Variant(value) => Ok(value.as_variant_cell_ptr() as usize as i64),
            Self::BindingHandle(handle) => Err(format!(
                "VarPtr(Variant) cannot expose internal binding handle {} as a VARIANT cell",
                handle.raw()
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_variant_cell_pointer(&self, pointer: i64) -> bool {
        matches!(self, Self::Variant(value) if value.as_variant_cell_ptr() as usize as i64 == pointer)
    }

    /// Project a retained slot to the legacy 4-byte JIT/interpreter slot token.
    pub(crate) fn project_compat_slot_i32(&self) -> Result<i32, String> {
        match self {
            Self::Variant(value) => value.project_compat_slot_i32(),
            Self::BindingHandle(handle) => Ok(handle.raw()),
        }
    }

    pub(crate) fn as_i32_lossy(&self) -> Option<i32> {
        self.project_compat_slot_i32().ok()
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
