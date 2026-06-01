use std::collections::BTreeMap;

use crate::frontend_symbols::SymbolId;
use crate::{
    DescriptorFamily, DescriptorIdentity, ProjectManifest, ReferenceKind, canonical_descriptor_id,
    descriptor_digest_from_fields,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalReferenceKind {
    TypeLibrary,
    Project,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalReferenceSymbol {
    pub symbol: SymbolId,
    pub kind: ExternalReferenceKind,
    pub descriptor: DescriptorIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalReferenceBinding {
    pub reference: ExternalReferenceSymbol,
    pub member_symbol: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalReferenceDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalReferenceIndex {
    references_by_name: BTreeMap<String, Vec<ExternalReferenceSymbol>>,
}

impl ExternalReferenceIndex {
    pub fn resolve(
        &self,
        name: &str,
        kind: ReferenceKind,
    ) -> Result<ExternalReferenceBinding, ExternalReferenceDiagnostic> {
        let folded = fold_reference_name(name);
        let Some(candidates) = self.references_by_name.get(&folded) else {
            return Err(unresolved_external_reference_diagnostic(name));
        };
        let Some(reference) = candidates
            .iter()
            .find(|candidate| candidate.kind == external_reference_kind(kind))
            .cloned()
        else {
            return Err(unresolved_external_reference_diagnostic(name));
        };
        Ok(bind_external_reference_member(reference, None))
    }

    pub fn resolve_kind(&self, name: &str) -> Option<ExternalReferenceKind> {
        self.references_by_name
            .get(&fold_reference_name(name))
            .and_then(|references| references.first())
            .map(|reference| reference.kind.clone())
    }
}

pub fn build_external_reference_index(manifest: &ProjectManifest) -> ExternalReferenceIndex {
    let mut references_by_name: BTreeMap<String, Vec<ExternalReferenceSymbol>> = BTreeMap::new();
    for (index, reference) in manifest.references.iter().enumerate() {
        let symbol = external_reference_symbol(
            SymbolId(index + 1),
            reference.reference_kind,
            descriptor_identity_for_reference(
                reference.reference_kind,
                &reference.referenced_project_name,
            ),
        );
        references_by_name
            .entry(fold_reference_name(&reference.referenced_project_name))
            .or_default()
            .push(symbol);
    }
    ExternalReferenceIndex { references_by_name }
}

pub fn external_reference_symbol(
    symbol: SymbolId,
    reference_kind: ReferenceKind,
    descriptor: DescriptorIdentity,
) -> ExternalReferenceSymbol {
    let kind = external_reference_kind(reference_kind);
    ExternalReferenceSymbol {
        symbol,
        kind,
        descriptor,
    }
}

pub fn bind_external_reference_member(
    reference: ExternalReferenceSymbol,
    member_symbol: Option<SymbolId>,
) -> ExternalReferenceBinding {
    ExternalReferenceBinding {
        reference,
        member_symbol,
    }
}

pub fn unresolved_external_reference_diagnostic(name: &str) -> ExternalReferenceDiagnostic {
    ExternalReferenceDiagnostic {
        code: "BIND-E-EXTERNAL-REFERENCE-UNRESOLVED".to_string(),
        message: format!("external reference `{name}` was not resolved"),
    }
}

fn external_reference_kind(reference_kind: ReferenceKind) -> ExternalReferenceKind {
    match reference_kind {
        ReferenceKind::TypeLibrary => ExternalReferenceKind::TypeLibrary,
        ReferenceKind::Project => ExternalReferenceKind::Project,
        ReferenceKind::HostInjected => ExternalReferenceKind::Native,
    }
}

fn descriptor_identity_for_reference(
    reference_kind: ReferenceKind,
    reference_name: &str,
) -> DescriptorIdentity {
    let kind = match reference_kind {
        ReferenceKind::TypeLibrary => "typelib",
        ReferenceKind::Project => "project",
        ReferenceKind::HostInjected => "native",
    };
    let folded = fold_reference_name(reference_name);
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::Interop,
        ["external-reference", kind, &folded],
    );
    let descriptor_digest = descriptor_digest_from_fields(
        DescriptorFamily::Interop,
        &descriptor_id,
        [
            ("kind", kind.to_string()),
            ("reference_name", reference_name.to_string()),
            ("folded_name", folded),
        ],
    );
    DescriptorIdentity {
        family: DescriptorFamily::Interop,
        descriptor_id,
        descriptor_digest,
    }
}

fn fold_reference_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DescriptorFamily, ModuleKind, ProjectKind, ProjectManifest, ProjectReference,
        project::module_unit_from_source,
    };

    fn descriptor(name: &str) -> DescriptorIdentity {
        DescriptorIdentity {
            family: DescriptorFamily::Interop,
            descriptor_id: name.to_string(),
            descriptor_digest: format!("{name}:digest"),
        }
    }

    #[test]
    fn external_references_bind_typelib_project_and_native_kinds() {
        assert_eq!(
            external_reference_symbol(SymbolId(1), ReferenceKind::TypeLibrary, descriptor("Excel"))
                .kind,
            ExternalReferenceKind::TypeLibrary
        );
        assert_eq!(
            external_reference_symbol(SymbolId(2), ReferenceKind::Project, descriptor("ProjectA"))
                .kind,
            ExternalReferenceKind::Project
        );
        assert_eq!(
            external_reference_symbol(SymbolId(3), ReferenceKind::HostInjected, descriptor("Host"))
                .kind,
            ExternalReferenceKind::Native
        );
    }

    #[test]
    fn external_references_bind_members_through_descriptor_backed_symbols() {
        let reference =
            external_reference_symbol(SymbolId(1), ReferenceKind::TypeLibrary, descriptor("Excel"));
        let binding = bind_external_reference_member(reference, Some(SymbolId(9)));
        assert_eq!(binding.member_symbol, Some(SymbolId(9)));
        assert!(
            binding.reference.descriptor.descriptor_id.contains("Excel"),
            "{binding:#?}"
        );
    }

    #[test]
    fn external_references_emit_stable_unresolved_diagnostic() {
        assert_eq!(
            unresolved_external_reference_diagnostic("Excel").code,
            "BIND-E-EXTERNAL-REFERENCE-UNRESOLVED"
        );
    }

    #[test]
    fn external_reference_index_resolves_manifest_references_through_descriptors() {
        let main_module = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
        )
        .expect("module parses");
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main_module],
            references: vec![ProjectReference {
                referenced_project_name: "OxVba".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };

        let index = build_external_reference_index(&manifest);
        let binding = index
            .resolve("oxvba", ReferenceKind::TypeLibrary)
            .expect("typelib reference resolves through frontend index");

        assert_eq!(binding.reference.kind, ExternalReferenceKind::TypeLibrary);
        assert_eq!(
            binding.reference.descriptor.descriptor_id,
            "interop:external-reference:typelib:oxvba"
        );
    }

    #[test]
    fn external_reference_index_resolves_project_and_native_kinds() {
        let manifest = ProjectManifest {
            project_name: "ProjectA".to_string(),
            project_kind: ProjectKind::Source,
            modules: Vec::new(),
            references: vec![
                ProjectReference {
                    referenced_project_name: "LibraryProject".to_string(),
                    reference_kind: ReferenceKind::Project,
                },
                ProjectReference {
                    referenced_project_name: "HostProject".to_string(),
                    reference_kind: ReferenceKind::HostInjected,
                },
            ],
            reference_projects: Vec::new(),
            conditional_constants: Default::default(),
        };

        let index = build_external_reference_index(&manifest);

        assert_eq!(
            index.resolve_kind("libraryproject"),
            Some(ExternalReferenceKind::Project)
        );
        assert_eq!(
            index.resolve_kind("hostproject"),
            Some(ExternalReferenceKind::Native)
        );
    }
}
