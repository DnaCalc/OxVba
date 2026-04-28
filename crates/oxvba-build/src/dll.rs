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

use std::cell::RefCell;
use oxvba_compiler::{{DeclareParamType, OxBundle}};
use oxvba_host::{{Engine, HostConfig, ProjectRuntimeSession}};
use oxvba_runtime::{{bstr::BStr, Variant}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

thread_local! {{
    static SESSION: RefCell<Option<(Engine, ProjectRuntimeSession)>> = RefCell::new(None);
}}

fn with_session<F, R>(f: F) -> R
where
    F: FnOnce(&Engine, &mut ProjectRuntimeSession) -> R,
{{
    SESSION.with(|cell| {{
        let mut slot = cell.borrow_mut();
        if slot.is_none() {{
            let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
                .expect("failed to deserialize embedded bundle");
            let engine = Engine::new(HostConfig::default());
            let session = engine.compile_and_prepare_session_from_bundle(&bundle)
                .expect("failed to prepare session from bundle");
            *slot = Some((engine, session));
        }}
        let (engine, session) = slot.as_mut().expect("session initialized");
        f(engine, session)
    }})
}}

trait IntoVariantArg {{
    fn into_variant_arg(self, ty: DeclareParamType) -> Variant;
}}

macro_rules! int_runtime_arg {{
    ($ty:ty) => {{
        impl IntoVariantArg for $ty {{
            fn into_variant_arg(self, ty: DeclareParamType) -> Variant {{
                match ty {{
                    DeclareParamType::LongLong => Variant::from_i64(self as i64),
                    DeclareParamType::LongPtr => Variant::from_i64(self as i64),
                    DeclareParamType::Boolean => Variant::from_bool(self != 0),
                    DeclareParamType::Byte
                    | DeclareParamType::Integer
                    | DeclareParamType::Long
                    | DeclareParamType::Currency => Variant::from_i32(self as i32),
                    DeclareParamType::Single => Variant::from_f32(self as f32),
                    DeclareParamType::Double => Variant::from_f64(self as f64),
                    DeclareParamType::Date => Variant::from_date_f64(self as f64),
                    DeclareParamType::String
                    | DeclareParamType::Variant
                    | DeclareParamType::Any => Variant::from_i64(self as i64),
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

impl IntoVariantArg for f32 {{
    fn into_variant_arg(self, ty: DeclareParamType) -> Variant {{
        match ty {{
            DeclareParamType::Single => Variant::from_f32(self),
            DeclareParamType::Date => Variant::from_date_f64(self as f64),
            _ => Variant::from_f64(self as f64),
        }}
    }}
}}

impl IntoVariantArg for f64 {{
    fn into_variant_arg(self, ty: DeclareParamType) -> Variant {{
        match ty {{
            DeclareParamType::Single => Variant::from_f32(self as f32),
            DeclareParamType::Date => Variant::from_date_f64(self),
            _ => Variant::from_f64(self),
        }}
    }}
}}

impl IntoVariantArg for *const u16 {{
    fn into_variant_arg(self, _ty: DeclareParamType) -> Variant {{
        if self.is_null() {{
            return Variant::from_string(BStr::from(""));
        }}
        let mut len = 0usize;
        unsafe {{
            while *self.add(len) != 0 {{
                len += 1;
            }}
            Variant::from_string(BStr::from(String::from_utf16_lossy(std::slice::from_raw_parts(
                self, len,
            ))))
        }}
    }}
}}

impl IntoVariantArg for *mut u8 {{
    fn into_variant_arg(self, _ty: DeclareParamType) -> Variant {{
        Variant::from_i64(self as isize as i64)
    }}
}}

fn marshal_to_variant<T: IntoVariantArg>(value: T, ty: DeclareParamType) -> Variant {{
    value.into_variant_arg(ty)
}}

trait FromVariantReturn {{
    fn from_variant_return(value: Variant) -> Self;
}}

macro_rules! int_runtime_return {{
    ($ty:ty) => {{
        impl FromVariantReturn for $ty {{
            fn from_variant_return(value: Variant) -> Self {{
                if let Some(n) = value.as_i32() {{
                    n as $ty
                }} else if let Some(n) = value.as_i64() {{
                    n as $ty
                }} else if let Some(flag) = value.as_bool() {{
                    if flag {{ -1i32 as $ty }} else {{ 0i32 as $ty }}
                }} else if let Some(n) = value.as_f64() {{
                    n as $ty
                }} else if let Some(n) = value.as_f32() {{
                    n as $ty
                }} else {{
                    0 as $ty
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

impl FromVariantReturn for f32 {{
    fn from_variant_return(value: Variant) -> Self {{
        value.as_f32()
            .or_else(|| value.as_f64().map(|value| value as f32))
            .or_else(|| value.as_i32().map(|value| value as f32))
            .or_else(|| value.as_i64().map(|value| value as f32))
            .unwrap_or(0.0)
    }}
}}

impl FromVariantReturn for f64 {{
    fn from_variant_return(value: Variant) -> Self {{
        value.as_f64()
            .or_else(|| value.as_f32().map(|value| value as f64))
            .or_else(|| value.as_i32().map(|value| value as f64))
            .or_else(|| value.as_i64().map(|value| value as f64))
            .unwrap_or(0.0)
    }}
}}

impl FromVariantReturn for *const u16 {{
    fn from_variant_return(value: Variant) -> Self {{
        let text = if let Some(text) = value.as_bstr() {{
            text.as_str().to_string()
        }} else if let Some(n) = value.as_i32() {{
            n.to_string()
        }} else if let Some(n) = value.as_i64() {{
            n.to_string()
        }} else if let Some(flag) = value.as_bool() {{
            if flag {{ "True".to_string() }} else {{ "False".to_string() }}
        }} else if let Some(n) = value.as_f64() {{
            n.to_string()
        }} else if let Some(n) = value.as_f32() {{
            n.to_string()
        }} else {{
            String::new()
        }};
        let mut utf16: Vec<u16> = text.encode_utf16().collect();
        utf16.push(0);
        Box::leak(utf16.into_boxed_slice()).as_ptr()
    }}
}}

impl FromVariantReturn for *mut u8 {{
    fn from_variant_return(value: Variant) -> Self {{
        if let Some(n) = value.as_i32() {{
            n as isize as *mut u8
        }} else if let Some(n) = value.as_i64() {{
            n as isize as *mut u8
        }} else {{
            std::ptr::null_mut()
        }}
    }}
}}

fn marshal_from_variant<T: FromVariantReturn>(value: Variant) -> T {{
    T::from_variant_return(value)
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
        .map(|(i, ty)| format!("        marshal_to_variant(arg{i}, DeclareParamType::{ty:?})"))
        .collect();

    let module = &export.module_name;
    let procedure = &export.procedure_name;
    let body = if return_type.is_some() {
        format!(
            r#"    let args: Vec<Variant> = vec![
{}
    ];
    with_session(|engine, session| {{
        let result = engine.invoke_procedure_with_variants(session, "{module}", "{procedure}", &args)
            .expect("invoke_procedure_with_variants failed");
        marshal_from_variant(result)
    }})"#,
            marshal_args.join(",\n"),
        )
    } else {
        format!(
            r#"    let args: Vec<Variant> = vec![
{}
    ];
    with_session(|engine, session| {{
        let _ = engine.invoke_procedure_with_variants(session, "{module}", "{procedure}", &args)
            .expect("invoke_procedure_with_variants failed");
    }})"#,
            marshal_args.join(",\n"),
        )
    };

    format!(
        r#"#[unsafe(no_mangle)]
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
        assert!(source.contains("#[unsafe(no_mangle)]"));
        assert!(source.contains("pub extern \"system\" fn CalcSum"));
        assert!(source.contains("arg0: i32"));
        assert!(source.contains("arg1: i32"));
        assert!(source.contains("-> i32"));
        assert!(source.contains("invoke_procedure_with_variants"));
        assert!(source.contains("use oxvba_compiler::{DeclareParamType, OxBundle};"));
        assert!(source.contains("use oxvba_runtime::{bstr::BStr, Variant};"));
        assert!(source.contains("thread_local!"));
        assert!(source.contains("let args: Vec<Variant>"));
        assert!(source.contains("fn marshal_to_variant<T: IntoVariantArg>"));
        assert!(source.contains("fn marshal_from_variant<T: FromVariantReturn>"));
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

    #[test]
    fn dll_shim_with_export_compiles_to_dll_artifact() {
        let temp_root =
            std::env::temp_dir().join(format!("oxvba_dll_compile_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).expect("create temp root");
        let bundle_path = temp_root.join("dummy.oxb");
        std::fs::write(&bundle_path, b"dummy bundle bytes").expect("write dummy bundle");
        let bundle_literal = bundle_path.to_string_lossy().replace('\\', "/");
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
        let source = generate_dll_shim("CompileProbe", &bundle_literal, &exports);
        let output_path = temp_root.join("CompileProbe.dll");

        crate::compile::compile_shim(&source, &output_path, crate::compile::ShimOutputType::Dll)
            .expect("compile generated DLL shim with exports");

        assert!(output_path.exists());
        assert!(std::fs::metadata(&output_path).expect("dll metadata").len() > 0);
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
