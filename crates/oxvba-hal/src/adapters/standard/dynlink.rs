use crate::{
    error::{HalError, HalResult},
    model::{CapabilityId, HalProfileId},
    traits::{DynLinkDescriptorView, DynamicLinkHal},
};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, RuntimeValue, Variant};
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
    let library = library.to_ascii_lowercase();
    let alias = alias.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
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
        let arg =
            self.runtime_value_project_compat_slot_i32(&arg, capability, "invoke_symbol", "arg")?;
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
        if descriptor.marshal_lane == "m1-native-ffi" && self.native_mode_enabled() {
            return self
                .invoke_descriptor_multi(descriptor, &[arg])
                .map(|(rv, _)| rv);
        }
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

    fn invoke_descriptor_variants(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        if descriptor.marshal_lane != "m1-native-ffi" || !self.native_mode_enabled() {
            let _binding = self.bind_descriptor(descriptor)?;
            let arg = args.first().cloned().unwrap_or_else(|| Variant::from_i32(0));
            let arg = self.variant_project_compat_slot_i32(
                &arg,
                CapabilityId::DynamicLinking,
                "invoke_descriptor_variants",
                "arg",
            )?;
            if self.native_mode_enabled()
                && matches!(self.profile, HalProfileId::Windows | HalProfileId::Linux)
            {
                return match descriptor.symbol.raw() {
                    s if s == external_symbol_token("host", "ping", "hostping") => {
                        Ok((Variant::from_i32(arg.saturating_add(1)), Vec::new()))
                    }
                    s if s == external_symbol_token("host", "double", "hostdouble") => {
                        Ok((Variant::from_i32(arg.saturating_mul(2)), Vec::new()))
                    }
                    _ => Err(HalError::adapter_fault(
                        self.profile,
                        CapabilityId::DynamicLinking,
                        "invoke_descriptor_variants",
                        format!(
                            "descriptor {} resolved to unsupported symbol token {} in host-backed lane",
                            descriptor.descriptor_id, descriptor.symbol
                        ),
                    )),
                };
            }
            return Ok((
                Variant::from_i32(descriptor.symbol.raw().saturating_add(arg)),
                Vec::new(),
            ));
        }
        let args = variants_to_runtime_values(self.profile, args)?;
        let (ret, writebacks) = self.invoke_descriptor_multi(descriptor, &args)?;
        runtime_result_to_variants(self.profile, ret, writebacks)
    }

    fn invoke_symbol(&self, symbol: DynLinkSymbol, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        let arg = self.runtime_value_project_compat_slot_i32(
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
            param_by_ref: &[],
            return_type: None,
        };
        self.invoke_descriptor(&descriptor, RuntimeValue::I32(arg))
    }

    fn invoke_symbol_variant(&self, symbol: DynLinkSymbol, arg: &Variant) -> HalResult<Variant> {
        let capability = CapabilityId::DynamicLinking;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol_variant"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol_variant"));
        }
        let arg = self.variant_project_compat_slot_i32(
            arg,
            CapabilityId::DynamicLinking,
            "invoke_symbol_variant",
            "arg",
        )?;
        if self.native_mode_enabled()
            && matches!(self.profile, HalProfileId::Windows | HalProfileId::Linux)
        {
            return match symbol.raw() {
                s if s == external_symbol_token("host", "ping", "hostping") => {
                    Ok(Variant::from_i32(arg.saturating_add(1)))
                }
                s if s == external_symbol_token("host", "double", "hostdouble") => {
                    Ok(Variant::from_i32(arg.saturating_mul(2)))
                }
                _ => Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol_variant",
                    format!(
                        "unsupported symbol token {} in host-backed lane",
                        symbol
                    ),
                )),
            };
        }
        Ok(Variant::from_i32(symbol.raw().saturating_add(arg)))
    }
}

fn variants_to_runtime_values(
    profile: HalProfileId,
    args: &[Variant],
) -> HalResult<Vec<RuntimeValue>> {
    args.iter()
        .map(|value| {
            value.to_runtime_value().map_err(|detail| {
                HalError::adapter_fault(
                    profile,
                    CapabilityId::DynamicLinking,
                    "invoke_descriptor_variants",
                    detail,
                )
            })
        })
        .collect()
}

fn runtime_result_to_variants(
    profile: HalProfileId,
    ret: RuntimeValue,
    writebacks: Vec<RuntimeValue>,
) -> HalResult<(Variant, Vec<Variant>)> {
    let ret = Variant::try_from_runtime_value(&ret).map_err(|detail| {
        HalError::adapter_fault(
            profile,
            CapabilityId::DynamicLinking,
            "invoke_descriptor_variants",
            detail,
        )
    })?;
    let writebacks = writebacks
        .iter()
        .map(|value| {
            Variant::try_from_runtime_value(value).map_err(|detail| {
                HalError::adapter_fault(
                    profile,
                    CapabilityId::DynamicLinking,
                    "invoke_descriptor_variants",
                    detail,
                )
            })
        })
        .collect::<HalResult<Vec<_>>>()?;
    Ok((ret, writebacks))
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
    let module = load_library(descriptor.library)
        .map_err(|msg| HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg))?;

    let proc_addr = if descriptor.ordinal_alias {
        let ordinal_str = descriptor
            .alias
            .strip_prefix('#')
            .unwrap_or(descriptor.alias);
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

    let mut byref_cells = Vec::new();
    let ffi_args: Vec<FfiArg> = args
        .iter()
        .enumerate()
        .map(|(i, rv)| {
            let param_type = descriptor
                .param_types
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or("Long");
            let by_ref = descriptor.param_by_ref.get(i).copied().unwrap_or(false);
            marshal_runtime_to_ffi(
                host.profile,
                rv,
                param_type,
                by_ref,
                descriptor.alias,
                descriptor.ordinal_alias,
                i,
                &mut byref_cells,
            )
        })
        .collect::<Result<_, _>>()?;

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

    let raw_result = invoke_stdcall(proc_addr, &ffi_args, return_type)
        .map_err(|msg| HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg))?;

    let result = unmarshal_ffi_to_runtime(raw_result, descriptor.return_type.as_deref());
    let mut writeback_values = args.to_vec();
    for cell in &byref_cells {
        writeback_values[cell.index] = cell.storage.read_back_value();
    }
    Ok((result, writeback_values))
}

#[cfg(not(target_os = "windows"))]
fn invoke_m1_native(
    host: &StandardHostServices,
    descriptor: &DynLinkDescriptorView<'_>,
    args: &[RuntimeValue],
) -> HalResult<(RuntimeValue, Vec<RuntimeValue>)> {
    use oxvba_com::windows_ffi_bridge::{
        FfiArg, FfiReturnType, get_proc_address, invoke_stdcall, load_library,
    };

    let capability = CapabilityId::DynamicLinking;

    if descriptor.ordinal_alias {
        return Err(HalError::adapter_fault(
            host.profile,
            capability,
            "invoke_symbol",
            format!(
                "ordinal-based symbol resolution (alias `{}`) is not supported on Unix platforms",
                descriptor.alias
            ),
        ));
    }
    if descriptor.param_by_ref.iter().copied().any(|flag| flag) {
        return Err(HalError::adapter_fault(
            host.profile,
            capability,
            "invoke_symbol",
            "native m1 ByRef scalar marshaling is not yet implemented on Unix hosts",
        ));
    }

    let module = load_library(descriptor.library)
        .map_err(|msg| HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg))?;

    let proc_addr = get_proc_address(module, descriptor.alias)
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
            marshal_runtime_to_ffi_unix(rv, param_type)
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

    let raw_result = invoke_stdcall(proc_addr, &ffi_args, return_type)
        .map_err(|msg| HalError::adapter_fault(host.profile, capability, "invoke_symbol", msg))?;

    let result = unmarshal_ffi_to_runtime(raw_result, descriptor.return_type.as_deref());
    Ok((result, Vec::new()))
}

#[cfg(target_os = "windows")]
struct NativeByRefCell {
    index: usize,
    storage: NativeByRefStorage,
}

#[cfg(target_os = "windows")]
enum NativeByRefStorage {
    I32(Box<i32>),
    I16(Box<i16>),
    U8(Box<u8>),
    Bool(Box<i16>),
    F32(Box<f32>),
    F64(Box<f64>),
    I64(Box<i64>),
    Currency(Box<i64>),
    Date(Box<f64>),
}

#[cfg(target_os = "windows")]
impl NativeByRefStorage {
    fn from_runtime_value(value: &RuntimeValue, param_type: &str) -> Result<Self, String> {
        Ok(match param_type {
            "Long" => Self::I32(Box::new(value.project_compat_slot_i32().unwrap_or(0))),
            "Integer" => Self::I16(Box::new(value.project_compat_slot_i32().unwrap_or(0) as i16)),
            "Byte" => Self::U8(Box::new(value.project_compat_slot_i32().unwrap_or(0) as u8)),
            "Boolean" => Self::Bool(Box::new(
                if value.project_compat_slot_i32().unwrap_or(0) != 0 {
                    -1
                } else {
                    0
                },
            )),
            "Double" => {
                let raw = match value {
                    RuntimeValue::F64(bits) => bits.as_f64(),
                    _ => value.project_compat_slot_i32().unwrap_or(0) as f64,
                };
                Self::F64(Box::new(raw))
            }
            "Single" => {
                let raw = match value {
                    RuntimeValue::F64(bits) => bits.as_f64() as f32,
                    _ => value.project_compat_slot_i32().unwrap_or(0) as f32,
                };
                Self::F32(Box::new(raw))
            }
            "LongLong" | "LongPtr" => {
                let raw = match value {
                    RuntimeValue::I64(v) => *v,
                    _ => value.project_compat_slot_i32().unwrap_or(0) as i64,
                };
                Self::I64(Box::new(raw))
            }
            "Currency" => {
                let raw = match value {
                    RuntimeValue::Currency(v) => v.scaled_i64(),
                    RuntimeValue::I64(v) => *v,
                    _ => {
                        i64::from(value.project_compat_slot_i32().unwrap_or(0))
                            * oxvba_runtime::CurrencyValue::SCALE
                    }
                };
                Self::Currency(Box::new(raw))
            }
            "Date" => {
                let raw = match value {
                    RuntimeValue::F64(v) => v.as_f64(),
                    RuntimeValue::I64(v) => *v as f64,
                    _ => value.project_compat_slot_i32().unwrap_or(0) as f64,
                };
                Self::Date(Box::new(raw))
            }
            other => {
                return Err(format!(
                    "native ByRef marshaling for declare parameter type `{other}` is not yet supported"
                ));
            }
        })
    }

    fn as_ffi_arg(&mut self) -> oxvba_com::windows_ffi_bridge::FfiArg {
        use oxvba_com::windows_ffi_bridge::FfiArg;
        use std::ffi::c_void;

        match self {
            Self::I32(value) => FfiArg::Pointer((&mut **value as *mut i32).cast::<c_void>()),
            Self::I16(value) => FfiArg::Pointer((&mut **value as *mut i16).cast::<c_void>()),
            Self::U8(value) => FfiArg::Pointer((&mut **value as *mut u8).cast::<c_void>()),
            Self::Bool(value) => FfiArg::Pointer((&mut **value as *mut i16).cast::<c_void>()),
            Self::F32(value) => FfiArg::Pointer((&mut **value as *mut f32).cast::<c_void>()),
            Self::F64(value) => FfiArg::Pointer((&mut **value as *mut f64).cast::<c_void>()),
            Self::I64(value) => FfiArg::Pointer((&mut **value as *mut i64).cast::<c_void>()),
            Self::Currency(value) => FfiArg::Pointer((&mut **value as *mut i64).cast::<c_void>()),
            Self::Date(value) => FfiArg::Pointer((&mut **value as *mut f64).cast::<c_void>()),
        }
    }

    fn read_back_value(&self) -> RuntimeValue {
        match self {
            Self::I32(value) => RuntimeValue::I32(**value),
            Self::I16(value) => RuntimeValue::I32(**value as i32),
            Self::U8(value) => RuntimeValue::I32(**value as i32),
            Self::Bool(value) => RuntimeValue::Bool(**value != 0),
            Self::F32(value) => {
                RuntimeValue::F64(oxvba_runtime::F64Value::from_single_f64(**value as f64))
            }
            Self::F64(value) => RuntimeValue::F64(oxvba_runtime::F64Value::from_f64(**value)),
            Self::I64(value) => RuntimeValue::I64(**value),
            Self::Currency(value) => {
                RuntimeValue::Currency(oxvba_runtime::CurrencyValue::from_scaled_i64(**value))
            }
            Self::Date(value) => RuntimeValue::F64(oxvba_runtime::F64Value::from_date_f64(**value)),
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn marshal_runtime_to_ffi_unix(
    value: &RuntimeValue,
    param_type: &str,
) -> oxvba_com::windows_ffi_bridge::FfiArg {
    use oxvba_com::windows_ffi_bridge::FfiArg;

    match param_type {
        "Long" => FfiArg::Long(value.project_compat_slot_i32().unwrap_or(0)),
        "Integer" => FfiArg::Integer(value.project_compat_slot_i32().unwrap_or(0) as i16),
        "Byte" => FfiArg::Byte(value.project_compat_slot_i32().unwrap_or(0) as u8),
        "Boolean" => FfiArg::Boolean(if value.project_compat_slot_i32().unwrap_or(0) != 0 {
            -1
        } else {
            0
        }),
        "Double" => {
            let f = match value {
                RuntimeValue::F64(bits) => bits.as_f64(),
                _ => value.project_compat_slot_i32().unwrap_or(0) as f64,
            };
            FfiArg::Double(f)
        }
        "Single" => {
            let f = match value {
                RuntimeValue::F64(bits) => bits.as_f64() as f32,
                _ => value.project_compat_slot_i32().unwrap_or(0) as f32,
            };
            FfiArg::Single(f)
        }
        "LongLong" | "LongPtr" => {
            let v = match value {
                RuntimeValue::I64(v) => *v,
                _ => value.project_compat_slot_i32().unwrap_or(0) as i64,
            };
            if let Some(pointer) = oxvba_runtime::pointer_helpers::lookup_pointer(v) {
                FfiArg::Pointer(pointer)
            } else {
                FfiArg::LongLong(v)
            }
        }
        "String" => {
            let text = match value {
                RuntimeValue::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            // On Unix, we still marshal as wide string for API compatibility.
            // Individual native functions that expect UTF-8 would need a separate
            // marshaling path in the future.
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            FfiArg::String(wide)
        }
        _ => FfiArg::Long(value.project_compat_slot_i32().unwrap_or(0)),
    }
}

#[cfg(target_os = "windows")]
fn marshal_runtime_to_ffi(
    profile: HalProfileId,
    value: &RuntimeValue,
    param_type: &str,
    by_ref: bool,
    alias: &str,
    ordinal_alias: bool,
    arg_index: usize,
    byref_cells: &mut Vec<NativeByRefCell>,
) -> Result<oxvba_com::windows_ffi_bridge::FfiArg, HalError> {
    use oxvba_com::windows_ffi_bridge::FfiArg;

    if by_ref {
        let mut storage =
            NativeByRefStorage::from_runtime_value(value, param_type).map_err(|msg| {
                HalError::adapter_fault(profile, CapabilityId::DynamicLinking, "invoke_symbol", msg)
            })?;
        let ffi_arg = storage.as_ffi_arg();
        byref_cells.push(NativeByRefCell {
            index: arg_index,
            storage,
        });
        return Ok(ffi_arg);
    }

    match param_type {
        "Long" => Ok(FfiArg::Long(value.project_compat_slot_i32().unwrap_or(0))),
        "Integer" => Ok(FfiArg::Integer(
            value.project_compat_slot_i32().unwrap_or(0) as i16,
        )),
        "Byte" => Ok(FfiArg::Byte(
            value.project_compat_slot_i32().unwrap_or(0) as u8
        )),
        "Boolean" => Ok(FfiArg::Boolean(
            if value.project_compat_slot_i32().unwrap_or(0) != 0 {
                -1
            } else {
                0
            },
        )),
        "Double" => {
            let f = match value {
                RuntimeValue::F64(bits) => bits.as_f64(),
                _ => value.project_compat_slot_i32().unwrap_or(0) as f64,
            };
            Ok(FfiArg::Double(f))
        }
        "Single" => {
            let f = match value {
                RuntimeValue::F64(bits) => bits.as_f64() as f32,
                _ => value.project_compat_slot_i32().unwrap_or(0) as f32,
            };
            Ok(FfiArg::Single(f))
        }
        "LongLong" | "LongPtr" => {
            let v = match value {
                RuntimeValue::I64(v) => *v,
                _ => value.project_compat_slot_i32().unwrap_or(0) as i64,
            };
            if let Some(pointer) = oxvba_runtime::pointer_helpers::lookup_pointer(v) {
                Ok(FfiArg::Pointer(pointer))
            } else {
                Ok(FfiArg::LongLong(v))
            }
        }
        "String" => {
            let text = match value {
                RuntimeValue::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            if !ordinal_alias && alias.ends_with('A') {
                return Ok(FfiArg::AnsiString(
                    oxvba_com::windows_ffi_bridge::ansi_c_string(&text),
                ));
            }
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            Ok(FfiArg::String(wide))
        }
        _ => Ok(FfiArg::Long(value.project_compat_slot_i32().unwrap_or(0))),
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
        Some("Single") => RuntimeValue::F64(oxvba_runtime::F64Value::from_f64(f32::from_bits(
            raw as u32,
        ) as f64)),
        Some("LongLong") | Some("LongPtr") => RuntimeValue::I64(raw),
        Some(_) => RuntimeValue::I32(raw as i32),
    }
}
