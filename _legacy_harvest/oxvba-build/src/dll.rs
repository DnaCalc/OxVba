//! DLL shim source generation with C ABI marshaling.

use oxvba_compiler::{DeclareParamType, ProcedureDescriptor, ProjectReflection, VbaType};
use oxvba_project::NativeExportDescriptor;

use crate::wrapper_plan::{
    CallableSelectionPlan, ProjectReflectionInput, WrapperConversionLane, WrapperDiagnosticsPolicy,
    WrapperGenerationPlan, WrapperOutputKind, WrapperPlanDiagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedNativeLibraryProfile {
    pub plan: WrapperGenerationPlan,
    pub thunks: Vec<NativeThunkPlan>,
    pub diagnostics: Vec<WrapperPlanDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeThunkPlan {
    pub exported_name: String,
    pub callable_id: String,
    pub module_name: String,
    pub procedure_name: String,
    pub param_types: Vec<DeclareParamType>,
    pub return_type: Option<DeclareParamType>,
}

pub fn build_wrapped_native_library_profile(
    project_name: &str,
    reflection: ProjectReflection,
    exports: &[NativeExportDescriptor],
) -> WrappedNativeLibraryProfile {
    let mut callable_ids = Vec::new();
    let mut thunks = Vec::new();
    let mut diagnostics = Vec::new();

    for export in exports {
        let Some(procedure) = reflection.procedures.iter().find(|candidate| {
            candidate
                .module_name
                .eq_ignore_ascii_case(&export.module_name)
                && candidate
                    .procedure_name
                    .eq_ignore_ascii_case(&export.procedure_name)
        }) else {
            diagnostics.push(WrapperPlanDiagnostic {
                code: "WRAPPED-NATIVE-CALLABLE-NOT-FOUND".to_string(),
                message: format!(
                    "{}.{} was not found in neutral reflection",
                    export.module_name, export.procedure_name
                ),
                callable_id: None,
            });
            continue;
        };
        let Some(thunk) = native_thunk_from_descriptor(export, procedure, &mut diagnostics) else {
            continue;
        };
        callable_ids.push(procedure.callable_id.clone());
        thunks.push(thunk);
    }

    let plan = WrapperGenerationPlan {
        plan_id: format!("wrapped-native-library:{project_name}"),
        input: ProjectReflectionInput {
            project_name: project_name.to_string(),
            reflection,
        },
        output_kind: WrapperOutputKind::NativeLibrary,
        callable_selection: CallableSelectionPlan::ExplicitCallableIds(callable_ids),
        conversion_lanes: vec![WrapperConversionLane::typed_scalar_first_tier()],
        diagnostics_policy: WrapperDiagnosticsPolicy {
            lane: "build-time-wrapper-diagnostics".to_string(),
            include_descriptor_identity: true,
            fail_on_unsupported_callable: true,
        },
        argument_parser: None,
    };

    WrappedNativeLibraryProfile {
        plan,
        thunks,
        diagnostics,
    }
}

pub fn generate_dll_shim_from_wrapper_profile(
    project_name: &str,
    oxb_path: &str,
    profile: &WrappedNativeLibraryProfile,
) -> String {
    let mut source = format!(
        r#"//! Auto-generated OxVBA wrapped native library shim for project "{project_name}".

use std::cell::RefCell;
use oxvba_host::{{PreparedVbaProject, ProjectSource, TypedValue, VbaHost}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

thread_local! {{
    static SESSION: RefCell<Option<PreparedVbaProject>> = RefCell::new(None);
}}

fn with_project<F, R>(f: F) -> R
where
    F: FnOnce(&mut PreparedVbaProject) -> R,
{{
    SESSION.with(|cell| {{
        let mut slot = cell.borrow_mut();
        if slot.is_none() {{
            let host = VbaHost::default();
            let loaded = host.load_project(ProjectSource::BundleBytes(BUNDLE_BYTES.to_vec()))
                .expect("failed to load embedded bundle");
            *slot = Some(loaded.prepare().expect("failed to prepare embedded bundle"));
        }}
        f(slot.as_mut().expect("prepared project initialized"))
    }})
}}

"#
    );
    source.push_str(
        r#"fn typed_return_to_native_long(value: TypedValue) -> i32 {
    match value { TypedValue::Long(value) => value, TypedValue::Integer(value) => value as i32, TypedValue::LongLong(value) => value as i32, _ => 0 }
}
fn typed_return_to_native_integer(value: TypedValue) -> i16 { typed_return_to_native_long(value) as i16 }
fn typed_return_to_native_longlong(value: TypedValue) -> i64 {
    match value { TypedValue::Long(value) => value as i64, TypedValue::Integer(value) => value as i64, TypedValue::LongLong(value) => value, _ => 0 }
}
fn typed_return_to_native_double(value: TypedValue) -> f64 {
    match value { TypedValue::Double(value) => value, TypedValue::Single(value) => value as f64, TypedValue::Long(value) => value as f64, _ => 0.0 }
}
fn typed_return_to_native_single(value: TypedValue) -> f32 { typed_return_to_native_double(value) as f32 }
fn typed_return_to_native_boolean(value: TypedValue) -> i16 {
    match value { TypedValue::Boolean(true) => -1, TypedValue::Boolean(false) => 0, _ => 0 }
}
fn typed_return_to_native_string(_value: TypedValue) -> *const u16 { std::ptr::null() }
fn typed_return_to_native_variant(_value: TypedValue) -> *mut u8 { std::ptr::null_mut() }

"#,
    );
    for thunk in &profile.thunks {
        source.push_str(&generate_neutral_native_thunk(thunk));
        source.push('\n');
    }
    source
}

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

fn native_thunk_from_descriptor(
    export: &NativeExportDescriptor,
    procedure: &ProcedureDescriptor,
    diagnostics: &mut Vec<WrapperPlanDiagnostic>,
) -> Option<NativeThunkPlan> {
    let mut param_types = Vec::new();
    for (index, param) in procedure.signature.parameters.iter().enumerate() {
        let Some(declare_type) = param
            .value_type
            .as_ref()
            .and_then(|ty| declare_type_from_vba_type(&ty.normalized))
        else {
            diagnostics.push(WrapperPlanDiagnostic {
                code: "WRAPPED-NATIVE-UNSUPPORTED-SIGNATURE".to_string(),
                message: format!(
                    "{}.{} parameter {index} is not supported by native wrapper profile",
                    procedure.module_name, procedure.procedure_name
                ),
                callable_id: Some(procedure.callable_id.clone()),
            });
            return None;
        };
        param_types.push(declare_type);
    }
    let return_type = match procedure.signature.return_type.as_ref() {
        Some(ty) => match declare_type_from_vba_type(&ty.normalized) {
            Some(ty) => Some(ty),
            None => {
                diagnostics.push(WrapperPlanDiagnostic {
                    code: "WRAPPED-NATIVE-UNSUPPORTED-SIGNATURE".to_string(),
                    message: format!(
                        "{}.{} return type is not supported by native wrapper profile",
                        procedure.module_name, procedure.procedure_name
                    ),
                    callable_id: Some(procedure.callable_id.clone()),
                });
                return None;
            }
        },
        None => None,
    };
    Some(NativeThunkPlan {
        exported_name: export.exported_name.clone(),
        callable_id: procedure.callable_id.clone(),
        module_name: procedure.module_name.clone(),
        procedure_name: procedure.procedure_name.clone(),
        param_types,
        return_type,
    })
}

fn declare_type_from_vba_type(ty: &VbaType) -> Option<DeclareParamType> {
    Some(match ty {
        VbaType::Long => DeclareParamType::Long,
        VbaType::Integer => DeclareParamType::Integer,
        VbaType::String => DeclareParamType::String,
        VbaType::Boolean => DeclareParamType::Boolean,
        VbaType::Double => DeclareParamType::Double,
        VbaType::Single => DeclareParamType::Single,
        VbaType::Currency => DeclareParamType::Currency,
        VbaType::Date => DeclareParamType::Date,
        VbaType::Byte => DeclareParamType::Byte,
        VbaType::LongLong => DeclareParamType::LongLong,
        VbaType::LongPtr => DeclareParamType::LongPtr,
        VbaType::Variant
        | VbaType::Any
        | VbaType::Object
        | VbaType::Array
        | VbaType::UserDefined(_)
        | VbaType::Unknown => return None,
    })
}

fn generate_neutral_native_thunk(thunk: &NativeThunkPlan) -> String {
    let params = thunk
        .param_types
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("arg{index}: {}", c_type_for(ty)))
        .collect::<Vec<_>>();
    let typed_args = thunk
        .param_types
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("        {},", typed_value_expr_for_arg(index, ty)))
        .collect::<Vec<_>>()
        .join("\n");
    let return_type = thunk
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", c_type_for(ty)))
        .unwrap_or_default();
    let return_expr = if let Some(ty) = thunk.return_type.as_ref() {
        format!(
            "typed_return_to_native_{}(result.value)",
            declare_param_type_suffix(ty)
        )
    } else {
        "()".to_string()
    };
    format!(
        r#"#[unsafe(no_mangle)]
pub extern "system" fn {name}({params}){return_type} {{
    let args = vec![
{typed_args}
    ];
    with_project(|project| {{
        let result = project.invoke_callable_typed("{callable_id}", Default::default(), &args)
            .expect("invoke_callable_typed failed");
        {return_expr}
    }})
}}
"#,
        name = thunk.exported_name,
        params = params.join(", "),
        callable_id = thunk.callable_id,
    )
}

fn typed_value_expr_for_arg(index: usize, ty: &DeclareParamType) -> String {
    match ty {
        DeclareParamType::Long => format!("TypedValue::Long(arg{index})"),
        DeclareParamType::Integer => format!("TypedValue::Integer(arg{index})"),
        DeclareParamType::String => {
            format!("TypedValue::String(String::new()) /* arg{index} string pointer */")
        }
        DeclareParamType::Boolean => format!("TypedValue::Boolean(arg{index} != 0)"),
        DeclareParamType::Double => format!("TypedValue::Double(arg{index})"),
        DeclareParamType::Single => format!("TypedValue::Single(arg{index})"),
        DeclareParamType::Currency => format!("TypedValue::LongLong(arg{index})"),
        DeclareParamType::Date => format!("TypedValue::Double(arg{index})"),
        DeclareParamType::Byte => format!("TypedValue::Long(arg{index} as i32)"),
        DeclareParamType::LongLong | DeclareParamType::LongPtr => {
            format!("TypedValue::LongLong(arg{index} as i64)")
        }
        DeclareParamType::Variant | DeclareParamType::Any => {
            format!("TypedValue::Empty /* unsupported arg{index} */")
        }
    }
}

fn declare_param_type_suffix(ty: &DeclareParamType) -> &'static str {
    match ty {
        DeclareParamType::Long => "long",
        DeclareParamType::Integer => "integer",
        DeclareParamType::String => "string",
        DeclareParamType::Boolean => "boolean",
        DeclareParamType::Double => "double",
        DeclareParamType::Single => "single",
        DeclareParamType::Currency => "longlong",
        DeclareParamType::Date => "double",
        DeclareParamType::Byte => "long",
        DeclareParamType::LongLong => "longlong",
        DeclareParamType::LongPtr => "longlong",
        DeclareParamType::Variant | DeclareParamType::Any => "variant",
    }
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

#[allow(dead_code)]
fn typed_return_to_native_long(value: oxvba_host::TypedValue) -> i32 {
    match value {
        oxvba_host::TypedValue::Long(value) => value,
        oxvba_host::TypedValue::Integer(value) => value as i32,
        oxvba_host::TypedValue::LongLong(value) => value as i32,
        _ => 0,
    }
}

#[allow(dead_code)]
fn typed_return_to_native_integer(value: oxvba_host::TypedValue) -> i16 {
    typed_return_to_native_long(value) as i16
}

#[allow(dead_code)]
fn typed_return_to_native_longlong(value: oxvba_host::TypedValue) -> i64 {
    match value {
        oxvba_host::TypedValue::Long(value) => value as i64,
        oxvba_host::TypedValue::Integer(value) => value as i64,
        oxvba_host::TypedValue::LongLong(value) => value,
        _ => 0,
    }
}

#[allow(dead_code)]
fn typed_return_to_native_double(value: oxvba_host::TypedValue) -> f64 {
    match value {
        oxvba_host::TypedValue::Double(value) => value,
        oxvba_host::TypedValue::Single(value) => value as f64,
        oxvba_host::TypedValue::Long(value) => value as f64,
        _ => 0.0,
    }
}

#[allow(dead_code)]
fn typed_return_to_native_single(value: oxvba_host::TypedValue) -> f32 {
    typed_return_to_native_double(value) as f32
}

#[allow(dead_code)]
fn typed_return_to_native_boolean(value: oxvba_host::TypedValue) -> i16 {
    match value {
        oxvba_host::TypedValue::Boolean(true) => -1,
        oxvba_host::TypedValue::Boolean(false) => 0,
        _ => 0,
    }
}

#[allow(dead_code)]
fn typed_return_to_native_string(_value: oxvba_host::TypedValue) -> *const u16 {
    std::ptr::null()
}

#[allow(dead_code)]
fn typed_return_to_native_variant(_value: oxvba_host::TypedValue) -> *mut u8 {
    std::ptr::null_mut()
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

#[cfg(test)]
mod wrapper_plan_tests {
    use super::*;
    use oxvba_compiler::{
        ModuleKind, ProjectKind, ProjectManifest, compile_project, module_unit_from_source,
    };
    use oxvba_project::CallingConvention;

    fn reflection() -> ProjectReflection {
        let manifest = ProjectManifest {
            project_name: "NativeWrap".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module_unit_from_source(
                "Math",
                ModuleKind::Procedural,
                concat!(
                    "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function\n",
                    "Public Function Other(a As Long) As Long\nOther = a\nEnd Function\n",
                    "Public Function Unsupported(v As Variant) As Variant\nUnsupported = v\nEnd Function"
                ),
            )
            .unwrap()],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };
        compile_project(&manifest).unwrap().project_reflection
    }

    fn export(exported_name: &str, module: &str, procedure: &str) -> NativeExportDescriptor {
        NativeExportDescriptor {
            exported_name: exported_name.to_string(),
            module_name: module.to_string(),
            procedure_name: procedure.to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: None,
            param_types: None,
            return_type: None,
            category: None,
            description: None,
            argument_descriptions: None,
        }
    }

    #[test]
    fn wrapped_native_profile_selects_explicit_descriptor_identity_only() {
        let profile = build_wrapped_native_library_profile(
            "NativeWrap",
            reflection(),
            &[export("NativeAdd", "Math", "Add")],
        );
        assert_eq!(profile.thunks.len(), 1);
        assert_eq!(profile.thunks[0].exported_name, "NativeAdd");
        assert!(profile.thunks[0].callable_id.contains("add"));
        let CallableSelectionPlan::ExplicitCallableIds(ids) = &profile.plan.callable_selection
        else {
            panic!("expected explicit callable ids");
        };
        assert_eq!(ids, &vec![profile.thunks[0].callable_id.clone()]);
        assert!(
            !profile
                .thunks
                .iter()
                .any(|thunk| thunk.procedure_name == "other")
        );
    }

    #[test]
    fn wrapped_native_profile_reports_unsupported_signatures() {
        let profile = build_wrapped_native_library_profile(
            "NativeWrap",
            reflection(),
            &[export("NativeUnsupported", "Math", "Unsupported")],
        );
        assert!(profile.thunks.is_empty());
        assert_eq!(
            profile.diagnostics[0].code,
            "WRAPPED-NATIVE-UNSUPPORTED-SIGNATURE"
        );
    }

    #[test]
    fn wrapped_native_generated_thunk_uses_neutral_callable_typed_path() {
        let profile = build_wrapped_native_library_profile(
            "NativeWrap",
            reflection(),
            &[export("NativeAdd", "Math", "Add")],
        );
        let source = generate_dll_shim_from_wrapper_profile("NativeWrap", "native.oxb", &profile);
        assert!(source.contains("pub extern \"system\" fn NativeAdd"));
        assert!(source.contains("invoke_callable_typed"));
        assert!(source.contains(&profile.thunks[0].callable_id));
        assert!(source.contains("TypedValue::Long(arg0)"));
    }
}
