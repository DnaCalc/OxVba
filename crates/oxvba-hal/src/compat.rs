//! Explicit HAL compatibility adapters.
//!
//! HAL implementations should prefer retained `Variant` entry points. This
//! module contains the deliberate projections needed by legacy `RuntimeValue`
//! trait methods.

use oxvba_runtime::{RuntimeValue, Variant};

use crate::{
    error::{HalError, HalResult},
    model::{CapabilityId, HalProfileId},
};

pub fn runtime_value_to_variant(
    profile: HalProfileId,
    capability: CapabilityId,
    operation: &'static str,
    argument: &'static str,
    value: RuntimeValue,
) -> HalResult<Variant> {
    match value {
        RuntimeValue::BindingHandle(handle) => Ok(Variant::from_i32(handle.raw())),
        value => Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                profile,
                capability,
                operation,
                format!("failed to project {argument} RuntimeValue into Variant: {detail}"),
            )
        }),
    }
}

pub fn variant_to_runtime_value(
    profile: HalProfileId,
    capability: CapabilityId,
    operation: &'static str,
    value: Variant,
) -> HalResult<RuntimeValue> {
    value.to_runtime_value().map_err(|detail| {
        HalError::adapter_fault(
            profile,
            capability,
            operation,
            format!("failed to project retained Variant result into RuntimeValue: {detail}"),
        )
    })
}
