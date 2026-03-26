use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Source,
    Host,
    Library,
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::{
        GraphPublicSymbolResolution, ModuleAttributes, ModuleKind, ModuleNode, ProjectGraph,
        ProjectKind, ReferenceBindingState, ReferenceKind, TypeLibraryCatalogEntry,
    };

    fn simple_proc_module(name: &str) -> ModuleNode {
        ModuleNode::new(
            name,
            ModuleKind::Procedural,
            ModuleAttributes {
                vb_name: name.to_string(),
                ..ModuleAttributes::default()
            },
        )
    }

    #[kani::proof]
    fn pmr_typelib_resolution_transitions_typelib_refs_out_of_unbound() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_importlib("StdOle", "stdole2.tlb")
            .expect("importlib hint should be accepted");

        let records = project.resolve_type_library_references(&[TypeLibraryCatalogEntry {
            library_name: "StdOle".to_string(),
            importlib: "stdole2.tlb".to_string(),
            libid: None,
            major_version: 2,
            minor_version: 0,
            lcid: None,
        }]);

        assert_eq!(records.len(), 1);
        assert_ne!(
            project.references[0].binding_state,
            ReferenceBindingState::Unbound
        );
    }

    #[kani::proof]
    fn pmr_active_resolution_prefers_local_symbol_before_reference_symbol() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        graph
            .create_project("LibA", ProjectKind::Library)
            .expect("project should be created");
        graph
            .set_active_project("Alpha")
            .expect("active project should exist");

        {
            let alpha = graph.project_mut("Alpha").expect("project should exist");
            alpha
                .add_module(simple_proc_module("LocalModule"))
                .expect("module should be accepted");
            alpha
                .register_public_symbol("LocalModule", "Run")
                .expect("symbol should register");
            alpha
                .add_reference("LibA", ReferenceKind::Project)
                .expect("reference should register");
        }
        {
            let lib = graph.project_mut("LibA").expect("project should exist");
            lib.add_module(simple_proc_module("RemoteModule"))
                .expect("module should be accepted");
            lib.register_public_symbol("RemoteModule", "Run")
                .expect("symbol should register");
        }

        let resolved = graph
            .resolve_active_public_symbol("Run")
            .expect("resolution should succeed");
        assert_eq!(
            resolved,
            GraphPublicSymbolResolution::Unique {
                project_name: "Alpha".to_string(),
                module_name: "localmodule".to_string(),
            }
        );
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceBindingState {
    Unbound,
    Bound,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HostExportKind {
    Sub,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostProcedureExport {
    pub module_name: String,
    pub procedure_name: String,
    pub kind: HostExportKind,
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
pub struct ModuleNode {
    pub module_name: String,
    pub module_kind: ModuleKind,
    pub attributes: ModuleAttributes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReference {
    pub referenced_project_name: String,
    pub precedence_index: u32,
    pub reference_kind: ReferenceKind,
    pub binding_state: ReferenceBindingState,
    pub importlib_hint: Option<String>,
    pub libid_hint: Option<String>,
    pub major_version_hint: Option<u16>,
    pub minor_version_hint: Option<u16>,
    pub lcid_hint: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibraryCatalogEntry {
    pub library_name: String,
    pub importlib: String,
    pub libid: Option<String>,
    pub major_version: u16,
    pub minor_version: u16,
    pub lcid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibraryBindingRecord {
    pub reference_name: String,
    pub status: TypeLibraryBindingStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLibraryBindingStatus {
    Bound {
        importlib: String,
        libid: Option<String>,
        major_version: u16,
        minor_version: u16,
        lcid: Option<u32>,
    },
    MissingImportLibHint,
    ImportLibUnresolved {
        importlib: String,
    },
    ImportLibAmbiguous {
        importlib: String,
        candidate_libraries: Vec<String>,
    },
    LibIdUnresolved {
        libid: String,
    },
    LibIdAmbiguous {
        libid: String,
        candidate_libraries: Vec<String>,
    },
}

impl TypeLibraryBindingStatus {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Bound { .. } => "PMR-I-TYPELIB-BOUND",
            Self::MissingImportLibHint => "PMR-E-TYPELIB-IMPORTLIB-MISSING",
            Self::ImportLibUnresolved { .. } => "PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED",
            Self::ImportLibAmbiguous { .. } => "PMR-E-TYPELIB-IMPORTLIB-AMBIGUOUS",
            Self::LibIdUnresolved { .. } => "PMR-E-TYPELIB-LIBID-UNRESOLVED",
            Self::LibIdAmbiguous { .. } => "PMR-E-TYPELIB-LIBID-AMBIGUOUS",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNode {
    pub project_name: String,
    pub project_kind: ProjectKind,
    pub module_order: Vec<String>,
    pub modules: BTreeMap<String, ModuleNode>,
    pub extensible_module_names: BTreeSet<String>,
    pub references: Vec<ProjectReference>,
    pub conditional_constants: BTreeMap<String, i32>,
    public_symbol_owners: BTreeMap<String, BTreeSet<String>>,
    host_exports: BTreeMap<String, BTreeMap<String, HostExportKind>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectGraph {
    projects: BTreeMap<String, ProjectNode>,
    active_project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicSymbolResolution {
    NotFound,
    Unique { module_name: String },
    RequiresQualification { module_names: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphPublicSymbolResolution {
    NotFound,
    Unique {
        project_name: String,
        module_name: String,
    },
    RequiresQualification {
        project_name: String,
        module_names: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectModelError {
    #[error("PMR-E-PROJECT-NAME-INVALID: project name `{name}` is not a valid VBA identifier")]
    ProjectNameInvalid { name: String },
    #[error("PMR-E-PROJECT-NAME-DUPLICATE: duplicate project name `{name}`")]
    ProjectNameDuplicate { name: String },
    #[error("PMR-E-MODULE-NAME-INVALID: module name `{name}` is not a valid VBA identifier")]
    ModuleNameInvalid { name: String },
    #[error("PMR-E-MODULE-NAME-LENGTH: module name `{name}` exceeds 31 characters")]
    ModuleNameLength { name: String },
    #[error("PMR-E-MODULE-NAME-DUPLICATE: duplicate module name `{name}`")]
    ModuleNameDuplicate { name: String },
    #[error(
        "PMR-E-MODULE-HEADER-VB-NAME: module `{module_name}` has mismatched `VB_Name` `{vb_name}`"
    )]
    ModuleHeaderVbNameMismatch {
        module_name: String,
        vb_name: String,
    },
    #[error(
        "PMR-E-MODULE-CLASS-ATTRIBUTE: source project class modules require `VB_GlobalNamespace=False` and `VB_Creatable=False`"
    )]
    SourceProjectClassAttributeConstraint,
    #[error(
        "PMR-E-OPTION-PRIVATE-MODULE-KIND: Option Private Module is only valid for procedural modules"
    )]
    OptionPrivateModuleKind,
    #[error(
        "PMR-E-MODULE-EXTENSION-PROJECT-KIND: extension module `{module_name}` requires a host project"
    )]
    ExtensionModuleProjectKind { module_name: String },
    #[error(
        "PMR-E-REFERENCE-NAME-INVALID: referenced project name `{name}` is not a valid VBA identifier"
    )]
    ReferenceNameInvalid { name: String },
    #[error("PMR-E-REFERENCE-DUPLICATE-TARGET: duplicate referenced project `{name}`")]
    ReferenceDuplicateTarget { name: String },
    #[error("PMR-E-REFERENCE-NOT-FOUND: referenced project `{name}` was not found")]
    ReferenceNotFound { name: String },
    #[error("PMR-E-TYPELIB-KIND-MISMATCH: reference `{name}` is not a TypeLibrary reference")]
    TypeLibraryKindMismatch { name: String },
    #[error("PMR-E-PROJECT-NOT-FOUND: project `{name}` was not found")]
    ProjectNotFound { name: String },
    #[error("PMR-E-ACTIVE-PROJECT-MISSING: no active project is set")]
    ActiveProjectMissing,
    #[error("PMR-E-MODULE-NOT-FOUND: module `{name}` was not found")]
    ModuleNotFound { name: String },
    #[error("PMR-E-SYMBOL-INVALID: public symbol `{name}` is not a valid VBA identifier")]
    SymbolInvalid { name: String },
    #[error(
        "PMR-E-HOST-EXPORT-MODULE-KIND: module `{name}` is not a procedural module and cannot expose host exports"
    )]
    HostExportModuleKindUnsupported { name: String },
    #[error(
        "PMR-E-HOST-EXPORT-NAME-INVALID: exported procedure name `{name}` is not a valid VBA identifier"
    )]
    HostExportProcedureInvalid { name: String },
    #[error(
        "PMR-E-HOST-EXTENSION-MODULE-KIND: host extension attach requires an extension module (`{module_name}`)"
    )]
    HostExtensionModuleKind { module_name: String },
    #[error(
        "PMR-E-HOST-EXTENSIBLE-MODULE-PROJECT-KIND: extensible module `{module_name}` requires a host project"
    )]
    HostExtensibleModuleProjectKind { module_name: String },
    #[error(
        "PMR-E-HOST-EXTENSION-TARGET-MISSING: extension module `{module_name}` does not match a registered extensible host module"
    )]
    HostExtensionTargetMissing { module_name: String },
}

impl ProjectModelError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectNameInvalid { .. } => "PMR-E-PROJECT-NAME-INVALID",
            Self::ProjectNameDuplicate { .. } => "PMR-E-PROJECT-NAME-DUPLICATE",
            Self::ModuleNameInvalid { .. } => "PMR-E-MODULE-NAME-INVALID",
            Self::ModuleNameLength { .. } => "PMR-E-MODULE-NAME-LENGTH",
            Self::ModuleNameDuplicate { .. } => "PMR-E-MODULE-NAME-DUPLICATE",
            Self::ModuleHeaderVbNameMismatch { .. } => "PMR-E-MODULE-HEADER-VB-NAME",
            Self::SourceProjectClassAttributeConstraint => "PMR-E-MODULE-CLASS-ATTRIBUTE",
            Self::OptionPrivateModuleKind => "PMR-E-OPTION-PRIVATE-MODULE-KIND",
            Self::ExtensionModuleProjectKind { .. } => "PMR-E-MODULE-EXTENSION-PROJECT-KIND",
            Self::ReferenceNameInvalid { .. } => "PMR-E-REFERENCE-NAME-INVALID",
            Self::ReferenceDuplicateTarget { .. } => "PMR-E-REFERENCE-DUPLICATE-TARGET",
            Self::ReferenceNotFound { .. } => "PMR-E-REFERENCE-NOT-FOUND",
            Self::TypeLibraryKindMismatch { .. } => "PMR-E-TYPELIB-KIND-MISMATCH",
            Self::ProjectNotFound { .. } => "PMR-E-PROJECT-NOT-FOUND",
            Self::ActiveProjectMissing => "PMR-E-ACTIVE-PROJECT-MISSING",
            Self::ModuleNotFound { .. } => "PMR-E-MODULE-NOT-FOUND",
            Self::SymbolInvalid { .. } => "PMR-E-SYMBOL-INVALID",
            Self::HostExportModuleKindUnsupported { .. } => "PMR-E-HOST-EXPORT-MODULE-KIND",
            Self::HostExportProcedureInvalid { .. } => "PMR-E-HOST-EXPORT-NAME-INVALID",
            Self::HostExtensionModuleKind { .. } => "PMR-E-HOST-EXTENSION-MODULE-KIND",
            Self::HostExtensibleModuleProjectKind { .. } => {
                "PMR-E-HOST-EXTENSIBLE-MODULE-PROJECT-KIND"
            }
            Self::HostExtensionTargetMissing { .. } => "PMR-E-HOST-EXTENSION-TARGET-MISSING",
        }
    }
}

impl ModuleNode {
    pub fn new(
        module_name: impl Into<String>,
        module_kind: ModuleKind,
        attributes: ModuleAttributes,
    ) -> Self {
        Self {
            module_name: module_name.into(),
            module_kind,
            attributes,
        }
    }
}

impl ProjectNode {
    pub fn new(
        project_name: impl Into<String>,
        project_kind: ProjectKind,
    ) -> Result<Self, ProjectModelError> {
        let project_name = project_name.into();
        if !is_valid_vba_identifier(&project_name) {
            return Err(ProjectModelError::ProjectNameInvalid { name: project_name });
        }
        Ok(Self {
            project_name,
            project_kind,
            module_order: Vec::new(),
            modules: BTreeMap::new(),
            extensible_module_names: BTreeSet::new(),
            references: Vec::new(),
            conditional_constants: BTreeMap::new(),
            public_symbol_owners: BTreeMap::new(),
            host_exports: BTreeMap::new(),
        })
    }

    pub fn register_extensible_module(
        &mut self,
        module_name: impl Into<String>,
    ) -> Result<(), ProjectModelError> {
        let module_name = module_name.into();
        if self.project_kind != ProjectKind::Host {
            return Err(ProjectModelError::HostExtensibleModuleProjectKind { module_name });
        }
        if !is_valid_vba_identifier(&module_name) {
            return Err(ProjectModelError::ModuleNameInvalid { name: module_name });
        }
        if module_name.chars().count() > 31 {
            return Err(ProjectModelError::ModuleNameLength { name: module_name });
        }
        self.extensible_module_names
            .insert(normalize_identifier(&module_name));
        Ok(())
    }

    pub fn add_module(&mut self, module: ModuleNode) -> Result<(), ProjectModelError> {
        if !is_valid_vba_identifier(&module.module_name) {
            return Err(ProjectModelError::ModuleNameInvalid {
                name: module.module_name,
            });
        }
        if module.module_name.chars().count() > 31 {
            return Err(ProjectModelError::ModuleNameLength {
                name: module.module_name,
            });
        }
        if module.attributes.vb_name.is_empty()
            || !module
                .attributes
                .vb_name
                .eq_ignore_ascii_case(&module.module_name)
        {
            return Err(ProjectModelError::ModuleHeaderVbNameMismatch {
                module_name: module.module_name,
                vb_name: module.attributes.vb_name,
            });
        }
        if self.project_kind == ProjectKind::Source
            && module.module_kind == ModuleKind::Class
            && (module.attributes.vb_global_namespace || module.attributes.vb_creatable)
        {
            return Err(ProjectModelError::SourceProjectClassAttributeConstraint);
        }
        if module.attributes.option_private_module && module.module_kind != ModuleKind::Procedural {
            return Err(ProjectModelError::OptionPrivateModuleKind);
        }
        if module.module_kind == ModuleKind::Extension && self.project_kind != ProjectKind::Host {
            return Err(ProjectModelError::ExtensionModuleProjectKind {
                module_name: module.module_name,
            });
        }

        let module_name = module.module_name.clone();
        let key = normalize_identifier(&module_name);
        if self.modules.contains_key(&key) {
            return Err(ProjectModelError::ModuleNameDuplicate { name: module_name });
        }

        self.module_order.push(module_name);
        self.modules.insert(key, module);
        Ok(())
    }

    pub fn attach_host_extension_module(
        &mut self,
        module: ModuleNode,
    ) -> Result<(), ProjectModelError> {
        if self.project_kind != ProjectKind::Host {
            return Err(ProjectModelError::ExtensionModuleProjectKind {
                module_name: module.module_name,
            });
        }
        if module.module_kind != ModuleKind::Extension {
            return Err(ProjectModelError::HostExtensionModuleKind {
                module_name: module.module_name,
            });
        }
        let module_name = module.module_name.clone();
        if !self
            .extensible_module_names
            .contains(&normalize_identifier(&module_name))
        {
            return Err(ProjectModelError::HostExtensionTargetMissing { module_name });
        }
        self.add_module(module)
    }

    pub fn add_reference(
        &mut self,
        referenced_project_name: impl Into<String>,
        reference_kind: ReferenceKind,
    ) -> Result<(), ProjectModelError> {
        let referenced_project_name = referenced_project_name.into();
        if !is_valid_vba_identifier(&referenced_project_name) {
            return Err(ProjectModelError::ReferenceNameInvalid {
                name: referenced_project_name,
            });
        }

        if self.references.iter().any(|existing| {
            existing
                .referenced_project_name
                .eq_ignore_ascii_case(&referenced_project_name)
        }) {
            return Err(ProjectModelError::ReferenceDuplicateTarget {
                name: referenced_project_name,
            });
        }

        self.references.push(ProjectReference {
            referenced_project_name,
            precedence_index: self.references.len() as u32,
            reference_kind,
            binding_state: ReferenceBindingState::Unbound,
            importlib_hint: None,
            libid_hint: None,
            major_version_hint: None,
            minor_version_hint: None,
            lcid_hint: None,
        });
        Ok(())
    }

    pub fn set_reference_importlib(
        &mut self,
        referenced_project_name: &str,
        importlib_hint: impl Into<String>,
    ) -> Result<(), ProjectModelError> {
        let key = normalize_identifier(referenced_project_name);
        let importlib_hint = importlib_hint.into().trim().to_string();
        let Some(reference) = self
            .references
            .iter_mut()
            .find(|reference| normalize_identifier(&reference.referenced_project_name) == key)
        else {
            return Err(ProjectModelError::ReferenceNotFound {
                name: referenced_project_name.to_string(),
            });
        };
        if reference.reference_kind != ReferenceKind::TypeLibrary {
            return Err(ProjectModelError::TypeLibraryKindMismatch {
                name: reference.referenced_project_name.clone(),
            });
        }
        reference.importlib_hint = if importlib_hint.is_empty() {
            None
        } else {
            Some(importlib_hint)
        };
        reference.binding_state = ReferenceBindingState::Unbound;
        Ok(())
    }

    pub fn set_reference_typelib_identity(
        &mut self,
        referenced_project_name: &str,
        libid_hint: impl Into<String>,
        major_version_hint: Option<u16>,
        minor_version_hint: Option<u16>,
        lcid_hint: Option<u32>,
    ) -> Result<(), ProjectModelError> {
        let key = normalize_identifier(referenced_project_name);
        let libid_hint = libid_hint.into().trim().to_string();
        let Some(reference) = self
            .references
            .iter_mut()
            .find(|reference| normalize_identifier(&reference.referenced_project_name) == key)
        else {
            return Err(ProjectModelError::ReferenceNotFound {
                name: referenced_project_name.to_string(),
            });
        };
        if reference.reference_kind != ReferenceKind::TypeLibrary {
            return Err(ProjectModelError::TypeLibraryKindMismatch {
                name: reference.referenced_project_name.clone(),
            });
        }
        reference.libid_hint = if libid_hint.is_empty() {
            None
        } else {
            Some(libid_hint)
        };
        reference.major_version_hint = major_version_hint;
        reference.minor_version_hint = minor_version_hint;
        reference.lcid_hint = lcid_hint;
        reference.binding_state = ReferenceBindingState::Unbound;
        Ok(())
    }

    pub fn resolve_type_library_references(
        &mut self,
        catalog: &[TypeLibraryCatalogEntry],
    ) -> Vec<TypeLibraryBindingRecord> {
        let mut records = Vec::new();
        let expected = self
            .references
            .iter()
            .filter(|reference| reference.reference_kind == ReferenceKind::TypeLibrary)
            .count();
        for reference in self
            .references
            .iter_mut()
            .filter(|reference| reference.reference_kind == ReferenceKind::TypeLibrary)
        {
            let matches = if let Some(libid_hint) = reference.libid_hint.as_ref() {
                let normalized_libid = normalize_guid_like(libid_hint);
                catalog
                    .iter()
                    .filter(|entry| {
                        entry
                            .libid
                            .as_ref()
                            .map(|value| normalize_guid_like(value) == normalized_libid)
                            .unwrap_or(false)
                    })
                    .filter(|entry| {
                        reference
                            .major_version_hint
                            .is_none_or(|major| major == entry.major_version)
                    })
                    .filter(|entry| {
                        reference
                            .minor_version_hint
                            .is_none_or(|minor| minor == entry.minor_version)
                    })
                    .filter(|entry| {
                        reference
                            .lcid_hint
                            .is_none_or(|lcid| entry.lcid == Some(lcid))
                    })
                    .collect::<Vec<_>>()
            } else if let Some(importlib_hint) = reference.importlib_hint.as_ref() {
                let normalized_hint = normalize_identifier(importlib_hint);
                catalog
                    .iter()
                    .filter(|entry| !entry.importlib.trim().is_empty())
                    .filter(|entry| normalize_identifier(&entry.importlib) == normalized_hint)
                    .collect::<Vec<_>>()
            } else {
                reference.binding_state = ReferenceBindingState::Failed;
                records.push(TypeLibraryBindingRecord {
                    reference_name: reference.referenced_project_name.clone(),
                    status: TypeLibraryBindingStatus::MissingImportLibHint,
                });
                continue;
            };
            match matches.as_slice() {
                [entry] => {
                    reference.binding_state = ReferenceBindingState::Bound;
                    records.push(TypeLibraryBindingRecord {
                        reference_name: reference.referenced_project_name.clone(),
                        status: TypeLibraryBindingStatus::Bound {
                            importlib: entry.importlib.clone(),
                            libid: entry.libid.clone(),
                            major_version: entry.major_version,
                            minor_version: entry.minor_version,
                            lcid: entry.lcid,
                        },
                    });
                }
                [] => {
                    reference.binding_state = ReferenceBindingState::Failed;
                    records.push(TypeLibraryBindingRecord {
                        reference_name: reference.referenced_project_name.clone(),
                        status: if let Some(libid_hint) = reference.libid_hint.as_ref() {
                            TypeLibraryBindingStatus::LibIdUnresolved {
                                libid: libid_hint.clone(),
                            }
                        } else {
                            TypeLibraryBindingStatus::ImportLibUnresolved {
                                importlib: reference.importlib_hint.clone().unwrap_or_default(),
                            }
                        },
                    });
                }
                _ => {
                    reference.binding_state = ReferenceBindingState::Failed;
                    let mut candidate_libraries = matches
                        .iter()
                        .map(|entry| entry.library_name.clone())
                        .collect::<Vec<_>>();
                    candidate_libraries.sort();
                    candidate_libraries.dedup();
                    records.push(TypeLibraryBindingRecord {
                        reference_name: reference.referenced_project_name.clone(),
                        status: if let Some(libid_hint) = reference.libid_hint.as_ref() {
                            TypeLibraryBindingStatus::LibIdAmbiguous {
                                libid: libid_hint.clone(),
                                candidate_libraries,
                            }
                        } else {
                            TypeLibraryBindingStatus::ImportLibAmbiguous {
                                importlib: reference.importlib_hint.clone().unwrap_or_default(),
                                candidate_libraries,
                            }
                        },
                    });
                }
            }
        }
        debug_assert_eq!(
            records.len(),
            expected,
            "type-library resolver must produce exactly one record per type-library reference"
        );
        debug_assert!(
            self.references
                .iter()
                .filter(|reference| reference.reference_kind == ReferenceKind::TypeLibrary)
                .all(|reference| reference.binding_state != ReferenceBindingState::Unbound),
            "all type-library references must transition out of Unbound after resolution"
        );
        records
    }

    pub fn register_public_symbol(
        &mut self,
        module_name: &str,
        symbol_name: &str,
    ) -> Result<(), ProjectModelError> {
        if !is_valid_vba_identifier(symbol_name) {
            return Err(ProjectModelError::SymbolInvalid {
                name: symbol_name.to_string(),
            });
        }

        let module_key = normalize_identifier(module_name);
        if !self.modules.contains_key(&module_key) {
            return Err(ProjectModelError::ModuleNotFound {
                name: module_name.to_string(),
            });
        }

        self.public_symbol_owners
            .entry(normalize_identifier(symbol_name))
            .or_default()
            .insert(module_key);
        Ok(())
    }

    pub fn resolve_public_symbol(&self, symbol_name: &str) -> PublicSymbolResolution {
        let key = normalize_identifier(symbol_name);
        let Some(owners) = self.public_symbol_owners.get(&key) else {
            return PublicSymbolResolution::NotFound;
        };
        if owners.len() == 1 {
            return PublicSymbolResolution::Unique {
                module_name: owners.iter().next().cloned().unwrap_or_default(),
            };
        }
        let module_names = owners.iter().cloned().collect();
        PublicSymbolResolution::RequiresQualification { module_names }
    }

    pub fn default_instance_enabled(&self, module_name: &str) -> Result<bool, ProjectModelError> {
        let module_key = normalize_identifier(module_name);
        let Some(module) = self.modules.get(&module_key) else {
            return Err(ProjectModelError::ModuleNotFound {
                name: module_name.to_string(),
            });
        };
        if module.module_kind != ModuleKind::Class {
            return Ok(false);
        }
        Ok(module.attributes.vb_predeclared_id || module.attributes.vb_global_namespace)
    }

    pub fn register_host_export(
        &mut self,
        module_name: &str,
        procedure_name: &str,
        kind: HostExportKind,
    ) -> Result<(), ProjectModelError> {
        if !is_valid_vba_identifier(procedure_name) {
            return Err(ProjectModelError::HostExportProcedureInvalid {
                name: procedure_name.to_string(),
            });
        }
        let module_key = normalize_identifier(module_name);
        let Some(module) = self.modules.get(&module_key) else {
            return Err(ProjectModelError::ModuleNotFound {
                name: module_name.to_string(),
            });
        };
        if module.module_kind != ModuleKind::Procedural {
            return Err(ProjectModelError::HostExportModuleKindUnsupported {
                name: module_name.to_string(),
            });
        }
        self.host_exports
            .entry(module_key)
            .or_default()
            .insert(normalize_identifier(procedure_name), kind);
        Ok(())
    }

    pub fn host_exports(&self) -> Vec<HostProcedureExport> {
        let mut exports = Vec::new();
        for module_name in &self.module_order {
            let module_key = normalize_identifier(module_name);
            if !self.modules.contains_key(&module_key) {
                continue;
            }
            let Some(entries) = self.host_exports.get(&module_key) else {
                continue;
            };
            for (procedure_name, kind) in entries {
                exports.push(HostProcedureExport {
                    module_name: module_key.clone(),
                    procedure_name: procedure_name.clone(),
                    kind: *kind,
                });
            }
        }
        exports.sort();
        exports
    }

    pub fn reference_visible_exports(&self) -> Vec<HostProcedureExport> {
        let mut exports = Vec::new();
        for module_name in &self.module_order {
            let module_key = normalize_identifier(module_name);
            let Some(module) = self.modules.get(&module_key) else {
                continue;
            };
            if module.attributes.option_private_module {
                continue;
            }
            let Some(entries) = self.host_exports.get(&module_key) else {
                continue;
            };
            for (procedure_name, kind) in entries {
                exports.push(HostProcedureExport {
                    module_name: module_key.clone(),
                    procedure_name: procedure_name.clone(),
                    kind: *kind,
                });
            }
        }
        exports.sort();
        exports
    }

    pub fn module(&self, module_name: &str) -> Option<&ModuleNode> {
        self.modules.get(&normalize_identifier(module_name))
    }

    pub fn is_extensible_module(&self, module_name: &str) -> bool {
        self.extensible_module_names
            .contains(&normalize_identifier(module_name))
    }
}

impl ProjectGraph {
    pub fn create_project(
        &mut self,
        project_name: impl Into<String>,
        project_kind: ProjectKind,
    ) -> Result<(), ProjectModelError> {
        let project = ProjectNode::new(project_name, project_kind)?;
        let key = normalize_identifier(&project.project_name);
        if self.projects.contains_key(&key) {
            return Err(ProjectModelError::ProjectNameDuplicate {
                name: project.project_name,
            });
        }
        if self.active_project.is_none() {
            self.active_project = Some(key.clone());
        }
        self.projects.insert(key, project);
        Ok(())
    }

    pub fn set_active_project(&mut self, project_name: &str) -> Result<(), ProjectModelError> {
        let key = normalize_identifier(project_name);
        if !self.projects.contains_key(&key) {
            return Err(ProjectModelError::ProjectNotFound {
                name: project_name.to_string(),
            });
        }
        self.active_project = Some(key);
        Ok(())
    }

    pub fn active_project(&self) -> Result<&ProjectNode, ProjectModelError> {
        let key = self
            .active_project
            .as_ref()
            .ok_or(ProjectModelError::ActiveProjectMissing)?;
        self.projects
            .get(key)
            .ok_or(ProjectModelError::ActiveProjectMissing)
    }

    pub fn active_project_mut(&mut self) -> Result<&mut ProjectNode, ProjectModelError> {
        let key = self
            .active_project
            .as_ref()
            .ok_or(ProjectModelError::ActiveProjectMissing)?
            .clone();
        self.projects
            .get_mut(&key)
            .ok_or(ProjectModelError::ActiveProjectMissing)
    }

    pub fn project(&self, project_name: &str) -> Option<&ProjectNode> {
        self.projects.get(&normalize_identifier(project_name))
    }

    pub fn project_mut(&mut self, project_name: &str) -> Option<&mut ProjectNode> {
        self.projects.get_mut(&normalize_identifier(project_name))
    }

    pub fn projects(&self) -> impl Iterator<Item = &ProjectNode> {
        self.projects.values()
    }

    pub fn resolve_active_public_symbol(
        &self,
        symbol_name: &str,
    ) -> Result<GraphPublicSymbolResolution, ProjectModelError> {
        let active_key = self
            .active_project
            .as_ref()
            .ok_or(ProjectModelError::ActiveProjectMissing)?;
        let active = self
            .projects
            .get(active_key)
            .ok_or(ProjectModelError::ActiveProjectMissing)?;

        match active.resolve_public_symbol(symbol_name) {
            PublicSymbolResolution::Unique { module_name } => {
                return Ok(GraphPublicSymbolResolution::Unique {
                    project_name: active.project_name.clone(),
                    module_name,
                });
            }
            PublicSymbolResolution::RequiresQualification { module_names } => {
                return Ok(GraphPublicSymbolResolution::RequiresQualification {
                    project_name: active.project_name.clone(),
                    module_names,
                });
            }
            PublicSymbolResolution::NotFound => {}
        }

        let mut references = active.references.iter().collect::<Vec<_>>();
        references.sort_by_key(|reference| reference.precedence_index);
        for reference in references {
            let referenced_key = normalize_identifier(&reference.referenced_project_name);
            let Some(referenced) = self.projects.get(&referenced_key) else {
                continue;
            };
            match referenced.resolve_public_symbol(symbol_name) {
                PublicSymbolResolution::Unique { module_name } => {
                    return Ok(GraphPublicSymbolResolution::Unique {
                        project_name: referenced.project_name.clone(),
                        module_name,
                    });
                }
                PublicSymbolResolution::RequiresQualification { module_names } => {
                    return Ok(GraphPublicSymbolResolution::RequiresQualification {
                        project_name: referenced.project_name.clone(),
                        module_names,
                    });
                }
                PublicSymbolResolution::NotFound => {}
            }
        }

        Ok(GraphPublicSymbolResolution::NotFound)
    }
}

pub type Project = ProjectNode;

fn normalize_identifier(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn normalize_guid_like(input: &str) -> String {
    input
        .trim()
        .trim_matches('{')
        .trim_matches('}')
        .to_ascii_lowercase()
}

fn is_valid_vba_identifier(input: &str) -> bool {
    let mut chars = input.trim().chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::{
        GraphPublicSymbolResolution, HostExportKind, ModuleAttributes, ModuleKind, ModuleNode,
        ProjectGraph, ProjectKind, ProjectModelError, PublicSymbolResolution,
        ReferenceBindingState, ReferenceKind, TypeLibraryBindingStatus, TypeLibraryCatalogEntry,
    };
    use std::path::{Path, PathBuf};

    fn simple_proc_module(name: &str) -> ModuleNode {
        let attrs = ModuleAttributes {
            vb_name: name.to_string(),
            ..ModuleAttributes::default()
        };
        ModuleNode::new(name, ModuleKind::Procedural, attrs)
    }

    fn simple_class_module(name: &str) -> ModuleNode {
        let attrs = ModuleAttributes {
            vb_name: name.to_string(),
            ..ModuleAttributes::default()
        };
        ModuleNode::new(name, ModuleKind::Class, attrs)
    }

    fn simple_extension_module(name: &str) -> ModuleNode {
        let attrs = ModuleAttributes {
            vb_name: name.to_string(),
            ..ModuleAttributes::default()
        };
        ModuleNode::new(name, ModuleKind::Extension, attrs)
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf()
    }

    fn repo_path(relative: &str) -> PathBuf {
        workspace_root().join(relative)
    }

    #[test]
    fn project_graph_rejects_invalid_project_name() {
        let mut graph = ProjectGraph::default();
        let err = graph
            .create_project("123bad", ProjectKind::Source)
            .expect_err("invalid project names must be rejected");
        assert_eq!(err.code(), "PMR-E-PROJECT-NAME-INVALID");
    }

    #[test]
    fn project_graph_rejects_duplicate_project_name_case_insensitive() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("first project should succeed");
        let err = graph
            .create_project("alpha", ProjectKind::Source)
            .expect_err("duplicate project name should fail");
        assert_eq!(err.code(), "PMR-E-PROJECT-NAME-DUPLICATE");
    }

    #[test]
    fn project_node_rejects_duplicate_module_name_case_insensitive() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_module(simple_proc_module("Worker"))
            .expect("first module should be accepted");
        let err = project
            .add_module(simple_proc_module("worker"))
            .expect_err("duplicate module names must be rejected");
        assert_eq!(err.code(), "PMR-E-MODULE-NAME-DUPLICATE");
    }

    #[test]
    fn project_node_rejects_module_name_over_31_chars() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        let long_name = "ABCDEFGHIJKLMNOPQRSTUVWXZY123456789";
        let attrs = ModuleAttributes {
            vb_name: long_name.to_string(),
            ..ModuleAttributes::default()
        };
        let module = ModuleNode::new(long_name, ModuleKind::Procedural, attrs);
        let err = project
            .add_module(module)
            .expect_err("module name length must be validated");
        assert_eq!(err.code(), "PMR-E-MODULE-NAME-LENGTH");
    }

    #[test]
    fn source_project_class_attribute_constraints_are_enforced() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let attrs = ModuleAttributes {
            vb_name: "Widget".to_string(),
            vb_global_namespace: true,
            vb_creatable: false,
            vb_predeclared_id: false,
            vb_exposed: false,
            option_private_module: false,
        };
        let err = project
            .add_module(ModuleNode::new("Widget", ModuleKind::Class, attrs))
            .expect_err("source class attribute rule should be enforced");
        assert_eq!(err.code(), "PMR-E-MODULE-CLASS-ATTRIBUTE");
    }

    #[test]
    fn option_private_module_is_rejected_for_non_procedural_modules() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let attrs = ModuleAttributes {
            vb_name: "Widget".to_string(),
            option_private_module: true,
            ..ModuleAttributes::default()
        };
        let err = project
            .add_module(ModuleNode::new("Widget", ModuleKind::Class, attrs))
            .expect_err("Option Private Module should be restricted to procedural modules");
        assert_eq!(err.code(), "PMR-E-OPTION-PRIVATE-MODULE-KIND");
    }

    #[test]
    fn extension_modules_are_rejected_for_non_host_projects() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let err = project
            .add_module(simple_extension_module("HostExt"))
            .expect_err("extension modules should require host projects");
        assert_eq!(err.code(), "PMR-E-MODULE-EXTENSION-PROJECT-KIND");
    }

    #[test]
    fn host_projects_accept_extension_module_attach() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("HostBook", ProjectKind::Host)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .register_extensible_module("HostExt")
            .expect("host project should accept extensible target registration");

        project
            .attach_host_extension_module(simple_extension_module("HostExt"))
            .expect("host projects should accept extension modules");

        let module = project.module("HostExt").expect("extension module should exist");
        assert_eq!(module.module_kind, ModuleKind::Extension);
    }

    #[test]
    fn host_extension_attach_rejects_non_extension_modules() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("HostBook", ProjectKind::Host)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .register_extensible_module("MainModule")
            .expect("host project should accept extensible target registration");

        let err = project
            .attach_host_extension_module(simple_proc_module("MainModule"))
            .expect_err("host extension attach should require extension modules");
        assert_eq!(err.code(), "PMR-E-HOST-EXTENSION-MODULE-KIND");
    }

    #[test]
    fn host_extension_attach_requires_registered_extensible_target() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("HostBook", ProjectKind::Host)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let err = project
            .attach_host_extension_module(simple_extension_module("HostExt"))
            .expect_err("host extension attach should require registered extensible target");
        assert_eq!(err.code(), "PMR-E-HOST-EXTENSION-TARGET-MISSING");
    }

    #[test]
    fn extensible_modules_require_host_projects() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let err = project
            .register_extensible_module("Sheet1")
            .expect_err("extensible module registration should require host projects");
        assert_eq!(err.code(), "PMR-E-HOST-EXTENSIBLE-MODULE-PROJECT-KIND");
    }

    #[test]
    fn extensible_module_registration_is_case_insensitive() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("HostBook", ProjectKind::Host)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .register_extensible_module("Sheet1")
            .expect("extensible module should register");

        assert!(project.is_extensible_module("sheet1"));
        project
            .attach_host_extension_module(simple_extension_module("SHEET1"))
            .expect("extension attach should match extensible target case-insensitively");
    }

    #[test]
    fn references_are_precedence_ordered_and_case_insensitive_unique() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        project
            .add_reference("CoreLib", ReferenceKind::Project)
            .expect("first reference should succeed");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("second reference should succeed");
        assert_eq!(project.references[0].precedence_index, 0);
        assert_eq!(project.references[1].precedence_index, 1);

        let err = project
            .add_reference("corelib", ReferenceKind::Project)
            .expect_err("duplicate reference target should fail");
        assert_eq!(err.code(), "PMR-E-REFERENCE-DUPLICATE-TARGET");
    }

    #[test]
    fn type_library_resolution_binds_unique_importlib_entry() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_importlib("StdOle", "stdole2.tlb")
            .expect("importlib hint should be set");
        let records = project.resolve_type_library_references(&[TypeLibraryCatalogEntry {
            library_name: "StdOle".to_string(),
            importlib: "stdole2.tlb".to_string(),
            libid: None,
            major_version: 2,
            minor_version: 0,
            lcid: None,
        }]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].reference_name, "StdOle");
        assert_eq!(records[0].status.code(), "PMR-I-TYPELIB-BOUND");
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Bound
        );
    }

    #[test]
    fn type_library_resolution_requires_importlib_hint() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        let records = project.resolve_type_library_references(&[]);
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].status,
            TypeLibraryBindingStatus::MissingImportLibHint
        );
        assert_eq!(records[0].status.code(), "PMR-E-TYPELIB-IMPORTLIB-MISSING");
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Failed
        );
    }

    #[test]
    fn set_reference_importlib_rejects_unknown_reference_name() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        let err = project
            .set_reference_importlib("StdOle", "stdole2.tlb")
            .expect_err("unknown references must be rejected");
        assert_eq!(err.code(), "PMR-E-REFERENCE-NOT-FOUND");
    }

    #[test]
    fn set_reference_importlib_rejects_non_typelib_references() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("CoreLib", ReferenceKind::Project)
            .expect("project reference should be accepted");
        let err = project
            .set_reference_importlib("CoreLib", "corelib.tlb")
            .expect_err("importlib hints must only apply to TypeLibrary references");
        assert_eq!(err.code(), "PMR-E-TYPELIB-KIND-MISMATCH");
    }

    #[test]
    fn type_library_resolution_reports_ambiguous_importlib() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_importlib("StdOle", "stdole2.tlb")
            .expect("importlib hint should be set");
        let records = project.resolve_type_library_references(&[
            TypeLibraryCatalogEntry {
                library_name: "StdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: None,
                major_version: 2,
                minor_version: 0,
                lcid: None,
            },
            TypeLibraryCatalogEntry {
                library_name: "AltStdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: None,
                major_version: 1,
                minor_version: 9,
                lcid: None,
            },
        ]);
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].status.code(),
            "PMR-E-TYPELIB-IMPORTLIB-AMBIGUOUS"
        );
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Failed
        );
    }

    #[test]
    fn type_library_resolution_binds_unique_libid_identity() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_typelib_identity(
                "StdOle",
                "{00020430-0000-0000-C000-000000000046}",
                Some(2),
                Some(0),
                Some(0),
            )
            .expect("libid identity should be set");
        let records = project.resolve_type_library_references(&[
            TypeLibraryCatalogEntry {
                library_name: "StdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: Some("00020430-0000-0000-C000-000000000046".to_string()),
                major_version: 2,
                minor_version: 0,
                lcid: Some(0),
            },
            TypeLibraryCatalogEntry {
                library_name: "AltStdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: Some("00020430-0000-0000-C000-000000000046".to_string()),
                major_version: 1,
                minor_version: 9,
                lcid: Some(0),
            },
        ]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status.code(), "PMR-I-TYPELIB-BOUND");
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Bound
        );
    }

    #[test]
    fn type_library_resolution_reports_ambiguous_libid_identity() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_typelib_identity(
                "StdOle",
                "{00020430-0000-0000-C000-000000000046}",
                None,
                None,
                None,
            )
            .expect("libid identity should be set");
        let records = project.resolve_type_library_references(&[
            TypeLibraryCatalogEntry {
                library_name: "StdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: Some("00020430-0000-0000-c000-000000000046".to_string()),
                major_version: 2,
                minor_version: 0,
                lcid: Some(0),
            },
            TypeLibraryCatalogEntry {
                library_name: "AltStdOle".to_string(),
                importlib: "stdole2_v2.tlb".to_string(),
                libid: Some("00020430-0000-0000-c000-000000000046".to_string()),
                major_version: 2,
                minor_version: 0,
                lcid: Some(1033),
            },
        ]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status.code(), "PMR-E-TYPELIB-LIBID-AMBIGUOUS");
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Failed
        );
    }

    #[test]
    fn type_library_resolution_reports_unresolved_libid_identity() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_typelib_identity(
                "StdOle",
                "{00020430-0000-0000-C000-000000000046}",
                Some(2),
                Some(0),
                Some(0),
            )
            .expect("libid identity should be set");
        let records = project.resolve_type_library_references(&[TypeLibraryCatalogEntry {
            library_name: "AltStdOle".to_string(),
            importlib: "stdole2_v2.tlb".to_string(),
            libid: Some("00020430-0000-0000-C000-000000000047".to_string()),
            major_version: 2,
            minor_version: 0,
            lcid: Some(0),
        }]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status.code(), "PMR-E-TYPELIB-LIBID-UNRESOLVED");
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Failed
        );
    }

    #[test]
    fn type_library_resolution_is_deterministic_across_catalog_order() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("reference should be accepted");
        project
            .set_reference_importlib("StdOle", " stdole2.tlb ")
            .expect("importlib hint should be set");

        let catalog_a = vec![
            TypeLibraryCatalogEntry {
                library_name: "AltStdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: None,
                major_version: 1,
                minor_version: 9,
                lcid: None,
            },
            TypeLibraryCatalogEntry {
                library_name: "StdOle".to_string(),
                importlib: "stdole2.tlb".to_string(),
                libid: None,
                major_version: 2,
                minor_version: 0,
                lcid: None,
            },
        ];
        let catalog_b = vec![catalog_a[1].clone(), catalog_a[0].clone()];

        let first = project.resolve_type_library_references(&catalog_a);
        project.references[0].binding_state = ReferenceBindingState::Unbound;
        let second = project.resolve_type_library_references(&catalog_b);
        assert_eq!(first, second);
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Failed
        );
    }

    #[test]
    fn type_library_resolution_does_not_mutate_non_typelib_references() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");
        project
            .add_reference("CoreLib", ReferenceKind::Project)
            .expect("project reference should be accepted");
        project
            .add_reference("StdOle", ReferenceKind::TypeLibrary)
            .expect("typelib reference should be accepted");
        project
            .set_reference_importlib("StdOle", "stdole2.tlb")
            .expect("importlib hint should be set");

        let before = project.references[0].binding_state;
        let _ = project.resolve_type_library_references(&[]);
        assert_eq!(before, ReferenceBindingState::Unbound);
        assert_eq!(
            project.references[0].binding_state,
            ReferenceBindingState::Unbound
        );
        assert_eq!(
            project.references[1].binding_state,
            ReferenceBindingState::Failed
        );
    }

    #[test]
    fn public_symbol_collisions_require_qualification() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        project
            .add_module(simple_proc_module("First"))
            .expect("first module should be accepted");
        project
            .add_module(simple_proc_module("Second"))
            .expect("second module should be accepted");

        project
            .register_public_symbol("First", "Work")
            .expect("first symbol owner should register");
        project
            .register_public_symbol("Second", "Work")
            .expect("second symbol owner should register");

        let resolution = project.resolve_public_symbol("Work");
        match resolution {
            PublicSymbolResolution::RequiresQualification { module_names } => {
                assert_eq!(
                    module_names,
                    vec!["first".to_string(), "second".to_string()]
                );
            }
            other => panic!("expected qualification requirement, got {other:?}"),
        }
    }

    #[test]
    fn class_default_instance_flag_is_derived_from_attributes() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let mut module = simple_class_module("Widget");
        module.attributes.vb_predeclared_id = true;
        project
            .add_module(module)
            .expect("class module should be accepted");

        let enabled = project
            .default_instance_enabled("Widget")
            .expect("module should exist");
        assert!(enabled);
    }

    #[test]
    fn project_graph_operations_are_deterministic_for_equal_inputs() {
        fn build_graph() -> ProjectGraph {
            let mut graph = ProjectGraph::default();
            graph
                .create_project("Alpha", ProjectKind::Source)
                .expect("project should be created");
            {
                let project = graph.active_project_mut().expect("active project expected");
                project
                    .add_module(simple_proc_module("MainModule"))
                    .expect("module should be accepted");
                project
                    .add_module(simple_proc_module("Helpers"))
                    .expect("module should be accepted");
                project
                    .add_reference("CoreLib", ReferenceKind::Project)
                    .expect("reference should be accepted");
                project
                    .add_reference("StdOle", ReferenceKind::TypeLibrary)
                    .expect("reference should be accepted");
                project
                    .register_public_symbol("MainModule", "Run")
                    .expect("symbol should register");
                project
                    .register_public_symbol("Helpers", "Run")
                    .expect("symbol should register");
            }
            graph
        }

        let g1 = build_graph();
        let g2 = build_graph();
        assert_eq!(g1, g2);
    }

    #[test]
    fn active_project_resolution_prefers_local_symbols_before_references() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        graph
            .create_project("LibA", ProjectKind::Library)
            .expect("project should be created");
        graph
            .set_active_project("Alpha")
            .expect("active project should exist");

        {
            let alpha = graph.project_mut("Alpha").expect("project should exist");
            alpha
                .add_module(simple_proc_module("LocalModule"))
                .expect("module should be accepted");
            alpha
                .register_public_symbol("LocalModule", "Run")
                .expect("symbol should register");
            alpha
                .add_reference("LibA", ReferenceKind::Project)
                .expect("reference should register");
        }
        {
            let lib = graph.project_mut("LibA").expect("project should exist");
            lib.add_module(simple_proc_module("RemoteModule"))
                .expect("module should be accepted");
            lib.register_public_symbol("RemoteModule", "Run")
                .expect("symbol should register");
        }

        let resolved = graph
            .resolve_active_public_symbol("Run")
            .expect("resolution should succeed");
        assert_eq!(
            resolved,
            GraphPublicSymbolResolution::Unique {
                project_name: "Alpha".to_string(),
                module_name: "localmodule".to_string(),
            }
        );
    }

    #[test]
    fn active_project_resolution_uses_reference_precedence_order_for_shadowing() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        graph
            .create_project("LibA", ProjectKind::Library)
            .expect("project should be created");
        graph
            .create_project("LibB", ProjectKind::Library)
            .expect("project should be created");
        graph
            .set_active_project("Alpha")
            .expect("active project should exist");

        {
            let alpha = graph.project_mut("Alpha").expect("project should exist");
            alpha
                .add_module(simple_proc_module("MainModule"))
                .expect("module should be accepted");
            alpha
                .add_reference("LibB", ReferenceKind::Project)
                .expect("reference should register");
            alpha
                .add_reference("LibA", ReferenceKind::Project)
                .expect("reference should register");
        }
        {
            let lib_a = graph.project_mut("LibA").expect("project should exist");
            lib_a
                .add_module(simple_proc_module("FirstModule"))
                .expect("module should be accepted");
            lib_a
                .register_public_symbol("FirstModule", "Compute")
                .expect("symbol should register");
        }
        {
            let lib_b = graph.project_mut("LibB").expect("project should exist");
            lib_b
                .add_module(simple_proc_module("ShadowModule"))
                .expect("module should be accepted");
            lib_b
                .register_public_symbol("ShadowModule", "Compute")
                .expect("symbol should register");
        }

        let resolved = graph
            .resolve_active_public_symbol("Compute")
            .expect("resolution should succeed");
        assert_eq!(
            resolved,
            GraphPublicSymbolResolution::Unique {
                project_name: "LibB".to_string(),
                module_name: "shadowmodule".to_string(),
            }
        );
    }

    #[test]
    fn formal_pmr_a5_claim_tiers_have_stable_status_values() {
        let path = repo_path("docs/spec/PROJECT_MODULE_REFERENCE_CLAUSE_CATALOG_V1.csv");
        let text = std::fs::read_to_string(path).expect("PMR clause catalog should be readable");
        let mut rows = text.lines();
        let header = rows.next().expect("header row expected");
        assert!(header.starts_with("clause_id,domain,status,"));

        let allowed = [
            "implemented-verified",
            "implemented-partial",
            "specified-pending",
        ];
        for row in rows {
            if row.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = row.split(',').collect();
            assert!(
                cols.len() >= 5,
                "expected at least 5 columns in PMR catalog row: {row}"
            );
            let status = cols[2].trim();
            assert!(
                allowed.contains(&status),
                "invalid status `{status}` in PMR catalog row: {row}"
            );
            if status == "implemented-verified" {
                let anchor = cols[4].trim();
                assert!(
                    !anchor.contains("planned"),
                    "implemented-verified row must not use planned-only anchor: {row}"
                );
            }
        }
    }

    #[test]
    fn error_codes_are_stable_for_machine_consumers() {
        let err = ProjectModelError::ModuleNameInvalid {
            name: "123".to_string(),
        };
        assert_eq!(err.code(), "PMR-E-MODULE-NAME-INVALID");
        assert!(err.to_string().contains("PMR-E-MODULE-NAME-INVALID"));
    }

    #[test]
    fn host_export_registry_exposes_public_procedural_entries() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        project
            .add_module(simple_proc_module("MainModule"))
            .expect("module should be accepted");
        project
            .register_host_export("MainModule", "Add", HostExportKind::Function)
            .expect("export should register");
        project
            .register_host_export("MainModule", "Ping", HostExportKind::Sub)
            .expect("export should register");

        let exports = project.host_exports();
        assert_eq!(exports.len(), 2);
        assert!(exports.iter().any(|entry| entry.module_name == "mainmodule"
            && entry.procedure_name == "add"
            && entry.kind == HostExportKind::Function));
    }

    #[test]
    fn host_export_registry_includes_option_private_modules_for_host_calls() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        let attrs = ModuleAttributes {
            vb_name: "PrivateModule".to_string(),
            option_private_module: true,
            ..ModuleAttributes::default()
        };
        project
            .add_module(ModuleNode::new(
                "PrivateModule",
                ModuleKind::Procedural,
                attrs,
            ))
            .expect("module should be accepted");
        project
            .register_host_export("PrivateModule", "Hidden", HostExportKind::Function)
            .expect("export should register");

        let exports = project.host_exports();
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].module_name, "privatemodule");
        assert_eq!(exports[0].procedure_name, "hidden");
        assert_eq!(exports[0].kind, HostExportKind::Function);

        let reference_visible = project.reference_visible_exports();
        assert!(
            reference_visible.is_empty(),
            "Option Private module exports are host-direct only and not reference-visible"
        );
    }

    #[test]
    fn host_export_registry_rejects_non_procedural_modules() {
        let mut graph = ProjectGraph::default();
        graph
            .create_project("Alpha", ProjectKind::Source)
            .expect("project should be created");
        let project = graph.active_project_mut().expect("active project expected");

        project
            .add_module(simple_class_module("Widget"))
            .expect("module should be accepted");
        let err = project
            .register_host_export("Widget", "Create", HostExportKind::Function)
            .expect_err("class modules should not register host exports");
        assert_eq!(err.code(), "PMR-E-HOST-EXPORT-MODULE-KIND");
    }
}
