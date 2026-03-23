//! Error types for the `.basproj` project system.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BasProjError {
    #[error("XML parse error: {0}")]
    XmlParse(String),

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

    #[error("duplicate native export name: {0}")]
    DuplicateExportName(String),

    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("module source file not found: {0}")]
    ModuleSourceNotFound(String),

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
