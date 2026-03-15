use crate::ComMemberToken;
use crate::runtime_state::{ComEventPath, ComEventSpec, ComMemberSpec};
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
const TEST_DISPID_ECHO_VARIANT: i32 = 16;
const TEST_DISPID_RAISE_EXCEPTION: i32 = 17;
const TEST_DISPID_RETURN_SMALLINT: i32 = 18;
const TEST_DISPID_RETURN_UNSIGNED_WORD: i32 = 19;
const TEST_DISPID_RETURN_SMALLINT_ARRAY: i32 = 20;
const TEST_DISPID_RETURN_BOOL_ARRAY: i32 = 21;
const TEST_DISPID_RETURN_STRING_ARRAY: i32 = 22;
const TEST_DISPID_RETURN_SELF_DISPATCH: i32 = 23;
const TEST_DISPID_RETURN_SELF_UNKNOWN: i32 = 24;
const TEST_DISPID_CLASSIFY_VARIANT_ARG: i32 = 25;
const TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG: i32 = 26;
const TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY: i32 = 27;
const TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY: i32 = 28;
const TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY: i32 = 29;
const TEST_DISPID_RETURN_SMALLINT_MATRIX: i32 = 30;
const TEST_DISPID_RETURN_PLAIN_UNKNOWN: i32 = 31;
const TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY: i32 = 32;
const TEST_DISPID_RETURN_LONG_ARRAY: i32 = 33;
const TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY: i32 = 34;

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
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "FireChanged".to_string(),
                token: TEST_DISPID_FIRE_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "FireChangedPair".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "FireChangedSourceInterface".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_DISPID_PING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "Lookup".to_string(),
                token: TEST_DISPID_LOOKUP,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "SetValue".to_string(),
                token: TEST_DISPID_SET_VALUE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "SetValueRef".to_string(),
                token: TEST_DISPID_SET_VALUE_REF,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "Value".to_string(),
                token: TEST_DISPID_VALUE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "SumPair".to_string(),
                token: TEST_DISPID_SUM_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "LookupPair".to_string(),
                token: TEST_DISPID_LOOKUP_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValue".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValueRef".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE_REF,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "EchoVariant".to_string(),
                token: TEST_DISPID_ECHO_VARIANT,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: true,
            },
            TypeLibMemberMetadata {
                name: "RaiseException".to_string(),
                token: TEST_DISPID_RAISE_EXCEPTION,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallInt".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedWord".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_WORD,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallIntArray".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnBoolArray".to_string(),
                token: TEST_DISPID_RETURN_BOOL_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnStringArray".to_string(),
                token: TEST_DISPID_RETURN_STRING_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallIntMatrix".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT_MATRIX,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknown".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknownArray".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnLongArray".to_string(),
                token: TEST_DISPID_RETURN_LONG_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedLongArray".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfDispatch".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfUnknown".to_string(),
                token: TEST_DISPID_RETURN_SELF_UNKNOWN,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ClassifyVariantArg".to_string(),
                token: TEST_DISPID_CLASSIFY_VARIANT_ARG,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ClassifyVariantArrayFirstElementArg".to_string(),
                token: TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfDispatchArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfTypedDispatchArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfTypedUnknownArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
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
            is_default_member: false,
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
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "FireValueChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_VALUE_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "FirePairChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_PAIR_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_EVENT_SERVER_DISPID_PING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
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
                is_default_member: false,
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
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
pub fn member_spec_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
    member: ComMemberToken,
) -> Option<ComMemberSpec> {
    blob.members
        .iter()
        .find(|candidate| candidate.token == member.raw())
        .map(map_member_metadata_to_spec)
}

pub fn event_spec_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
    event: ComMemberToken,
) -> Option<ComEventSpec> {
    blob.events
        .iter()
        .find(|candidate| candidate.token == event.raw())
        .map(map_event_metadata_to_spec)
}

pub fn source_interface_event_spec_supported(spec: &ComEventSpec) -> bool {
    matches!(spec.path, ComEventPath::SourceInterface)
        && spec.callback_arity == 1
        && spec
            .connection_point_iid
            .as_deref()
            .is_some_and(|iid| iid.eq_ignore_ascii_case(IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR))
}

fn map_member_metadata_to_spec(member: &TypeLibMemberMetadata) -> ComMemberSpec {
    ComMemberSpec {
        name: member.name.clone(),
        requires_argument: member.requires_argument,
        invoke_kind: member.invoke_kind,
        parameter_names: member.parameter_names.clone(),
        is_default_member: member.is_default_member,
    }
}

fn map_event_metadata_to_spec(event: &TypeLibEventMetadata) -> ComEventSpec {
    ComEventSpec {
        callback_arity: usize::from(event.callback_arity),
        path: match event.dispatch_path {
            TypeLibEventDispatchPath::Dispatch => ComEventPath::Dispatch,
            TypeLibEventDispatchPath::SourceInterface => ComEventPath::SourceInterface,
        },
        connection_point_iid: event.connection_point_iid.clone(),
        dispatch_member_id: event.dispatch_member_id,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_typelib_metadata, event_spec_from_typelib_metadata,
        known_typelib_identity_for_prog_id_name, member_spec_from_typelib_metadata,
        source_interface_event_spec_supported,
    };
    use crate::{ComMemberToken, TEST_DISPID_EXISTS, TypeLibMemberInvokeKind};

    #[test]
    fn member_spec_lookup_uses_catalog_metadata() {
        let identity = known_typelib_identity_for_prog_id_name("OxVba.TestDispatch").unwrap();
        let blob = build_typelib_metadata(&identity);
        let spec =
            member_spec_from_typelib_metadata(&blob, ComMemberToken::new(TEST_DISPID_EXISTS))
                .expect("member spec");
        assert_eq!(spec.name, "Exists");
        assert_eq!(spec.invoke_kind, TypeLibMemberInvokeKind::Method);
        assert_eq!(spec.parameter_names, vec!["value".to_string()]);
    }

    #[test]
    fn supported_source_interface_event_is_catalog_driven() {
        let identity = known_typelib_identity_for_prog_id_name("OxVba.TestDispatch").unwrap();
        let blob = build_typelib_metadata(&identity);
        let spec = event_spec_from_typelib_metadata(
            &blob,
            ComMemberToken::new(super::TEST_EVENT_CHANGED_SOURCE_INTERFACE),
        )
        .expect("event spec");
        assert!(source_interface_event_spec_supported(&spec));
    }
}
