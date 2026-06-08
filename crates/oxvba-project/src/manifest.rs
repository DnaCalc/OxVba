//! Project manifest types — the `.basproj`/`.vbp` loader's in-memory shape.
//!
//! These data types originated in the (now-deleted) `oxvba-compiler` and were
//! relocated here when the legacy compiler was removed: they describe a loaded
//! project's modules/references, independent of any execution backend. The clean
//! closure builder ([`crate::closure`]) adapts them into `oxvba_symbol::manifest`
//! types for `oxvba_bind::bind_projects`.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Source,
    Host,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Procedural,
    Class,
    Document,
    Form,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Project,
    TypeLibrary,
    HostInjected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExportKind {
    Sub,
    Function,
}

/// A scalar parameter/return type for a native (`Declare Lib`) export descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclareParamType {
    Long,
    Integer,
    String,
    Boolean,
    Double,
    Single,
    Currency,
    Date,
    Byte,
    LongLong,
    LongPtr,
    Variant,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleAttributes {
    pub vb_name: String,
    pub vb_global_namespace: bool,
    pub vb_creatable: bool,
    pub vb_predeclared_id: bool,
    pub vb_exposed: bool,
    pub option_private_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleUnit {
    pub module_name: String,
    pub module_kind: ModuleKind,
    pub attributes: ModuleAttributes,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReference {
    pub referenced_project_name: String,
    pub reference_kind: ReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedProjectManifest {
    pub project_name: String,
    pub modules: Vec<ModuleUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifest {
    pub project_name: String,
    pub project_kind: ProjectKind,
    pub modules: Vec<ModuleUnit>,
    pub references: Vec<ProjectReference>,
    pub reference_projects: Vec<ReferencedProjectManifest>,
    pub conditional_constants: BTreeMap<String, i32>,
}
