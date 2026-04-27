//! Explicit COM compatibility adapters.
//!
//! COM transport should prefer retained `Variant` and `ComValue` carriers.
//! This module contains legacy `RuntimeValue` and slot-token projections.

use oxvba_runtime::{RuntimeValue, Variant};

use crate::ComValue;

pub fn com_value_from_runtime_value(value: &RuntimeValue) -> ComValue {
    match value {
        RuntimeValue::BindingHandle(handle) => ComValue::I32(handle.raw()),
        value => {
            let variant = Variant::try_from_runtime_value(value)
                .expect("legacy RuntimeValue must project to retained Variant");
            ComValue::from_variant(&variant)
                .expect("legacy RuntimeValue Variant must project to ComValue")
        }
    }
}

pub fn com_value_from_runtime_token(value: i32) -> ComValue {
    com_value_from_runtime_value(&RuntimeValue::from_compat_slot_i32(value))
}

pub fn com_value_to_runtime_value(value: &ComValue) -> RuntimeValue {
    value
        .to_variant()
        .and_then(|value| value.to_runtime_value())
        .expect("COM value must project to legacy RuntimeValue")
}

pub fn com_value_to_runtime_token(value: &ComValue) -> Result<i32, String> {
    value.to_variant()?.project_compat_slot_i32()
}
