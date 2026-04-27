//! DLL shim source generation with C ABI marshaling.

use oxvba_compiler::DeclareParamType;
use oxvba_project::NativeExportDescriptor;

/// Generate Rust source code for a DLL shim with native exports.
pub fn generate_dll_shim(
    project_name: &str,
    oxb_path: &str,
    exports: &[NativeExportDescriptor],
) -> String {
    let mut source = format!(
        r#"//! Auto-generated OxVBA DLL shim for project "{project_name}".

use std::sync::OnceLock;
use oxvba_compiler::{{DeclareParamType, OxBundle}};
use oxvba_host::{{Engine, HostConfig, ProjectRuntimeSession}};
use oxvba_runtime::{{BStr, F64Value, RuntimeValue}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

static SESSION: OnceLock<std::sync::Mutex<(Engine, ProjectRuntimeSession)>> = OnceLock::new();

fn with_session<F, R>(f: F) -> R
where
    F: FnOnce(&Engine, &mut ProjectRuntimeSession) -> R,
{{
    let pair = SESSION.get_or_init(|| {{
        let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
            .expect("failed to deserialize embedded bundle");
        let engine = Engine::new(HostConfig::default());
        let session = engine.compile_and_prepare_session_from_bundle(&bundle)
            .expect("failed to prepare session from bundle");
        std::sync::Mutex::new((engine, session))
    }});
    let mut guard = pair.lock().expect("session lock poisoned");
    let (ref engine, ref mut session) = *guard;
    f(engine, session)
}}

trait IntoRuntimeArg {{
    fn into_runtime_arg(self, ty: DeclareParamType) -> RuntimeValue;
}}

macro_rules! int_runtime_arg {{
    ($ty:ty) => {{
        impl IntoRuntimeArg for $ty {{
            fn into_runtime_arg(self, ty: DeclareParamType) -> RuntimeValue {{
                match ty {{
                    DeclareParamType::LongLong => RuntimeValue::I64(self as i64),
                    DeclareParamType::LongPtr => RuntimeValue::I64(self as i64),
                    DeclareParamType::Boolean => RuntimeValue::Bool(self != 0),
                    DeclareParamType::Byte
                    | DeclareParamType::Integer
                    | DeclareParamType::Long
                    | DeclareParamType::Currency => RuntimeValue::I32(self as i32),
                    DeclareParamType::Single
                    | DeclareParamType::Double
                    | DeclareParamType::Date => RuntimeValue::F64(F64Value::from_f64(self as f64)),
                    DeclareParamType::String
                    | DeclareParamType::Variant
                    | DeclareParamType::Any => RuntimeValue::I64(self as i64),
                }}
            }}
        }}
    }};
}}

int_runtime_arg!(i16);
int_runtime_arg!(i32);
int_runtime_arg!(i64);
int_runtime_arg!(isize);
int_runtime_arg!(u8);

impl IntoRuntimeArg for f32 {{
    fn into_runtime_arg(self, ty: DeclareParamType) -> RuntimeValue {{
        match ty {{
            DeclareParamType::Single => RuntimeValue::F64(F64Value::from_single_f64(self as f64)),
            DeclareParamType::Date => RuntimeValue::F64(F64Value::from_date_f64(self as f64)),
            _ => RuntimeValue::F64(F64Value::from_f64(self as f64)),
        }}
    }}
}}

impl IntoRuntimeArg for f64 {{
    fn into_runtime_arg(self, ty: DeclareParamType) -> RuntimeValue {{
        match ty {{
            DeclareParamType::Single => RuntimeValue::F64(F64Value::from_single_f64(self)),
            DeclareParamType::Date => RuntimeValue::F64(F64Value::from_date_f64(self)),
            _ => RuntimeValue::F64(F64Value::from_f64(self)),
        }}
    }}
}}

impl IntoRuntimeArg for *const u16 {{
    fn into_runtime_arg(self, _ty: DeclareParamType) -> RuntimeValue {{
        if self.is_null() {{
            return RuntimeValue::String(BStr::from(""));
        }}
        let mut len = 0usize;
        unsafe {{
            while *self.add(len) != 0 {{
                len += 1;
            }}
            RuntimeValue::String(BStr::from(String::from_utf16_lossy(std::slice::from_raw_parts(
                self, len,
            ))))
        }}
    }}
}}

impl IntoRuntimeArg for *mut u8 {{
    fn into_runtime_arg(self, _ty: DeclareParamType) -> RuntimeValue {{
        RuntimeValue::I64(self as isize as i64)
    }}
}}

fn marshal_to_runtime<T: IntoRuntimeArg>(value: T, ty: DeclareParamType) -> RuntimeValue {{
    value.into_runtime_arg(ty)
}}

trait FromRuntimeReturn {{
    fn from_runtime_return(value: RuntimeValue) -> Self;
}}

macro_rules! int_runtime_return {{
    ($ty:ty) => {{
        impl FromRuntimeReturn for $ty {{
            fn from_runtime_return(value: RuntimeValue) -> Self {{
                match value {{
                    RuntimeValue::I32(n) => n as $ty,
                    RuntimeValue::I64(n) => n as $ty,
                    RuntimeValue::Bool(flag) => if flag {{ -1i32 as $ty }} else {{ 0i32 as $ty }},
                    RuntimeValue::F64(value) => value.as_f64() as $ty,
                    _ => 0 as $ty,
                }}
            }}
        }}
    }};
}}

int_runtime_return!(i16);
int_runtime_return!(i32);
int_runtime_return!(i64);
int_runtime_return!(isize);
int_runtime_return!(u8);

impl FromRuntimeReturn for f32 {{
    fn from_runtime_return(value: RuntimeValue) -> Self {{
        match value {{
            RuntimeValue::F64(value) => value.as_f64() as f32,
            RuntimeValue::I32(n) => n as f32,
            RuntimeValue::I64(n) => n as f32,
            _ => 0.0,
        }}
    }}
}}

impl FromRuntimeReturn for f64 {{
    fn from_runtime_return(value: RuntimeValue) -> Self {{
        match value {{
            RuntimeValue::F64(value) => value.as_f64(),
            RuntimeValue::I32(n) => n as f64,
            RuntimeValue::I64(n) => n as f64,
            _ => 0.0,
        }}
    }}
}}

impl FromRuntimeReturn for *const u16 {{
    fn from_runtime_return(value: RuntimeValue) -> Self {{
        let text = match value {{
            RuntimeValue::String(text) => text.as_str().to_string(),
            RuntimeValue::I32(n) => n.to_string(),
            RuntimeValue::I64(n) => n.to_string(),
            RuntimeValue::Bool(flag) => if flag {{ "True".to_string() }} else {{ "False".to_string() }},
            RuntimeValue::F64(value) => value.as_f64().to_string(),
            _ => String::new(),
        }};
        let mut utf16: Vec<u16> = text.encode_utf16().collect();
        utf16.push(0);
        Box::leak(utf16.into_boxed_slice()).as_ptr()
    }}
}}

impl FromRuntimeReturn for *mut u8 {{
    fn from_runtime_return(value: RuntimeValue) -> Self {{
        match value {{
            RuntimeValue::I32(n) => n as isize as *mut u8,
            RuntimeValue::I64(n) => n as isize as *mut u8,
            _ => std::ptr::null_mut(),
        }}
    }}
}}

fn marshal_from_runtime<T: FromRuntimeReturn>(value: RuntimeValue) -> T {{
    T::from_runtime_return(value)
}}

"#
    );

    for export in exports {
        source.push_str(&generate_export_function(export));
        source.push('\n');
    }

    source
}

fn generate_export_function(export: &NativeExportDescriptor) -> String {
    let name = &export.exported_name;
    let cc = match export.calling_convention {
        oxvba_project::CallingConvention::Stdcall => "system",
        oxvba_project::CallingConvention::Cdecl => "C",
    };

    let param_types = export.param_types.as_deref().unwrap_or(&[]);
    let return_type = export.return_type.as_ref().and_then(|r| r.as_ref());

    let params: Vec<String> = param_types
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("arg{}: {}", i, c_type_for(ty)))
        .collect();

    let ret = return_type
        .map(|ty| format!(" -> {}", c_type_for(ty)))
        .unwrap_or_default();

    let marshal_args: Vec<String> = param_types
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("        marshal_to_runtime(arg{i}, DeclareParamType::{ty:?})"))
        .collect();

    let module = &export.module_name;
    let procedure = &export.procedure_name;
    let body = if return_type.is_some() {
        format!(
            r#"    let args: Vec<RuntimeValue> = vec![
{}
    ];
    with_session(|engine, session| {{
        let result = engine.invoke_procedure(session, "{module}", "{procedure}", &args)
            .expect("invoke_procedure failed");
        marshal_from_runtime(result)
    }})"#,
            marshal_args.join(",\n"),
        )
    } else {
        format!(
            r#"    let args: Vec<RuntimeValue> = vec![
{}
    ];
    with_session(|engine, session| {{
        let _ = engine.invoke_procedure(session, "{module}", "{procedure}", &args)
            .expect("invoke_procedure failed");
    }})"#,
            marshal_args.join(",\n"),
        )
    };

    format!(
        r#"#[no_mangle]
pub extern "{cc}" fn {name}({params}){ret} {{
{body}
}}
"#,
        params = params.join(", "),
    )
}

fn c_type_for(ty: &DeclareParamType) -> &'static str {
    match ty {
        DeclareParamType::Long => "i32",
        DeclareParamType::Integer => "i16",
        DeclareParamType::String => "*const u16",
        DeclareParamType::Boolean => "i16",
        DeclareParamType::Double => "f64",
        DeclareParamType::Single => "f32",
        DeclareParamType::Currency => "i64",
        DeclareParamType::Date => "f64",
        DeclareParamType::Byte => "u8",
        DeclareParamType::LongLong => "i64",
        DeclareParamType::LongPtr => "isize",
        DeclareParamType::Variant => "*mut u8",
        DeclareParamType::Any => "*mut u8",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_project::CallingConvention;

    #[test]
    fn dll_shim_generates_export() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "CalcSum".to_string(),
            module_name: "Math".to_string(),
            procedure_name: "Sum".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Long, DeclareParamType::Long]),
            return_type: Some(Some(DeclareParamType::Long)),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_dll_shim("MathLib", "math.oxb", &exports);
        assert!(source.contains("#[no_mangle]"));
        assert!(source.contains("pub extern \"system\" fn CalcSum"));
        assert!(source.contains("arg0: i32"));
        assert!(source.contains("arg1: i32"));
        assert!(source.contains("-> i32"));
        assert!(source.contains("invoke_procedure"));
        assert!(source.contains("use oxvba_compiler::{DeclareParamType, OxBundle};"));
        assert!(source.contains("fn marshal_to_runtime<T: IntoRuntimeArg>"));
        assert!(source.contains("fn marshal_from_runtime<T: FromRuntimeReturn>"));
        assert!(source.contains("\"Math\""));
        assert!(source.contains("\"Sum\""));
    }

    #[test]
    fn dll_shim_sub_has_no_return() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "DoWork".to_string(),
            module_name: "Mod1".to_string(),
            procedure_name: "DoWork".to_string(),
            calling_convention: CallingConvention::Cdecl,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Sub),
            param_types: Some(vec![]),
            return_type: Some(None),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_dll_shim("Lib", "lib.oxb", &exports);
        assert!(source.contains("pub extern \"C\" fn DoWork()"));
        // The function itself should not have a return type
        assert!(source.contains("fn DoWork() {\n"));
    }
}
