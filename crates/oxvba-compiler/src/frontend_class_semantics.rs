use crate::frontend_symbols::SymbolId;
use crate::{ObjectActivationDescriptor, ObjectEventBindingDescriptor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassConstructionKind {
    NewExpression,
    AsNewLazy,
    PredeclaredInstance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassConstructionRoute {
    pub class_symbol: SymbolId,
    pub instance_symbol: Option<SymbolId>,
    pub kind: ClassConstructionKind,
    pub activation: Option<ObjectActivationDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassFieldKind {
    Ordinary,
    WithEvents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassMemberRuntimeKind {
    Field,
    Property,
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassFieldRoute {
    pub class_symbol: SymbolId,
    pub field_symbol: SymbolId,
    pub field_kind: ClassFieldKind,
    pub member_kind: ClassMemberRuntimeKind,
    pub event_binding: Option<ObjectEventBindingDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeObjectFieldMetadata {
    pub class_symbol: SymbolId,
    pub field_symbol: SymbolId,
    pub slot: usize,
    pub refcounted: bool,
    pub initialized_by_as_new: bool,
    pub withevents: bool,
}

pub fn class_construction_route(
    class_symbol: SymbolId,
    instance_symbol: Option<SymbolId>,
    kind: ClassConstructionKind,
    activation: Option<ObjectActivationDescriptor>,
) -> ClassConstructionRoute {
    ClassConstructionRoute {
        class_symbol,
        instance_symbol,
        kind,
        activation,
    }
}

pub fn class_field_route(
    class_symbol: SymbolId,
    field_symbol: SymbolId,
    field_kind: ClassFieldKind,
    member_kind: ClassMemberRuntimeKind,
    event_binding: Option<ObjectEventBindingDescriptor>,
) -> ClassFieldRoute {
    ClassFieldRoute {
        class_symbol,
        field_symbol,
        field_kind,
        member_kind,
        event_binding,
    }
}

pub fn runtime_object_field_metadata(
    class_symbol: SymbolId,
    field_symbol: SymbolId,
    slot: usize,
    field_kind: ClassFieldKind,
    initialized_by_as_new: bool,
) -> RuntimeObjectFieldMetadata {
    RuntimeObjectFieldMetadata {
        class_symbol,
        field_symbol,
        slot,
        refcounted: true,
        initialized_by_as_new,
        withevents: field_kind == ClassFieldKind::WithEvents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_semantics_distinguish_new_as_new_and_predeclared_instances() {
        assert_eq!(
            class_construction_route(
                SymbolId(1),
                None,
                ClassConstructionKind::NewExpression,
                None
            )
            .kind,
            ClassConstructionKind::NewExpression
        );
        assert_eq!(
            class_construction_route(
                SymbolId(1),
                Some(SymbolId(2)),
                ClassConstructionKind::AsNewLazy,
                None
            )
            .kind,
            ClassConstructionKind::AsNewLazy
        );
        assert_eq!(
            class_construction_route(
                SymbolId(1),
                Some(SymbolId(3)),
                ClassConstructionKind::PredeclaredInstance,
                None
            )
            .kind,
            ClassConstructionKind::PredeclaredInstance
        );
    }

    #[test]
    fn class_semantics_record_ordinary_and_withevents_fields() {
        let ordinary = class_field_route(
            SymbolId(1),
            SymbolId(2),
            ClassFieldKind::Ordinary,
            ClassMemberRuntimeKind::Field,
            None,
        );
        assert_eq!(ordinary.field_kind, ClassFieldKind::Ordinary);

        let withevents = class_field_route(
            SymbolId(1),
            SymbolId(3),
            ClassFieldKind::WithEvents,
            ClassMemberRuntimeKind::Field,
            None,
        );
        assert_eq!(withevents.field_kind, ClassFieldKind::WithEvents);
    }

    #[test]
    fn class_semantics_record_runtime_object_field_metadata() {
        let ordinary = runtime_object_field_metadata(
            SymbolId(1),
            SymbolId(2),
            4,
            ClassFieldKind::Ordinary,
            false,
        );
        assert!(ordinary.refcounted);
        assert!(!ordinary.withevents);

        let withevents = runtime_object_field_metadata(
            SymbolId(1),
            SymbolId(3),
            5,
            ClassFieldKind::WithEvents,
            true,
        );
        assert!(withevents.refcounted);
        assert!(withevents.withevents);
        assert!(withevents.initialized_by_as_new);
    }
}
