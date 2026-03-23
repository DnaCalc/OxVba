//! Intermediate `.basproj` model types.
//!
//! These types represent the raw XML content before mapping to `ProjectManifest`.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The intermediate representation of a parsed `.basproj` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasProj {
    /// SDK identifier from `<Project Sdk="...">`, e.g. `"OxVba.Sdk/0.1.0"`.
    pub sdk: String,
    /// Scalar properties from `<PropertyGroup>` elements.
    pub properties: BasProjProperties,
    /// Module items from `<ItemGroup>` elements.
    pub modules: Vec<BasProjModule>,
    /// Project reference items.
    pub project_references: Vec<BasProjProjectReference>,
    /// COM type library reference items.
    pub com_references: Vec<BasProjComReference>,
    /// Native library reference items.
    pub native_references: Vec<BasProjNativeReference>,
    /// Native export items.
    pub native_exports: Vec<BasProjNativeExport>,
}

/// Scalar properties extracted from `<PropertyGroup>` elements.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BasProjProperties {
    pub output_type: Option<OutputType>,
    pub project_name: Option<String>,
    pub entry_point: Option<String>,
    pub runtime_flavor: Option<RuntimeFlavor>,
    pub default_runtime_profile: Option<String>,
    pub default_policy_preset: Option<String>,
    pub default_root_object: Option<String>,
    pub define_constants: Option<String>,
}

/// Output type for the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    HostModule,
    Library,
    Exe,
    Addin,
    ComServer,
    ComExe,
}

/// COM class instancing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instancing {
    Private,
    PublicNotCreatable,
    MultiUse,
    GlobalMultiUse,
    SingleUse,
    GlobalSingleUse,
}

/// Runtime flavor selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFlavor {
    Lite,
    Jit,
}

/// The kind of module item in the project file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasProjModuleKind {
    /// `<Module>` — procedural `.bas` module.
    Module,
    /// `<ClassModule>` — class `.cls` module.
    ClassModule,
    /// `<DocumentModule>` — code-behind `.cls` module.
    DocumentModule,
}

/// A module item parsed from `.basproj`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasProjModule {
    pub kind: BasProjModuleKind,
    /// The `Include` attribute — relative path to the source file.
    pub include: String,
    /// Class module metadata.
    pub vb_predeclared_id: bool,
    pub vb_exposed: bool,
    pub vb_global_namespace: bool,
    pub vb_creatable: bool,
    /// Document module metadata.
    pub host_document_type: Option<String>,
    /// COM server class metadata.
    pub instancing: Option<Instancing>,
    pub prog_id: Option<String>,
    pub description: Option<String>,
}

/// A `<ProjectReference>` item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasProjProjectReference {
    /// Relative path to the referenced `.basproj` file.
    pub include: String,
}

/// A `<COMReference>` item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasProjComReference {
    /// The `Include` attribute — library name.
    pub include: String,
    pub guid: Option<String>,
    pub version_major: Option<u16>,
    pub version_minor: Option<u16>,
    pub lcid: Option<u32>,
    pub import_lib: Option<String>,
}

/// A `<NativeReference>` item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasProjNativeReference {
    /// The `Include` attribute — reference name.
    pub include: String,
    pub path: Option<String>,
}

/// A `<NativeExport>` item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasProjNativeExport {
    /// The `Include` attribute — exported function name.
    pub include: String,
    pub module: Option<String>,
    pub procedure: Option<String>,
    pub calling_convention: Option<CallingConvention>,
    pub ordinal: Option<u16>,
    /// XLL addin metadata.
    pub category: Option<String>,
    pub description: Option<String>,
    pub argument_descriptions: Option<String>,
}

/// Calling convention for native exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Stdcall,
    Cdecl,
}

/// Extra metadata for a class module used during generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassModuleMetadata {
    pub instancing: Option<Instancing>,
    pub prog_id: Option<String>,
    pub description: Option<String>,
}

/// Descriptor for a native export derived from `.basproj` + compiled procedure info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExportDescriptor {
    pub exported_name: String,
    pub module_name: String,
    pub procedure_name: String,
    pub calling_convention: CallingConvention,
    pub ordinal: Option<u16>,
    /// Populated by validate.rs after compilation.
    pub kind: Option<oxvba_compiler::ExportKind>,
    pub param_types: Option<Vec<oxvba_compiler::DeclareParamType>>,
    pub return_type: Option<Option<oxvba_compiler::DeclareParamType>>,
    /// XLL addin metadata (carried through from `.basproj` for round-trip fidelity).
    pub category: Option<String>,
    pub description: Option<String>,
    pub argument_descriptions: Option<String>,
}

/// Parsed conditional compilation constants.
pub fn parse_define_constants(raw: &str) -> BTreeMap<String, i32> {
    let mut map = BTreeMap::new();
    for part in raw.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((key, val)) = part.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            if !key.is_empty() {
                if let Ok(v) = val.parse::<i32>() {
                    map.insert(key.to_string(), v);
                }
            }
        } else {
            // Key without value defaults to 1
            map.insert(part.to_string(), 1);
        }
    }
    map
}

/// Extract the project directory from a `.basproj` file path.
pub fn project_dir(basproj_path: &std::path::Path) -> PathBuf {
    basproj_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
