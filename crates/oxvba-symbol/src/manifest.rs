//! The complete project-manifest domain the symbol model resolves against.
//!
//! This is a **full, self-contained** copy/merge of the compiled project manifest
//! (`oxvba-compiler::project`) and the richer `.basproj` reference/module detail
//! (`oxvba-project::model`). It is deliberately *not* a minimal mirror: every
//! field the project / COM / native / host providers need to resolve fully lives
//! here, so no resolution detail is pushed back onto the caller. The caller maps
//! its own manifest in with a mechanical adapter. Pure *build* concerns
//! (output/target/runtime-flavor, native exports, XLL metadata) are intentionally
//! excluded — they are not symbol-resolution inputs.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolProjectManifest {
    pub project_name: String,
    pub project_kind: ProjectKind,
    pub modules: Vec<ModuleUnit>,
    pub references: Vec<ProjectReference>,
    pub reference_projects: Vec<ReferencedProjectManifest>,
    /// `#If`/`#Const` conditional-compilation constants.
    pub conditional_constants: BTreeMap<String, i32>,
    pub conditional_compilation_target: crate::cond_comp::ConditionalCompilationTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Source,
    Host,
    Library,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleUnit {
    pub module_name: String,
    pub module_kind: ModuleKind,
    pub attributes: ModuleAttributes,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Procedural,
    Class,
    Document,
    Form,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleAttributes {
    pub vb_name: String,
    pub vb_global_namespace: bool,
    pub vb_creatable: bool,
    pub vb_predeclared_id: bool,
    pub vb_exposed: bool,
    pub option_private_module: bool,
    // Document/class/COM-server resolution metadata (from `.basproj`).
    /// e.g. `"Excel.Sheet"`, `"Word.Document"`.
    pub host_document_type: Option<String>,
    pub instancing: Option<Instancing>,
    /// COM ProgID for `New`/`CreateObject`.
    pub prog_id: Option<String>,
    pub description: Option<String>,
}

impl ModuleAttributes {
    /// A bare module with just a VB name — convenient for tests / procedural modules.
    pub fn named(vb_name: impl Into<String>) -> Self {
        Self {
            vb_name: vb_name.into(),
            vb_global_namespace: false,
            vb_creatable: false,
            vb_predeclared_id: false,
            vb_exposed: false,
            option_private_module: false,
            host_document_type: None,
            instancing: None,
            prog_id: None,
            description: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instancing {
    Private,
    PublicNotCreatable,
    MultiUse,
    GlobalMultiUse,
    SingleUse,
    GlobalSingleUse,
}

/// A project reference, with the full detail each provider needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectReference {
    /// A sibling/referenced VBA project.
    Project { referenced_project_name: String },
    /// A host-injected project (host object model arrives this way).
    HostInjected { referenced_project_name: String },
    /// A `<COMReference>` — drives typelib resolution (early/late COM).
    TypeLibrary {
        name: String,
        guid: Option<String>,
        version_major: Option<u16>,
        version_minor: Option<u16>,
        lcid: Option<u32>,
        import_lib: Option<String>,
    },
    /// A `<NativeReference>` — the target library for `Declare Lib`.
    Native { name: String, path: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedProjectManifest {
    pub project_name: String,
    pub project_kind: ProjectKind,
    pub modules: Vec<ModuleUnit>,
}
