use crate::frontend_hir::{HirExprId, HirPropertyKind, HirStmtId};
use crate::frontend_symbols::SymbolId;
use crate::frontend_type_hooks::HirAssignmentIntent;
use crate::{CoercionKindDescriptor, VbaTypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultMemberAction {
    Read,
    Write,
    Invoke,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyAccessorRoute {
    pub property: SymbolId,
    pub kind: HirPropertyKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultMemberRoute {
    pub receiver: SymbolId,
    pub member: SymbolId,
    pub action: DefaultMemberAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentSemantics {
    pub stmt: HirStmtId,
    pub target: HirExprId,
    pub value: HirExprId,
    pub intent: HirAssignmentIntent,
    pub coercion: CoercionKindDescriptor,
    pub target_type: VbaTypeId,
    pub value_type: VbaTypeId,
    pub diagnostic: Option<AssignmentDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentDiagnostic {
    pub code: String,
    pub message: String,
}

pub fn property_accessor(property: SymbolId, kind: HirPropertyKind) -> PropertyAccessorRoute {
    PropertyAccessorRoute { property, kind }
}

pub fn default_member_route(
    receiver: SymbolId,
    member: SymbolId,
    action: DefaultMemberAction,
) -> DefaultMemberRoute {
    DefaultMemberRoute {
        receiver,
        member,
        action,
    }
}

pub fn assignment_semantics(
    stmt: HirStmtId,
    target: HirExprId,
    value: HirExprId,
    intent: HirAssignmentIntent,
    target_type: VbaTypeId,
    value_type: VbaTypeId,
) -> AssignmentSemantics {
    let coercion = match intent {
        HirAssignmentIntent::Let => CoercionKindDescriptor::Let,
        HirAssignmentIntent::Set => CoercionKindDescriptor::Set,
    };
    let diagnostic = assignment_diagnostic(intent, target_type, value_type);
    AssignmentSemantics {
        stmt,
        target,
        value,
        intent,
        coercion,
        target_type,
        value_type,
        diagnostic,
    }
}

fn assignment_diagnostic(
    intent: HirAssignmentIntent,
    target_type: VbaTypeId,
    value_type: VbaTypeId,
) -> Option<AssignmentDiagnostic> {
    match (intent, target_type, value_type) {
        (HirAssignmentIntent::Set, VbaTypeId::Object, VbaTypeId::Object) => None,
        (HirAssignmentIntent::Set, _, _) => Some(AssignmentDiagnostic {
            code: "BIND-E-SET-REQUIRES-OBJECT".to_string(),
            message: "Set assignment requires an object target and object value".to_string(),
        }),
        (HirAssignmentIntent::Let, VbaTypeId::Object, _) => Some(AssignmentDiagnostic {
            code: "BIND-E-LET-OBJECT-TARGET".to_string(),
            message: "Let assignment cannot assign directly to an object target".to_string(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_semantics_distinguishes_property_get_let_set() {
        assert_eq!(
            property_accessor(SymbolId(1), HirPropertyKind::Get).kind,
            HirPropertyKind::Get
        );
        assert_eq!(
            property_accessor(SymbolId(1), HirPropertyKind::Let).kind,
            HirPropertyKind::Let
        );
        assert_eq!(
            property_accessor(SymbolId(1), HirPropertyKind::Set).kind,
            HirPropertyKind::Set
        );
    }

    #[test]
    fn assignment_semantics_tracks_default_member_read_write_invoke() {
        assert_eq!(
            default_member_route(SymbolId(1), SymbolId(2), DefaultMemberAction::Read).action,
            DefaultMemberAction::Read
        );
        assert_eq!(
            default_member_route(SymbolId(1), SymbolId(2), DefaultMemberAction::Write).action,
            DefaultMemberAction::Write
        );
        assert_eq!(
            default_member_route(SymbolId(1), SymbolId(2), DefaultMemberAction::Invoke).action,
            DefaultMemberAction::Invoke
        );
    }

    #[test]
    fn assignment_semantics_maps_let_set_coercions_and_object_diagnostics() {
        let let_scalar = assignment_semantics(
            HirStmtId(1),
            HirExprId(2),
            HirExprId(3),
            HirAssignmentIntent::Let,
            VbaTypeId::Long,
            VbaTypeId::String,
        );
        assert_eq!(let_scalar.coercion, CoercionKindDescriptor::Let);
        assert_eq!(let_scalar.diagnostic, None);

        let set_scalar = assignment_semantics(
            HirStmtId(4),
            HirExprId(5),
            HirExprId(6),
            HirAssignmentIntent::Set,
            VbaTypeId::Long,
            VbaTypeId::Object,
        );
        assert_eq!(set_scalar.coercion, CoercionKindDescriptor::Set);
        assert_eq!(
            set_scalar
                .diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("BIND-E-SET-REQUIRES-OBJECT")
        );
    }
}
