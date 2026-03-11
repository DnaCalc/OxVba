use crate::typelib::{
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};

const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
const EXCEL_APPLICATION_PROGID: &str = "Excel.Application";
const OXVBA_TEST_EVENT_SERVER_PROGID: &str = "OxVba.TestEventServer";

const IID_OXVBA_TEST_DISPATCH_EVENTS_STR: &str = "11111112-2222-3333-4444-555555555556";
const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR: &str = "11111113-2222-3333-4444-555555555557";
const IID_EXCEL_APPLICATION_EVENTS_STR: &str = "00024413-0000-0000-C000-000000000046";
const IID_OXVBA_TEST_EVENT_SERVER_EVENTS_STR: &str = "E2A30001-0001-0001-0001-000000000002";

const TEST_DISPID_COUNT: i32 = 1;
const TEST_DISPID_EXISTS: i32 = 2;
const TEST_DISPID_FIRE_CHANGED: i32 = 3;
const TEST_DISPID_FIRE_CHANGED_PAIR: i32 = 4;
const TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE: i32 = 11;
const TEST_DISPID_PING: i32 = 5;
const TEST_DISPID_LOOKUP: i32 = 6;
const TEST_DISPID_SET_VALUE: i32 = 7;
const TEST_DISPID_SET_VALUE_REF: i32 = 8;
const TEST_DISPID_VALUE: i32 = 9;
const TEST_DISPID_EXCEL_QUIT: i32 = 10;
const TEST_DISPID_SUM_PAIR: i32 = 12;
const TEST_DISPID_LOOKUP_PAIR: i32 = 13;
const TEST_DISPID_SET_INDEXED_VALUE: i32 = 14;
const TEST_DISPID_SET_INDEXED_VALUE_REF: i32 = 15;

const TEST_EVENT_CHANGED: i32 = 1;
const TEST_EVENT_CHANGED_SOURCE_INTERFACE: i32 = 2;
const TEST_EVENT_CHANGED_PAIR: i32 = 3;
const TEST_EVENT_EXCEL_APP_QUIT: i32 = 10;

const TEST_EVENT_SERVER_DISPID_FIRE_SIMPLE: i32 = 101;
const TEST_EVENT_SERVER_DISPID_FIRE_VALUE_CHANGED: i32 = 102;
const TEST_EVENT_SERVER_DISPID_FIRE_PAIR_CHANGED: i32 = 103;
const TEST_EVENT_SERVER_DISPID_PING: i32 = 104;
const TEST_EVENT_SERVER_EVENT_SIMPLE: i32 = 1;
const TEST_EVENT_SERVER_EVENT_VALUE_CHANGED: i32 = 2;
const TEST_EVENT_SERVER_EVENT_PAIR_CHANGED: i32 = 3;

fn normalize_ci_token(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn normalize_guid_like(input: &str) -> String {
    input
        .trim()
        .trim_matches('{')
        .trim_matches('}')
        .to_ascii_lowercase()
}

pub fn resolve_known_typelib_identity(
    request: &TypeLibResolveRequest,
) -> Option<TypeLibResolvedIdentity> {
    let normalized_importlib = request.importlib_hint.as_deref().map(normalize_ci_token);
    let normalized_libid = request.libid_hint.as_deref().map(normalize_guid_like);

    if normalized_importlib
        .as_deref()
        .is_some_and(|value| value == "stdole2.tlb")
        || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "00020430-0000-0000-c000-000000000046")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: request.reference_name.clone(),
            importlib: "stdole2.tlb".to_string(),
            libid: Some("00020430-0000-0000-C000-000000000046".to_string()),
            major_version: 2,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:stdole2:2.0:0".to_string(),
        });
    }

    if normalized_importlib
        .as_deref()
        .is_some_and(|value| value == "oxvba_testdispatch.tlb")
        || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "11111111-2222-3333-4444-555555555555")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: request.reference_name.clone(),
            importlib: "oxvba_testdispatch.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch:1.0:0".to_string(),
        });
    }

    if normalized_importlib
        .as_deref()
        .is_some_and(|value| value == "excel.exe")
        || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "00020813-0000-0000-c000-000000000046")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: request.reference_name.clone(),
            importlib: "excel.exe".to_string(),
            libid: Some("00020813-0000-0000-C000-000000000046".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:excel.application:1.0:0".to_string(),
        });
    }

    if normalized_importlib
        .as_deref()
        .is_some_and(|value| value == "oxvba_testeventserver.tlb")
        || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "e2a30001-0001-0001-0001-000000000001")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: request.reference_name.clone(),
            importlib: "oxvba_testeventserver.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000001".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserver:1.0:0".to_string(),
        });
    }

    if normalized_importlib
        .as_deref()
        .is_some_and(|value| value == "scrrun.dll")
        || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "420b2830-e718-11cf-893d-00a0c9054228")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: request.reference_name.clone(),
            importlib: "scrrun.dll".to_string(),
            libid: Some("420B2830-E718-11CF-893D-00A0C9054228".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:scripting.dictionary:1.0:0".to_string(),
        });
    }

    None
}

pub fn known_typelib_identity_for_prog_id_name(
    prog_id_name: &str,
) -> Option<TypeLibResolvedIdentity> {
    if prog_id_name.eq_ignore_ascii_case("Scripting.Dictionary") {
        return Some(TypeLibResolvedIdentity {
            reference_name: "Scripting.Dictionary".to_string(),
            importlib: "scrrun.dll".to_string(),
            libid: Some("420B2830-E718-11CF-893D-00A0C9054228".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:scripting.dictionary:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(EXCEL_APPLICATION_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: EXCEL_APPLICATION_PROGID.to_string(),
            importlib: "excel.exe".to_string(),
            libid: Some("00020813-0000-0000-C000-000000000046".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:excel.application:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_EVENT_SERVER_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba.TestEventServer".to_string(),
            importlib: "oxvba_testeventserver.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000001".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserver:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba.TestDispatch".to_string(),
            importlib: "oxvba_testdispatch.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch:1.0:0".to_string(),
        });
    }
    None
}

pub fn build_typelib_metadata(identity: &TypeLibResolvedIdentity) -> TypeLibMetadataBlob {
    let (member_name_to_token, members, events) = if identity
        .importlib
        .eq_ignore_ascii_case("oxvba_testdispatch.tlb")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555555")
        }) {
        let members = vec![
            TypeLibMemberMetadata {
                name: "Count".to_string(),
                token: TEST_DISPID_COUNT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "FireChanged".to_string(),
                token: TEST_DISPID_FIRE_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "FireChangedPair".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "FireChangedSourceInterface".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_DISPID_PING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
            },
            TypeLibMemberMetadata {
                name: "Lookup".to_string(),
                token: TEST_DISPID_LOOKUP,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "SetValue".to_string(),
                token: TEST_DISPID_SET_VALUE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "SetValueRef".to_string(),
                token: TEST_DISPID_SET_VALUE_REF,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "Value".to_string(),
                token: TEST_DISPID_VALUE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
            },
            TypeLibMemberMetadata {
                name: "SumPair".to_string(),
                token: TEST_DISPID_SUM_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
            },
            TypeLibMemberMetadata {
                name: "LookupPair".to_string(),
                token: TEST_DISPID_LOOKUP_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValue".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValueRef".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE_REF,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "EchoVariant".to_string(),
                token: 16,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "RaiseException".to_string(),
                token: 17,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
            },
        ];
        let events = vec![
            TypeLibEventMetadata {
                name: "Changed".to_string(),
                token: TEST_EVENT_CHANGED,
                callback_arity: 1,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_DISPATCH_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_CHANGED),
            },
            TypeLibEventMetadata {
                name: "ChangedSourceInterface".to_string(),
                token: TEST_EVENT_CHANGED_SOURCE_INTERFACE,
                callback_arity: 1,
                dispatch_path: TypeLibEventDispatchPath::SourceInterface,
                connection_point_iid: Some(IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR.to_string()),
                dispatch_member_id: None,
            },
            TypeLibEventMetadata {
                name: "ChangedPair".to_string(),
                token: TEST_EVENT_CHANGED_PAIR,
                callback_arity: 2,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_DISPATCH_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_CHANGED_PAIR),
            },
        ];
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        (member_name_to_token, members, events)
    } else if identity.importlib.eq_ignore_ascii_case("excel.exe")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("00020813-0000-0000-C000-000000000046")
        })
    {
        let members = vec![TypeLibMemberMetadata {
            name: "Quit".to_string(),
            token: TEST_DISPID_EXCEL_QUIT,
            requires_argument: false,
            invoke_kind: TypeLibMemberInvokeKind::Method,
            parameter_names: Vec::new(),
        }];
        let events = vec![TypeLibEventMetadata {
            name: "Quit".to_string(),
            token: TEST_EVENT_EXCEL_APP_QUIT,
            callback_arity: 0,
            dispatch_path: TypeLibEventDispatchPath::Dispatch,
            connection_point_iid: Some(IID_EXCEL_APPLICATION_EVENTS_STR.to_string()),
            dispatch_member_id: None,
        }];
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        (member_name_to_token, members, events)
    } else if identity
        .importlib
        .eq_ignore_ascii_case("oxvba_testeventserver.tlb")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000001")
        })
    {
        let members = vec![
            TypeLibMemberMetadata {
                name: "FireSimpleEvent".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_SIMPLE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
            },
            TypeLibMemberMetadata {
                name: "FireValueChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_VALUE_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "FirePairChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_PAIR_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_EVENT_SERVER_DISPID_PING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
            },
        ];
        let events = vec![
            TypeLibEventMetadata {
                name: "SimpleEvent".to_string(),
                token: TEST_EVENT_SERVER_EVENT_SIMPLE,
                callback_arity: 0,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_EVENT_SERVER_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_SERVER_EVENT_SIMPLE),
            },
            TypeLibEventMetadata {
                name: "ValueChanged".to_string(),
                token: TEST_EVENT_SERVER_EVENT_VALUE_CHANGED,
                callback_arity: 1,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_EVENT_SERVER_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_SERVER_EVENT_VALUE_CHANGED),
            },
            TypeLibEventMetadata {
                name: "PairChanged".to_string(),
                token: TEST_EVENT_SERVER_EVENT_PAIR_CHANGED,
                callback_arity: 2,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_EVENT_SERVER_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_SERVER_EVENT_PAIR_CHANGED),
            },
        ];
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        (member_name_to_token, members, events)
    } else if identity.importlib.eq_ignore_ascii_case("scrrun.dll")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("420B2830-E718-11CF-893D-00A0C9054228")
        })
    {
        let members = vec![
            TypeLibMemberMetadata {
                name: "Count".to_string(),
                token: TEST_DISPID_COUNT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
            },
        ];
        let events = vec![TypeLibEventMetadata {
            name: "Exists".to_string(),
            token: TEST_EVENT_CHANGED,
            callback_arity: 1,
            dispatch_path: TypeLibEventDispatchPath::Dispatch,
            connection_point_iid: None,
            dispatch_member_id: Some(TEST_EVENT_CHANGED),
        }];
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        (member_name_to_token, members, events)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };

    TypeLibMetadataBlob {
        identity: identity.clone(),
        member_name_to_token,
        members,
        events,
    }
}
