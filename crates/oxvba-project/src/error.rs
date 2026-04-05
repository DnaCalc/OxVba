//! Error types for the `.basproj` project system.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BasProjError {
    #[error("XML parse error: {0}")]
    XmlParse(String),

    #[error("VBP parse error: {0}")]
    VbpParse(String),

    #[error("VBP unsupported: {0}")]
    VbpUnsupported(String),

    #[error("missing required attribute `{attribute}` on <{element}>")]
    MissingAttribute { element: String, attribute: String },

    #[error("missing required property: {0}")]
    MissingProperty(String),

    #[error("missing required metadata `{metadata}` on <{element} Include=\"{include}\">")]
    MissingMetadata {
        element: String,
        include: String,
        metadata: String,
    },

    #[error("invalid OutputType: {0}")]
    InvalidOutputType(String),

    #[error("EntryPoint is required for OutputType={0}")]
    EntryPointRequired(String),

    #[error("invalid EntryPoint: {0}")]
    EntryPointInvalid(String),

    #[error("EntryPoint not found: {0}")]
    EntryPointNotFound(String),

    #[error("EntryPoint is ambiguous: {0}")]
    EntryPointAmbiguous(String),

    #[error(
        "top-level executable statements are not supported for OutputType={output_type} (module `{module_name}`)"
    )]
    TopLevelMainlineUnsupported {
        output_type: String,
        module_name: String,
    },

    #[error("project discovery is ambiguous in {directory}: {kind} candidates are {candidates:?}")]
    ProjectDiscoveryAmbiguous {
        directory: String,
        kind: String,
        candidates: Vec<String>,
    },

    #[error("unsupported workspace target `{path}` with extension `.{extension}`")]
    UnsupportedPath { path: String, extension: String },

    #[error(
        "host project edits are only supported for `.basproj` workspaces (got {workspace_kind} at {path})"
    )]
    HostProjectEditUnsupportedWorkspace {
        path: String,
        workspace_kind: String,
    },

    #[error("host project edit plan is invalid: {0}")]
    HostProjectEditPlanInvalid(String),

    #[error("duplicate native export name: {0}")]
    DuplicateExportName(String),

    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("module source file not found: {0}")]
    ModuleSourceNotFound(String),

    #[error("invalid module source `{include}`: {message}")]
    ModuleSourceInvalid { include: String, message: String },

    #[error("import file not found: {0}")]
    ImportFileNotFound(String),

    #[error("cyclic project reference at {path}: cycle is {cycle:?}")]
    CyclicProjectReference { path: String, cycle: Vec<String> },

    #[error("project reference not found: {include}")]
    ProjectReferenceNotFound { include: String },

    #[error("native reference not found: {include} (resolved to {resolved_path})")]
    NativeReferenceNotFound {
        include: String,
        resolved_path: String,
    },

    #[error("export procedure not found: {exported_name} -> {module_name}.{procedure_name}")]
    ExportProcedureNotFound {
        exported_name: String,
        module_name: String,
        procedure_name: String,
    },

    #[error("export module is not procedural: {exported_name} -> {module_name}")]
    ExportModuleNotProcedural {
        exported_name: String,
        module_name: String,
    },

    #[error("export procedure is not public: {exported_name} -> {module_name}.{procedure_name}")]
    ExportProcedureNotPublic {
        exported_name: String,
        module_name: String,
        procedure_name: String,
    },

    #[error("COM class not exposed: {class_name}")]
    ComClassNotExposed { class_name: String },

    #[error("ComServer project has no creatable classes")]
    ComServerNoCreatableClasses,
}
