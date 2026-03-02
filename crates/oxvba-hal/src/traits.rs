//! HAL trait contracts.
//!
//! Source anchors (Foundation canonical mirror):
//! - `CreateObject`: CONF-discovered-ms-vbal-250520-f945507e-0325
//! - `InputBox`: CONF-discovered-ms-vbal-250520-f945507e-0329
//! - `MsgBox`: CONF-discovered-ms-vbal-250520-f945507e-0337
//! - `Shell`: CONF-discovered-ms-vbal-250520-f945507e-0346
//! - `Dir`: CONF-discovered-ms-vbal-250520-f945507e-0282
//! - `FreeFile`: CONF-discovered-ms-vbal-250520-f945507e-0286

use crate::{
    error::HalResult,
    model::{HalDescriptor, HalProfileId, HostPolicy},
};

/// VM/runtime value token crossing the current HAL boundary.
/// This is intentionally `i32` for the current register-window runtime representation.
pub type ValueToken = i32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynLinkDescriptorView<'a> {
    pub descriptor_id: u32,
    pub declared_name: &'a str,
    pub library: &'a str,
    pub alias: &'a str,
    pub ordinal_alias: bool,
    pub symbol: ValueToken,
    pub marshal_lane: &'a str,
    pub calling_convention: &'a str,
    pub selection_policy: &'a str,
}

pub trait HostServices: Send + Sync {
    fn profile(&self) -> HalProfileId;
    fn descriptor(&self) -> HalDescriptor;
    fn policy(&self) -> &HostPolicy;

    fn ui(&self) -> &dyn UiInteractionHal;
    fn events(&self) -> &dyn EventPumpHal;
    fn fs(&self) -> &dyn FileSystemHal;
    fn process(&self) -> &dyn ProcessEnvHal;
    fn com(&self) -> &dyn ComHal;
    fn time_locale(&self) -> &dyn TimeLocaleHal;
    fn dynlink(&self) -> &dyn DynamicLinkHal;
    fn diag(&self) -> &dyn DiagnosticsHal;
}

pub trait UiInteractionHal: Send + Sync {
    /// Deterministically implements `MsgBox` interaction or a policy/capability error.
    fn msg_box(&self, prompt: ValueToken, style: ValueToken) -> HalResult<ValueToken>;
    /// Deterministically implements `InputBox` interaction or a policy/capability error.
    fn input_box(&self, prompt: ValueToken, default_value: ValueToken) -> HalResult<ValueToken>;
}

pub trait EventPumpHal: Send + Sync {
    /// Deterministically pumps host events, or reports unsupported behavior.
    fn do_events(&self) -> HalResult<ValueToken>;
}

pub trait FileSystemHal: Send + Sync {
    fn open(&self, path: ValueToken, mode: ValueToken) -> HalResult<ValueToken>;
    fn close(&self, handle: ValueToken) -> HalResult<ValueToken>;
    fn seek(&self, handle: ValueToken, position: ValueToken) -> HalResult<ValueToken>;
    fn eof(&self, handle: ValueToken) -> HalResult<ValueToken>;
    fn lof(&self, handle: ValueToken) -> HalResult<ValueToken>;
    fn free_file(&self, range_selector: ValueToken) -> HalResult<ValueToken>;
}

pub trait ProcessEnvHal: Send + Sync {
    fn shell(&self, command: ValueToken, window_style: ValueToken) -> HalResult<ValueToken>;
    fn environ(&self, key: ValueToken) -> HalResult<ValueToken>;
    fn dir(&self, path: ValueToken, attrs: ValueToken) -> HalResult<ValueToken>;
}

pub trait ComHal: Send + Sync {
    fn create_object(&self, prog_id: ValueToken) -> HalResult<ValueToken>;
    fn dispatch_invoke(
        &self,
        object: ValueToken,
        member: ValueToken,
        arg: ValueToken,
    ) -> HalResult<ValueToken>;
}

pub trait TimeLocaleHal: Send + Sync {
    fn date_serial_now(&self) -> HalResult<ValueToken>;
    fn time_serial_now(&self) -> HalResult<ValueToken>;
    fn timer_ticks(&self) -> HalResult<ValueToken>;
}

pub trait DynamicLinkHal: Send + Sync {
    /// Resolves descriptor metadata into an invocation binding token.
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<ValueToken> {
        Ok(descriptor.symbol)
    }

    /// Optional argument normalization/writeback preparation hook.
    fn prepare_invoke(&self, _binding: ValueToken, arg: ValueToken) -> HalResult<ValueToken> {
        Ok(arg)
    }

    /// Invokes a previously bound symbol token.
    fn invoke_bound(&self, binding: ValueToken, arg: ValueToken) -> HalResult<ValueToken> {
        self.invoke_symbol(binding, arg)
    }

    /// Descriptor-driven invoke path used by VM/host integrations.
    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: ValueToken,
    ) -> HalResult<ValueToken> {
        let binding = self.bind_descriptor(descriptor)?;
        let prepared = self.prepare_invoke(binding, arg)?;
        self.invoke_bound(binding, prepared)
    }

    /// Legacy symbol-token invoke path retained for backward compatibility.
    fn invoke_symbol(&self, symbol: ValueToken, arg: ValueToken) -> HalResult<ValueToken>;
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit(&self, code: ValueToken, payload: ValueToken) -> HalResult<ValueToken>;
}
