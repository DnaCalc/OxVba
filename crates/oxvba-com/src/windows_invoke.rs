#![allow(clippy::result_large_err)]

use crate::{
    ComBinding, ComInvokeArg, ComInvokeRequest, VariantResultValue, set_variant_from_com_value,
    take_variant_result_value, take_variant_result_variant,
};
use oxvba_diagnostics::{Diagnostic, DiagnosticPhase, extract_prefixed_code};
use oxvba_runtime::{ObjectRef, RuntimeByRefSlot, RuntimeCallResult, RuntimeValueType, Variant};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{SysFreeString, SysStringLen};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{
    DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF,
    DISPPARAMS, EXCEPINFO,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::{VARIANT, VT_ERROR};

#[cfg(target_os = "windows")]
const IID_NULL: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};

#[cfg(target_os = "windows")]
const COM_DISP_E_PARAMNOTFOUND: i32 = 0x8002_0004u32 as i32;

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIUnknownVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        iid: *const windows_sys::core::GUID,
        interface: *mut *mut core::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIDispatchVtbl {
    unknown: RawIUnknownVtbl,
    get_type_info_count:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, pctinfo: *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        itinfo: u32,
        lcid: u32,
        pptinfo: *mut *mut core::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        names: *mut *mut u16,
        count: u32,
        lcid: u32,
        dispids: *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        dispid_member: i32,
        riid: *const windows_sys::core::GUID,
        lcid: u32,
        w_flags: u16,
        params: *mut DISPPARAMS,
        result: *mut VARIANT,
        excep_info: *mut EXCEPINFO,
        pu_arg_err: *mut u32,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIDispatch {
    vtbl: *const RawIDispatchVtbl,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeExceptionInfo {
    pub source: Option<String>,
    pub description: Option<String>,
    pub help_file: Option<String>,
    pub help_context: Option<u32>,
    pub scode: Option<i32>,
    pub wcode: Option<u16>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeFailure {
    pub label: &'static str,
    pub dispid: i32,
    pub hr: Option<i32>,
    pub arg_err: Option<u32>,
    pub excep: Option<ComInvokeExceptionInfo>,
    pub detail: Option<String>,
}

#[cfg(target_os = "windows")]
fn render_invoke_fault_message(failure: &ComInvokeFailure) -> String {
    let label = failure.classification_label();
    let mut suffix = String::new();
    if let Some(hr) = failure.hr {
        suffix.push_str(&format!("hresult=0x{:08X};", hr as u32));
    }
    if let Some(value) = failure.arg_err {
        suffix.push_str(&format!("arg_err={value};"));
    }
    if let Some(excep) = &failure.excep {
        if let Some(scode) = excep.scode {
            suffix.push_str(&format!("excep_scode=0x{:08X};", scode as u32));
        }
        if let Some(wcode) = excep.wcode {
            suffix.push_str(&format!("excep_wcode={wcode};"));
        }
    }
    let prefix = if suffix.is_empty() {
        format!("com-dispatch-{label}")
    } else {
        format!("com-dispatch-{label};{suffix}")
    };
    format!("{prefix} {}", failure.render())
}

#[cfg(target_os = "windows")]
impl ComInvokeFailure {
    pub fn classification_label(&self) -> &'static str {
        if self.hr.is_none()
            && self.arg_err.is_none()
            && let Some(label) = classify_invoke_detail_label(self.detail.as_deref())
        {
            return label;
        }
        map_com_hresult_label(self.hr.map(|hr| hr as u32), self.arg_err)
    }

    /// The VBA `Err.Number` this COM dispatch failure should surface.
    ///
    /// Mirrors the standard OLE Automation contract: an `EXCEPINFO`-bearing
    /// failure carries the VBA number directly (`wCode` when nonzero, otherwise
    /// derived from `scode`); a plain `DISP_E_*`/`E_*` HRESULT maps through the
    /// canonical automation table. See [`map_com_hresult_vba_number`].
    pub fn vba_error_number(&self) -> i32 {
        vba_number_from_dispatch_codes(
            self.hr.map(|hr| hr as u32),
            self.excep.as_ref().and_then(|excep| excep.scode),
            self.excep.as_ref().and_then(|excep| excep.wcode),
        )
    }

    /// The COM-supplied `EXCEPINFO.bstrDescription`, when present — the text VBA
    /// would surface as `Err.Description` (e.g. "Database already exists.").
    pub fn vba_description(&self) -> Option<&str> {
        self.excep
            .as_ref()
            .and_then(|excep| excep.description.as_deref())
    }

    pub fn render(&self) -> String {
        let mut message = format!(
            "IDispatch::Invoke({} dispid={}) failed",
            self.label, self.dispid
        );
        if let Some(hr) = self.hr {
            message.push_str(&format!(" with HRESULT {:#010X}", hr as u32));
        }
        if let Some(arg_err) = self.arg_err {
            message.push_str(&format!(" (arg_err={arg_err})"));
        }
        if let Some(excep) = &self.excep {
            if let Some(source) = &excep.source {
                message.push_str(&format!(
                    " excep_source=\"{}\"",
                    sanitize_error_text(source)
                ));
            }
            if let Some(description) = &excep.description {
                message.push_str(&format!(
                    " excep_description=\"{}\"",
                    sanitize_error_text(description)
                ));
            }
            if let Some(help_file) = &excep.help_file {
                message.push_str(&format!(
                    " excep_help_file=\"{}\"",
                    sanitize_error_text(help_file)
                ));
            }
            if let Some(help_context) = excep.help_context {
                message.push_str(&format!(" excep_help_context={help_context}"));
            }
            if let Some(scode) = excep.scode {
                message.push_str(&format!(" excep_scode={:#010X}", scode as u32));
            }
            if let Some(wcode) = excep.wcode {
                message.push_str(&format!(" excep_wcode={wcode}"));
            }
        }
        if let Some(detail) = &self.detail {
            message.push_str(&format!(" detail=\"{}\"", sanitize_error_text(detail)));
        }
        message
    }

    pub fn to_diagnostic(&self) -> Diagnostic {
        let rendered = self.render();
        let code = self
            .detail
            .as_deref()
            .and_then(|detail| extract_prefixed_code(detail, "COM-E-"))
            .unwrap_or_else(|| "COM-E-DISPATCH-INVOKE-FAILED".to_string());
        let mut diagnostic = Diagnostic::error(code, DiagnosticPhase::Com, rendered)
            .with_metadata("label", self.label)
            .with_metadata("classification", self.classification_label())
            .with_metadata("dispid", self.dispid.to_string())
            .with_vba_error_number(self.vba_error_number());
        if let Some(hr) = self.hr {
            diagnostic = diagnostic.with_metadata("hresult", format!("0x{:08X}", hr as u32));
        }
        if let Some(arg_err) = self.arg_err {
            diagnostic = diagnostic.with_metadata("arg_err", arg_err.to_string());
        }
        diagnostic
    }
}

#[cfg(target_os = "windows")]
pub fn map_com_hresult_label(hresult: Option<u32>, arg_err: Option<u32>) -> &'static str {
    if arg_err.is_some() {
        return "arg-error";
    }
    match hresult {
        Some(0x8004_0154) => "class-not-registered",
        Some(0x8004_01F3) => "invalid-class-string",
        Some(0x8000_4002) => "no-interface",
        Some(0x8002_0003) => "member-not-found",
        Some(0x8002_0004) => "param-not-found",
        Some(0x8002_0006) => "unknown-name",
        Some(0x8002_0005) => "type-mismatch",
        Some(0x8002_000E) => "bad-param-count",
        Some(0x8002_0009) => "exception-raised",
        Some(0x8007_0057) => "invalid-argument",
        Some(_) => "native-failure",
        None => "fault-unspecified",
    }
}

/// Map a raw COM dispatch HRESULT to the VBA `Err.Number` the runtime should
/// surface (the non-`EXCEPINFO` path).
///
/// This is the canonical OLE Automation mapping VBA itself applies when an
/// `IDispatch::Invoke` returns a `DISP_E_*`/`E_*` failure that carries no
/// raised-exception `EXCEPINFO`:
///
/// | HRESULT                          | scode        | VBA Err.Number |
/// |----------------------------------|--------------|----------------|
/// | `DISP_E_UNKNOWNNAME`             | `0x80020006` | 438            |
/// | `DISP_E_MEMBERNOTFOUND`          | `0x80020003` | 438            |
/// | `DISP_E_BADPARAMCOUNT`           | `0x8002000E` | 449            |
/// | `DISP_E_PARAMNOTFOUND`           | `0x80020004` | 449            |
/// | `DISP_E_TYPEMISMATCH`            | `0x80020005` | 13             |
/// | `DISP_E_BADVARTYPE`              | `0x80020008` | 13             |
/// | `DISP_E_OVERFLOW`                | `0x8002000A` | 6              |
/// | `DISP_E_BADINDEX`               | `0x8002000B` | 9              |
/// | `DISP_E_DIVBYZERO`              | `0x80020012` | 11             |
/// | `E_NOINTERFACE`                  | `0x80004002` | 430            |
/// | FACILITY_CONTROL (`0x800Axxxx`)  | —            | `scode & 0xFFFF` |
/// | (anything else / none)           | —            | 5              |
///
/// A FACILITY_CONTROL automation HRESULT encodes the VBA number in its low word
/// (e.g. `0x800A0C84` → `0x0C84` = 3204, `0x800A01C9` → `0x01C9` = 457), which
/// is why an `EXCEPINFO`-less server error still surfaces its real number.
///
/// `DISP_E_BADPARAMCOUNT` maps to 449 ("Argument not optional"): VBA surfaces a
/// missing required argument (the `Scripting.Dictionary.Add "onlyKey"` shape) as
/// 449, not 450.
pub fn map_com_hresult_vba_number(hresult: Option<u32>) -> i32 {
    let Some(hr) = hresult else {
        return 5;
    };
    match hr {
        0x8002_0006 | 0x8002_0003 => 438, // DISP_E_UNKNOWNNAME / DISP_E_MEMBERNOTFOUND
        0x8002_000E => 449,               // DISP_E_BADPARAMCOUNT (missing required arg → 449)
        0x8002_0004 => 449,               // DISP_E_PARAMNOTFOUND
        0x8002_0005 | 0x8002_0008 => 13,  // DISP_E_TYPEMISMATCH / DISP_E_BADVARTYPE
        0x8002_000A => 6,                 // DISP_E_OVERFLOW
        0x8002_000B => 9,                 // DISP_E_BADINDEX (subscript out of range)
        0x8002_0012 => 11,                // DISP_E_DIVBYZERO
        0x8000_4002 => 430,               // E_NOINTERFACE
        _ => automation_scode_to_vba_number(hr),
    }
}

/// Derive the VBA `Err.Number` a COM dispatch failure should surface from its
/// raw `(hr, scode, wcode)` codes — the single source of truth shared by
/// [`ComInvokeFailure::vba_error_number`] and the message-recovered HAL fault
/// path (`com_dispatch_adapter_fault`, which only has the rendered codes).
///
/// Mirrors the OLE Automation contract: an `EXCEPINFO`-bearing failure carries
/// the VBA number directly (`wCode` when nonzero, otherwise derived from
/// `scode`); a plain `DISP_E_*`/`E_*` HRESULT maps through the canonical
/// automation table. A `None`/zero `scode`/`wcode` falls through to the bare
/// HRESULT mapping.
pub fn vba_number_from_dispatch_codes(
    hr: Option<u32>,
    scode: Option<i32>,
    wcode: Option<u16>,
) -> i32 {
    if let Some(wcode) = wcode
        && wcode != 0
    {
        return i32::from(wcode);
    }
    if let Some(scode) = scode
        && scode != 0
    {
        // An EXCEPINFO scode is itself an HRESULT/SCODE: route it through the
        // full automation table (DISP_E_* + FACILITY_CONTROL), not just the
        // FACILITY_CONTROL low-word rule, so e.g. `DISP_E_BADINDEX`
        // (0x8002000B → subscript-out-of-range 9) surfaces correctly.
        return map_com_hresult_vba_number(Some(scode as u32));
    }
    map_com_hresult_vba_number(hr)
}

/// Derive the VBA `Err.Number` from an SCODE/HRESULT, honoring the
/// FACILITY_CONTROL automation convention (`0x800Axxxx` carries the VBA number
/// in its low 16 bits). Any other code that is not a recognized FACILITY_CONTROL
/// automation HRESULT falls back to 5 ("invalid procedure call or argument").
fn automation_scode_to_vba_number(scode: u32) -> i32 {
    // FACILITY_CONTROL automation HRESULTs are 0x800A0000..=0x800AFFFF: the low
    // word is the literal VBA Err.Number the server raised.
    if (0x800A_0000..=0x800A_FFFF).contains(&scode) {
        return (scode & 0xFFFF) as i32;
    }
    5
}

#[cfg(target_os = "windows")]
fn classify_invoke_detail_label(detail: Option<&str>) -> Option<&'static str> {
    let detail = detail?;
    if detail.contains("exceeds i64 carrier range") {
        return Some("carrier-overflow");
    }
    if detail.starts_with("unsupported VARIANT BYREF return type vt=") {
        return Some("unsupported-byref-return");
    }
    None
}

#[cfg(target_os = "windows")]
fn sanitize_error_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Read a callee-transferred BSTR into an owned `String` and free it with
/// `SysFreeString` (we own the transferred string). Shared with the vtable
/// marshaller, which has the same retval-BSTR ownership shape.
///
/// # Safety
/// `bstr`, when non-null, must be a BSTR whose ownership has transferred to us
/// (so freeing it exactly once here is correct) and which is otherwise valid.
pub(crate) unsafe fn bstr_to_string_and_free(bstr: windows_sys::core::BSTR) -> Option<String> {
    if bstr.is_null() {
        return None;
    }
    let len = usize::try_from(SysStringLen(bstr)).unwrap_or(0);
    let slice = std::slice::from_raw_parts(bstr, len);
    let text = String::from_utf16_lossy(slice);
    SysFreeString(bstr);
    Some(text)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Consume any owned BSTR fields from an `EXCEPINFO` and convert the non-empty details into the
/// shared invoke-failure payload used by the Windows COM bridge.
///
/// # Safety
/// The caller must provide a valid writable `EXCEPINFO` pointer whose BSTR fields, when non-null,
/// are owned by the caller and may be released exactly once by this function.
pub unsafe fn take_excepinfo(excep: &mut EXCEPINFO) -> Option<ComInvokeExceptionInfo> {
    let source = bstr_to_string_and_free(excep.bstrSource);
    let description = bstr_to_string_and_free(excep.bstrDescription);
    let help_file = bstr_to_string_and_free(excep.bstrHelpFile);
    excep.bstrSource = std::ptr::null_mut();
    excep.bstrDescription = std::ptr::null_mut();
    excep.bstrHelpFile = std::ptr::null_mut();
    let help_context = if excep.dwHelpContext != 0 {
        Some(excep.dwHelpContext)
    } else {
        None
    };
    let scode = if excep.scode != 0 {
        Some(excep.scode)
    } else {
        None
    };
    let wcode = if excep.wCode != 0 {
        Some(excep.wCode)
    } else {
        None
    };
    if source.is_none()
        && description.is_none()
        && help_file.is_none()
        && help_context.is_none()
        && scode.is_none()
        && wcode.is_none()
    {
        None
    } else {
        Some(ComInvokeExceptionInfo {
            source,
            description,
            help_file,
            help_context,
            scode,
            wcode,
        })
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_missing_arg(variant: &mut VARIANT) {
    variant.Anonymous.Anonymous.vt = VT_ERROR;
    variant.Anonymous.Anonymous.Anonymous.scode = COM_DISP_E_PARAMNOTFOUND;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn clear_variant_args(args: &mut [VARIANT]) {
    for variant in args {
        let _ = windows_sys::Win32::System::Variant::VariantClear(variant);
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Execute a Windows `IDispatch::Invoke` call over the shared semantic COM request carrier and
/// return the classified semantic result shape.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// `resolve_object`, `query_dispatch_from_unknown`, and `add_ref_dispatch` must uphold the COM
/// ownership and identity rules for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_dispatch_variant_result<FResolveObject, FQueryUnknown, FAddRefDispatch>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
) -> Result<VariantResultValue, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                let value = value.to_com_value();
                if let Err(detail) = set_variant_from_com_value(
                    &mut variant,
                    &value,
                    resolve_object,
                    add_ref_dispatch,
                ) {
                    // VARIANT has no Drop: free the args marshalled so far
                    // (owned BSTRs/SAFEARRAYs/AddRef'd interfaces) before
                    // propagating (W1-com-001).
                    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
                    clear_variant_args(&mut invoke_args);
                    return Err(ComInvokeFailure {
                        label,
                        dispid,
                        hr: None,
                        arg_err: None,
                        excep: None,
                        detail: Some(detail),
                    });
                }
            }
            None => set_variant_missing_arg(&mut variant),
        }
        invoke_args.push(variant);
    }

    let mut named_arg_dispids_reversed: Vec<i32> =
        named_arg_dispids.iter().rev().copied().collect();
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if named_arg_dispids_reversed.is_empty() {
            std::ptr::null_mut()
        } else {
            named_arg_dispids_reversed.as_mut_ptr()
        },
        cArgs: args.len() as u32,
        cNamedArgs: named_arg_dispids.len() as u32,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }
    take_variant_result_value(&mut result, query_dispatch_from_unknown, add_ref_dispatch).map_err(
        |detail| ComInvokeFailure {
            label,
            dispid,
            hr: None,
            arg_err: None,
            excep: None,
            detail: Some(detail),
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Windows `IDispatch::Invoke` call that projects the result
/// into the retained Variant value shape, delegating any dispatch-backed
/// result rebinding to the caller.
///
/// New value-model call sites that do not need dispatch-backed rebinding
/// should prefer [`invoke_dispatch_variant_result`].
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object
/// handles or returned interface pointers they touch.
pub unsafe fn invoke_dispatch_variant<
    FResolveObject,
    FQueryUnknown,
    FAddRefDispatch,
    FBindDispatch,
>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    prog_id_hint: &str,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch: FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<Variant, String>,
{
    let mut result = std::mem::zeroed();
    let mut excep = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                let value = value.to_com_value();
                if let Err(detail) = set_variant_from_com_value(
                    &mut variant,
                    &value,
                    resolve_object,
                    add_ref_dispatch,
                ) {
                    // Free already-marshalled args before propagating (W1-com-001).
                    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
                    clear_variant_args(&mut invoke_args);
                    return Err(validation_failure(label, dispid, detail));
                }
            }
            None => set_variant_missing_arg(&mut variant),
        }
        invoke_args.push(variant);
    }
    let mut named_arg_dispids_reversed =
        named_arg_dispids.iter().copied().rev().collect::<Vec<_>>();
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if named_arg_dispids_reversed.is_empty() {
            std::ptr::null_mut()
        } else {
            named_arg_dispids_reversed.as_mut_ptr()
        },
        cArgs: args.len() as u32,
        cNamedArgs: named_arg_dispids.len() as u32,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }
    take_variant_result_variant(
        &mut result,
        query_dispatch_from_unknown,
        add_ref_dispatch,
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, op: &'static str| {
            bind_dispatch_result(dispatch, prog_id_hint, op)
        },
        prog_id_hint,
        "dispatch_invoke",
    )
    .map_err(|detail| ComInvokeFailure {
        label,
        dispid,
        hr: None,
        arg_err: None,
        excep: None,
        detail: Some(detail),
    })
}
fn validation_failure(
    label: &'static str,
    dispid: i32,
    detail: impl Into<String>,
) -> ComInvokeFailure {
    ComInvokeFailure {
        label,
        dispid,
        hr: None,
        arg_err: None,
        excep: None,
        detail: Some(detail.into()),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// `resolve_object` must return stable live `IDispatch` pointers for any object-handle arguments.
pub unsafe fn invoke_dispatch_legacy_i32_result<FResolveObject>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                let value = value.to_com_value();
                let mut add_ref_dispatch = |_dispatch: *mut core::ffi::c_void| {};
                if let Err(detail) = set_variant_from_com_value(
                    &mut variant,
                    &value,
                    resolve_object,
                    &mut add_ref_dispatch,
                ) {
                    // Free already-marshalled args before propagating (W1-com-001).
                    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
                    clear_variant_args(&mut invoke_args);
                    return Err(ComInvokeFailure {
                        label,
                        dispid,
                        hr: None,
                        arg_err: None,
                        excep: None,
                        detail: Some(detail),
                    });
                }
            }
            None => set_variant_missing_arg(&mut variant),
        }
        invoke_args.push(variant);
    }

    let mut named_arg_dispids_reversed: Vec<i32> =
        named_arg_dispids.iter().rev().copied().collect();
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if named_arg_dispids_reversed.is_empty() {
            std::ptr::null_mut()
        } else {
            named_arg_dispids_reversed.as_mut_ptr()
        },
        cArgs: args.len() as u32,
        cNamedArgs: named_arg_dispids.len() as u32,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }

    let token = match crate::variant_to_com_value(&result) {
        Ok(value) => match value.to_runtime_token() {
            Ok(token) => token,
            Err(detail) => {
                let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
                return Err(validation_failure(label, dispid, detail));
            }
        },
        Err(detail) => {
            let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
            return Err(validation_failure(label, dispid, detail));
        }
    };
    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// `resolve_object` must return stable live `IDispatch` pointers for any object-handle arguments.
pub unsafe fn invoke_dispatch_legacy_i32_result_positional<FResolveObject>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[crate::ComValue],
    property_put_named_arg: bool,
    label: &'static str,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        let mut add_ref_dispatch = |_dispatch: *mut core::ffi::c_void| {};
        if let Err(detail) =
            set_variant_from_com_value(&mut variant, arg, resolve_object, &mut add_ref_dispatch)
        {
            // Free already-marshalled args before propagating (W1-com-001).
            let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
            clear_variant_args(&mut invoke_args);
            return Err(validation_failure(label, dispid, detail));
        }
        invoke_args.push(variant);
    }

    let mut named_arg = crate::COM_DISPID_PROPERTYPUT;
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if property_put_named_arg {
            &mut named_arg
        } else {
            std::ptr::null_mut()
        },
        cArgs: args.len() as u32,
        cNamedArgs: if property_put_named_arg { 1 } else { 0 },
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }

    let token = match crate::variant_to_com_value(&result) {
        Ok(value) => match value.to_runtime_token() {
            Ok(token) => token,
            Err(detail) => {
                let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
                return Err(validation_failure(label, dispid, detail));
            }
        },
        Err(detail) => {
            let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
            return Err(validation_failure(label, dispid, detail));
        }
    };
    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// the callback closures must uphold COM ownership rules for resolved object arguments.
pub unsafe fn invoke_member_spec_legacy_i32_result<FResolveNamedArgDispids, FResolveObject>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    resolve_named_arg_dispids: &mut FResolveNamedArgDispids,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    let canonical_args;
    let args = match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyPut
        | crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            canonical_args = crate::invoke_policy::canonicalize_member_known_args(spec, args)
                .map_err(|detail| validation_failure("dispatch_invoke", dispid, detail))?;
            canonical_args.as_slice()
        }
        _ => args,
    };
    if spec.requires_argument && args.iter().all(|arg| arg.value.is_none()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "member requires argument but DispatchInvoke omitted the third argument",
        ));
    }
    if !spec.requires_argument {
        return match spec.invoke_kind {
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                &[],
                &[],
                "property-get",
                resolve_object,
            ),
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                &[],
                &[],
                "method",
                resolve_object,
            ),
            crate::TypeLibMemberInvokeKind::PropertyPut
            | crate::TypeLibMemberInvokeKind::PropertyPutRef => Err(validation_failure(
                "dispatch_invoke",
                dispid,
                "member requires argument for property put/putref dispatch",
            )),
        };
    }
    match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("property-get", dispid, detail))?;
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                args,
                &named_arg_dispids,
                "property-get",
                resolve_object,
            )
        }
        crate::TypeLibMemberInvokeKind::Method => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("method", dispid, detail))?;
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                args,
                &named_arg_dispids,
                "method",
                resolve_object,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPut => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-put", dispid, detail))?;
            if named_arg_dispids.is_empty()
                && args
                    .iter()
                    .all(|arg| arg.name.is_none() && arg.value.is_some())
            {
                let positional_args: Result<Vec<crate::ComValue>, String> = args
                    .iter()
                    .filter_map(|arg| arg.value.clone())
                    .map(|value| {
                        value.to_legacy_dispatch_token()?;
                        Ok(value.to_com_value())
                    })
                    .collect();
                if let Ok(positional_args) = positional_args {
                    return invoke_dispatch_legacy_i32_result_positional(
                        dispatch,
                        dispid,
                        DISPATCH_PROPERTYPUT,
                        &positional_args,
                        true,
                        "property-put",
                        resolve_object,
                    );
                }
            }
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUT,
                args,
                &all_named,
                "property-put",
                resolve_object,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-putref", dispid, detail))?;
            if named_arg_dispids.is_empty()
                && args
                    .iter()
                    .all(|arg| arg.name.is_none() && arg.value.is_some())
            {
                let positional_args: Result<Vec<crate::ComValue>, String> = args
                    .iter()
                    .filter_map(|arg| arg.value.clone())
                    .map(|value| {
                        value.to_legacy_dispatch_token()?;
                        Ok(value.to_com_value())
                    })
                    .collect();
                if let Ok(positional_args) = positional_args {
                    return invoke_dispatch_legacy_i32_result_positional(
                        dispatch,
                        dispid,
                        DISPATCH_PROPERTYPUTREF,
                        &positional_args,
                        true,
                        "property-putref",
                        resolve_object,
                    );
                }
            }
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUTREF,
                args,
                &all_named,
                "property-putref",
                resolve_object,
            )
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Member-metadata-backed Windows `IDispatch::Invoke` call.
///
/// Arguments are marshalled from retained `ComInvokeValue`/`Variant` payloads,
/// then the result is returned as retained [`Variant`] values.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_member_spec_variant<
    FResolveNamedArgDispids,
    FResolveObject,
    FQueryUnknown,
    FAddRefDispatch,
    FBindDispatch,
>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    resolve_named_arg_dispids: &mut FResolveNamedArgDispids,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch: FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<Variant, String>,
{
    let canonical_args;
    let args = match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyPut
        | crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            canonical_args = crate::invoke_policy::canonicalize_member_known_args(spec, args)
                .map_err(|detail| validation_failure("dispatch_invoke", dispid, detail))?;
            canonical_args.as_slice()
        }
        _ => args,
    };
    if spec.requires_argument && args.iter().all(|arg| arg.value.is_none()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "member requires argument but DispatchInvoke omitted the third argument",
        ));
    }
    if !spec.requires_argument {
        return match spec.invoke_kind {
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                &[],
                &[],
                "property-get",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                &[],
                &[],
                "method",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::PropertyPut
            | crate::TypeLibMemberInvokeKind::PropertyPutRef => Err(validation_failure(
                "dispatch_invoke",
                dispid,
                "member requires argument for property put/putref dispatch",
            )),
        };
    }
    match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("property-get", dispid, detail))?;
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                args,
                &named_arg_dispids,
                "property-get",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
        crate::TypeLibMemberInvokeKind::Method => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("method", dispid, detail))?;
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                args,
                &named_arg_dispids,
                "method",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPut => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-put", dispid, detail))?;
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUT,
                args,
                &all_named,
                "property-put",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-putref", dispid, detail))?;
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUTREF,
                args,
                &all_named,
                "property-putref",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Direct-DISPID Windows `IDispatch::Invoke` call.
///
/// Arguments are marshalled from retained `ComInvokeValue`/`Variant` payloads,
/// then the result is returned as retained [`Variant`] values.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_direct_dispid_variant<
    FResolveObject,
    FQueryUnknown,
    FAddRefDispatch,
    FBindDispatch,
>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    invoke_kind: crate::TypeLibMemberInvokeKind,
    requires_argument: bool,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch: FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<Variant, String>,
{
    if requires_argument && args.iter().all(|arg| arg.value.is_none()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "member requires argument but DispatchInvoke omitted the third argument",
        ));
    }
    if !requires_argument {
        return match invoke_kind {
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                &[],
                &[],
                "property-get",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                &[],
                &[],
                "method",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::PropertyPut
            | crate::TypeLibMemberInvokeKind::PropertyPutRef => Err(validation_failure(
                "dispatch_invoke",
                dispid,
                "member requires argument for property put/putref dispatch",
            )),
        };
    }
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "named arguments require a resolved COM member name and are unsupported for direct-DISPID dispatch",
        ));
    }
    match invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_PROPERTYGET,
            args,
            &[],
            "property-get",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
        crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_METHOD,
            args,
            &[],
            "method",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
        crate::TypeLibMemberInvokeKind::PropertyPut => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_PROPERTYPUT,
            args,
            &[crate::COM_DISPID_PROPERTYPUT],
            "property-put",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
        crate::TypeLibMemberInvokeKind::PropertyPutRef => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_PROPERTYPUTREF,
            args,
            &[crate::COM_DISPID_PROPERTYPUT],
            "property-putref",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// the callback closures must uphold COM ownership rules for resolved object arguments.
pub unsafe fn invoke_bound_dispatch_legacy_i32_result<
    FKnownSpec,
    FResolveNamedArgDispids,
    FResolveObject,
>(
    dispatch: *mut crate::RawIDispatch,
    member: crate::ComMemberToken,
    args: &[ComInvokeArg],
    known_member_spec: &mut FKnownSpec,
    resolve_named_arg_dispids: &mut FResolveNamedArgDispids,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FKnownSpec: FnMut(crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    if let Some(spec) = known_member_spec(member)
        .map_err(|detail| validation_failure("dispatch_invoke", member.raw(), detail))?
    {
        let dispid = crate::get_dispid_by_name(dispatch, &spec.name)
            .map_err(|detail| validation_failure("dispatch_invoke", member.raw(), detail))?;
        return invoke_member_spec_legacy_i32_result(
            dispatch.cast(),
            dispid,
            &spec,
            args,
            resolve_named_arg_dispids,
            resolve_object,
        );
    }
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(validation_failure(
            "dispatch_invoke",
            member.raw(),
            "named arguments require a resolved COM member name and remain unsupported for default-member/direct-DISPID dispatch",
        ));
    }
    invoke_dispatch_legacy_i32_result(
        dispatch.cast(),
        member.raw(),
        DISPATCH_PROPERTYGET,
        args,
        &[],
        "property-get",
        resolve_object,
    )
}

/// Re-flavor a resolved member spec as a property put / put-ref when the call
/// site carries that hint. A get/put-sharing dispid resolves to a single
/// (typically GET) spec; a `CompareMode = x` / `Set Item(k) = obj` request must
/// still dispatch a PROPERTYPUT / PROPERTYPUTREF. Overriding `invoke_kind` (and
/// forcing `requires_argument`, since a put always supplies the value) keeps the
/// existing put marshaling in `invoke_member_spec_variant` (which keys off
/// `spec.invoke_kind`) honest. A method / get hint (or none) leaves the spec as
/// resolved.
/// Map a call-site invoke-kind hint to the [`TypeLibMemberInvokeKind`] a member
/// spec is resolved under. A put / put-ref hint selects the matching write FUNCDESC;
/// anything else (get / method / no hint) resolves on the read side (the spec lookup
/// falls back from `PropertyGet` to `Method`).
#[cfg(target_os = "windows")]
fn intended_invoke_kind(hint: Option<crate::ComInvokeKind>) -> crate::TypeLibMemberInvokeKind {
    match hint {
        Some(crate::ComInvokeKind::PropertyPut) => crate::TypeLibMemberInvokeKind::PropertyPut,
        Some(crate::ComInvokeKind::PropertyPutRef) => {
            crate::TypeLibMemberInvokeKind::PropertyPutRef
        }
        _ => crate::TypeLibMemberInvokeKind::PropertyGet,
    }
}

#[cfg(target_os = "windows")]
fn apply_put_hint(
    mut spec: crate::ComMemberSpec,
    hint: Option<crate::ComInvokeKind>,
) -> crate::ComMemberSpec {
    let put_kind = match hint {
        Some(crate::ComInvokeKind::PropertyPut) => crate::TypeLibMemberInvokeKind::PropertyPut,
        Some(crate::ComInvokeKind::PropertyPutRef) => {
            crate::TypeLibMemberInvokeKind::PropertyPutRef
        }
        _ => return spec,
    };
    // Invoke-kind-keyed resolution already handed us the dedicated put/putref
    // spec — with its own vtable slot and ABI param shape (index params PLUS the
    // trailing value). Trust it: re-flavoring it as the same put kind is a no-op,
    // and clearing its slot/params would needlessly forfeit the vtable fast path.
    if spec.invoke_kind == put_kind && spec.vtable_slot.is_some() {
        spec.requires_argument = true;
        return spec;
    }
    spec.invoke_kind = put_kind;
    spec.requires_argument = true;
    // The resolved spec describes the GET (the get/put-sharing dispid's canonical
    // member): its `vtable_slot` / `parameter_types` / `parameter_names` /
    // `return_type` are the GET's, not the PUT's. Calling the GET slot as a PUT
    // would be a slot mismatch, so clear the slot to decline the vtable fast path;
    // the IDispatch PROPERTYPUT path dispatches the put correctly by dispid alone.
    spec.vtable_slot = None;
    // The PUT carries the GET's index params PLUS a trailing implicit value param
    // the GET's metadata does not enumerate. Clearing the parameter metadata skips
    // the `canonicalize_member_known_args` arity check (which would reject the value
    // arg as one-too-many) and lets the IDispatch PROPERTYPUT path pass the
    // positional `index… , value` list straight through.
    spec.parameter_names.clear();
    spec.parameter_types.clear();
    spec.parameter_optional_defaults.clear();
    spec
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
pub fn execute_bound_variant<
    FTryVtable,
    FResolveMember,
    FInvokeMember,
    FInvokeDirect,
    FInvokeBound,
>(
    binding: &ComBinding,
    request: &ComInvokeRequest,
    cached_dispid: Option<i32>,
    try_vtable_invoke: &mut FTryVtable,
    resolve_member_dispid: &mut FResolveMember,
    invoke_member_spec: &mut FInvokeMember,
    invoke_direct_dispid: &mut FInvokeDirect,
    invoke_bound_dispatch: &mut FInvokeBound,
) -> Result<Variant, String>
where
    FTryVtable: FnMut(i32, &[i32]) -> Result<Option<i32>, String>,
    FResolveMember: FnMut(
        i32,
        crate::TypeLibMemberInvokeKind,
        Option<i32>,
    ) -> Result<Option<(i32, crate::ComMemberSpec)>, String>,
    FInvokeMember:
        FnMut(i32, &crate::ComMemberSpec, &[ComInvokeArg], &str) -> Result<Variant, String>,
    FInvokeDirect: FnMut(
        i32,
        crate::TypeLibMemberInvokeKind,
        bool,
        &[ComInvokeArg],
        &str,
    ) -> Result<Variant, String>,
    FInvokeBound: FnMut(i32, &[ComInvokeArg], &str) -> Result<Variant, String>,
{
    let plan = crate::plan_bound_runtime_invoke(binding, request, cached_dispid)?;
    let effective_member = plan.effective_member;
    let effective_cached_dispid = plan.effective_cached_dispid;
    let named_default_member_spec = plan.named_default_member_spec;
    let direct_dispatch_spec = plan.direct_dispatch_spec;
    let legacy_vtable_candidate_args = plan.legacy_vtable_candidate_args;
    let args = request.args.as_slice();
    // The access kind this call intends, so member-spec resolution selects the
    // matching get / let / set FUNCDESC (each with its own vtable slot) rather
    // than collapsing a read/write property to a single spec.
    let intended_kind = intended_invoke_kind(request.invoke_kind_hint);

    if let Some(positional_values) = legacy_vtable_candidate_args.as_ref()
        && let Some(value) = try_vtable_invoke(effective_member.raw(), positional_values)?
    {
        return Ok(Variant::from_i32(value));
    }

    if let Some((token, spec)) = named_default_member_spec {
        let (dispid, spec) =
            resolve_member_dispid(token.raw(), intended_kind, effective_cached_dispid)?
                .map(|(dispid, _)| (dispid, spec))
                .ok_or_else(|| {
                    "default member identity unavailable for named late-bound dispatch".to_string()
                })?;
        let spec = apply_put_hint(spec, request.invoke_kind_hint);
        return invoke_member_spec(dispid, &spec, args, &binding.prog_id_name);
    }

    if let Some((dispid, spec)) = resolve_member_dispid(
        effective_member.raw(),
        intended_kind,
        effective_cached_dispid,
    )? {
        // When the put/set FUNCDESC has its own spec (invoke-kind-keyed), the
        // resolver already returned it with the correct slot+ABI; `apply_put_hint`
        // is then a no-op. When only a GET spec exists (the typelib has no separate
        // put metadata), `apply_put_hint` re-flavors it as a PROPERTYPUT/PUTREF and
        // declines the vtable so the IDispatch put path takes effect — otherwise the
        // write would be silently demoted to a read.
        let spec = apply_put_hint(spec, request.invoke_kind_hint);
        return invoke_member_spec(dispid, &spec, args, &binding.prog_id_name);
    }

    if let Some(spec) = direct_dispatch_spec {
        return invoke_direct_dispid(
            effective_member.raw(),
            spec.invoke_kind,
            spec.requires_argument,
            args,
            &binding.prog_id_name,
        );
    }

    match request.invoke_kind_hint {
        Some(crate::ComInvokeKind::PropertyPut) => {
            return invoke_direct_dispid(
                effective_member.raw(),
                crate::TypeLibMemberInvokeKind::PropertyPut,
                true,
                args,
                &binding.prog_id_name,
            );
        }
        Some(crate::ComInvokeKind::PropertyPutRef) => {
            return invoke_direct_dispid(
                effective_member.raw(),
                crate::TypeLibMemberInvokeKind::PropertyPutRef,
                true,
                args,
                &binding.prog_id_name,
            );
        }
        _ => {}
    }

    invoke_bound_dispatch(effective_member.raw(), args, &binding.prog_id_name)
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
pub fn execute_bound_runtime_call_result<
    FTryVtable,
    FResolveMember,
    FInvokeMember,
    FInvokeDirect,
    FInvokeBound,
>(
    binding: &ComBinding,
    request: &ComInvokeRequest,
    cached_dispid: Option<i32>,
    try_vtable_invoke: &mut FTryVtable,
    resolve_member_dispid: &mut FResolveMember,
    invoke_member_spec: &mut FInvokeMember,
    invoke_direct_dispid: &mut FInvokeDirect,
    invoke_bound_dispatch: &mut FInvokeBound,
) -> Result<RuntimeCallResult, String>
where
    FTryVtable: FnMut(i32, &[i32]) -> Result<Option<i32>, String>,
    FResolveMember: FnMut(
        i32,
        crate::TypeLibMemberInvokeKind,
        Option<i32>,
    ) -> Result<Option<(i32, crate::ComMemberSpec)>, String>,
    FInvokeMember: FnMut(
        i32,
        &crate::ComMemberSpec,
        &[ComInvokeArg],
        &str,
    ) -> Result<RuntimeCallResult, String>,
    FInvokeDirect: FnMut(
        i32,
        crate::TypeLibMemberInvokeKind,
        bool,
        &[ComInvokeArg],
        &str,
    ) -> Result<RuntimeCallResult, String>,
    FInvokeBound: FnMut(i32, &[ComInvokeArg], &str) -> Result<RuntimeCallResult, String>,
{
    let plan = crate::plan_bound_runtime_invoke(binding, request, cached_dispid)?;
    let effective_member = plan.effective_member;
    let effective_cached_dispid = plan.effective_cached_dispid;
    let named_default_member_spec = plan.named_default_member_spec;
    let direct_dispatch_spec = plan.direct_dispatch_spec;
    let legacy_vtable_candidate_args = plan.legacy_vtable_candidate_args;
    let args = request.args.as_slice();
    let intended_kind = intended_invoke_kind(request.invoke_kind_hint);

    if let Some(positional_values) = legacy_vtable_candidate_args.as_ref()
        && let Some(value) = try_vtable_invoke(effective_member.raw(), positional_values)?
    {
        return Ok(RuntimeCallResult::value(Variant::from_i32(value)));
    }

    if let Some((token, spec)) = named_default_member_spec {
        let (dispid, spec) =
            resolve_member_dispid(token.raw(), intended_kind, effective_cached_dispid)?
                .map(|(dispid, _)| (dispid, spec))
                .ok_or_else(|| {
                    "default member identity unavailable for named late-bound dispatch".to_string()
                })?;
        let spec = apply_put_hint(spec, request.invoke_kind_hint);
        return invoke_member_spec(dispid, &spec, args, &binding.prog_id_name);
    }

    if let Some((dispid, spec)) = resolve_member_dispid(
        effective_member.raw(),
        intended_kind,
        effective_cached_dispid,
    )? {
        let spec = apply_put_hint(spec, request.invoke_kind_hint);
        return invoke_member_spec(dispid, &spec, args, &binding.prog_id_name);
    }

    if let Some(spec) = direct_dispatch_spec {
        return invoke_direct_dispid(
            effective_member.raw(),
            spec.invoke_kind,
            spec.requires_argument,
            args,
            &binding.prog_id_name,
        );
    }

    match request.invoke_kind_hint {
        Some(crate::ComInvokeKind::PropertyPut) => {
            return invoke_direct_dispid(
                effective_member.raw(),
                crate::TypeLibMemberInvokeKind::PropertyPut,
                true,
                args,
                &binding.prog_id_name,
            );
        }
        Some(crate::ComInvokeKind::PropertyPutRef) => {
            return invoke_direct_dispid(
                effective_member.raw(),
                crate::TypeLibMemberInvokeKind::PropertyPutRef,
                true,
                args,
                &binding.prog_id_name,
            );
        }
        _ => {}
    }

    invoke_bound_dispatch(effective_member.raw(), args, &binding.prog_id_name)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn invoke_member_spec_variant_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Variant, ComInvokeFailure> {
    invoke_member_spec_variant(
        dispatch,
        dispid,
        spec,
        args,
        prog_id_hint,
        &mut |member_name, args| {
            crate::resolve_named_argument_dispids(
                dispatch.cast::<crate::RawIDispatch>(),
                member_name,
                args,
            )
        },
        &mut |handle| {
            crate::resolve_bound_native_dispatch_shared(com_state, handle)
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |unknown: *mut core::ffi::c_void| {
            crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |dispatch: *mut core::ffi::c_void| {
            crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        },
        // SAFETY: take_variant_result_variant hands this callback a pointer
        // that is null or carries the one reference retained via
        // add_ref_dispatch before the result VARIANT was cleared; ownership of
        // that reference transfers to the bindings map, satisfying
        // bind_native_runtime_object_result_shared's contract.
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(Variant::from_object_ref)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn invoke_direct_dispid_variant_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    invoke_kind: crate::TypeLibMemberInvokeKind,
    requires_argument: bool,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Variant, ComInvokeFailure> {
    invoke_direct_dispid_variant(
        dispatch,
        dispid,
        invoke_kind,
        requires_argument,
        args,
        prog_id_hint,
        &mut |handle| {
            crate::resolve_bound_native_dispatch_shared(com_state, handle)
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |unknown: *mut core::ffi::c_void| {
            crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |dispatch: *mut core::ffi::c_void| {
            crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        },
        // SAFETY: take_variant_result_variant hands this callback a pointer
        // that is null or carries the one reference retained via
        // add_ref_dispatch before the result VARIANT was cleared; ownership of
        // that reference transfers to the bindings map, satisfying
        // bind_native_runtime_object_result_shared's contract.
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(Variant::from_object_ref)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc,
    clippy::too_many_arguments
)]
pub unsafe fn invoke_dispatch_variant_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Variant, ComInvokeFailure> {
    invoke_dispatch_variant(
        dispatch,
        dispid,
        flags,
        args,
        named_arg_dispids,
        label,
        prog_id_hint,
        &mut |handle| {
            crate::resolve_bound_native_dispatch_shared(com_state, handle)
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |unknown: *mut core::ffi::c_void| {
            crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |dispatch: *mut core::ffi::c_void| {
            crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        },
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(Variant::from_object_ref)
        },
    )
}

/// The standard OLE Automation enumerator DISPID (`DISPID_NEWENUM`). Invoking it
/// returns the collection's `IEnumVARIANT` (wrapped in a `VT_UNKNOWN`).
#[cfg(target_os = "windows")]
pub const COM_DISPID_NEWENUM: i32 = -4;

/// Snapshot a bound COM collection's elements for VBA `For Each` (BUG 3).
///
/// Resolves the bound native `IDispatch` for `object`, invokes `DISPID_NEWENUM`
/// (`-4`) with `DISPATCH_METHOD | DISPATCH_PROPERTYGET` to obtain the
/// collection's enumerator, and drives `IEnumVARIANT::Next` to completion. The
/// enumerator result returns as a `VT_UNKNOWN` whose decode path
/// (`take_variant_result_variant` → `unknown_to_variant_value`) already
/// `QueryInterface`s `IEnumVARIANT`, materializes every element into a
/// SAFEARRAY-backed Variant, and binds object-valued elements as `ObjectRef`s —
/// releasing the enumerator and each interim reference. We then unwrap that
/// SAFEARRAY into the element `Vec`.
///
/// AV-safety: the decode QI-guards `IEnumVARIANT`; an object that exposes no
/// enumerator surfaces a clean error (no enumerator interface), which the caller
/// (vm2) treats as an empty/non-enumerable `For Each` rather than touching an
/// unverified pointer.
///
/// A non-collection scalar result (e.g. a server that answers `DISPID_NEWENUM`
/// with a number) decodes to a non-array Variant; we surface a clean error in
/// that case so the caller does not iterate a single scalar as one element.
///
/// # Safety
/// The bridge's bindings map must own one retained `IDispatch` reference for
/// `object` for the duration of the call (the standard adapter guarantees this
/// from the VM thread that owns the binding), and the current thread must be
/// COM-initialized.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn enumerate_object_with_shared_state(
    object: ObjectRef,
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Vec<Variant>, String> {
    // Resolve the live bound dispatch; a projection-only binding (native == 0) or
    // an unknown handle has nothing to enumerate.
    let dispatch = crate::resolve_bound_native_dispatch_shared(com_state, object)?;
    if dispatch.is_null() {
        return Err(format!(
            "COM-E-ENUM-NO-DISPATCH: object `{prog_id_hint}` has no live IDispatch to enumerate"
        ));
    }
    let result = invoke_dispatch_variant_with_shared_state(
        dispatch.cast(),
        COM_DISPID_NEWENUM,
        DISPATCH_METHOD | DISPATCH_PROPERTYGET,
        &[],
        &[],
        "for-each-newenum",
        prog_id_hint,
        com_state,
    )
    .map_err(|failure| render_invoke_fault_message(&failure))?;
    match result.as_safearray() {
        Some(array) => Ok(array.variant_elements().unwrap_or_default()),
        None => Err(format!(
            "COM-E-ENUM-NOT-COLLECTION: object `{prog_id_hint}` DISPID_NEWENUM did not yield an enumerable collection"
        )),
    }
}

#[cfg(target_os = "windows")]
#[allow(clippy::missing_safety_doc)]
pub unsafe fn invoke_bound_dispatch_variant_with_shared_state<FKnownSpec>(
    dispatch: *mut crate::RawIDispatch,
    prog_id: &str,
    member: crate::ComMemberToken,
    args: &[ComInvokeArg],
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
    known_member_spec: &mut FKnownSpec,
) -> Result<Variant, String>
where
    FKnownSpec:
        FnMut(&ComBinding, crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
{
    let plan = crate::plan_unbound_runtime_invoke(
        member,
        args,
        known_member_spec(
            &ComBinding::new(prog_id.to_string(), dispatch as usize),
            member,
        )?,
    )?;
    match plan {
        crate::UnboundRuntimeInvokePlan::MemberSpec(spec) => {
            // SAFETY: this unsafe fn's contract requires `dispatch` to be a
            // live IDispatch for the duration of the call; the in-module
            // caller passes the pointer recovered from a live binding entry,
            // which the bindings map keeps retained.
            let dispid = unsafe { crate::get_dispid_by_name(dispatch, &spec.name) }?;
            // SAFETY: same live, bindings-map-retained `dispatch` as the
            // dispid lookup above; the shared-state helper installs callbacks
            // that uphold COM retention rules for arguments and results.
            unsafe {
                invoke_member_spec_variant_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    &spec,
                    args,
                    prog_id,
                    com_state,
                )
            }
            .map_err(|failure| render_invoke_fault_message(&failure))
        }
        // SAFETY: this unsafe fn's contract requires `dispatch` to be a live
        // IDispatch (the in-module caller passes the bindings-map-retained
        // pointer); the shared-state helper installs callbacks that uphold COM
        // retention rules for arguments and results.
        crate::UnboundRuntimeInvokePlan::DirectGetOrCall { dispid } => unsafe {
            // Invoke kind is unknown for this trusted dispid, so let the server choose
            // between a method call and a property read. DAO/Jet strictly reject a method
            // dispatched as a bare property-get; the combined flag is what real Automation
            // clients (incl. VBA) issue when the call could be either.
            invoke_dispatch_variant_with_shared_state(
                dispatch.cast(),
                dispid.raw(),
                DISPATCH_METHOD | DISPATCH_PROPERTYGET,
                args,
                &[],
                "get-or-call",
                prog_id,
                com_state,
            )
        }
        .map_err(|failure| render_invoke_fault_message(&failure)),
    }
}

/// Whether a FUNCDESC parameter/return VARTYPE is in the vtable marshalling set
/// (the exact semantic shapes [`crate::windows_vtable::vtable_invoke`] marshals).
/// Wire metadata admits additional layouts such as explicit SAFEARRAY pointers;
/// semantic VARTYPEs outside this set still gate the call to the IDispatch
/// fallback rather than risking a wrong-ABI vtable call.
#[cfg(target_os = "windows")]
pub fn is_v1_vtable_vartype(param_type: crate::TypeLibParamType) -> bool {
    param_type.supports_vtable_param_abi()
}

#[cfg(target_os = "windows")]
fn is_vtable_return_vartype(param_type: crate::TypeLibParamType) -> bool {
    param_type.supports_vtable_return_abi()
}

/// Per-bridge transport counters incremented at the exact return sites of a
/// successful early-bound member call: a vtable success bumps `vtable`, an
/// IDispatch `Invoke` success bumps `idispatch`. A host test reads these to
/// prove which transport carried a given member (the LAST call in a live script
/// — e.g. `app.Quit` — is not the asserted member, so a counter, not a
/// last-transport snapshot, is what lets the assertion target a specific call).
#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
pub struct ComTransportCounters<'a> {
    pub vtable: &'a std::sync::atomic::AtomicU64,
    pub idispatch: &'a std::sync::atomic::AtomicU64,
}

#[cfg(target_os = "windows")]
impl ComTransportCounters<'_> {
    fn record_vtable(&self) {
        self.vtable
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_idispatch(&self) {
        self.idispatch
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VtableDeclineReason {
    MissingSlot,
    ReservedComSlot,
    NotDualInterface,
    MissingSlotBound,
    SlotOutOfBounds,
    MissingInterfaceIid,
    NonStdcall,
    PropertyPutRefDeferred,
    MissingByRefSlot,
    ByRefSlotTypeMismatch,
    TooManyArgs,
    UnsynthesizableTrailingArgs,
    ScalarMethodReturnWithSynthesizedArgs,
    UnsupportedParameterType(crate::TypeLibParamType),
    UnsupportedReturnType(crate::TypeLibParamType),
    ParameterWireTypeArityMismatch,
    UnsupportedParameterWireType,
    UnsupportedReturnWireType,
    MissingObjectParameterIid,
    MissingRecordParameterWireType,
    MissingRecordReturnInfo,
    MissingRecordSafeArrayElementInfo,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VtableInvocationPlan {
    pub(crate) slot: u16,
    pub(crate) slot_bound: u16,
    pub(crate) interface_iid: crate::ComInterfaceIid,
    pub(crate) parameter_types: Vec<crate::TypeLibParamType>,
    pub(crate) parameter_wire_types: Vec<crate::TypeLibWireType>,
    pub(crate) parameter_iids: Vec<Option<crate::ComInterfaceIid>>,
    pub(crate) parameter_byref_slots: Vec<Option<RuntimeByRefSlot>>,
    pub(crate) return_type: Option<crate::TypeLibParamType>,
    pub(crate) return_wire_type: Option<crate::TypeLibWireType>,
    pub(crate) invoke_kind: crate::TypeLibMemberInvokeKind,
    pub(crate) label: &'static str,
}

#[cfg(all(target_os = "windows", test))]
fn build_vtable_invocation_plan(
    spec: &crate::ComMemberSpec,
    positional_arg_count: usize,
    return_type: Option<crate::TypeLibParamType>,
    label: &'static str,
) -> Result<VtableInvocationPlan, VtableDeclineReason> {
    let byref_slots = vec![None; positional_arg_count];
    build_vtable_invocation_plan_with_byrefs(
        spec,
        positional_arg_count,
        &byref_slots,
        None,
        return_type,
        label,
    )
}

#[cfg(target_os = "windows")]
fn build_vtable_invocation_plan_with_byrefs(
    spec: &crate::ComMemberSpec,
    positional_arg_count: usize,
    supplied_byref_slots: &[Option<RuntimeByRefSlot>],
    supplied_values: Option<&[Variant]>,
    return_type: Option<crate::TypeLibParamType>,
    label: &'static str,
) -> Result<VtableInvocationPlan, VtableDeclineReason> {
    let Some(slot) = spec.vtable_slot else {
        return Err(VtableDeclineReason::MissingSlot);
    };
    // Never vtable-call an IUnknown (0..=2) or IDispatch (3..=6) slot.
    if slot < 7 {
        return Err(VtableDeclineReason::ReservedComSlot);
    }
    // A vtable slot is only callable when sourced from a real custom INTERFACE
    // (FDUAL + TKIND_INTERFACE). A pure dispinterface member must NOT be slot-called.
    if !spec.is_dual || spec.source_typekind != Some(crate::SourceTypeKind::Interface) {
        return Err(VtableDeclineReason::NotDualInterface);
    }
    // AV-SAFETY NET: the slot must be in bounds of the source INTERFACE's live
    // vtable (cbSizeVft/8). A missing bound or an out-of-range slot declines.
    let slot_bound = match spec.vtable_slot_bound {
        Some(bound) if slot < bound => bound,
        Some(_) => return Err(VtableDeclineReason::SlotOutOfBounds),
        None => return Err(VtableDeclineReason::MissingSlotBound),
    };
    // A usable dual interface IID is mandatory; it is the QueryInterface target.
    let interface_iid = match spec.interface_iid {
        Some(iid) if !iid.is_null() => iid,
        _ => return Err(VtableDeclineReason::MissingInterfaceIid),
    };
    if !spec.callconv_is_stdcall {
        return Err(VtableDeclineReason::NonStdcall);
    }
    if spec.invoke_kind == crate::TypeLibMemberInvokeKind::PropertyPutRef
        && !is_supported_vtable_putref_shape(spec, return_type)
    {
        return Err(VtableDeclineReason::PropertyPutRefDeferred);
    }
    if positional_arg_count > spec.parameter_types.len() {
        return Err(VtableDeclineReason::TooManyArgs);
    }
    if supplied_byref_slots.len() != positional_arg_count {
        return Err(VtableDeclineReason::MissingByRefSlot);
    }
    if positional_arg_count < spec.parameter_types.len() {
        if !trailing_optionals_are_synthesizable(spec, positional_arg_count) {
            return Err(VtableDeclineReason::UnsynthesizableTrailingArgs);
        }
        if spec.invoke_kind == crate::TypeLibMemberInvokeKind::Method
            && matches!(
                return_type,
                Some(
                    crate::TypeLibParamType::Long
                        | crate::TypeLibParamType::Integer
                        | crate::TypeLibParamType::Byte
                        | crate::TypeLibParamType::LongLong
                )
            )
        {
            return Err(VtableDeclineReason::ScalarMethodReturnWithSynthesizedArgs);
        }
    }
    if let Some(param_type) = spec
        .parameter_types
        .iter()
        .find(|param_type| !is_v1_vtable_vartype(**param_type))
    {
        return Err(VtableDeclineReason::UnsupportedParameterType(*param_type));
    }
    if let Some(rt) = return_type
        && !is_vtable_return_vartype(rt)
    {
        return Err(VtableDeclineReason::UnsupportedReturnType(rt));
    }
    if let Err(issue) = crate::typelib::validate_vtable_wire_signature(
        &spec.parameter_types,
        &spec.parameter_wire_types,
        &spec.parameter_iids,
        return_type,
        spec.return_wire_type.as_ref(),
    ) {
        return Err(match issue {
            crate::typelib::TypeLibVtableSignatureIssue::UnsupportedParameterType(param_type) => {
                VtableDeclineReason::UnsupportedParameterType(param_type)
            }
            crate::typelib::TypeLibVtableSignatureIssue::UnsupportedReturnType(return_type) => {
                VtableDeclineReason::UnsupportedReturnType(return_type)
            }
            crate::typelib::TypeLibVtableSignatureIssue::ParameterWireTypeArityMismatch => {
                VtableDeclineReason::ParameterWireTypeArityMismatch
            }
            crate::typelib::TypeLibVtableSignatureIssue::UnsupportedParameterWireType => {
                VtableDeclineReason::UnsupportedParameterWireType
            }
            crate::typelib::TypeLibVtableSignatureIssue::UnsupportedReturnWireType => {
                VtableDeclineReason::UnsupportedReturnWireType
            }
            crate::typelib::TypeLibVtableSignatureIssue::MissingObjectParameterIid => {
                VtableDeclineReason::MissingObjectParameterIid
            }
            crate::typelib::TypeLibVtableSignatureIssue::MissingRecordParameterWireType => {
                VtableDeclineReason::MissingRecordParameterWireType
            }
            crate::typelib::TypeLibVtableSignatureIssue::MissingRecordReturnInfo => {
                VtableDeclineReason::MissingRecordReturnInfo
            }
        });
    }
    if let Some(values) = supplied_values {
        if values.len() != positional_arg_count {
            return Err(VtableDeclineReason::TooManyArgs);
        }
        if supplied_record_safearray_missing_record_info(
            &spec.parameter_wire_types,
            values,
            positional_arg_count,
        ) {
            return Err(VtableDeclineReason::MissingRecordSafeArrayElementInfo);
        }
    }
    let mut parameter_byref_slots = vec![None; spec.parameter_types.len()];
    for (index, (param_type, wire_type)) in spec
        .parameter_types
        .iter()
        .zip(
            spec.parameter_wire_types
                .iter()
                .map(Some)
                .chain(std::iter::repeat(None)),
        )
        .take(spec.parameter_types.len())
        .enumerate()
    {
        let requires_byref_slot = param_type.is_by_ref()
            || wire_type.is_some_and(crate::TypeLibWireType::is_byref_safearray_wire);
        if requires_byref_slot {
            let Some(Some(slot)) = supplied_byref_slots.get(index) else {
                return Err(VtableDeclineReason::MissingByRefSlot);
            };
            let expected_type =
                if wire_type.is_some_and(crate::TypeLibWireType::is_byref_safearray_wire) {
                    Some(RuntimeValueType::Variant)
                } else {
                    expected_runtime_type_for_byref(*param_type)
                };
            if let Some(expected_type) = expected_type
                && slot
                    .expected_type
                    .is_some_and(|actual| actual != expected_type)
            {
                return Err(VtableDeclineReason::ByRefSlotTypeMismatch);
            }
            parameter_byref_slots[index] = Some(*slot);
        }
    }
    Ok(VtableInvocationPlan {
        slot,
        slot_bound,
        interface_iid,
        parameter_types: spec.parameter_types.clone(),
        parameter_wire_types: spec.parameter_wire_types.clone(),
        parameter_iids: spec.parameter_iids.clone(),
        parameter_byref_slots,
        return_type,
        return_wire_type: spec.return_wire_type.clone(),
        invoke_kind: spec.invoke_kind,
        label,
    })
}

#[cfg(target_os = "windows")]
fn supplied_record_safearray_missing_record_info(
    wire_types: &[crate::TypeLibWireType],
    values: &[Variant],
    positional_arg_count: usize,
) -> bool {
    for (wire_type, value) in wire_types
        .iter()
        .zip(values.iter())
        .take(positional_arg_count)
    {
        let is_record_array = matches!(
            wire_type,
            crate::TypeLibWireType::SafeArray { element_vt: 36, .. }
                | crate::TypeLibWireType::ByRefSafeArray { element_vt: 36, .. }
        );
        if !is_record_array {
            continue;
        }
        let descriptor_record_info = match wire_type {
            crate::TypeLibWireType::SafeArray { record_info, .. }
            | crate::TypeLibWireType::ByRefSafeArray { record_info, .. } => record_info.as_ref(),
            _ => None,
        };
        let Some(array) = value.as_safearray() else {
            return true;
        };
        if array.is_empty() && descriptor_record_info.is_some() {
            continue;
        }
        let Some(elements) = array.variant_elements() else {
            return true;
        };
        if elements
            .iter()
            .any(|element| element.as_com_record().is_none())
        {
            return true;
        }
        let mut records = elements
            .iter()
            .map(|element| element.as_com_record().expect("validated record element"));
        let Some(first_record) = records.next() else {
            return !elements.is_empty() || descriptor_record_info.is_none();
        };
        if let Some(descriptor_record_info) = descriptor_record_info
            && !descriptor_record_info_matches_record(descriptor_record_info, &first_record)
        {
            return true;
        }
        let first_record_info = first_record.record_info_ptr();
        if records.any(|record| {
            // SAFETY: both pointers come from live runtime ComRecord values cloned
            // from the supplied argument array and are only queried for type
            // identity.
            unsafe {
                !crate::windows_variant::record_info_matches(
                    first_record_info,
                    record.record_info_ptr(),
                )
            }
        }) {
            return true;
        };
    }
    false
}

#[cfg(target_os = "windows")]
fn descriptor_record_info_matches_record(
    descriptor: &crate::TypeLibRecordInfo,
    record: &oxvba_runtime::ComRecord,
) -> bool {
    let raw = crate::windows_variant::record_info_from_descriptor(descriptor);
    let Ok(raw) = raw else {
        return false;
    };
    // SAFETY: `raw` is an owned IRecordInfo reference from OleAut and
    // `record.record_info_ptr()` is the live record type identity.
    let matches =
        unsafe { crate::windows_variant::record_info_matches(raw, record.record_info_ptr()) };
    // SAFETY: balances the owned IRecordInfo reference returned by OleAut.
    unsafe { crate::windows_variant::release_record_info_for_descriptor(raw) };
    matches
}

#[cfg(target_os = "windows")]
fn expected_runtime_type_for_byref(
    param_type: crate::TypeLibParamType,
) -> Option<RuntimeValueType> {
    match param_type {
        crate::TypeLibParamType::ByRefVariant => Some(RuntimeValueType::Variant),
        crate::TypeLibParamType::ByRefLong => Some(RuntimeValueType::Long),
        crate::TypeLibParamType::ByRefInteger => Some(RuntimeValueType::Integer),
        crate::TypeLibParamType::ByRefDouble => Some(RuntimeValueType::Double),
        crate::TypeLibParamType::ByRefSingle => Some(RuntimeValueType::Single),
        crate::TypeLibParamType::ByRefCurrency => Some(RuntimeValueType::Currency),
        crate::TypeLibParamType::ByRefDate => Some(RuntimeValueType::Date),
        crate::TypeLibParamType::ByRefDecimal => Some(RuntimeValueType::Decimal),
        crate::TypeLibParamType::ByRefString => Some(RuntimeValueType::String),
        crate::TypeLibParamType::ByRefObject => Some(RuntimeValueType::Object),
        crate::TypeLibParamType::ByRefLongPtr => Some(RuntimeValueType::LongPtr),
        crate::TypeLibParamType::ByRefByte => Some(RuntimeValueType::Byte),
        crate::TypeLibParamType::ByRefBoolean => Some(RuntimeValueType::Boolean),
        crate::TypeLibParamType::ByRefLongLong => Some(RuntimeValueType::LongLong),
        crate::TypeLibParamType::ByRefRecord => Some(RuntimeValueType::Record),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn is_supported_vtable_putref_shape(
    spec: &crate::ComMemberSpec,
    return_type: Option<crate::TypeLibParamType>,
) -> bool {
    if !(return_type.is_none()
        && spec.return_type.is_none()
        && spec.return_wire_type.is_none()
        && spec.parameter_types.len() == 1)
    {
        return false;
    }
    if spec.parameter_types.as_slice() == [crate::TypeLibParamType::Object] {
        return matches!(
            spec.parameter_wire_types.as_slice(),
            [crate::TypeLibWireType::InterfacePointer { .. }]
        ) && spec.parameter_iids.len() == 1
            && spec
                .parameter_iids
                .first()
                .copied()
                .flatten()
                .is_some_and(|iid| !iid.is_null());
    }
    true
}

/// True when every declared parameter from `supplied_count..` (the ones the guest
/// did not supply) is one the vtable dispatch site can synthesize: a typelib
/// default ([`OptionalParamDefault::HasDefault`]), an optional VARIANT
/// ([`OptionalParamDefault::OptionalVariant`]), or a hidden `[lcid]`
/// ([`OptionalParamDefault::Lcid`], always synthesized with `LOCALE_NEUTRAL`). A
/// [`Required`] or an [`OptionalNoDefault`] in the missing tail — or a metadata
/// source that carries no `parameter_optional_defaults` at all (fixture/catalog)
/// — declines, so a member with no synthesis metadata keeps the pre-D3
/// exact-arity behavior.
#[cfg(target_os = "windows")]
fn trailing_optionals_are_synthesizable(
    spec: &crate::ComMemberSpec,
    supplied_count: usize,
) -> bool {
    // Without per-parameter synthesis rules we cannot prove the tail is droppable.
    if spec.parameter_optional_defaults.len() != spec.parameter_types.len() {
        return false;
    }
    spec.parameter_optional_defaults[supplied_count..]
        .iter()
        .all(|rule| {
            matches!(
                rule,
                crate::OptionalParamDefault::HasDefault(_)
                    | crate::OptionalParamDefault::OptionalVariant
                    | crate::OptionalParamDefault::Lcid
            )
        })
}

/// Synthesize the trailing arguments (declared order) the guest did not supply for
/// a vtable slot call: a `HasDefault` rule materializes its typelib default value,
/// an `OptionalVariant` rule materializes a `VT_ERROR`/`DISP_E_PARAMNOTFOUND`
/// Variant (the standard "missing optional" marshaling), and an `Lcid` rule
/// materializes `LOCALE_NEUTRAL` (0) as the hidden `[lcid]` the vtable ABI injects
/// ahead of the `[out,retval]`. Returns `None` if any missing entry is not
/// synthesizable (the gate already proved otherwise, so this is a
/// belt-and-suspenders decline). The returned vector is appended AFTER the
/// supplied args, before the marshaller's `[out,retval]` cell.
#[cfg(target_os = "windows")]
fn synthesize_trailing_optional_args(
    spec: &crate::ComMemberSpec,
    supplied_count: usize,
) -> Option<Vec<Variant>> {
    if spec.parameter_optional_defaults.len() != spec.parameter_types.len() {
        return None;
    }
    let mut synthesized = Vec::with_capacity(spec.parameter_types.len() - supplied_count);
    for rule in &spec.parameter_optional_defaults[supplied_count..] {
        match rule {
            crate::OptionalParamDefault::HasDefault(default) => {
                synthesized.push(default.to_variant());
            }
            crate::OptionalParamDefault::OptionalVariant => {
                synthesized.push(Variant::from_error_code(COM_DISP_E_PARAMNOTFOUND));
            }
            // LOCALE_NEUTRAL (0). The slot's LCID param is a `Long` in
            // `parameter_types`, so the marshaller passes this as a VT_I4 by value.
            crate::OptionalParamDefault::Lcid => {
                synthesized.push(Variant::from_i32(0));
            }
            crate::OptionalParamDefault::Required
            | crate::OptionalParamDefault::OptionalNoDefault => return None,
        }
    }
    Some(synthesized)
}

/// `IProxyManager` — `{00000008-0000-0000-C000-000000000046}`. EVERY COM
/// marshaling proxy (out-of-process / cross-apartment) implements it; a direct
/// in-process object does not. A successful `QueryInterface` for it ⇒ the pointer
/// we hold is a PROXY. This is the in/out-of-process discriminator: a DIRECT
/// in-process interface (DAO) FAILS this QI and is unconditionally
/// vtable-callable; an out-of-process proxy SUCCEEDS it, and whether ITS dual
/// interface is vtable-callable then depends on the interface's proxy/stub CLSID
/// (`proxy_interface_is_vtable_safe`): a `PSOAInterface` ({00020424-…}, the OLE
/// Automation universal marshaler) proxy is a TYPELIB-ALIGNED vtable whose slots
/// match the typelib `oVft` — slot-callable (Excel `_Application` {000208D5} is
/// PSOA); a `PSDispatch` / any other / missing-CLSID proxy is NOT, and a typelib
/// slot there would over-read it → host AV, so it falls back to IDispatch.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const IID_IPROXYMANAGER: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0000_0008,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

/// True when `object` is a COM marshaling proxy (it answers `QueryInterface` for
/// `IID_IProxyManager`). Releases the probe reference. AV-free: a pure QI.
///
/// # Safety
/// `object` must be a live COM interface pointer for the duration of the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub unsafe fn dispatch_is_marshaling_proxy(object: *mut core::ffi::c_void) -> bool {
    // SAFETY: `object` is a live interface pointer per this fn's contract;
    // query_interface_pointer reads its IUnknown vtable and, on success, hands
    // back one owned reference we Release immediately.
    match unsafe { crate::query_interface_pointer(object, &IID_IPROXYMANAGER) } {
        Ok(proxy) => {
            // SAFETY: `proxy` is the single reference the QI handed us; Release it
            // exactly once (we only needed its existence as the proxy signal).
            unsafe { crate::release_unknown(proxy) };
            true
        }
        Err(_) => false,
    }
}

/// For a marshaling PROXY (the caller already confirmed
/// [`dispatch_is_marshaling_proxy`]), decide whether the dual interface `iid`'s
/// vtable slots are SAFE to slot-call: true iff the interface is marshaled by
/// `PSOAInterface` (`HKCR\Interface\{iid}\ProxyStubClsid32 == {00020424-…}`, the
/// OLE Automation universal marshaler whose proxy is a typelib-aligned vtable).
/// `PSDispatch` / any other CLSID / a missing entry ⇒ false (the typelib slot
/// would over-read a non-aligned proxy vtable and AV the host → IDispatch
/// fallback). The PSOA/non-PSOA verdict is cached per-IID in the shared state so
/// the (pure, AV-free) registry probe runs once per interface IID, not per call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn proxy_interface_is_vtable_safe(
    iid: crate::ComInterfaceIid,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> bool {
    let iid_braces = crate::windows_typelib_loader::guid_to_string(&iid.to_guid());
    // Fast path: a prior call already decided this IID. A poisoned lock degrades
    // to the safe answer (treat as non-PSOA → IDispatch fallback, never a slot
    // call on an unverified proxy).
    if let Ok(state) = com_state.lock()
        && let Some(cached) = state.psoa_interface_cache_get(&iid_braces)
    {
        return cached;
    }
    // Miss: probe the registry once (pure reads, no COM-object touch → no AV) and
    // memoize the verdict.
    let is_psoa = crate::windows_typelib_loader::interface_is_psoa_marshaled(&iid_braces);
    if let Ok(mut state) = com_state.lock() {
        state.psoa_interface_cache_put(iid_braces, is_psoa);
    }
    is_psoa
}

/// Attempt an early-bound member call through the COM vtable, with a clean
/// fall-back vs propagate distinction.
///
/// Returns:
/// - `Ok(Some(value))` — the vtable call ran and succeeded; the caller must NOT
///   also run the IDispatch path (and should record the vtable transport).
/// - `Ok(None)` — the member is ineligible (gate failed) OR the marshaller
///   reported an unsupported shape (a validation failure, `hr == None`); the
///   caller falls back to the unchanged IDispatch path.
/// - `Err(failure)` — the vtable call genuinely failed with a real COM HRESULT
///   (`hr == Some(hr < 0)`); the caller PROPAGATES it (no silent fallback that
///   would mask the failure).
///
/// # Safety
/// `dispatch` must be a live dual-interface pointer for the bound object (the
/// bindings map holds one retained reference, keeping it live for this call).
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[allow(clippy::result_large_err)]
pub unsafe fn try_vtable_member_spec_invoke_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prefer_vtable: bool,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Option<Variant>, ComInvokeFailure> {
    if !prefer_vtable {
        return Ok(None);
    }
    // AV-SAFETY: only proceed on a live, pointer-aligned object pointer. A null or
    // misaligned `dispatch` cannot be a real COM interface, so QueryInterface-ing it
    // (and later Releasing the result) would dereference garbage; decline to the
    // IDispatch path, which validates the handle through its own resolution.
    if dispatch.is_null()
        || !(dispatch as usize).is_multiple_of(std::mem::align_of::<*const core::ffi::c_void>())
    {
        return Ok(None);
    }
    // The vtable carries left-to-right positional params only; any NAMED argument
    // is a shape the IDispatch path owns.
    if args.iter().any(|arg| arg.name.is_some()) {
        return Ok(None);
    }
    // Omitted (value-less) args are admitted ONLY as a TRAILING run (D3): the slot
    // call must pass the full positional list, and we can synthesize trailing
    // optionals but not express an interior gap. So count the leading supplied
    // (value-bearing) prefix and require every arg after it to be omitted; an
    // interior omission (a Some after a None) falls back to IDispatch.
    let supplied_count = args.iter().take_while(|arg| arg.value.is_some()).count();
    if args[supplied_count..].iter().any(|arg| arg.value.is_some()) {
        return Ok(None);
    }
    let (label, return_type) = match spec.invoke_kind {
        // A property-get / method returns whatever its FUNCDESC declares — a value
        // (`Some`) or nothing (`None`, a void method like `Quit`).
        crate::TypeLibMemberInvokeKind::PropertyGet => ("property-get", spec.return_type),
        crate::TypeLibMemberInvokeKind::Method => ("method", spec.return_type),
        // A property-put's HRESULT-only member returns no value; the trailing
        // value argument is an ordinary positional [in] param to the vtable slot.
        crate::TypeLibMemberInvokeKind::PropertyPut => ("property-put", None),
        // PropertyPutRef (Set p = obj) is HRESULT-only when the admission table
        // proves the object/interface putref ABI shape; unsupported putref
        // shapes still fall back through that same table-driven boundary.
        crate::TypeLibMemberInvokeKind::PropertyPutRef => ("property-putref", None),
    };
    // Gate on the SUPPLIED positional count (the gate widens to admit fewer
    // supplied than declared when the missing trailing params are synthesizable).
    let supplied_byref_slots: Vec<Option<RuntimeByRefSlot>> = args[..supplied_count]
        .iter()
        .map(|arg| arg.by_ref)
        .collect();
    if supplied_byref_slots.iter().any(Option::is_some) {
        // This value-only API cannot return writebacks. Decline before any slot
        // call so ByRef mutations are never silently lost.
        return Ok(None);
    }
    // Marshal the supplied positional args to `Variant` (also used by the plan
    // builder for argument-sensitive facts such as SAFEARRAY(VT_RECORD)
    // IRecordInfo availability).
    let mut variant_args: Vec<Variant> = args[..supplied_count]
        .iter()
        .filter_map(|arg| arg.value.as_ref().map(|v| v.variant().clone()))
        .collect();
    if variant_args.len() != supplied_count {
        // A value went missing between the prefix count and here; be safe.
        return Ok(None);
    }
    let plan = match build_vtable_invocation_plan_with_byrefs(
        spec,
        supplied_count,
        &supplied_byref_slots,
        Some(&variant_args),
        return_type,
        label,
    ) {
        Ok(plan) => plan,
        Err(_) => return Ok(None),
    };
    // Append the synthesized trailing optionals (declared order) so the slot call
    // passes the full positional list the FUNCDESC declares.
    if supplied_count < spec.parameter_types.len() {
        match synthesize_trailing_optional_args(spec, supplied_count) {
            Some(extra) => variant_args.extend(extra),
            // The gate proved these are synthesizable; if that ever disagrees,
            // decline rather than slot-call with a short positional list.
            None => return Ok(None),
        }
    }

    let mut resolve_object = |handle: ObjectRef| {
        crate::resolve_bound_native_dispatch_shared(com_state, handle)
            .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
    };
    // The vtable [out,retval] interface convention transfers one reference to
    // us; hand it to the bindings map, exactly as the IDispatch result path does.
    let mut bind_dispatch_result = |dispatch: *mut core::ffi::c_void| {
        // SAFETY: a non-null pointer here carries the one reference the callee
        // AddRef'd for the [out,retval]; ownership transfers to the bindings map,
        // satisfying bind_native_runtime_object_result_shared's contract.
        unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                &spec.name,
            )
        }
        .map(Variant::from_object_ref)
    };

    // The member's typelib-declared DUAL interface IID (the gate proved it is
    // present and non-null) — both the QI target below and the PSOA registry key.
    let interface_iid = plan.interface_iid;
    let iid = interface_iid.to_guid();

    // OUT-OF-PROCESS DISCRIMINATOR (HOST-AV SAFETY), gated on the interface's
    // proxy/stub CLSID. An object that answers `QueryInterface(IID_IProxyManager)`
    // is a marshaling PROXY (out-of-process / cross-apartment). Whether its
    // dual-interface vtable slots are SAFE to slot-call depends on HOW the
    // interface is marshaled:
    //   - PSOAInterface (`{00020424-…}`, the OLE Automation universal marshaler):
    //     the proxy is a TYPELIB-ALIGNED vtable proxy whose slots line up with the
    //     typelib `oVft`-derived slots. The bound-checked this-call is ABI-safe —
    //     Excel `_Application` `{000208D5}` is PSOA, which is why its
    //     `[propget,lcid]` `Build`/`Version` slot-call correctly (with the S1 LCID
    //     param-shape fix that injects the hidden LCID ahead of the retval).
    //   - PSDispatch (`{00020420-…}`) / any other CLSID / a missing entry: the
    //     proxy is NOT a typelib-aligned vtable (PSDispatch is a 7-slot
    //     IDispatch-only vtable), so a typelib slot would over-read it and AV the
    //     host. Fall back to the proven IDispatch path (correct value, no AV).
    // A DIRECT in-process interface (DAO) FAILS the IProxyManager QI and is NOT a
    // proxy ⇒ no registry check, vtable as before. The PSOA decision is cached
    // per-IID in the shared state so the registry probe runs once per interface.
    // SAFETY: `dispatch` is the live, bindings-map-retained interface pointer.
    if unsafe { dispatch_is_marshaling_proxy(dispatch) }
        && !proxy_interface_is_vtable_safe(interface_iid, com_state)
    {
        return Ok(None);
    }

    // QueryInterface the object for the member's typelib-declared DUAL interface
    // IID — this is how the VBA IDE holds an early-bound reference. A non-aliasing
    // in-process tear-off is ACCEPTED (the old `ptr::eq(dispatch, interface)`
    // aliasing-only restriction is GONE): the bound check below makes a
    // bound-validated slot on a QI'd interface (in-process OR a PSOA out-of-process
    // proxy) safe and correct. If the QI fails (E_NOINTERFACE / null) we fall back
    // to IDispatch; we NEVER call a slot on an unverified pointer (no host AV).
    // SAFETY: `dispatch` is the live, bindings-map-retained interface pointer for
    // this bound object; QueryInterface reads its IUnknown vtable and, on success,
    // hands back one fresh reference we own (Released below on every path).
    let interface = match unsafe { crate::query_interface_pointer(dispatch, &iid) } {
        Ok(interface) => interface,
        // E_NOINTERFACE or any failing QI: this object does not expose the dual
        // interface in a vtable-callable form here — fall back to IDispatch.
        Err(_) => return Ok(None),
    };

    // AV-SAFETY: a real interface pointer is pointer-aligned. A misaligned (or null)
    // QI result is not a usable interface — neither slot-callable nor safely
    // Releasable (Release would dereference garbage). Decline to IDispatch WITHOUT
    // releasing it.
    if interface.is_null()
        || !(interface as usize).is_multiple_of(std::mem::align_of::<*const core::ffi::c_void>())
    {
        return Ok(None);
    }

    // AV-SAFETY NET (re-asserted at the dispatch site, not just the gate): the slot
    // MUST be inside the source INTERFACE's live vtable (cbSizeVft/8). The gate
    // already checked this, but we re-verify here so a slot call can never over-run
    // the live vtable — the access violation the probe root-caused. Without a known
    // bound, or with an out-of-range slot, fall back to IDispatch.
    if plan.slot >= plan.slot_bound {
        // SAFETY: Release the QI'd reference we own, then fall back.
        unsafe { crate::release_unknown(interface) };
        return Ok(None);
    }

    // SAFETY: `interface` is the QI'd dual-interface pointer carrying one reference
    // we own, on either a DIRECT in-process object or a PSOA-marshaled proxy (a
    // PSDispatch / non-typelib-aligned proxy was excluded above). In both admitted
    // cases its vtable is a typelib-aligned slot table we can index directly. The
    // gate + the bound re-check above proved `7 <= slot < cbSizeVft/8` with a
    // CC_STDCALL, fully-typed, v1-marshallable signature, so the slot's ABI is
    // `HRESULT slot(this, params…, retval*)` — exactly vtable_invoke's contract.
    let result = unsafe {
        crate::windows_vtable::vtable_invoke(
            interface,
            &plan,
            &variant_args,
            dispid,
            &mut resolve_object,
            &mut bind_dispatch_result,
        )
    };

    // Release the QI'd interface reference on every path (success, COM error, and
    // unsupported-shape) — QI added one ref; we own and drop exactly that one.
    // SAFETY: `interface` is the single reference `query_interface_pointer` handed
    // us; we Release it exactly once here and never use it afterward.
    unsafe {
        crate::release_unknown(interface);
    }

    match result {
        Ok(value) => Ok(Some(value)),
        // A real COM error (the call ran and the server returned hr < 0):
        // PROPAGATE — never silently fall back and mask a genuine failure.
        Err(failure) if failure.hr.is_some() => Err(failure),
        // An unsupported-shape / validation signal (hr == None, detail set): the
        // marshaller declined this call. Fall back to the IDispatch path.
        Err(_unsupported) => Ok(None),
    }
}

/// Attempt an early-bound member call through the COM vtable and return the
/// runtime call-result carrier, including any ByRef writebacks.
///
/// The fallback contract matches [`try_vtable_member_spec_invoke_with_shared_state`]:
/// `Ok(None)` means no vtable call ran and the caller may use a fallback path
/// only if that fallback can preserve the requested writeback semantics.
///
/// # Safety
/// `dispatch` must be a live dual-interface pointer for the bound object. The
/// bindings map must retain that COM reference for the duration of the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[allow(clippy::result_large_err)]
pub unsafe fn try_vtable_member_spec_invoke_result_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prefer_vtable: bool,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Option<RuntimeCallResult>, ComInvokeFailure> {
    if !prefer_vtable {
        return Ok(None);
    }
    if dispatch.is_null()
        || !(dispatch as usize).is_multiple_of(std::mem::align_of::<*const core::ffi::c_void>())
    {
        return Ok(None);
    }
    if args.iter().any(|arg| arg.name.is_some()) {
        return Ok(None);
    }
    let supplied_count = args.iter().take_while(|arg| arg.value.is_some()).count();
    if args[supplied_count..].iter().any(|arg| arg.value.is_some()) {
        return Ok(None);
    }
    let (label, return_type) = match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => ("property-get", spec.return_type),
        crate::TypeLibMemberInvokeKind::Method => ("method", spec.return_type),
        crate::TypeLibMemberInvokeKind::PropertyPut => ("property-put", None),
        crate::TypeLibMemberInvokeKind::PropertyPutRef => ("property-putref", None),
    };
    let supplied_byref_slots: Vec<Option<RuntimeByRefSlot>> = args[..supplied_count]
        .iter()
        .map(|arg| arg.by_ref)
        .collect();
    let mut variant_args: Vec<Variant> = args[..supplied_count]
        .iter()
        .filter_map(|arg| arg.value.as_ref().map(|v| v.variant().clone()))
        .collect();
    if variant_args.len() != supplied_count {
        return Ok(None);
    }
    let plan = match build_vtable_invocation_plan_with_byrefs(
        spec,
        supplied_count,
        &supplied_byref_slots,
        Some(&variant_args),
        return_type,
        label,
    ) {
        Ok(plan) => plan,
        Err(_) => return Ok(None),
    };
    if supplied_count < spec.parameter_types.len() {
        match synthesize_trailing_optional_args(spec, supplied_count) {
            Some(extra) => variant_args.extend(extra),
            None => return Ok(None),
        }
    }

    let mut resolve_object = |handle: ObjectRef| {
        crate::resolve_bound_native_dispatch_shared(com_state, handle)
            .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
    };
    let mut bind_dispatch_result = |dispatch: *mut core::ffi::c_void| {
        // SAFETY: a non-null pointer here carries the one reference the callee
        // transferred for the return/writeback cell; ownership transfers to the
        // shared bindings map.
        unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                &spec.name,
            )
        }
        .map(Variant::from_object_ref)
    };

    let interface_iid = plan.interface_iid;
    let iid = interface_iid.to_guid();
    // SAFETY: `dispatch` is live for this call by this function's safety
    // contract; this QI probe only reads its IUnknown vtable.
    if unsafe { dispatch_is_marshaling_proxy(dispatch) }
        && !proxy_interface_is_vtable_safe(interface_iid, com_state)
    {
        return Ok(None);
    }
    // SAFETY: `dispatch` is the live, retained interface pointer for this bound
    // object; QueryInterface returns one owned reference on success.
    let interface = match unsafe { crate::query_interface_pointer(dispatch, &iid) } {
        Ok(interface) => interface,
        Err(_) => return Ok(None),
    };
    if interface.is_null()
        || !(interface as usize).is_multiple_of(std::mem::align_of::<*const core::ffi::c_void>())
    {
        return Ok(None);
    }
    if plan.slot >= plan.slot_bound {
        // SAFETY: `interface` is the single owned QI reference from above.
        unsafe { crate::release_unknown(interface) };
        return Ok(None);
    }

    // SAFETY: the plan gate proved the slot, call convention, wire shape, and
    // writeback slots; `interface` is the QI'd pointer for that interface.
    let result = unsafe {
        crate::windows_vtable::vtable_invoke_with_writebacks(
            interface,
            &plan,
            &variant_args,
            dispid,
            &mut resolve_object,
            &mut bind_dispatch_result,
        )
    };
    // SAFETY: `interface` is the single owned QI reference from above, released
    // exactly once after the vtable attempt.
    unsafe {
        crate::release_unknown(interface);
    }

    match result {
        Ok(result) => {
            let mut call_result = RuntimeCallResult::value(result.value);
            call_result.writebacks = result.writebacks;
            Ok(Some(call_result))
        }
        Err(failure) if failure.hr.is_some() => Err(failure),
        Err(_unsupported) => Ok(None),
    }
}

/// Non-x64 Windows stub: the libffi this-call vtable marshaller is x64-only
/// (`windows_vtable` is gated to `target_arch = "x86_64"`), so every member
/// falls back to the IDispatch path here.
#[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
#[allow(clippy::result_large_err)]
pub unsafe fn try_vtable_member_spec_invoke_with_shared_state(
    _dispatch: *mut core::ffi::c_void,
    _dispid: i32,
    _spec: &crate::ComMemberSpec,
    _args: &[ComInvokeArg],
    _prefer_vtable: bool,
    _com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Option<Variant>, ComInvokeFailure> {
    Ok(None)
}

#[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
#[allow(clippy::result_large_err)]
pub unsafe fn try_vtable_member_spec_invoke_result_with_shared_state(
    _dispatch: *mut core::ffi::c_void,
    _dispid: i32,
    _spec: &crate::ComMemberSpec,
    _args: &[ComInvokeArg],
    _prefer_vtable: bool,
    _com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Option<RuntimeCallResult>, ComInvokeFailure> {
    Ok(None)
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments, clippy::missing_safety_doc)]
pub unsafe fn execute_bound_variant_with_shared_state<FTryVtable, FKnownSpec>(
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
    request: &ComInvokeRequest,
    prefer_vtable: bool,
    transport: ComTransportCounters<'_>,
    try_vtable_invoke: &mut FTryVtable,
    known_member_spec: &mut FKnownSpec,
) -> Result<Option<Variant>, String>
where
    FTryVtable:
        FnMut(*mut crate::RawIDispatch, &ComBinding, i32, &[i32]) -> Result<Option<i32>, String>,
    FKnownSpec:
        FnMut(&ComBinding, crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
{
    let (binding, cached_dispid) = {
        let state = com_state.lock().map_err(|_| {
            "COM-E-STATE-LOCK-POISONED: dispatch_invoke state lock poisoned".to_string()
        })?;
        let binding = state
            .bindings
            .get(&crate::ComObjectToken::new(request.object.raw()))
            .cloned();
        let cached_dispid = binding
            .as_ref()
            .and_then(|entry| entry.member_dispids.get(&request.member).copied());
        (binding, cached_dispid)
    };
    let Some(binding) = binding else {
        return Ok(None);
    };
    if binding.native_dispatch == 0 {
        return Ok(None);
    }
    let dispatch = binding.native_dispatch as *mut crate::RawIDispatch;
    let mut resolve_member_dispid =
        |member: i32,
         intended_kind: crate::TypeLibMemberInvokeKind,
         _cached_dispid: Option<i32>| {
            let mut state = com_state.lock().map_err(|_| {
                "COM-E-STATE-LOCK-POISONED: dispatch_invoke state lock poisoned".to_string()
            })?;
            // SAFETY: `dispatch` was recovered from a live bindings-map entry
            // (checked non-zero above); the bindings map owns one retained
            // IDispatch reference, keeping the pointer live for this lookup.
            unsafe {
                crate::resolve_member_dispid_cached(
                    &mut state,
                    dispatch,
                    request.object.clone(),
                    &binding,
                    crate::ComMemberToken::new(member),
                    intended_kind,
                    None,
                )
            }
        };
    let mut invoke_member_spec =
        |dispid: i32, spec: &crate::ComMemberSpec, invoke_args: &[ComInvokeArg], prog_id: &str| {
            // Early-bound vtable fast path: when the policy prefers it AND the
            // resolved member carries a full, callable, v1-marshallable dual
            // signature, dispatch through the COM vtable slot directly. An
            // ineligible member or an unsupported shape returns Ok(None) → the
            // unchanged IDispatch path below; a real COM error (hr < 0) is
            // propagated, never silently swallowed.
            // SAFETY: `dispatch` is the live bindings-map-retained dual interface
            // for this bound object (checked non-zero above), exactly what the
            // vtable attempt's `# Safety` requires.
            match unsafe {
                try_vtable_member_spec_invoke_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    spec,
                    invoke_args,
                    prefer_vtable,
                    com_state,
                )
            } {
                Ok(Some(value)) => {
                    transport.record_vtable();
                    return Ok(value);
                }
                Ok(None) => {}
                Err(failure) => {
                    return Err(render_invoke_fault_message(&failure));
                }
            }
            // SAFETY: `dispatch` was recovered from a live bindings-map entry
            // whose retained IDispatch reference outlives this call (the
            // invoking runtime holds the object); the shared-state helper
            // installs callbacks that uphold COM retention rules.
            let value = unsafe {
                invoke_member_spec_variant_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    spec,
                    invoke_args,
                    prog_id,
                    com_state,
                )
            }
            .map_err(|failure| render_invoke_fault_message(&failure))?;
            transport.record_idispatch();
            Ok(value)
        };
    let mut invoke_direct_dispid = |member: i32,
                                    invoke_kind: crate::TypeLibMemberInvokeKind,
                                    requires_argument: bool,
                                    invoke_args: &[ComInvokeArg],
                                    prog_id: &str| {
        // SAFETY: `dispatch` was recovered from a live bindings-map entry
        // whose retained IDispatch reference outlives this call (the invoking
        // runtime holds the object); the shared-state helper installs
        // callbacks that uphold COM retention rules.
        unsafe {
            invoke_direct_dispid_variant_with_shared_state(
                dispatch.cast(),
                member,
                invoke_kind,
                requires_argument,
                invoke_args,
                prog_id,
                com_state,
            )
        }
        .map_err(|failure| render_invoke_fault_message(&failure))
    };
    // SAFETY: `dispatch` was recovered from a live bindings-map entry whose
    // retained IDispatch reference outlives this call (the invoking runtime
    // holds the object), as required by the callee's contract.
    let mut invoke_bound_dispatch = |member: i32, invoke_args: &[ComInvokeArg], prog_id: &str| unsafe {
        invoke_bound_dispatch_variant_with_shared_state(
            dispatch,
            prog_id,
            crate::ComMemberToken::new(member),
            invoke_args,
            com_state,
            known_member_spec,
        )
    };
    let mut try_vtable =
        |member: i32, positional: &[i32]| try_vtable_invoke(dispatch, &binding, member, positional);
    let value = execute_bound_variant(
        &binding,
        request,
        cached_dispid,
        &mut try_vtable,
        &mut resolve_member_dispid,
        &mut invoke_member_spec,
        &mut invoke_direct_dispid,
        &mut invoke_bound_dispatch,
    )?;
    let _ = crate::windows_runtime_state::queue_projection_event_callbacks_shared(
        com_state,
        request.object.clone(),
        &binding,
        request.member,
        request.args.as_slice(),
    )?;
    Ok(Some(value))
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments, clippy::missing_safety_doc)]
pub unsafe fn execute_bound_runtime_call_result_with_shared_state<FTryVtable, FKnownSpec>(
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
    request: &ComInvokeRequest,
    prefer_vtable: bool,
    transport: ComTransportCounters<'_>,
    try_vtable_invoke: &mut FTryVtable,
    known_member_spec: &mut FKnownSpec,
) -> Result<Option<RuntimeCallResult>, String>
where
    FTryVtable:
        FnMut(*mut crate::RawIDispatch, &ComBinding, i32, &[i32]) -> Result<Option<i32>, String>,
    FKnownSpec:
        FnMut(&ComBinding, crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
{
    let (binding, cached_dispid) = {
        let state = com_state.lock().map_err(|_| {
            "COM-E-STATE-LOCK-POISONED: dispatch_invoke state lock poisoned".to_string()
        })?;
        let binding = state
            .bindings
            .get(&crate::ComObjectToken::new(request.object.raw()))
            .cloned();
        let cached_dispid = binding
            .as_ref()
            .and_then(|entry| entry.member_dispids.get(&request.member).copied());
        (binding, cached_dispid)
    };
    let Some(binding) = binding else {
        return Ok(None);
    };
    if binding.native_dispatch == 0 {
        return Ok(None);
    }
    let dispatch = binding.native_dispatch as *mut crate::RawIDispatch;
    let has_byref_args = request.args.iter().any(|arg| arg.by_ref.is_some());
    let mut resolve_member_dispid =
        |member: i32,
         intended_kind: crate::TypeLibMemberInvokeKind,
         _cached_dispid: Option<i32>| {
            let mut state = com_state.lock().map_err(|_| {
                "COM-E-STATE-LOCK-POISONED: dispatch_invoke state lock poisoned".to_string()
            })?;
            // SAFETY: `dispatch` was recovered from a live bindings-map entry
            // that owns a retained IDispatch reference for this call.
            unsafe {
                crate::resolve_member_dispid_cached(
                    &mut state,
                    dispatch,
                    request.object.clone(),
                    &binding,
                    crate::ComMemberToken::new(member),
                    intended_kind,
                    None,
                )
            }
        };
    let mut invoke_member_spec = |dispid: i32,
                                  spec: &crate::ComMemberSpec,
                                  invoke_args: &[ComInvokeArg],
                                  prog_id: &str| {
        // SAFETY: `dispatch` is the live bindings-map-retained dispatch pointer
        // for this bound object; the helper performs the vtable gate before any
        // slot access.
        match unsafe {
            try_vtable_member_spec_invoke_result_with_shared_state(
                dispatch.cast(),
                dispid,
                spec,
                invoke_args,
                prefer_vtable,
                com_state,
            )
        } {
            Ok(Some(result)) => {
                transport.record_vtable();
                return Ok(result);
            }
            Ok(None) => {}
            Err(failure) => return Err(render_invoke_fault_message(&failure)),
        }
        if invoke_args.iter().any(|arg| arg.by_ref.is_some()) {
            return Err(format!(
                "COM-E-BYREF-FALLBACK-UNSUPPORTED: vtable declined `{}` on `{prog_id}`, and the IDispatch fallback cannot return runtime ByRef writebacks",
                spec.name
            ));
        }
        // SAFETY: `dispatch` is the live bindings-map-retained dispatch pointer
        // for this call; the shared-state helper installs the standard COM
        // argument/result ownership callbacks.
        let value = unsafe {
            invoke_member_spec_variant_with_shared_state(
                dispatch.cast(),
                dispid,
                spec,
                invoke_args,
                prog_id,
                com_state,
            )
        }
        .map_err(|failure| render_invoke_fault_message(&failure))?;
        transport.record_idispatch();
        Ok(RuntimeCallResult::value(value))
    };
    let mut invoke_direct_dispid = |member: i32,
                                    invoke_kind: crate::TypeLibMemberInvokeKind,
                                    requires_argument: bool,
                                    invoke_args: &[ComInvokeArg],
                                    prog_id: &str| {
        if invoke_args.iter().any(|arg| arg.by_ref.is_some()) {
            return Err(format!(
                "COM-E-BYREF-FALLBACK-UNSUPPORTED: direct-DISPID member {member} on `{prog_id}` cannot return runtime ByRef writebacks"
            ));
        }
        // SAFETY: `dispatch` is the live bindings-map-retained dispatch pointer
        // for this direct-DISPID fallback call.
        unsafe {
            invoke_direct_dispid_variant_with_shared_state(
                dispatch.cast(),
                member,
                invoke_kind,
                requires_argument,
                invoke_args,
                prog_id,
                com_state,
            )
        }
        .map(RuntimeCallResult::value)
        .map_err(|failure| render_invoke_fault_message(&failure))
    };
    let mut invoke_bound_dispatch = |member: i32, invoke_args: &[ComInvokeArg], prog_id: &str| {
        if invoke_args.iter().any(|arg| arg.by_ref.is_some()) {
            return Err(format!(
                "COM-E-BYREF-FALLBACK-UNSUPPORTED: bound member {member} on `{prog_id}` cannot return runtime ByRef writebacks"
            ));
        }
        // SAFETY: `dispatch` is the live bindings-map-retained dispatch
        // pointer for this bound fallback call.
        unsafe {
            invoke_bound_dispatch_variant_with_shared_state(
                dispatch,
                prog_id,
                crate::ComMemberToken::new(member),
                invoke_args,
                com_state,
                known_member_spec,
            )
            .map(RuntimeCallResult::value)
        }
    };
    let mut try_vtable =
        |member: i32, positional: &[i32]| try_vtable_invoke(dispatch, &binding, member, positional);
    let result = execute_bound_runtime_call_result(
        &binding,
        request,
        cached_dispid,
        &mut try_vtable,
        &mut resolve_member_dispid,
        &mut invoke_member_spec,
        &mut invoke_direct_dispid,
        &mut invoke_bound_dispatch,
    )?;
    let _ = crate::windows_runtime_state::queue_projection_event_callbacks_shared(
        com_state,
        request.object.clone(),
        &binding,
        request.member,
        request.args.as_slice(),
    )?;
    if has_byref_args && result.writebacks.is_empty() {
        return Err(
            "COM-E-BYREF-WRITEBACK-MISSING: ByRef runtime call completed without writebacks"
                .to_string(),
        );
    }
    Ok(Some(result))
}

#[cfg(all(target_os = "windows", test))]
mod gate_tests {
    use super::{
        VtableDeclineReason, build_vtable_invocation_plan,
        build_vtable_invocation_plan_with_byrefs, is_v1_vtable_vartype,
        synthesize_trailing_optional_args,
    };
    use crate::{ComInterfaceIid, ComMemberSpec, SourceTypeKind, TypeLibMemberInvokeKind};
    use oxvba_runtime::safe_array::SafeArray;
    use oxvba_runtime::{RuntimeByRefSlot, Variant};

    /// A vtable-eligible spec: a real custom INTERFACE dual, CC_STDCALL, a slot
    /// at `slot` inside `bound`, a non-null IID, and a no-arg `Long` getter. The
    /// individual rejection tests then flip ONE field to prove the gate declines.
    fn eligible_spec(slot: u16, bound: u16) -> ComMemberSpec {
        ComMemberSpec {
            name: "Value".to_string(),
            requires_argument: false,
            invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            parameter_names: Vec::new(),
            is_default_member: false,
            vtable_slot: Some(slot),
            parameter_types: Vec::new(),
            parameter_wire_types: Vec::new(),
            parameter_iids: Vec::new(),
            parameter_optional_defaults: Vec::new(),
            return_type: Some(crate::TypeLibParamType::Long),
            return_wire_type: Some(crate::TypeLibWireType::Automation(
                crate::TypeLibParamType::Long,
            )),
            callconv_is_stdcall: true,
            interface_iid: Some(ComInterfaceIid {
                data1: 0x1234_5678,
                data2: 0x9abc,
                data3: 0xdef0,
                data4: [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
            }),
            is_dual: true,
            source_typekind: Some(SourceTypeKind::Interface),
            vtable_slot_bound: Some(bound),
        }
    }

    fn vtable_plan_label(invoke_kind: TypeLibMemberInvokeKind) -> &'static str {
        match invoke_kind {
            TypeLibMemberInvokeKind::PropertyGet => "property-get",
            TypeLibMemberInvokeKind::Method => "method",
            TypeLibMemberInvokeKind::PropertyPut => "property-put",
            TypeLibMemberInvokeKind::PropertyPutRef => "property-putref",
        }
    }

    fn vtable_gate_decline_reason(
        spec: &ComMemberSpec,
        positional_arg_count: usize,
        return_type: Option<crate::TypeLibParamType>,
    ) -> Option<VtableDeclineReason> {
        build_vtable_invocation_plan(
            spec,
            positional_arg_count,
            return_type,
            vtable_plan_label(spec.invoke_kind),
        )
        .err()
    }

    fn vtable_gate_admits(
        spec: &ComMemberSpec,
        positional_arg_count: usize,
        return_type: Option<crate::TypeLibParamType>,
    ) -> bool {
        build_vtable_invocation_plan(
            spec,
            positional_arg_count,
            return_type,
            vtable_plan_label(spec.invoke_kind),
        )
        .is_ok()
    }

    #[test]
    fn gate_admits_interface_dual_slot_in_bound() {
        // DAO Field.Value: slot 17, partner cbSizeVft=464 → bound 58. ACCEPT.
        let spec = eligible_spec(17, 58);
        assert!(vtable_gate_admits(
            &spec,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        assert_eq!(
            vtable_gate_decline_reason(&spec, 0, Some(crate::TypeLibParamType::Long)),
            None
        );
    }

    #[test]
    fn gate_decline_reasons_cover_descriptor_safety_predicates() {
        let mut missing_slot = eligible_spec(17, 58);
        missing_slot.vtable_slot = None;
        assert_eq!(
            vtable_gate_decline_reason(&missing_slot, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingSlot)
        );

        let reserved = eligible_spec(6, 58);
        assert_eq!(
            vtable_gate_decline_reason(&reserved, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::ReservedComSlot)
        );

        let mut missing_bound = eligible_spec(17, 58);
        missing_bound.vtable_slot_bound = None;
        assert_eq!(
            vtable_gate_decline_reason(&missing_bound, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingSlotBound)
        );

        let out_of_bounds = eligible_spec(58, 58);
        assert_eq!(
            vtable_gate_decline_reason(&out_of_bounds, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::SlotOutOfBounds)
        );

        let mut missing_iid = eligible_spec(17, 58);
        missing_iid.interface_iid = None;
        assert_eq!(
            vtable_gate_decline_reason(&missing_iid, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingInterfaceIid)
        );

        let mut non_stdcall = eligible_spec(17, 58);
        non_stdcall.callconv_is_stdcall = false;
        assert_eq!(
            vtable_gate_decline_reason(&non_stdcall, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::NonStdcall)
        );

        let mut putref = eligible_putref_object_spec();
        assert_eq!(
            vtable_gate_decline_reason(&putref, 1, None),
            None,
            "object/interface PropertyPutRef is now an admitted vtable shape"
        );
        assert!(
            vtable_gate_admits(&putref, 1, None),
            "PropertyPutRef object assignment should use the cleaned vtable path"
        );
        putref.parameter_wire_types = Vec::new();
        assert_eq!(
            vtable_gate_decline_reason(&putref, 1, None),
            Some(VtableDeclineReason::PropertyPutRefDeferred),
            "putref without explicit interface wire metadata remains fallback"
        );
        let mut putref_extra_iid = eligible_putref_object_spec();
        putref_extra_iid.parameter_iids.push(None);
        assert_eq!(
            vtable_gate_decline_reason(&putref_extra_iid, 1, None),
            Some(VtableDeclineReason::PropertyPutRefDeferred),
            "putref with malformed IID arity remains fallback"
        );
    }

    fn eligible_putref_object_spec() -> ComMemberSpec {
        let mut spec = eligible_spec(17, 58);
        spec.invoke_kind = TypeLibMemberInvokeKind::PropertyPutRef;
        spec.parameter_types = vec![crate::TypeLibParamType::Object];
        spec.parameter_wire_types = vec![crate::TypeLibWireType::InterfacePointer {
            name: "ITestDispatch".to_string(),
        }];
        spec.parameter_iids = vec![spec.interface_iid];
        spec.return_type = None;
        spec.return_wire_type = None;
        spec
    }

    #[test]
    fn gate_rejects_dispinterface_member() {
        // A pure dispinterface member (source_typekind == Dispatch) has no
        // callable vtable slot, even with a slot+IID present. REJECT.
        let mut spec = eligible_spec(17, 58);
        spec.source_typekind = Some(SourceTypeKind::Dispatch);
        assert_eq!(
            vtable_gate_decline_reason(&spec, 0, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::NotDualInterface)
        );
        assert!(!vtable_gate_admits(
            &spec,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        // Likewise a non-FDUAL member.
        let mut not_dual = eligible_spec(17, 58);
        not_dual.is_dual = false;
        assert!(!vtable_gate_admits(
            &not_dual,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
    }

    #[test]
    fn gate_rejects_slot_at_or_past_bound() {
        // THE AV-SAFETY NET: slot 98 against a 92-slot bound (the Recordset.Close
        // over-run the probe root-caused) must REJECT.
        let over = eligible_spec(98, 92);
        assert!(!vtable_gate_admits(
            &over,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        // Slot exactly == bound is also out of range (valid indices are 0..bound).
        let at_bound = eligible_spec(58, 58);
        assert!(!vtable_gate_admits(
            &at_bound,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        // A missing bound declines too — we never slot-call without a known bound.
        let mut no_bound = eligible_spec(17, 58);
        no_bound.vtable_slot_bound = None;
        assert!(!vtable_gate_admits(
            &no_bound,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
    }

    /// A 3-VARIANT-param spec (e.g. DAO `OpenRecordset(source, [type], [options])`
    /// shape) with the trailing two declared `OptionalVariant`. The `source` is
    /// required (no default rule needed for the leading supplied arg).
    fn three_param_optional_variant_spec() -> ComMemberSpec {
        let mut spec = eligible_spec(20, 58);
        spec.name = "OpenRecordset".to_string();
        spec.invoke_kind = TypeLibMemberInvokeKind::Method;
        spec.requires_argument = true;
        spec.parameter_types = vec![
            crate::TypeLibParamType::Variant,
            crate::TypeLibParamType::Variant,
            crate::TypeLibParamType::Variant,
        ];
        spec.parameter_optional_defaults = vec![
            crate::OptionalParamDefault::Required,
            crate::OptionalParamDefault::OptionalVariant,
            crate::OptionalParamDefault::OptionalVariant,
        ];
        spec.return_type = Some(crate::TypeLibParamType::Object);
        spec.return_wire_type = Some(crate::TypeLibWireType::InterfacePointer {
            name: "Recordset".to_string(),
        });
        spec
    }

    #[test]
    fn gate_admits_omitted_trailing_optional_variants() {
        // D3: 1 supplied of 3 declared, the missing two are trailing OptionalVariant
        // → ADMIT (the dispatch site synthesizes VT_ERROR/DISP_E_PARAMNOTFOUND).
        let spec = three_param_optional_variant_spec();
        assert!(vtable_gate_admits(
            &spec,
            1,
            Some(crate::TypeLibParamType::Object)
        ));
        // Exact arity (all 3 supplied) still admits.
        assert!(vtable_gate_admits(
            &spec,
            3,
            Some(crate::TypeLibParamType::Object)
        ));
        // 2 supplied of 3: the one missing trailing optional is synthesizable.
        assert!(vtable_gate_admits(
            &spec,
            2,
            Some(crate::TypeLibParamType::Object)
        ));
    }

    #[test]
    fn gate_admits_omitted_trailing_default_value() {
        // D3: a trailing optional carrying a typelib default value is synthesizable.
        let mut spec = three_param_optional_variant_spec();
        spec.parameter_types[1] = crate::TypeLibParamType::Long;
        spec.parameter_optional_defaults[1] =
            crate::OptionalParamDefault::HasDefault(crate::ComDefaultValue::Int(5));
        // 1 supplied of 3: missing [1]=HasDefault(Long), [2]=OptionalVariant → ADMIT.
        assert!(vtable_gate_admits(
            &spec,
            1,
            Some(crate::TypeLibParamType::Object)
        ));
    }

    #[test]
    fn gate_rejects_omitted_required_or_unsynthesizable_optional() {
        // A missing REQUIRED param in the tail declines (can't drop it).
        let mut required_tail = three_param_optional_variant_spec();
        required_tail.parameter_optional_defaults[2] = crate::OptionalParamDefault::Required;
        assert!(!vtable_gate_admits(
            &required_tail,
            1,
            Some(crate::TypeLibParamType::Object)
        ));
        // A missing optional NON-VARIANT with no default declines (no safe value).
        let mut no_default = three_param_optional_variant_spec();
        no_default.parameter_types[2] = crate::TypeLibParamType::Long;
        no_default.parameter_optional_defaults[2] = crate::OptionalParamDefault::OptionalNoDefault;
        assert!(!vtable_gate_admits(
            &no_default,
            1,
            Some(crate::TypeLibParamType::Object)
        ));
        // No synthesis metadata at all (empty defaults vector) keeps pre-D3
        // exact-arity behavior: 1 supplied of 3 declared declines.
        let mut no_metadata = three_param_optional_variant_spec();
        no_metadata.parameter_optional_defaults = Vec::new();
        assert!(!vtable_gate_admits(
            &no_metadata,
            1,
            Some(crate::TypeLibParamType::Object)
        ));
        // More supplied than declared is always a mismatch.
        let spec = three_param_optional_variant_spec();
        assert_eq!(
            vtable_gate_decline_reason(&spec, 4, Some(crate::TypeLibParamType::Object)),
            Some(VtableDeclineReason::TooManyArgs)
        );
        assert!(!vtable_gate_admits(
            &spec,
            4,
            Some(crate::TypeLibParamType::Object)
        ));
    }

    #[test]
    fn gate_return_trust_guard_declines_scalar_returning_method_on_omitted_optional() {
        // D3 RETURN-TRUST GUARD: a Method declared to return a bare integer scalar
        // (the DAO `OpenRecordset` quirk — declared `Long`, actually returns an
        // object) must DECLINE the D3 omitted-optional widening and fall back to
        // IDispatch, since the vtable result-decode would mis-decode an object as a
        // scalar.
        for scalar in [
            crate::TypeLibParamType::Long,
            crate::TypeLibParamType::Integer,
            crate::TypeLibParamType::Byte,
            crate::TypeLibParamType::LongLong,
        ] {
            let mut spec = three_param_optional_variant_spec();
            spec.invoke_kind = TypeLibMemberInvokeKind::Method;
            spec.return_type = Some(scalar);
            spec.return_wire_type = Some(crate::TypeLibWireType::Automation(scalar));
            // 1 supplied of 3 (omitted optionals) + scalar-returning Method → DECLINE.
            assert!(
                !vtable_gate_admits(&spec, 1, Some(scalar)),
                "scalar-returning Method with omitted optionals must decline ({scalar:?})"
            );
            // But EXACT arity is unaffected by the guard (it only gates the widening),
            // so the same scalar-returning Method at full arity stays admitted.
            assert!(
                vtable_gate_admits(&spec, 3, Some(scalar)),
                "exact-arity scalar-returning Method must still admit ({scalar:?})"
            );
        }
        // A void (None) or Object-returning Method with omitted optionals is
        // decode-safe and stays admitted (CreateDatabase/Execute shape).
        let mut void_method = three_param_optional_variant_spec();
        void_method.return_type = None;
        assert!(vtable_gate_admits(&void_method, 1, None));
        let object_method = three_param_optional_variant_spec(); // returns Object
        assert!(vtable_gate_admits(
            &object_method,
            1,
            Some(crate::TypeLibParamType::Object)
        ));
        // A scalar-returning PROPERTY-GET with omitted optionals stays admitted (a
        // property-get scalar return is trustworthy — e.g. an indexed Field.Value).
        let mut scalar_getter = three_param_optional_variant_spec();
        scalar_getter.invoke_kind = TypeLibMemberInvokeKind::PropertyGet;
        scalar_getter.return_type = Some(crate::TypeLibParamType::Long);
        scalar_getter.return_wire_type = Some(crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::Long,
        ));
        assert!(vtable_gate_admits(
            &scalar_getter,
            1,
            Some(crate::TypeLibParamType::Long)
        ));
    }

    /// COM-matrix A12 (the riskiest seam, always-on regression guard). A
    /// `[propget, lcid]`-with-a-real-arg member's true vtable ABI is
    /// `HRESULT get_X(this, <real args...>, LCID, T* pRet)` — the hidden `[lcid]`
    /// sits AFTER the real positional args and BEFORE the `[out,retval]`. The
    /// synthesis machinery walks `parameter_optional_defaults[supplied..]` in
    /// DECLARED order, so the gate must only admit a shape where the lcid is in the
    /// missing TRAILING run (after every guest-supplied positional). A shape whose
    /// declared order puts the lcid in the PREFIX (ahead of a still-required real
    /// arg) must DECLINE — admitting it would pair the synthesized lcid against the
    /// wrong slot and place the retval pointer one slot early (the host AV).
    #[test]
    fn gate_lcid_must_be_trailing_not_prefix() {
        // lcid-TRAILING: `f(real, [lcid])`. Guest supplies the 1 real arg; the
        // missing tail is exactly `[Lcid]` -> synthesizable -> ADMIT.
        let mut trailing = eligible_spec(20, 58);
        trailing.name = "International".to_string();
        trailing.invoke_kind = TypeLibMemberInvokeKind::PropertyGet;
        trailing.requires_argument = true;
        trailing.parameter_types =
            vec![crate::TypeLibParamType::Long, crate::TypeLibParamType::Long];
        trailing.parameter_wire_types = vec![
            crate::TypeLibWireType::Automation(crate::TypeLibParamType::Long),
            crate::TypeLibWireType::Automation(crate::TypeLibParamType::Long),
        ];
        trailing.parameter_optional_defaults = vec![
            crate::OptionalParamDefault::Required,
            crate::OptionalParamDefault::Lcid,
        ];
        trailing.return_type = Some(crate::TypeLibParamType::Variant);
        trailing.return_wire_type = Some(crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::Variant,
        ));
        assert!(
            vtable_gate_admits(&trailing, 1, Some(crate::TypeLibParamType::Variant)),
            "lcid-trailing [Required, Lcid] with 1 supplied real arg must ADMIT \
             (the tail [Lcid] is synthesizable)"
        );
        // And the synthesizer fills exactly one trailing arg: the LCID (VT_I4 = 0).
        let synth =
            synthesize_trailing_optional_args(&trailing, 1).expect("trailing lcid must synthesize");
        assert_eq!(synth.len(), 1, "exactly the hidden lcid is synthesized");
        assert_eq!(
            synth[0].as_i32(),
            Some(0),
            "the synthesized lcid is LOCALE_NEUTRAL (0)"
        );

        // lcid-PREFIX: `f([lcid], real)`. The guest's 1 supplied positional fills
        // index 0 (the lcid slot in this bad shape), leaving the missing tail
        // `[Required]` -> NOT synthesizable -> DECLINE. The gate must never admit a
        // member whose lcid is ahead of a still-required real arg.
        let mut prefix = trailing.clone();
        prefix.parameter_optional_defaults = vec![
            crate::OptionalParamDefault::Lcid,
            crate::OptionalParamDefault::Required,
        ];
        assert!(
            !vtable_gate_admits(&prefix, 1, Some(crate::TypeLibParamType::Variant)),
            "lcid-prefix [Lcid, Required] must DECLINE (a required real arg is left \
             unsynthesizable in the missing tail)"
        );
    }

    /// COM-matrix A12 edge-VT decline half. The vtable marshaller handles the
    /// scalar/Variant/Object semantic set, with explicit wire metadata covering
    /// non-scalar layouts such as SAFEARRAY. Out-of-set semantic VARTYPEs still
    /// gate the call to the IDispatch fallback rather than risk a wrong-ABI slot
    /// call.
    #[test]
    fn gate_v1_vtable_vartype_rejects_out_of_set() {
        // In-set shapes are admitted (sanity).
        for ok in [
            crate::TypeLibParamType::Variant,
            crate::TypeLibParamType::Long,
            crate::TypeLibParamType::Integer,
            crate::TypeLibParamType::String,
            crate::TypeLibParamType::Boolean,
            crate::TypeLibParamType::Double,
            crate::TypeLibParamType::Single,
            crate::TypeLibParamType::Currency,
            crate::TypeLibParamType::Date,
            crate::TypeLibParamType::Object,
            crate::TypeLibParamType::Byte,
            crate::TypeLibParamType::LongLong,
            crate::TypeLibParamType::Decimal,
            crate::TypeLibParamType::Record,
        ] {
            assert!(is_v1_vtable_vartype(ok), "{ok:?} must be in the v1 set");
        }
        for ok in [
            crate::TypeLibParamType::ByRefVariant,
            crate::TypeLibParamType::ByRefLong,
            crate::TypeLibParamType::ByRefInteger,
            crate::TypeLibParamType::ByRefDouble,
            crate::TypeLibParamType::ByRefSingle,
            crate::TypeLibParamType::ByRefCurrency,
            crate::TypeLibParamType::ByRefDate,
            crate::TypeLibParamType::ByRefDecimal,
            crate::TypeLibParamType::ByRefByte,
            crate::TypeLibParamType::ByRefBoolean,
            crate::TypeLibParamType::ByRefLongLong,
            crate::TypeLibParamType::ByRefString,
            crate::TypeLibParamType::ByRefObject,
            crate::TypeLibParamType::ByRefLongPtr,
            crate::TypeLibParamType::ByRefRecord,
        ] {
            assert!(
                is_v1_vtable_vartype(ok),
                "{ok:?} must be in the writeback-capable vtable set"
            );
        }
        // Out-of-set parameter shapes still decline before slot-call.
        for bad in [crate::TypeLibParamType::LongPtr] {
            assert!(
                !is_v1_vtable_vartype(bad),
                "{bad:?} must be OUTSIDE the v1 set (decline to IDispatch)"
            );
        }
        // Decimal retvals are supported through caller-owned DECIMAL out cells,
        // while still-unsupported retval shapes decline outright.
        let mut decimal_ret = eligible_spec(17, 58);
        decimal_ret.return_type = Some(crate::TypeLibParamType::Decimal);
        decimal_ret.return_wire_type = Some(crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::Decimal,
        ));
        assert_eq!(
            vtable_gate_decline_reason(&decimal_ret, 0, Some(crate::TypeLibParamType::Decimal)),
            None,
            "Decimal retvals are admitted as caller-owned out cells"
        );
        assert!(
            vtable_gate_admits(&decimal_ret, 0, Some(crate::TypeLibParamType::Decimal)),
            "a Decimal-returning member should use the vtable path when other safety facts are present"
        );

        let mut longptr_ret = eligible_spec(17, 58);
        longptr_ret.return_type = Some(crate::TypeLibParamType::LongPtr);
        longptr_ret.return_wire_type = Some(crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::LongPtr,
        ));
        assert_eq!(
            vtable_gate_decline_reason(&longptr_ret, 0, Some(crate::TypeLibParamType::LongPtr)),
            Some(VtableDeclineReason::UnsupportedReturnType(
                crate::TypeLibParamType::LongPtr
            ))
        );

        let mut record_ret = eligible_spec(17, 58);
        record_ret.return_type = Some(crate::TypeLibParamType::Record);
        record_ret.return_wire_type = Some(crate::TypeLibWireType::Record {
            name: "TestLib.Point".to_string(),
            record_info: None,
        });
        assert_eq!(
            vtable_gate_decline_reason(&record_ret, 0, Some(crate::TypeLibParamType::Record)),
            Some(VtableDeclineReason::MissingRecordReturnInfo),
            "record retvals require explicit IRecordInfo allocation metadata"
        );
        record_ret.return_wire_type = Some(crate::TypeLibWireType::Record {
            name: "TestLib.Point".to_string(),
            record_info: Some(crate::TypeLibRecordInfo {
                libid: crate::ComInterfaceIid {
                    data1: 0x1111_1111,
                    data2: 0x2222,
                    data3: 0x3333,
                    data4: [4, 5, 6, 7, 8, 9, 10, 11],
                },
                major: 1,
                minor: 0,
                lcid: 0,
                type_guid: crate::ComInterfaceIid {
                    data1: 0xAAAA_AAAA,
                    data2: 0xBBBB,
                    data3: 0xCCCC,
                    data4: [12, 13, 14, 15, 16, 17, 18, 19],
                },
            }),
        });
        assert_eq!(
            vtable_gate_decline_reason(&record_ret, 0, Some(crate::TypeLibParamType::Record)),
            None,
            "record retvals are admitted when the descriptor carries allocation metadata"
        );
    }

    #[test]
    fn gate_admits_explicit_interface_pointer_return_wire_shape() {
        let mut spec = eligible_spec(17, 58);
        spec.return_type = Some(crate::TypeLibParamType::Object);
        spec.return_wire_type = Some(crate::TypeLibWireType::InterfacePointer {
            name: "ITestDispatch".to_string(),
        });
        assert_eq!(
            vtable_gate_decline_reason(&spec, 0, Some(crate::TypeLibParamType::Object)),
            None,
            "explicit interface-pointer object returns are admitted"
        );
        assert!(
            vtable_gate_admits(&spec, 0, Some(crate::TypeLibParamType::Object)),
            "InterfacePointer return metadata should not collapse to IDispatch fallback"
        );
    }

    #[test]
    fn gate_admits_scalar_property_putref_through_common_signature_table() {
        let mut spec = eligible_spec(17, 58);
        spec.invoke_kind = crate::TypeLibMemberInvokeKind::PropertyPutRef;
        spec.parameter_types = vec![crate::TypeLibParamType::Long];
        spec.parameter_wire_types = vec![crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::Long,
        )];
        spec.parameter_iids = vec![None];
        spec.return_type = None;
        spec.return_wire_type = None;

        assert_eq!(
            vtable_gate_decline_reason(&spec, 1, None),
            None,
            "non-object putref shapes should be admitted when the normal signature table supports them"
        );
    }

    #[test]
    fn gate_admits_byref_long_only_with_writeback_slot() {
        let mut spec = eligible_spec(17, 58);
        spec.parameter_types = vec![crate::TypeLibParamType::ByRefLong];
        spec.parameter_wire_types = vec![crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::ByRefLong,
        )];
        spec.parameter_iids = vec![None];
        assert_eq!(
            vtable_gate_decline_reason(&spec, 1, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingByRefSlot)
        );
        let slot = RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::Long));
        assert!(
            build_vtable_invocation_plan_with_byrefs(
                &spec,
                1,
                &[Some(slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .is_ok(),
            "ByRef Long is admitted when the caller supplies a writeback slot"
        );
        let wrong_slot = RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::String));
        assert_eq!(
            build_vtable_invocation_plan_with_byrefs(
                &spec,
                1,
                &[Some(wrong_slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .expect_err("mismatched ByRef slot type must decline"),
            VtableDeclineReason::ByRefSlotTypeMismatch
        );
    }

    #[test]
    fn gate_decline_reasons_cover_abi_shape_predicates() {
        let mut bad_param = eligible_spec(17, 58);
        bad_param.parameter_types = vec![crate::TypeLibParamType::ByRefVariant];
        bad_param.parameter_wire_types = vec![crate::TypeLibWireType::Automation(
            crate::TypeLibParamType::ByRefVariant,
        )];
        assert_eq!(
            vtable_gate_decline_reason(&bad_param, 1, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingByRefSlot)
        );

        let mut object_arg = eligible_spec(17, 58);
        object_arg.parameter_types = vec![crate::TypeLibParamType::Object];
        object_arg.parameter_iids = vec![None];
        assert_eq!(
            vtable_gate_decline_reason(&object_arg, 1, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingObjectParameterIid)
        );

        let mut mismatched_wire_arity = eligible_spec(17, 58);
        mismatched_wire_arity.parameter_types = vec![crate::TypeLibParamType::Variant];
        mismatched_wire_arity.parameter_wire_types = Vec::new();
        assert_eq!(
            vtable_gate_decline_reason(
                &mismatched_wire_arity,
                1,
                Some(crate::TypeLibParamType::Long)
            ),
            None,
            "empty wire metadata remains backward-compatible"
        );
        mismatched_wire_arity.parameter_wire_types = vec![
            crate::TypeLibWireType::Automation(crate::TypeLibParamType::Variant),
            crate::TypeLibWireType::Automation(crate::TypeLibParamType::Variant),
        ];
        assert_eq!(
            vtable_gate_decline_reason(
                &mismatched_wire_arity,
                1,
                Some(crate::TypeLibParamType::Long)
            ),
            Some(VtableDeclineReason::ParameterWireTypeArityMismatch)
        );

        let mut safearray_param = eligible_spec(17, 58);
        safearray_param.parameter_types = vec![crate::TypeLibParamType::Variant];
        safearray_param.parameter_wire_types = vec![crate::TypeLibWireType::SafeArrayVariant];
        assert_eq!(
            vtable_gate_decline_reason(&safearray_param, 1, Some(crate::TypeLibParamType::Long)),
            None,
            "explicit SAFEARRAY parameter wire metadata is admitted"
        );

        let mut record_safearray_param = eligible_spec(17, 58);
        record_safearray_param.parameter_types = vec![crate::TypeLibParamType::Variant];
        record_safearray_param.parameter_wire_types = vec![crate::TypeLibWireType::SafeArray {
            element_vt: 36,
            record_info: None,
        }];
        let empty_record_array = Variant::from_safearray(SafeArray::from_variants(Vec::new()));
        assert_eq!(
            build_vtable_invocation_plan_with_byrefs(
                &record_safearray_param,
                1,
                &[None],
                Some(&[empty_record_array]),
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .expect_err("empty record array lacks IRecordInfo for vtable allocation"),
            VtableDeclineReason::MissingRecordSafeArrayElementInfo,
            "SAFEARRAY(VT_RECORD) without a runtime record element declines before marshalling"
        );
        record_safearray_param.parameter_wire_types = vec![crate::TypeLibWireType::SafeArray {
            element_vt: 36,
            record_info: Some(crate::TypeLibRecordInfo {
                libid: crate::ComInterfaceIid {
                    data1: 0x1111_1111,
                    data2: 0x2222,
                    data3: 0x3333,
                    data4: [0x44; 8],
                },
                major: 1,
                minor: 0,
                lcid: 0,
                type_guid: crate::ComInterfaceIid {
                    data1: 0x5555_5555,
                    data2: 0x6666,
                    data3: 0x7777,
                    data4: [0x88; 8],
                },
            }),
        }];
        let empty_descriptor_record_array =
            Variant::from_safearray(SafeArray::from_variants(Vec::new()));
        build_vtable_invocation_plan_with_byrefs(
            &record_safearray_param,
            1,
            &[None],
            Some(&[empty_descriptor_record_array]),
            Some(crate::TypeLibParamType::Long),
            "method",
        )
        .expect("descriptor-backed empty SAFEARRAY(VT_RECORD) should be admitted");
        let non_record_array =
            Variant::from_safearray(SafeArray::from_variants(vec![Variant::from_i32(1)]));
        assert_eq!(
            build_vtable_invocation_plan_with_byrefs(
                &record_safearray_param,
                1,
                &[None],
                Some(&[non_record_array]),
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .expect_err("non-record array lacks IRecordInfo for vtable allocation"),
            VtableDeclineReason::MissingRecordSafeArrayElementInfo,
            "SAFEARRAY(VT_RECORD) rejects non-record runtime arrays at admission"
        );

        let mut byref_safearray_param = eligible_spec(17, 58);
        byref_safearray_param.parameter_types = vec![crate::TypeLibParamType::Variant];
        byref_safearray_param.parameter_wire_types =
            vec![crate::TypeLibWireType::ByRefSafeArrayVariant];
        assert_eq!(
            vtable_gate_decline_reason(
                &byref_safearray_param,
                1,
                Some(crate::TypeLibParamType::Long)
            ),
            Some(VtableDeclineReason::MissingByRefSlot)
        );
        let byref_safearray_slot =
            RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::Variant));
        assert!(
            build_vtable_invocation_plan_with_byrefs(
                &byref_safearray_param,
                1,
                &[Some(byref_safearray_slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .is_ok(),
            "ByRef SAFEARRAY(VARIANT) is admitted with a writeback slot"
        );

        let mut typed_byref_safearray_param = eligible_spec(17, 58);
        typed_byref_safearray_param.parameter_types = vec![crate::TypeLibParamType::Variant];
        typed_byref_safearray_param.parameter_wire_types =
            vec![crate::TypeLibWireType::ByRefSafeArray {
                element_vt: 3,
                record_info: None,
            }];
        assert_eq!(
            vtable_gate_decline_reason(
                &typed_byref_safearray_param,
                1,
                Some(crate::TypeLibParamType::Long)
            ),
            Some(VtableDeclineReason::MissingByRefSlot),
            "typed ByRef SAFEARRAY metadata also requires a runtime writeback slot"
        );
        assert!(
            build_vtable_invocation_plan_with_byrefs(
                &typed_byref_safearray_param,
                1,
                &[Some(byref_safearray_slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .is_ok(),
            "typed ByRef SAFEARRAY is admitted with a writeback slot"
        );
        let wrong_byref_safearray_slot =
            RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::Long));
        assert_eq!(
            build_vtable_invocation_plan_with_byrefs(
                &typed_byref_safearray_param,
                1,
                &[Some(wrong_byref_safearray_slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .expect_err("mismatched SAFEARRAY writeback slot type must decline"),
            VtableDeclineReason::ByRefSlotTypeMismatch
        );

        let mut record_param = eligible_spec(17, 58);
        record_param.parameter_types = vec![crate::TypeLibParamType::Record];
        record_param.parameter_wire_types = vec![crate::TypeLibWireType::Record {
            name: "TestLib.Point".to_string(),
            record_info: None,
        }];
        assert_eq!(
            vtable_gate_decline_reason(&record_param, 1, Some(crate::TypeLibParamType::Long)),
            None,
            "record parameters are admitted when explicit record wire metadata is present"
        );
        record_param.parameter_wire_types.clear();
        assert_eq!(
            vtable_gate_decline_reason(&record_param, 1, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingRecordParameterWireType),
            "record parameters decline when the descriptor lacks explicit record wire metadata"
        );

        let mut byref_record_param = eligible_spec(17, 58);
        byref_record_param.parameter_types = vec![crate::TypeLibParamType::ByRefRecord];
        byref_record_param.parameter_wire_types = vec![crate::TypeLibWireType::ByRefRecord {
            name: "TestLib.Point".to_string(),
            record_info: None,
        }];
        assert_eq!(
            vtable_gate_decline_reason(&byref_record_param, 1, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::MissingByRefSlot),
            "ByRef record parameters require a runtime writeback slot"
        );
        let record_slot = RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::Record));
        assert!(
            build_vtable_invocation_plan_with_byrefs(
                &byref_record_param,
                1,
                &[Some(record_slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .is_ok(),
            "ByRef record parameters are admitted with explicit wire metadata and a writeback slot"
        );
        byref_record_param.parameter_wire_types.clear();
        assert_eq!(
            build_vtable_invocation_plan_with_byrefs(
                &byref_record_param,
                1,
                &[Some(record_slot)],
                None,
                Some(crate::TypeLibParamType::Long),
                "method",
            )
            .expect_err("missing ByRefRecord wire metadata must decline"),
            VtableDeclineReason::MissingRecordParameterWireType
        );

        let mut safearray_return = eligible_spec(17, 58);
        safearray_return.return_type = Some(crate::TypeLibParamType::Variant);
        safearray_return.return_wire_type = Some(crate::TypeLibWireType::SafeArrayVariant);
        assert_eq!(
            vtable_gate_decline_reason(
                &safearray_return,
                0,
                Some(crate::TypeLibParamType::Variant)
            ),
            None,
            "explicit SAFEARRAY return wire metadata is admitted"
        );

        let mut byref_safearray_return = eligible_spec(17, 58);
        byref_safearray_return.return_type = Some(crate::TypeLibParamType::Variant);
        byref_safearray_return.return_wire_type =
            Some(crate::TypeLibWireType::ByRefSafeArrayVariant);
        assert_eq!(
            vtable_gate_decline_reason(
                &byref_safearray_return,
                0,
                Some(crate::TypeLibParamType::Variant)
            ),
            Some(VtableDeclineReason::UnsupportedReturnWireType)
        );

        let mut unsynthesizable = three_param_optional_variant_spec();
        unsynthesizable.parameter_optional_defaults[2] =
            crate::OptionalParamDefault::OptionalNoDefault;
        unsynthesizable.parameter_types[2] = crate::TypeLibParamType::Long;
        assert_eq!(
            vtable_gate_decline_reason(&unsynthesizable, 1, Some(crate::TypeLibParamType::Object)),
            Some(VtableDeclineReason::UnsynthesizableTrailingArgs)
        );

        let mut scalar_method = three_param_optional_variant_spec();
        scalar_method.return_type = Some(crate::TypeLibParamType::Long);
        assert_eq!(
            vtable_gate_decline_reason(&scalar_method, 1, Some(crate::TypeLibParamType::Long)),
            Some(VtableDeclineReason::ScalarMethodReturnWithSynthesizedArgs)
        );
    }
}

#[cfg(all(target_os = "windows", test))]
mod vba_error_number_tests {
    use super::{
        ComInvokeExceptionInfo, ComInvokeFailure, automation_scode_to_vba_number,
        map_com_hresult_vba_number, vba_number_from_dispatch_codes,
    };

    fn excep_failure(scode: Option<i32>, wcode: Option<u16>) -> ComInvokeFailure {
        ComInvokeFailure {
            label: "method",
            dispid: 0,
            hr: Some(0x8002_0009u32 as i32), // DISP_E_EXCEPTION
            arg_err: None,
            excep: Some(ComInvokeExceptionInfo {
                source: Some("Server".to_string()),
                description: Some("Database already exists.".to_string()),
                help_file: None,
                help_context: None,
                scode,
                wcode,
            }),
            detail: None,
        }
    }

    fn hresult_failure(hr: u32) -> ComInvokeFailure {
        ComInvokeFailure {
            label: "method",
            dispid: 0,
            hr: Some(hr as i32),
            arg_err: None,
            excep: None,
            detail: None,
        }
    }

    #[test]
    fn facility_control_scode_low_word_is_the_vba_number() {
        // 0x800A0C84 -> 0x0C84 = 3204 ("Database already exists.")
        assert_eq!(automation_scode_to_vba_number(0x800A_0C84), 3204);
        // 0x800A01C9 -> 0x01C9 = 457 (duplicate key in Scripting.Dictionary.Add)
        assert_eq!(automation_scode_to_vba_number(0x800A_01C9), 457);
        // 0x800A0009 -> 0x0009 = 9 (subscript out of range)
        assert_eq!(automation_scode_to_vba_number(0x800A_0009), 9);
    }

    #[test]
    fn non_facility_control_scode_falls_back_to_five() {
        // A non-FACILITY_CONTROL HRESULT carries no VBA number → 5.
        assert_eq!(automation_scode_to_vba_number(0x8007_0057), 5);
    }

    #[test]
    fn hresult_table_maps_canonical_automation_codes() {
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_0003)), 438); // DISP_E_MEMBERNOTFOUND
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_0006)), 438); // DISP_E_UNKNOWNNAME
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_000E)), 449); // DISP_E_BADPARAMCOUNT
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_0004)), 449); // DISP_E_PARAMNOTFOUND
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_0005)), 13); // DISP_E_TYPEMISMATCH
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_0008)), 13); // DISP_E_BADVARTYPE
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_000A)), 6); // DISP_E_OVERFLOW
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_000B)), 9); // DISP_E_BADINDEX
        assert_eq!(map_com_hresult_vba_number(Some(0x8002_0012)), 11); // DISP_E_DIVBYZERO
        assert_eq!(map_com_hresult_vba_number(Some(0x8000_4002)), 430); // E_NOINTERFACE
        // FACILITY_CONTROL on the bare-HRESULT path also yields the low word.
        assert_eq!(map_com_hresult_vba_number(Some(0x800A_0C84)), 3204);
        // Default (unrecognized / unspecified) → 5.
        assert_eq!(map_com_hresult_vba_number(Some(0x8000_FFFF)), 5);
        assert_eq!(map_com_hresult_vba_number(None), 5);
    }

    #[test]
    fn excepinfo_scode_takes_priority_over_dispe_exception_hresult() {
        // DISP_E_EXCEPTION (0x80020009) carries the real number in EXCEPINFO.scode.
        let failure = excep_failure(Some(0x800A_0C84u32 as i32), None);
        assert_eq!(failure.vba_error_number(), 3204);
        assert_eq!(failure.vba_description(), Some("Database already exists."));
    }

    #[test]
    fn excepinfo_nonzero_wcode_wins() {
        // A nonzero wCode is the VBA number directly; scode is then ignored.
        let failure = excep_failure(Some(0x800A_0C84u32 as i32), Some(457));
        assert_eq!(failure.vba_error_number(), 457);
    }

    #[test]
    fn member_not_found_hresult_surfaces_438() {
        assert_eq!(hresult_failure(0x8002_0003).vba_error_number(), 438);
    }

    #[test]
    fn unmapped_hresult_defaults_to_five() {
        assert_eq!(hresult_failure(0x8007_0057).vba_error_number(), 5);
    }

    #[test]
    fn dispatch_codes_helper_matches_failure_priority() {
        // wcode wins outright.
        assert_eq!(
            vba_number_from_dispatch_codes(
                Some(0x8002_0009),
                Some(0x800A_0C84u32 as i32),
                Some(457)
            ),
            457
        );
        // A FACILITY_CONTROL scode yields its low word even under DISP_E_EXCEPTION.
        assert_eq!(
            vba_number_from_dispatch_codes(Some(0x8002_0009), Some(0x800A_01C9u32 as i32), None),
            457
        );
        // A DISP_E_* scode (e.g. BADINDEX from a worksheet index over-read) routes
        // through the full automation table, not just FACILITY_CONTROL → 9.
        assert_eq!(
            vba_number_from_dispatch_codes(Some(0x8002_0009), Some(0x8002_000Bu32 as i32), None),
            9
        );
        // No scode/wcode → the bare HRESULT table (BADPARAMCOUNT → 449).
        assert_eq!(
            vba_number_from_dispatch_codes(Some(0x8002_000E), None, None),
            449
        );
        // Nothing at all → 5.
        assert_eq!(vba_number_from_dispatch_codes(None, None, None), 5);
    }
}
