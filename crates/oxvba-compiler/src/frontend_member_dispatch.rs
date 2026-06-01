use crate::frontend_project_symbols::{ProjectSymbolKind, ProjectSymbolRoute};
use crate::frontend_symbols::SymbolId;
use oxvba_com::TypeLibMemberInvokeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberDispatchClass {
    EarlyBoundProject {
        target: SymbolId,
        kind: ProjectSymbolKind,
    },
    ImportedCom {
        type_name: String,
        member_name: String,
        dispatch_id: Option<i32>,
        invoke_kind: TypeLibMemberInvokeKind,
    },
    LateBoundDispatch {
        receiver: SymbolId,
        member_name: String,
    },
    DefaultMember {
        receiver: SymbolId,
        default_member: SymbolId,
    },
    HostGlobal {
        host_name: String,
        member_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDispatchDecision {
    pub class: MemberDispatchClass,
    pub evidence: String,
}

pub fn classify_project_member(route: ProjectSymbolRoute) -> MemberDispatchDecision {
    MemberDispatchDecision {
        class: MemberDispatchClass::EarlyBoundProject {
            target: route.symbol,
            kind: route.kind,
        },
        evidence: "binder-owned project symbol route".to_string(),
    }
}

pub fn classify_imported_com_member(
    type_name: impl Into<String>,
    member_name: impl Into<String>,
    dispatch_id: Option<i32>,
    invoke_kind: TypeLibMemberInvokeKind,
) -> MemberDispatchDecision {
    MemberDispatchDecision {
        class: MemberDispatchClass::ImportedCom {
            type_name: type_name.into(),
            member_name: member_name.into(),
            dispatch_id,
            invoke_kind,
        },
        evidence: "imported COM descriptor route".to_string(),
    }
}

pub fn classify_late_bound_dispatch(
    receiver: SymbolId,
    member_name: impl Into<String>,
) -> MemberDispatchDecision {
    MemberDispatchDecision {
        class: MemberDispatchClass::LateBoundDispatch {
            receiver,
            member_name: member_name.into(),
        },
        evidence: "receiver has no early-bound member route".to_string(),
    }
}

pub fn classify_default_member(
    receiver: SymbolId,
    default_member: SymbolId,
) -> MemberDispatchDecision {
    MemberDispatchDecision {
        class: MemberDispatchClass::DefaultMember {
            receiver,
            default_member,
        },
        evidence: "default member metadata route".to_string(),
    }
}

pub fn classify_host_global(
    host_name: impl Into<String>,
    member_name: impl Into<String>,
) -> MemberDispatchDecision {
    MemberDispatchDecision {
        class: MemberDispatchClass::HostGlobal {
            host_name: host_name.into(),
            member_name: member_name.into(),
        },
        evidence: "host-provided global route".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_dispatch_classifies_early_bound_project_member() {
        let decision = classify_project_member(ProjectSymbolRoute {
            symbol: SymbolId(7),
            kind: ProjectSymbolKind::Procedure,
        });
        assert!(matches!(
            decision.class,
            MemberDispatchClass::EarlyBoundProject {
                target: SymbolId(7),
                kind: ProjectSymbolKind::Procedure
            }
        ));
    }

    #[test]
    fn member_dispatch_classifies_imported_com_and_late_bound_routes() {
        let imported = classify_imported_com_member(
            "Excel.Range",
            "Value",
            Some(6),
            TypeLibMemberInvokeKind::PropertyPut,
        );
        assert!(matches!(
            imported.class,
            MemberDispatchClass::ImportedCom {
                dispatch_id: Some(6),
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                ..
            }
        ));

        let late = classify_late_bound_dispatch(SymbolId(3), "Value");
        assert!(matches!(
            late.class,
            MemberDispatchClass::LateBoundDispatch {
                receiver: SymbolId(3),
                ..
            }
        ));
    }

    #[test]
    fn member_dispatch_classifies_default_member_and_host_global_routes() {
        let default_member = classify_default_member(SymbolId(1), SymbolId(2));
        assert!(matches!(
            default_member.class,
            MemberDispatchClass::DefaultMember {
                receiver: SymbolId(1),
                default_member: SymbolId(2)
            }
        ));

        let host = classify_host_global("Application", "WorksheetFunction");
        assert!(matches!(
            host.class,
            MemberDispatchClass::HostGlobal {
                ref host_name,
                ref member_name
            } if host_name == "Application" && member_name == "WorksheetFunction"
        ));
    }
}
