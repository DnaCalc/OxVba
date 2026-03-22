use crate::{
    error::{HalError, HalResult},
    model::{CapabilityId, HalProfileId},
    traits::{DynLinkDescriptorView, DynamicLinkHal},
};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, RuntimeValue};
use std::collections::BTreeMap;

use super::StandardHostServices;

#[derive(Debug, Default)]
pub(super) struct DynLinkBindingState {
    next_binding: i32,
    pub(super) descriptors: BTreeMap<u32, BindingHandle>,
    pub(super) bindings: BTreeMap<BindingHandle, DynLinkSymbol>,
}

impl DynLinkBindingState {
    pub(super) fn allocate_binding(&mut self) -> BindingHandle {
        self.next_binding = self.next_binding.saturating_add(1).max(1);
        BindingHandle::new(80_000i32.saturating_add(self.next_binding))
    }
}

pub(super) fn external_symbol_token(library: &str, alias: &str, name: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in library
        .bytes()
        .chain([b'!'])
        .chain(alias.bytes())
        .chain([b'!'])
        .chain(name.bytes())
    {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash & 0x7fff_ffff).max(1) as i32
}

impl DynamicLinkHal for StandardHostServices {
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<BindingHandle> {
        let capability = CapabilityId::DynamicLinking;
        const LANE_M0: &str = "m0-deterministic";
        const LANE_M1: &str = "m1-native-ffi";
        const CONV_PLATFORM_DEFAULT: &str = "platform-default";
        const POLICY_CASE_INSENSITIVE: &str = "case-insensitive-canonical";
        const POLICY_ORDINAL_LITERAL: &str = "ordinal-literal-canonical";

        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol"));
        }

        if descriptor.marshal_lane != LANE_M0 && descriptor.marshal_lane != LANE_M1 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "unsupported marshaling lane `{}` for descriptor {}",
                    descriptor.marshal_lane, descriptor.descriptor_id
                ),
            ));
        }
        if descriptor.calling_convention != CONV_PLATFORM_DEFAULT {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "unsupported calling convention `{}` for descriptor {}",
                    descriptor.calling_convention, descriptor.descriptor_id
                ),
            ));
        }
        let expected_selection_policy = if descriptor.ordinal_alias {
            POLICY_ORDINAL_LITERAL
        } else {
            POLICY_CASE_INSENSITIVE
        };
        let legacy_symbol_mode = descriptor.selection_policy == "legacy-symbol";
        if !legacy_symbol_mode && descriptor.selection_policy != expected_selection_policy {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "unsupported selection policy `{}` for descriptor {} (expected `{}`)",
                    descriptor.selection_policy,
                    descriptor.descriptor_id,
                    expected_selection_policy
                ),
            ));
        }
        if descriptor.declared_name.trim().is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "descriptor {} has empty declared_name",
                    descriptor.descriptor_id
                ),
            ));
        }
        if descriptor.library.trim().is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!("descriptor {} has empty library", descriptor.descriptor_id),
            ));
        }
        if descriptor.alias.trim().is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!("descriptor {} has empty alias", descriptor.descriptor_id),
            ));
        }
        if descriptor.ordinal_alias {
            let ordinal_digits = descriptor.alias.strip_prefix('#').ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "ordinal alias descriptor {} must start with `#`",
                        descriptor.descriptor_id
                    ),
                )
            })?;
            if ordinal_digits.is_empty() || !ordinal_digits.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "ordinal alias descriptor {} must contain decimal digits after `#`",
                        descriptor.descriptor_id
                    ),
                ));
            }
        }
        if legacy_symbol_mode
            && !(descriptor.declared_name == "<legacy>"
                && descriptor.library == "<legacy>"
                && descriptor.alias == "<legacy>")
        {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "legacy selection policy is only valid for legacy descriptors (id={})",
                    descriptor.descriptor_id
                ),
            ));
        }

        let mut state = self.dynlink_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                "dynlink binding table lock poisoned",
            )
        })?;
        if let Some(existing) = state.descriptors.get(&descriptor.descriptor_id).copied() {
            let Some(existing_symbol) = state.bindings.get(&existing).copied() else {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "descriptor {} binding {} is missing from dynlink registry",
                        descriptor.descriptor_id, existing
                    ),
                ));
            };
            if existing_symbol != descriptor.symbol {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "descriptor {} binding mismatch: existing={} resolved_symbol={} new_symbol={}",
                        descriptor.descriptor_id, existing, existing_symbol, descriptor.symbol
                    ),
                ));
            }
            return Ok(existing);
        }
        let binding = state.allocate_binding();
        state.descriptors.insert(descriptor.descriptor_id, binding);
        state.bindings.insert(binding, descriptor.symbol);
        Ok(binding)
    }

    fn prepare_invoke(
        &self,
        _binding: BindingHandle,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Ok(arg)
    }

    fn invoke_bound(&self, binding: BindingHandle, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::DynamicLinking;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol"));
        }
        let symbol = {
            let state = self.dynlink_state.lock().map_err(|_| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    "dynlink binding table lock poisoned",
                )
            })?;
            state.bindings.get(&binding).copied().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "binding handle {} is not resolved in dynlink registry",
                        binding
                    ),
                )
            })?
        };
        let arg = self.runtime_value_to_legacy_i32(&arg, capability, "invoke_symbol", "arg")?;
        if self.native_mode_enabled()
            && matches!(self.profile, HalProfileId::Windows | HalProfileId::Linux)
        {
            return match symbol.raw() {
                s if s == external_symbol_token("host", "ping", "hostping") => {
                    Ok(RuntimeValue::I32(arg.saturating_add(1)))
                }
                s if s == external_symbol_token("host", "double", "hostdouble") => {
                    Ok(RuntimeValue::I32(arg.saturating_mul(2)))
                }
                _ => Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "binding handle {} resolved to unsupported symbol token {} in host-backed lane",
                        binding, symbol
                    ),
                )),
            };
        }
        Ok(RuntimeValue::I32(symbol.raw().saturating_add(arg)))
    }

    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        let binding = self.bind_descriptor(descriptor)?;
        let prepared = self.prepare_invoke(binding, arg)?;
        self.invoke_bound(binding, prepared)
    }

    fn invoke_descriptor_multi(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        args: &[RuntimeValue],
    ) -> HalResult<(RuntimeValue, Vec<RuntimeValue>)> {
        if descriptor.marshal_lane == "m1-native-ffi" && self.native_mode_enabled() {
            return invoke_m1_native(self, descriptor, args);
        }
        // Fall back to m0 deterministic (single-arg)
        let arg = args.first().cloned().unwrap_or(RuntimeValue::I32(0));
        self.invoke_descriptor(descriptor, arg)
            .map(|rv| (rv, Vec::new()))
    }

    fn invoke_symbol(&self, symbol: DynLinkSymbol, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        let arg = self.runtime_value_to_legacy_i32(
            &arg,
            CapabilityId::DynamicLinking,
            "invoke_symbol",
            "arg",
        )?;
        let descriptor = DynLinkDescriptorView {
            descriptor_id: symbol.raw() as u32,
            declared_name: "<legacy>",
            library: "<legacy>",
            alias: "<legacy>",
            ordinal_alias: false,
            symbol,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "legacy-symbol",
            param_count: 0,
            param_types: &[],
            return_type: None,
        };
        self.invoke_descriptor(&descriptor, RuntimeValue::I32(arg))
    }
}

// ── m1-native-ffi invocation ──

#[cfg(target_os = "windows")]
fn invoke_m1_native(
    host: &StandardHostServices,
    descriptor: &DynLinkDescriptorView<'_>,
    args: &[RuntimeValue],
) -> HalResult<(RuntimeValue, Vec<RuntimeValue>)> {
    use oxvba_com::windows_ffi_bridge::{
        FfiArg, FfiReturnType, get_proc_address, get_proc_address_ordinal, invoke_stdcall,
        load_library,
    };

    let capability = CapabilityId::DynamicLinking;
    let module = load_library(descriptor.library).map_err(|msg| {
        HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg)
    })?;

    let proc_addr = if descriptor.ordinal_alias {
        let ordinal_str = descriptor.alias.strip_prefix('#').unwrap_or(descriptor.alias);
        let ordinal: u16 = ordinal_str.parse().map_err(|_| {
            HalError::adapter_fault(
                host.profile,
                capability,
                "invoke_symbol",
                format!("invalid ordinal `{}`", descriptor.alias),
            )
        })?;
        get_proc_address_ordinal(module, ordinal)
    } else {
        get_proc_address(module, descriptor.alias)
    }
    .map_err(|msg| HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg))?;

    let ffi_args: Vec<FfiArg> = args
        .iter()
        .enumerate()
        .map(|(i, rv)| {
            let param_type = descriptor
                .param_types
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or("Long");
            marshal_runtime_to_ffi(rv, param_type)
        })
        .collect();

    let return_type = match descriptor.return_type.as_deref() {
        None => FfiReturnType::Void,
        Some("Long") => FfiReturnType::Long,
        Some("Integer") => FfiReturnType::Integer,
        Some("Byte") => FfiReturnType::Byte,
        Some("Boolean") => FfiReturnType::Boolean,
        Some("Double") => FfiReturnType::Double,
        Some("Single") => FfiReturnType::Single,
        Some("LongLong") => FfiReturnType::LongLong,
        Some("LongPtr") => FfiReturnType::LongPtr,
        Some(_) => FfiReturnType::Long,
    };

    let raw_result = invoke_stdcall(proc_addr, &ffi_args, return_type).map_err(|msg| {
        HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg)
    })?;

    let result = unmarshal_ffi_to_runtime(raw_result, descriptor.return_type.as_deref());
    Ok((result, Vec::new()))
}

#[cfg(not(target_os = "windows"))]
fn invoke_m1_native(
    host: &StandardHostServices,
    _descriptor: &DynLinkDescriptorView<'_>,
    _args: &[RuntimeValue],
) -> HalResult<(RuntimeValue, Vec<RuntimeValue>)> {
    Err(HalError::adapter_fault(
        host.profile,
        CapabilityId::DynamicLinking,
        "invoke_symbol",
        "m1-native-ffi is not yet supported on this platform",
    ))
}

#[cfg(target_os = "windows")]
fn marshal_runtime_to_ffi(
    value: &RuntimeValue,
    param_type: &str,
) -> oxvba_com::windows_ffi_bridge::FfiArg {
    use oxvba_com::windows_ffi_bridge::FfiArg;

    match param_type {
        "Long" => FfiArg::Long(value.to_legacy_i32().unwrap_or(0)),
        "Integer" => FfiArg::Integer(value.to_legacy_i32().unwrap_or(0) as i16),
        "Byte" => FfiArg::Byte(value.to_legacy_i32().unwrap_or(0) as u8),
        "Boolean" => FfiArg::Boolean(if value.to_legacy_i32().unwrap_or(0) != 0 {
            -1
        } else {
            0
        }),
        "Double" => {
            let f = match value {
                RuntimeValue::F64(bits) => bits.as_f64(),
                _ => value.to_legacy_i32().unwrap_or(0) as f64,
            };
            FfiArg::Double(f)
        }
        "Single" => {
            let f = match value {
                RuntimeValue::F64(bits) => bits.as_f64() as f32,
                _ => value.to_legacy_i32().unwrap_or(0) as f32,
            };
            FfiArg::Single(f)
        }
        "LongLong" | "LongPtr" => {
            let v = match value {
                RuntimeValue::I64(v) => *v,
                _ => value.to_legacy_i32().unwrap_or(0) as i64,
            };
            FfiArg::LongLong(v)
        }
        "String" => {
            let text = match value {
                RuntimeValue::String(s) => s.0.clone(),
                _ => String::new(),
            };
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            FfiArg::String(wide)
        }
        _ => FfiArg::Long(value.to_legacy_i32().unwrap_or(0)),
    }
}

fn unmarshal_ffi_to_runtime(raw: i64, return_type: Option<&str>) -> RuntimeValue {
    match return_type {
        None => RuntimeValue::I32(0),
        Some("Long") => RuntimeValue::I32(raw as i32),
        Some("Integer") => RuntimeValue::I32(raw as i16 as i32),
        Some("Byte") => RuntimeValue::I32(raw as u8 as i32),
        Some("Boolean") => RuntimeValue::Bool(raw != 0),
        Some("Double") => RuntimeValue::F64(oxvba_runtime::F64Value::from_f64(f64::from_bits(
            raw as u64,
        ))),
        Some("Single") => RuntimeValue::F64(oxvba_runtime::F64Value::from_f64(
            f32::from_bits(raw as u32) as f64,
        )),
        Some("LongLong") | Some("LongPtr") => RuntimeValue::I64(raw),
        Some(_) => RuntimeValue::I32(raw as i32),
    }
}
