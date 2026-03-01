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
    fn invoke_symbol(&self, symbol: ValueToken, arg: ValueToken) -> HalResult<ValueToken>;
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit(&self, code: ValueToken, payload: ValueToken) -> HalResult<ValueToken>;
}
