use crate::frontend_hir::{
    HirExprId, HirExprKind, HirLiteral, HirPropertyKind, HirStmtId, HirStmtKind,
};
use crate::frontend_symbols::SymbolId;
use crate::frontend_type_hooks::{HirAssignmentIntent, TypedHirModule};
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

pub fn collect_property_accessors_from_typed_hir(
    typed: &TypedHirModule,
) -> Vec<PropertyAccessorRoute> {
    typed
        .module
        .arenas
        .properties()
        .iter()
        .map(|property| property_accessor(property.symbol, property.kind))
        .collect()
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

pub fn collect_assignment_semantics_from_typed_hir(
    typed: &TypedHirModule,
) -> Vec<AssignmentSemantics> {
    let mut out = Vec::new();
    for (stmt, intent) in typed.hooks.assignment_intents() {
        let Some(stmt_data) = typed.module.arenas.stmt(stmt) else {
            continue;
        };
        let (target, value) = match stmt_data.kind {
            HirStmtKind::Let { target, value, .. } | HirStmtKind::Set { target, value } => {
                (target, value)
            }
            _ => continue,
        };
        let Some(target_type) = infer_expr_type(typed, target) else {
            continue;
        };
        let Some(value_type) = infer_expr_type(typed, value) else {
            continue;
        };
        out.push(assignment_semantics(
            stmt,
            target,
            value,
            intent,
            target_type,
            value_type,
        ));
    }
    out
}

fn infer_expr_type(typed: &TypedHirModule, expr: HirExprId) -> Option<VbaTypeId> {
    match typed.module.arenas.expr(expr).map(|expr| &expr.kind)? {
        HirExprKind::Name(symbol) => typed
            .hooks
            .declared_type(*symbol)
            .map(|hook| hook.runtime_type),
        HirExprKind::Literal(HirLiteral::Bool(_)) => Some(VbaTypeId::Boolean),
        HirExprKind::Literal(HirLiteral::Int(_)) => Some(VbaTypeId::Long),
        HirExprKind::Literal(HirLiteral::String(_)) => Some(VbaTypeId::String),
        HirExprKind::Literal(HirLiteral::Empty | HirLiteral::Null) => Some(VbaTypeId::Variant),
        HirExprKind::Literal(HirLiteral::Nothing) => Some(VbaTypeId::Object),
        HirExprKind::Binary { lhs, rhs, .. } => {
            let lhs_type = infer_expr_type(typed, *lhs)?;
            let rhs_type = infer_expr_type(typed, *rhs)?;
            Some(binary_result_type(lhs_type, rhs_type))
        }
        _ => None,
    }
}

fn binary_result_type(lhs: VbaTypeId, rhs: VbaTypeId) -> VbaTypeId {
    if lhs == VbaTypeId::Variant || rhs == VbaTypeId::Variant {
        return VbaTypeId::Variant;
    }
    if lhs == VbaTypeId::String || rhs == VbaTypeId::String {
        return VbaTypeId::String;
    }
    if lhs == VbaTypeId::Double || rhs == VbaTypeId::Double {
        return VbaTypeId::Double;
    }
    if lhs == VbaTypeId::Single || rhs == VbaTypeId::Single {
        return VbaTypeId::Single;
    }
    if lhs == VbaTypeId::LongLong || rhs == VbaTypeId::LongLong {
        return VbaTypeId::LongLong;
    }
    if lhs == VbaTypeId::Long || rhs == VbaTypeId::Long {
        return VbaTypeId::Long;
    }
    lhs
}

fn assignment_diagnostic(
    intent: HirAssignmentIntent,
    target_type: VbaTypeId,
    value_type: VbaTypeId,
) -> Option<AssignmentDiagnostic> {
    match (intent, target_type, value_type) {
        (HirAssignmentIntent::Set, target_type, _) if is_scalar_type(target_type) => {
            Some(AssignmentDiagnostic {
                code: "BIND-E-SET-REQUIRES-OBJECT".to_string(),
                message: "Set assignment requires an object or variant target".to_string(),
            })
        }
        (HirAssignmentIntent::Set, _, value_type) if is_scalar_type(value_type) => {
            Some(AssignmentDiagnostic {
                code: "BIND-E-SET-REQUIRES-OBJECT-VALUE".to_string(),
                message: "Set assignment requires an object or variant value".to_string(),
            })
        }
        (HirAssignmentIntent::Let, VbaTypeId::Object, _) => Some(AssignmentDiagnostic {
            code: "BIND-E-LET-OBJECT-TARGET".to_string(),
            message: "Let assignment cannot assign directly to an object target".to_string(),
        }),
        _ => None,
    }
}

fn is_scalar_type(ty: VbaTypeId) -> bool {
    matches!(
        ty,
        VbaTypeId::Boolean
            | VbaTypeId::Byte
            | VbaTypeId::Integer
            | VbaTypeId::Long
            | VbaTypeId::LongLong
            | VbaTypeId::LongPtr
            | VbaTypeId::Single
            | VbaTypeId::Double
            | VbaTypeId::Currency
            | VbaTypeId::Date
            | VbaTypeId::String
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_type_hooks::collect_type_hooks_from_source;

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

    #[test]
    fn assignment_semantics_collects_from_typed_hir_route() {
        let typed = collect_type_hooks_from_source(
            "Module1",
            "Sub Main()\nDim obj As Object\nDim value As Long\nSet obj = Nothing\nvalue = obj\nEnd Sub",
        )
        .expect("typed hir");

        let semantics = collect_assignment_semantics_from_typed_hir(&typed);
        assert_eq!(semantics.len(), 2);
        assert!(
            semantics.iter().any(|item| {
                item.intent == HirAssignmentIntent::Set
                    && item.coercion == CoercionKindDescriptor::Set
                    && item.target_type == VbaTypeId::Object
                    && item.value_type == VbaTypeId::Object
                    && item.diagnostic.is_none()
            }),
            "Set object assignment should be represented without diagnostic: {semantics:?}"
        );
        assert!(
            semantics.iter().any(|item| {
                item.intent == HirAssignmentIntent::Let
                    && item.coercion == CoercionKindDescriptor::Let
                    && item.target_type == VbaTypeId::Long
                    && item.value_type == VbaTypeId::Object
            }),
            "Let assignment should carry typed HIR source/target facts: {semantics:?}"
        );
    }

    #[test]
    fn property_accessors_collect_from_typed_hir_route() {
        let typed = collect_type_hooks_from_source(
            "Module1",
            "Property Get Value()\nEnd Property\nProperty Let Value(ByVal newValue)\nEnd Property\nProperty Set Value(ByVal newValue)\nEnd Property",
        )
        .expect("typed hir");

        let accessors = collect_property_accessors_from_typed_hir(&typed);
        assert_eq!(
            accessors
                .iter()
                .filter(|accessor| accessor.kind == HirPropertyKind::Get)
                .count(),
            1
        );
        assert_eq!(
            accessors
                .iter()
                .filter(|accessor| accessor.kind == HirPropertyKind::Let)
                .count(),
            1
        );
        assert_eq!(
            accessors
                .iter()
                .filter(|accessor| accessor.kind == HirPropertyKind::Set)
                .count(),
            1
        );
    }
}
