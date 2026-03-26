use std::collections::BTreeMap;

use crate::{
    error::HalResult,
    model::{CapabilityId, HalProfileId},
    HalError, HalErrorKind,
};
use crate::traits::{ProjectCatalogHal, ProjectReferenceHal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectDescriptorKind {
    Source,
    Host,
    Library,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectDescriptor {
    pub project_name: String,
    pub kind: ProjectDescriptorKind,
    pub supports_extension_modules: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectReferenceKind {
    Project,
    TypeLibrary,
    HostInjected,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectReferenceDescriptor {
    pub project_name: String,
    pub referenced_name: String,
    pub kind: ProjectReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostExtensionModuleChange {
    pub project_name: String,
    pub module_name: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedProjectReference {
    Project(ProjectDescriptor),
    Unresolved { referenced_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectCallbackError {
    ProjectNotFound { project_name: String },
    ReferenceUnresolved { referenced_name: String },
    AdapterFault { message: String },
}

pub type ProjectCallbackResult<T> = Result<T, ProjectCallbackError>;

#[derive(Debug, Clone)]
pub struct InMemoryProjectCatalog {
    profile: HalProfileId,
    projects: BTreeMap<String, ProjectDescriptor>,
    references: BTreeMap<String, Vec<ProjectReferenceDescriptor>>,
}

impl InMemoryProjectCatalog {
    pub fn new(profile: HalProfileId) -> Self {
        Self {
            profile,
            projects: BTreeMap::new(),
            references: BTreeMap::new(),
        }
    }

    pub fn add_project(&mut self, descriptor: ProjectDescriptor) {
        self.projects
            .insert(normalize_project_name(&descriptor.project_name), descriptor);
    }

    pub fn add_reference(&mut self, descriptor: ProjectReferenceDescriptor) {
        let key = normalize_project_name(&descriptor.project_name);
        self.references.entry(key).or_default().push(descriptor);
    }

    pub(crate) fn profile(&self) -> HalProfileId {
        self.profile
    }
}

impl ProjectCatalogHal for InMemoryProjectCatalog {
    fn list_projects(&self) -> HalResult<Vec<ProjectDescriptor>> {
        let mut projects = self.projects.values().cloned().collect::<Vec<_>>();
        projects.sort();
        Ok(projects)
    }

    fn get_project(&self, project_name: &str) -> HalResult<ProjectDescriptor> {
        self.projects
            .get(&normalize_project_name(project_name))
            .cloned()
            .ok_or_else(|| project_not_found(self.profile(), "get_project", project_name))
    }
}

impl ProjectReferenceHal for InMemoryProjectCatalog {
    fn list_references(&self, project_name: &str) -> HalResult<Vec<ProjectReferenceDescriptor>> {
        let descriptor = self.get_project(project_name)?;
        let mut references = self
            .references
            .get(&normalize_project_name(&descriptor.project_name))
            .cloned()
            .unwrap_or_default();
        references.sort();
        Ok(references)
    }

    fn resolve_reference(
        &self,
        reference: &ProjectReferenceDescriptor,
    ) -> HalResult<ResolvedProjectReference> {
        self.projects
            .get(&normalize_project_name(&reference.referenced_name))
            .cloned()
            .map(ResolvedProjectReference::Project)
            .ok_or_else(|| project_reference_unresolved(self.profile(), &reference.referenced_name))
    }
}

fn normalize_project_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

pub(crate) fn project_not_found(
    profile: HalProfileId,
    operation: &'static str,
    project_name: &str,
) -> HalError {
    HalError {
        kind: HalErrorKind::AdapterFault,
        stable_code: "HAL-E-PROJ-NOT-FOUND",
        profile,
        capability: CapabilityId::ProjectCatalog,
        operation,
        message: format!("project `{project_name}` was not found"),
    }
}

pub(crate) fn project_reference_unresolved(
    profile: HalProfileId,
    referenced_name: &str,
) -> HalError {
    HalError {
        kind: HalErrorKind::AdapterFault,
        stable_code: "HAL-E-PROJ-REF-UNRESOLVED",
        profile,
        capability: CapabilityId::ProjectReferenceProvider,
        operation: "resolve_reference",
        message: format!("reference `{referenced_name}` could not be resolved"),
    }
}

#[cfg(test)]
mod tests {
    use crate::traits::{ProjectCatalogHal, ProjectReferenceHal};

    use super::{
        InMemoryProjectCatalog, ProjectDescriptor, ProjectDescriptorKind,
        ProjectReferenceDescriptor, ProjectReferenceKind, ResolvedProjectReference,
    };

    #[test]
    fn in_memory_project_catalog_lists_projects_deterministically() {
        let mut catalog = InMemoryProjectCatalog::new(crate::model::HalProfileId::Windows);
        catalog.add_project(ProjectDescriptor {
            project_name: "Workbook".to_string(),
            kind: ProjectDescriptorKind::Host,
            supports_extension_modules: true,
        });
        catalog.add_project(ProjectDescriptor {
            project_name: "Alpha".to_string(),
            kind: ProjectDescriptorKind::Source,
            supports_extension_modules: false,
        });

        let listed = catalog.list_projects().expect("list should succeed");
        assert_eq!(listed[0].project_name, "Alpha");
        assert_eq!(listed[1].project_name, "Workbook");
    }

    #[test]
    fn in_memory_project_catalog_reports_missing_project_with_stable_code() {
        let catalog = InMemoryProjectCatalog::new(crate::model::HalProfileId::Windows);
        let err = catalog
            .get_project("Missing")
            .expect_err("missing project should fail");
        assert_eq!(err.stable_code, "HAL-E-PROJ-NOT-FOUND");
    }

    #[test]
    fn in_memory_project_catalog_resolves_project_references() {
        let mut catalog = InMemoryProjectCatalog::new(crate::model::HalProfileId::Windows);
        catalog.add_project(ProjectDescriptor {
            project_name: "Workbook".to_string(),
            kind: ProjectDescriptorKind::Host,
            supports_extension_modules: true,
        });
        catalog.add_project(ProjectDescriptor {
            project_name: "HostExt".to_string(),
            kind: ProjectDescriptorKind::Library,
            supports_extension_modules: false,
        });
        let reference = ProjectReferenceDescriptor {
            project_name: "Workbook".to_string(),
            referenced_name: "HostExt".to_string(),
            kind: ProjectReferenceKind::Project,
        };
        catalog.add_reference(reference.clone());

        let listed = catalog
            .list_references("Workbook")
            .expect("reference list should succeed");
        assert_eq!(listed, vec![reference.clone()]);

        let resolved = catalog
            .resolve_reference(&reference)
            .expect("reference should resolve");
        assert_eq!(
            resolved,
            ResolvedProjectReference::Project(ProjectDescriptor {
                project_name: "HostExt".to_string(),
                kind: ProjectDescriptorKind::Library,
                supports_extension_modules: false,
            })
        );
    }

    #[test]
    fn in_memory_project_catalog_reports_unresolved_reference_with_stable_code() {
        let catalog = InMemoryProjectCatalog::new(crate::model::HalProfileId::Windows);
        let err = catalog
            .resolve_reference(&ProjectReferenceDescriptor {
                project_name: "Workbook".to_string(),
                referenced_name: "Missing".to_string(),
                kind: ProjectReferenceKind::Project,
            })
            .expect_err("missing reference target should fail");
        assert_eq!(err.stable_code, "HAL-E-PROJ-REF-UNRESOLVED");
    }
}
