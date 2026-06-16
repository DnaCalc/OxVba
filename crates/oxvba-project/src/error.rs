//! Error types for the `.basproj` project system.

use oxvba_diagnostics::{Diagnostic, DiagnosticPhase};
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

    #[error("invalid BuildTarget: {0}")]
    InvalidBuildTarget(String),

    #[error("invalid RuntimeFlavor: {0}")]
    InvalidRuntimeFlavor(String),

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

impl BasProjError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        let (code, help) = match self {
            BasProjError::XmlParse(_) => ("PROJ-E-XML-PARSE", Some("Fix the .basproj XML syntax.")),
            BasProjError::VbpParse(_) => ("PROJ-E-VBP-PARSE", Some("Fix the .vbp syntax.")),
            BasProjError::VbpUnsupported(_) => (
                "PROJ-E-VBP-UNSUPPORTED",
                Some(
                    "Convert this project shape to .basproj or narrow the unsupported .vbp feature.",
                ),
            ),
            BasProjError::MissingAttribute { .. } => (
                "PROJ-E-MISSING-ATTRIBUTE",
                Some("Add the required XML attribute to the project file."),
            ),
            BasProjError::MissingProperty(_) => (
                "PROJ-E-MISSING-PROPERTY",
                Some("Add the required project property."),
            ),
            BasProjError::MissingMetadata { .. } => (
                "PROJ-E-MISSING-METADATA",
                Some("Add the required metadata item to the module/reference entry."),
            ),
            BasProjError::InvalidOutputType(_) => (
                "PROJ-E-INVALID-OUTPUT-TYPE",
                Some("Use one of the supported OutputType values."),
            ),
            BasProjError::InvalidBuildTarget(_) => (
                "PROJ-E-INVALID-BUILD-TARGET",
                Some("Use one of the supported BuildTarget values."),
            ),
            BasProjError::InvalidRuntimeFlavor(_) => (
                "PROJ-E-INVALID-RUNTIME-FLAVOR",
                Some("Use one of the supported RuntimeFlavor values."),
            ),
            BasProjError::EntryPointRequired(_) => (
                "PROJ-E-ENTRYPOINT-REQUIRED",
                Some("Set EntryPoint to a public procedure such as ModuleName.Main."),
            ),
            BasProjError::EntryPointInvalid(_) => (
                "PROJ-E-ENTRYPOINT-INVALID",
                Some("Use an entry point in ModuleName.ProcedureName form."),
            ),
            BasProjError::EntryPointNotFound(_) => (
                "PROJ-E-ENTRYPOINT-NOT-FOUND",
                Some("Check the entry point spelling and module inclusion."),
            ),
            BasProjError::EntryPointAmbiguous(_) => (
                "PROJ-E-ENTRYPOINT-AMBIGUOUS",
                Some("Qualify the entry point with its module name."),
            ),
            BasProjError::TopLevelMainlineUnsupported { .. } => (
                "PROJ-E-TOP-LEVEL-MAINLINE-UNSUPPORTED",
                Some("Move executable top-level statements into an explicit procedure."),
            ),
            BasProjError::ProjectDiscoveryAmbiguous { .. } => (
                "PROJ-E-DISCOVERY-AMBIGUOUS",
                Some("Pass the exact project file path."),
            ),
            BasProjError::UnsupportedPath { .. } => (
                "PROJ-E-UNSUPPORTED-PATH",
                Some("Use a .basproj or .vbp project file."),
            ),
            BasProjError::HostProjectEditUnsupportedWorkspace { .. } => (
                "PROJ-E-HOST-EDIT-UNSUPPORTED-WORKSPACE",
                Some("Use a .basproj workspace for host project edits."),
            ),
            BasProjError::HostProjectEditPlanInvalid(_) => ("PROJ-E-HOST-EDIT-PLAN-INVALID", None),
            BasProjError::DuplicateExportName(_) => (
                "PROJ-E-DUPLICATE-EXPORT-NAME",
                Some("Give each native export a unique exported name."),
            ),
            BasProjError::Io { .. } => ("PROJ-E-IO", None),
            BasProjError::ModuleSourceNotFound(_) => (
                "PROJ-E-MODULE-SOURCE-NOT-FOUND",
                Some("Check the module Include path relative to the project file."),
            ),
            BasProjError::ModuleSourceInvalid { .. } => ("PROJ-E-MODULE-SOURCE-INVALID", None),
            BasProjError::ImportFileNotFound(_) => (
                "PROJ-E-IMPORT-FILE-NOT-FOUND",
                Some("Check the import Include path relative to the project file."),
            ),
            BasProjError::CyclicProjectReference { .. } => (
                "PROJ-E-CYCLIC-PROJECT-REFERENCE",
                Some(
                    "Remove the project-reference cycle or split shared code into an acyclic dependency.",
                ),
            ),
            BasProjError::ProjectReferenceNotFound { .. } => (
                "PROJ-E-PROJECT-REFERENCE-NOT-FOUND",
                Some("Check the referenced project path."),
            ),
            BasProjError::NativeReferenceNotFound { .. } => (
                "PROJ-E-NATIVE-REFERENCE-NOT-FOUND",
                Some("Check the native library path."),
            ),
            BasProjError::ExportProcedureNotFound { .. } => (
                "PROJ-E-EXPORT-PROCEDURE-NOT-FOUND",
                Some("Check the exported module and procedure names."),
            ),
            BasProjError::ExportModuleNotProcedural { .. } => (
                "PROJ-E-EXPORT-MODULE-NOT-PROCEDURAL",
                Some("Native exports must target a procedural module."),
            ),
            BasProjError::ExportProcedureNotPublic { .. } => (
                "PROJ-E-EXPORT-PROCEDURE-NOT-PUBLIC",
                Some("Make the exported procedure Public."),
            ),
            BasProjError::ComClassNotExposed { .. } => (
                "PROJ-E-COM-CLASS-NOT-EXPOSED",
                Some("Expose the class in the project COM server surface."),
            ),
            BasProjError::ComServerNoCreatableClasses => (
                "PROJ-E-COM-SERVER-NO-CREATABLE-CLASSES",
                Some("Add at least one creatable public class to the ComServer project."),
            ),
        };
        let mut diagnostic =
            Diagnostic::error(code, DiagnosticPhase::ProjectLoad, self.to_string());
        if let Some(help) = help {
            diagnostic = diagnostic.with_help(help);
        }
        diagnostic
    }
}

#[cfg(test)]
mod tests {
    use super::BasProjError;

    #[test]
    fn invalid_output_type_has_stable_code() {
        let diagnostic = BasProjError::InvalidOutputType("Macro".to_string()).to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "PROJ-E-INVALID-OUTPUT-TYPE");
    }
}
