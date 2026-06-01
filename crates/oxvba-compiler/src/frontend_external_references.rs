use crate::frontend_symbols::SymbolId;
use crate::{DescriptorIdentity, ReferenceKind};

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

pub fn external_reference_symbol(
    symbol: SymbolId,
    reference_kind: ReferenceKind,
    descriptor: DescriptorIdentity,
) -> ExternalReferenceSymbol {
    let kind = match reference_kind {
        ReferenceKind::TypeLibrary => ExternalReferenceKind::TypeLibrary,
        ReferenceKind::Project => ExternalReferenceKind::Project,
        ReferenceKind::HostInjected => ExternalReferenceKind::Native,
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DescriptorFamily;

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
}
