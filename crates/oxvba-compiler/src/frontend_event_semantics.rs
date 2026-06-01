use crate::frontend_symbols::SymbolId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithEventsRoute {
    pub source_field: SymbolId,
    pub event_source_type: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaiseEventRoute {
    pub event_symbol: SymbolId,
    pub args: Vec<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandlerRoute {
    pub source_field: SymbolId,
    pub event_symbol: SymbolId,
    pub handler_symbol: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementsRoute {
    pub class_symbol: SymbolId,
    pub interface_symbol: SymbolId,
    pub members: Vec<ImplementsMemberRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementsMemberRoute {
    pub interface_member: SymbolId,
    pub implementation_member: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDiagnostic {
    pub code: String,
    pub message: String,
}

pub fn withevents_route(source_field: SymbolId, event_source_type: SymbolId) -> WithEventsRoute {
    WithEventsRoute {
        source_field,
        event_source_type,
    }
}

pub fn raise_event_route(event_symbol: SymbolId, args: Vec<SymbolId>) -> RaiseEventRoute {
    RaiseEventRoute { event_symbol, args }
}

pub fn event_handler_route(
    source_field: SymbolId,
    event_symbol: SymbolId,
    handler_symbol: SymbolId,
) -> EventHandlerRoute {
    EventHandlerRoute {
        source_field,
        event_symbol,
        handler_symbol,
    }
}

pub fn implements_route(
    class_symbol: SymbolId,
    interface_symbol: SymbolId,
    members: Vec<ImplementsMemberRoute>,
) -> ImplementsRoute {
    ImplementsRoute {
        class_symbol,
        interface_symbol,
        members,
    }
}

pub fn missing_event_handler_diagnostic(event_name: &str) -> EventDiagnostic {
    EventDiagnostic {
        code: "BIND-E-EVENT-HANDLER-NOT-FOUND".to_string(),
        message: format!("event handler for `{event_name}` was not found"),
    }
}

pub fn missing_implements_member_diagnostic(member_name: &str) -> EventDiagnostic {
    EventDiagnostic {
        code: "BIND-E-IMPLEMENTS-MEMBER-NOT-FOUND".to_string(),
        message: format!("implementation for `{member_name}` was not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_semantics_route_withevents_raiseevent_and_handlers() {
        assert_eq!(
            withevents_route(SymbolId(1), SymbolId(2)).event_source_type,
            SymbolId(2)
        );
        assert_eq!(
            raise_event_route(SymbolId(3), vec![SymbolId(4)]).args,
            vec![SymbolId(4)]
        );
        assert_eq!(
            event_handler_route(SymbolId(1), SymbolId(3), SymbolId(5)).handler_symbol,
            SymbolId(5)
        );
    }

    #[test]
    fn event_semantics_route_implements_members() {
        let route = implements_route(
            SymbolId(1),
            SymbolId(2),
            vec![ImplementsMemberRoute {
                interface_member: SymbolId(3),
                implementation_member: SymbolId(4),
            }],
        );
        assert_eq!(route.class_symbol, SymbolId(1));
        assert_eq!(route.interface_symbol, SymbolId(2));
        assert_eq!(route.members[0].implementation_member, SymbolId(4));
    }

    #[test]
    fn event_semantics_emit_stable_diagnostics() {
        assert_eq!(
            missing_event_handler_diagnostic("Click").code,
            "BIND-E-EVENT-HANDLER-NOT-FOUND"
        );
        assert_eq!(
            missing_implements_member_diagnostic("IFoo_Bar").code,
            "BIND-E-IMPLEMENTS-MEMBER-NOT-FOUND"
        );
    }
}
