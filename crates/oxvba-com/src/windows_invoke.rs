#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{SysFreeString, SysStringLen};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::EXCEPINFO;

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeExceptionInfo {
    pub source: Option<String>,
    pub description: Option<String>,
    pub scode: Option<i32>,
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
impl ComInvokeFailure {
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
            if let Some(scode) = excep.scode {
                message.push_str(&format!(" excep_scode={:#010X}", scode as u32));
            }
        }
        if let Some(detail) = &self.detail {
            message.push_str(&format!(" detail=\"{}\"", sanitize_error_text(detail)));
        }
        message
    }
}

#[cfg(target_os = "windows")]
fn sanitize_error_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn bstr_to_string_and_free(bstr: windows_sys::core::BSTR) -> Option<String> {
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
    let _ = bstr_to_string_and_free(excep.bstrHelpFile);
    excep.bstrSource = std::ptr::null_mut();
    excep.bstrDescription = std::ptr::null_mut();
    excep.bstrHelpFile = std::ptr::null_mut();
    let scode = if excep.scode != 0 {
        Some(excep.scode)
    } else {
        None
    };
    if source.is_none() && description.is_none() && scode.is_none() {
        None
    } else {
        Some(ComInvokeExceptionInfo {
            source,
            description,
            scode,
        })
    }
}
