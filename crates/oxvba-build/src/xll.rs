//! XLL entry point and function registration code generation.

use oxvba_project::NativeExportDescriptor;

use crate::xloper;

/// Generate Rust source code for an XLL add-in shim.
pub fn generate_xll_shim(
    project_name: &str,
    oxb_path: &str,
    exports: &[NativeExportDescriptor],
) -> String {
    let mut source = format!(
        r#"//! Auto-generated OxVBA XLL add-in for project "{project_name}".

#![allow(non_snake_case)]

use std::sync::OnceLock;

use oxvba_compiler::{{DeclareParamType, OxBundle}};
use oxvba_host::{{Engine, HostConfig, ProjectRuntimeSession}};
use oxvba_runtime::{{BStr, F64Value, RuntimeValue}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

#[repr(C)]
pub struct XLOPER12 {{
    pub xltype: u32,
    pub value: usize,
}}

struct XllRegistration {{
    procedure: &'static str,
    type_text: &'static str,
    function_text: &'static str,
    argument_text: &'static str,
    category: &'static str,
    function_help: &'static str,
}}

const XLF_REGISTER: i32 = 149;
const XL_TYPE_NUM: u32 = 0x0001;
const XL_TYPE_STR: u32 = 0x0002;
const XL_TYPE_BOOL: u32 = 0x0004;
const XL_TYPE_INT: u32 = 0x0020;
const XL_TYPE_NIL: u32 = 0x0100;

static SESSION: OnceLock<std::sync::Mutex<(Engine, ProjectRuntimeSession)>> = OnceLock::new();

fn with_session<F, R>(f: F) -> R
where
    F: FnOnce(&Engine, &mut ProjectRuntimeSession) -> R,
{{
    let pair = SESSION.get_or_init(|| {{
        let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
            .expect("failed to deserialize embedded bundle");
        let engine = Engine::new(HostConfig::default());
        let session = engine
            .compile_and_prepare_session_from_bundle(&bundle)
            .expect("failed to prepare XLL session from bundle");
        std::sync::Mutex::new((engine, session))
    }});
    let mut guard = pair.lock().expect("XLL session lock poisoned");
    let (ref engine, ref mut session) = *guard;
    f(engine, session)
}}

#[cfg(target_os = "windows")]
unsafe extern "system" {{
    fn Excel12v(
        xlfn: i32,
        oper_result: *mut XLOPER12,
        count: i32,
        opers: *mut *mut XLOPER12,
    ) -> i32;
}}

"#
    );

    source.push_str(&generate_registration_table(project_name, exports));

    for export in exports {
        source.push_str(&generate_export_wrapper(export));
    }

    // xlAutoOpen
    source.push_str(&generate_xl_auto_open());

    // xlAutoClose
    source.push_str(
        r#"
#[no_mangle]
pub extern "system" fn xlAutoClose() -> i32 {
    // Cleanup: unregister functions
    1
}

"#,
    );

    // xlAutoFree12
    source.push_str(
        r#"#[no_mangle]
pub extern "system" fn xlAutoFree12(p: *mut u8) {
    // Free XLOPER12 memory allocated by the add-in
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)); }
    }
}

"#,
    );

    source.push_str(generate_xll_runtime_helpers());

    source
}

fn generate_registration_table(project_name: &str, exports: &[NativeExportDescriptor]) -> String {
    let mut source = String::from("const REGISTRATIONS: &[XllRegistration] = &[\n");

    for export in exports {
        let param_types = export.param_types.as_deref().unwrap_or(&[]);
        let return_type = export.return_type.as_ref().and_then(|r| r.as_ref());
        let type_string = xloper::build_type_string(param_types, return_type);
        let category = export.category.as_deref().unwrap_or(project_name);
        let function_help = export.description.as_deref().unwrap_or("");
        let argument_text = export.argument_descriptions.as_deref().unwrap_or("");

        source.push_str(&format!(
            "    XllRegistration {{ procedure: {}, type_text: {}, function_text: {}, argument_text: {}, category: {}, function_help: {} }},\n",
            rust_string_literal(&export.exported_name),
            rust_string_literal(&type_string),
            rust_string_literal(&export.exported_name),
            rust_string_literal(argument_text),
            rust_string_literal(category),
            rust_string_literal(function_help),
        ));
    }

    source.push_str("];\n\n");
    source
}

fn generate_xl_auto_open() -> String {
    String::from(
        r#"#[no_mangle]
pub extern "system" fn xlAutoOpen() -> i32 {
    for registration in REGISTRATIONS {
        if !register_xll_function(registration) {
            return 0;
        }
    }
    1
}

#[cfg(target_os = "windows")]
fn register_xll_function(registration: &XllRegistration) -> bool {
    let mut result = XLOPER12 { xltype: 0, value: 0 };
    let mut procedure = xll_string(registration.procedure);
    let mut type_text = xll_string(registration.type_text);
    let mut function_text = xll_string(registration.function_text);
    let mut argument_text = xll_string(registration.argument_text);
    let mut category = xll_string(registration.category);
    let mut function_help = xll_string(registration.function_help);
    let mut args = [
        &mut procedure as *mut XLOPER12,
        &mut type_text as *mut XLOPER12,
        &mut function_text as *mut XLOPER12,
        &mut argument_text as *mut XLOPER12,
        &mut category as *mut XLOPER12,
        &mut function_help as *mut XLOPER12,
    ];
    unsafe { Excel12v(XLF_REGISTER, &mut result, args.len() as i32, args.as_mut_ptr()) == 0 }
}

#[cfg(not(target_os = "windows"))]
fn register_xll_function(_registration: &XllRegistration) -> bool {
    true
}

fn xll_string(text: &str) -> XLOPER12 {
    XLOPER12 {
        xltype: 0x0002,
        value: text.as_ptr() as usize,
    }
}

"#,
    )
}

fn generate_export_wrapper(export: &NativeExportDescriptor) -> String {
    let name = &export.exported_name;
    let params = export.param_types.as_deref().unwrap_or(&[]);
    let module = &export.module_name;
    let procedure = &export.procedure_name;

    let signature_params = params
        .iter()
        .enumerate()
        .map(|(i, _)| format!("arg{i}: *const XLOPER12"))
        .collect::<Vec<_>>()
        .join(", ");
    let marshal_args = params
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("        xll_arg_to_runtime(arg{i}, DeclareParamType::{ty:?})"))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        r#"#[no_mangle]
pub extern "system" fn {name}({signature_params}) -> *mut XLOPER12 {{
    let args: Vec<RuntimeValue> = vec![
{marshal_args}
    ];
    let result = with_session(|engine, session| {{
        engine
            .invoke_procedure(session, "{module}", "{procedure}", &args)
            .expect("XLL procedure invocation failed")
    }});
    runtime_to_xll(result)
}}

"#
    )
}

fn generate_xll_runtime_helpers() -> &'static str {
    r#"fn xll_arg_to_runtime(arg: *const XLOPER12, ty: DeclareParamType) -> RuntimeValue {
    if arg.is_null() {
        return RuntimeValue::Empty;
    }
    let value = unsafe { &*arg };
    match ty {
        DeclareParamType::Double | DeclareParamType::Date => {
            RuntimeValue::F64(F64Value::from_f64(value.value as f64))
        }
        DeclareParamType::Single => RuntimeValue::F64(F64Value::from_single_f64(value.value as f64)),
        DeclareParamType::Boolean => RuntimeValue::Bool(value.value != 0),
        DeclareParamType::LongLong | DeclareParamType::LongPtr => RuntimeValue::I64(value.value as i64),
        DeclareParamType::String => RuntimeValue::String(BStr::from("")),
        DeclareParamType::Byte
        | DeclareParamType::Currency
        | DeclareParamType::Integer
        | DeclareParamType::Long => RuntimeValue::I32(value.value as i32),
        DeclareParamType::Variant | DeclareParamType::Any => RuntimeValue::I64(value.value as i64),
    }
}

fn runtime_to_xll(value: RuntimeValue) -> *mut XLOPER12 {
    let result = match value {
        RuntimeValue::I32(value) => XLOPER12 {
            xltype: XL_TYPE_INT,
            value: value as isize as usize,
        },
        RuntimeValue::I64(value) => XLOPER12 {
            xltype: XL_TYPE_INT,
            value: value as isize as usize,
        },
        RuntimeValue::F64(value) => XLOPER12 {
            xltype: XL_TYPE_NUM,
            value: value.as_f64().to_bits() as usize,
        },
        RuntimeValue::Bool(value) => XLOPER12 {
            xltype: XL_TYPE_BOOL,
            value: usize::from(value),
        },
        RuntimeValue::String(text) => XLOPER12 {
            xltype: XL_TYPE_STR,
            value: text.as_str().as_ptr() as usize,
        },
        _ => XLOPER12 {
            xltype: XL_TYPE_NIL,
            value: 0,
        },
    };
    Box::into_raw(Box::new(result))
}

"#
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::DeclareParamType;
    use oxvba_project::CallingConvention;

    #[test]
    fn xll_shim_has_required_entry_points() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "MyFunc".to_string(),
            module_name: "Mod1".to_string(),
            procedure_name: "MyFunc".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Double]),
            return_type: Some(Some(DeclareParamType::Double)),
            category: Some("Pricing".to_string()),
            description: Some("Calculates a value".to_string()),
            argument_descriptions: Some("spot".to_string()),
        }];

        let source = generate_xll_shim("TestAddin", "test.oxb", &exports);
        assert!(source.contains("xlAutoOpen"));
        assert!(source.contains("xlAutoClose"));
        assert!(source.contains("xlAutoFree12"));
        assert!(source.contains("MyFunc"));
        assert!(source.contains("XLF_REGISTER"));
        assert!(source.contains("Excel12v"));
        assert!(source.contains("REGISTRATIONS"));
        assert!(source.contains("type_text: \"BB\""));
        assert!(source.contains("category: \"Pricing\""));
        assert!(source.contains("function_help: \"Calculates a value\""));
        assert!(source.contains("argument_text: \"spot\""));
        assert!(source.contains("pub extern \"system\" fn MyFunc(arg0: *const XLOPER12)"));
        assert!(source.contains("xll_arg_to_runtime(arg0, DeclareParamType::Double)"));
        assert!(source.contains(".invoke_procedure(session, \"Mod1\", \"MyFunc\", &args)"));
        assert!(source.contains("fn runtime_to_xll(value: RuntimeValue) -> *mut XLOPER12"));
    }

    #[test]
    fn xll_registration_type_string() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "Add".to_string(),
            module_name: "Math".to_string(),
            procedure_name: "Add".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Double, DeclareParamType::Double]),
            return_type: Some(Some(DeclareParamType::Double)),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_xll_shim("Math", "math.oxb", &exports);
        assert!(source.contains("type_text: \"BBB\""));
    }
}
