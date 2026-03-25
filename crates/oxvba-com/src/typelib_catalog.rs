use crate::ComMemberToken;
use crate::runtime_state::{ComEventPath, ComEventSpec, ComMemberSpec};
use crate::typelib::{
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};

const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
const OXVBA_TEST_DISPATCH_NO_DEFAULT_PROGID: &str = "OxVba.TestDispatchNoDefault";
const OXVBA_TEST_DISPATCH_AMBIGUOUS_DEFAULT_PROGID: &str = "OxVba.TestDispatchAmbiguousDefault";
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
// Real Excel.Application DISPIDs from the Excel type library.
// Real Scripting.Dictionary DISPIDs from the Scripting type library.
const DICT_DISPID_ITEM: i32 = 0; // default member
const DICT_DISPID_ADD: i32 = 0x60020001u32 as i32;
const DICT_DISPID_REMOVE: i32 = 0x60020003u32 as i32;
const DICT_DISPID_REMOVEALL: i32 = 0x60020005u32 as i32;
const DICT_DISPID_KEYS: i32 = 0x60020007u32 as i32;
const DICT_DISPID_ITEMS: i32 = 0x60020008u32 as i32;
// Real Excel.Application DISPIDs from the Excel type library.
const EXCEL_DISPID_VISIBLE: i32 = 558;
const EXCEL_DISPID_WORKBOOKS: i32 = 572;
const EXCEL_DISPID_SCREEN_UPDATING: i32 = 382;
const EXCEL_DISPID_DISPLAY_ALERTS: i32 = 343;
const TEST_DISPID_SUM_PAIR: i32 = 12;
const TEST_DISPID_LOOKUP_PAIR: i32 = 13;
const TEST_DISPID_SET_INDEXED_VALUE: i32 = 14;
const TEST_DISPID_SET_INDEXED_VALUE_REF: i32 = 15;
const TEST_DISPID_ECHO_VARIANT: i32 = 16;
const TEST_DISPID_RAISE_EXCEPTION: i32 = 17;
const TEST_DISPID_RAISE_RICH_EXCEPTION: i32 = 88;
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
const TEST_DISPID_RETURN_LONG: i32 = 35;
const TEST_DISPID_RETURN_UNSIGNED_LONG: i32 = 36;
const TEST_DISPID_RETURN_BYTE: i32 = 37;
const TEST_DISPID_RETURN_BYTE_ARRAY: i32 = 38;
const TEST_DISPID_RETURN_SIGNED_BYTE: i32 = 39;
const TEST_DISPID_RETURN_SIGNED_BYTE_ARRAY: i32 = 40;
const TEST_DISPID_RETURN_PLATFORM_INT: i32 = 41;
const TEST_DISPID_RETURN_PLATFORM_UINT: i32 = 42;
const TEST_DISPID_RETURN_PLATFORM_INT_ARRAY: i32 = 43;
const TEST_DISPID_RETURN_PLATFORM_UINT_ARRAY: i32 = 44;
const TEST_DISPID_RETURN_HYPER: i32 = 45;
const TEST_DISPID_RETURN_UNSIGNED_HYPER: i32 = 46;
const TEST_DISPID_RETURN_HYPER_ARRAY: i32 = 47;
const TEST_DISPID_RETURN_UNSIGNED_HYPER_ARRAY: i32 = 48;
const TEST_DISPID_RETURN_DOUBLE: i32 = 49;
const TEST_DISPID_RETURN_DOUBLE_ARRAY: i32 = 50;
const TEST_DISPID_RETURN_SINGLE: i32 = 51;
const TEST_DISPID_RETURN_SINGLE_ARRAY: i32 = 52;
const TEST_DISPID_RETURN_DATE: i32 = 53;
const TEST_DISPID_RETURN_DATE_ARRAY: i32 = 54;
const TEST_DISPID_RETURN_CURRENCY: i32 = 55;
const TEST_DISPID_RETURN_CURRENCY_ARRAY: i32 = 56;
const TEST_DISPID_RETURN_DECIMAL: i32 = 57;
const TEST_DISPID_RETURN_DECIMAL_ARRAY: i32 = 58;
const TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG: i32 = 59;
const TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG_ARRAY: i32 = 60;
const TEST_DISPID_RETURN_WIDE_PLATFORM_UINT: i32 = 61;
const TEST_DISPID_RETURN_WIDE_PLATFORM_UINT_ARRAY: i32 = 62;
const TEST_DISPID_RETURN_BOOL: i32 = 63;
const TEST_DISPID_RETURN_STRING: i32 = 64;
const TEST_DISPID_RETURN_EMPTY: i32 = 65;
const TEST_DISPID_RETURN_NULL: i32 = 66;
const TEST_DISPID_RETURN_ERROR: i32 = 67;
const TEST_DISPID_RETURN_BYREF_LONG: i32 = 68;
const TEST_DISPID_RETURN_BYREF_LONG_ARRAY: i32 = 69;
const TEST_DISPID_RETURN_WIDE_HYPER: i32 = 70;
const TEST_DISPID_RETURN_WIDE_HYPER_ARRAY: i32 = 71;
const TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER: i32 = 72;
const TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER_ARRAY: i32 = 73;
const TEST_DISPID_RETURN_VARIANT_MATRIX: i32 = 74;
const TEST_DISPID_RETURN_PLAIN_UNKNOWN_VARIANT_ARRAY: i32 = 75;
const TEST_DISPID_RETURN_MISSING_MEMBER_NAME: i32 = 76;
const TEST_DISPID_RETURN_PING_MEMBER_NAME: i32 = 77;
const TEST_DISPID_RETURN_LOOKUP_MEMBER_NAME: i32 = 78;
const TEST_DISPID_RETURN_SUM_PAIR_MEMBER_NAME: i32 = 79;
const TEST_DISPID_RETURN_LOOKUP_PAIR_MEMBER_NAME: i32 = 80;
const TEST_DISPID_RETURN_SET_VALUE_MEMBER_NAME: i32 = 81;
const TEST_DISPID_RETURN_SET_VALUE_REF_MEMBER_NAME: i32 = 82;
const TEST_DISPID_RETURN_SET_INDEXED_VALUE_MEMBER_NAME: i32 = 83;
const TEST_DISPID_RETURN_SET_INDEXED_VALUE_REF_MEMBER_NAME: i32 = 84;
const TEST_DISPID_RETURN_VALUE_MEMBER_NAME: i32 = 85;
const TEST_DISPID_RETURN_DEFAULT_MEMBER_NAME: i32 = 86;

const TEST_EVENT_CHANGED: i32 = 1;
const TEST_EVENT_CHANGED_SOURCE_INTERFACE: i32 = 2;
const TEST_EVENT_CHANGED_PAIR: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLibMemberLookupResult {
    Resolved(ComMemberToken, ComMemberSpec),
    Missing,
    Ambiguous,
}
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

    // Fallback: try live registry-based typelib loading
    crate::windows_typelib_loader::resolve_typelib_identity_from_registry(request).ok()
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
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_NO_DEFAULT_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: OXVBA_TEST_DISPATCH_NO_DEFAULT_PROGID.to_string(),
            importlib: "oxvba_testdispatch_nodefault.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555556".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch-nodefault:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_AMBIGUOUS_DEFAULT_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: OXVBA_TEST_DISPATCH_AMBIGUOUS_DEFAULT_PROGID.to_string(),
            importlib: "oxvba_testdispatch_ambiguousdefault.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555557".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch-ambiguousdefault:1.0:0".to_string(),
        });
    }
    None
}

pub fn build_typelib_metadata(identity: &TypeLibResolvedIdentity) -> TypeLibMetadataBlob {
    let (create_object_selector, member_name_to_token, members, events) = if identity
        .importlib
        .eq_ignore_ascii_case("oxvba_testdispatch.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_nodefault.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_ambiguousdefault.tlb")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555555")
                || libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555556")
                || libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555557")
        }) {
        let mut members = vec![
            TypeLibMemberMetadata {
                name: "Count".to_string(),
                token: TEST_DISPID_COUNT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "FireChanged".to_string(),
                token: TEST_DISPID_FIRE_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "FireChangedPair".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "FireChangedSourceInterface".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_DISPID_PING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Lookup".to_string(),
                token: TEST_DISPID_LOOKUP,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SetValue".to_string(),
                token: TEST_DISPID_SET_VALUE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SetValueRef".to_string(),
                token: TEST_DISPID_SET_VALUE_REF,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Value".to_string(),
                token: TEST_DISPID_VALUE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SumPair".to_string(),
                token: TEST_DISPID_SUM_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "LookupPair".to_string(),
                token: TEST_DISPID_LOOKUP_PAIR,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValue".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValueRef".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE_REF,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "EchoVariant".to_string(),
                token: TEST_DISPID_ECHO_VARIANT,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: true,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "RaiseException".to_string(),
                token: TEST_DISPID_RAISE_EXCEPTION,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "RaiseRichException".to_string(),
                token: TEST_DISPID_RAISE_RICH_EXCEPTION,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallInt".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedWord".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_WORD,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByte".to_string(),
                token: TEST_DISPID_RETURN_BYTE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSignedByte".to_string(),
                token: TEST_DISPID_RETURN_SIGNED_BYTE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformInt".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_INT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformUInt".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_UINT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnHyper".to_string(),
                token: TEST_DISPID_RETURN_HYPER,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedHyper".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_HYPER,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDouble".to_string(),
                token: TEST_DISPID_RETURN_DOUBLE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSingle".to_string(),
                token: TEST_DISPID_RETURN_SINGLE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDate".to_string(),
                token: TEST_DISPID_RETURN_DATE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnCurrency".to_string(),
                token: TEST_DISPID_RETURN_CURRENCY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDecimal".to_string(),
                token: TEST_DISPID_RETURN_DECIMAL,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnBool".to_string(),
                token: TEST_DISPID_RETURN_BOOL,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnString".to_string(),
                token: TEST_DISPID_RETURN_STRING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnMissingMemberName".to_string(),
                token: TEST_DISPID_RETURN_MISSING_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPingMemberName".to_string(),
                token: TEST_DISPID_RETURN_PING_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLookupMemberName".to_string(),
                token: TEST_DISPID_RETURN_LOOKUP_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSumPairMemberName".to_string(),
                token: TEST_DISPID_RETURN_SUM_PAIR_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLookupPairMemberName".to_string(),
                token: TEST_DISPID_RETURN_LOOKUP_PAIR_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetValueMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_VALUE_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetValueRefMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_VALUE_REF_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetIndexedValueMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_INDEXED_VALUE_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetIndexedValueRefMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_INDEXED_VALUE_REF_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnValueMemberName".to_string(),
                token: TEST_DISPID_RETURN_VALUE_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDefaultMemberName".to_string(),
                token: TEST_DISPID_RETURN_DEFAULT_MEMBER_NAME,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnEmpty".to_string(),
                token: TEST_DISPID_RETURN_EMPTY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnNull".to_string(),
                token: TEST_DISPID_RETURN_NULL,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnError".to_string(),
                token: TEST_DISPID_RETURN_ERROR,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByRefLong".to_string(),
                token: TEST_DISPID_RETURN_BYREF_LONG,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByRefLongArray".to_string(),
                token: TEST_DISPID_RETURN_BYREF_LONG_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideHyper".to_string(),
                token: TEST_DISPID_RETURN_WIDE_HYPER,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideHyperArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_HYPER_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedHyper".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedHyperArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnVariantMatrix".to_string(),
                token: TEST_DISPID_RETURN_VARIANT_MATRIX,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknownVariantArray".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN_VARIANT_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLong".to_string(),
                token: TEST_DISPID_RETURN_LONG,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedLong".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_LONG,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallIntArray".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnBoolArray".to_string(),
                token: TEST_DISPID_RETURN_BOOL_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnStringArray".to_string(),
                token: TEST_DISPID_RETURN_STRING_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallIntMatrix".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT_MATRIX,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknown".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknownArray".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByteArray".to_string(),
                token: TEST_DISPID_RETURN_BYTE_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSignedByteArray".to_string(),
                token: TEST_DISPID_RETURN_SIGNED_BYTE_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformIntArray".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_INT_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformUIntArray".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_UINT_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnHyperArray".to_string(),
                token: TEST_DISPID_RETURN_HYPER_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedHyperArray".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_HYPER_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDoubleArray".to_string(),
                token: TEST_DISPID_RETURN_DOUBLE_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSingleArray".to_string(),
                token: TEST_DISPID_RETURN_SINGLE_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDateArray".to_string(),
                token: TEST_DISPID_RETURN_DATE_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnCurrencyArray".to_string(),
                token: TEST_DISPID_RETURN_CURRENCY_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDecimalArray".to_string(),
                token: TEST_DISPID_RETURN_DECIMAL_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedLong".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedLongArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWidePlatformUInt".to_string(),
                token: TEST_DISPID_RETURN_WIDE_PLATFORM_UINT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWidePlatformUIntArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_PLATFORM_UINT_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLongArray".to_string(),
                token: TEST_DISPID_RETURN_LONG_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedLongArray".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfDispatch".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SelfDispatch".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfUnknown".to_string(),
                token: TEST_DISPID_RETURN_SELF_UNKNOWN,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "SelfUnknown".to_string(),
                token: TEST_DISPID_RETURN_SELF_UNKNOWN,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ClassifyVariantArg".to_string(),
                token: TEST_DISPID_CLASSIFY_VARIANT_ARG,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ClassifyVariantArrayFirstElementArg".to_string(),
                token: TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfDispatchArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfTypedDispatchArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfTypedUnknownArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
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
        if identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_nodefault.tlb")
        {
            for member in &mut members {
                member.is_default_member = false;
            }
        } else if identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_ambiguousdefault.tlb")
        {
            for member in &mut members {
                member.is_default_member = member.name.eq_ignore_ascii_case("EchoVariant")
                    || member.name.eq_ignore_ascii_case("Value");
            }
        }
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        (Some(4), member_name_to_token, members, events)
    } else if identity.importlib.eq_ignore_ascii_case("excel.exe")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("00020813-0000-0000-C000-000000000046")
        })
    {
        let members = vec![
            TypeLibMemberMetadata {
                name: "Quit".to_string(),
                token: TEST_DISPID_EXCEL_QUIT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Visible".to_string(),
                token: EXCEL_DISPID_VISIBLE,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Visible".to_string(),
                token: EXCEL_DISPID_VISIBLE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["RHS".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Workbooks".to_string(),
                token: EXCEL_DISPID_WORKBOOKS,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ScreenUpdating".to_string(),
                token: EXCEL_DISPID_SCREEN_UPDATING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "ScreenUpdating".to_string(),
                token: EXCEL_DISPID_SCREEN_UPDATING,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["RHS".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "DisplayAlerts".to_string(),
                token: EXCEL_DISPID_DISPLAY_ALERTS,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "DisplayAlerts".to_string(),
                token: EXCEL_DISPID_DISPLAY_ALERTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["RHS".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
        ];
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
        (None, member_name_to_token, members, events)
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
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "FireValueChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_VALUE_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "FirePairChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_PAIR_CHANGED,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_EVENT_SERVER_DISPID_PING,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
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
        (None, member_name_to_token, members, events)
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
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["Key".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Item".to_string(),
                token: DICT_DISPID_ITEM,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["Key".to_string()],
                is_default_member: true,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Item".to_string(),
                token: DICT_DISPID_ITEM,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["Key".to_string(), "pRetItem".to_string()],
                is_default_member: true,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Add".to_string(),
                token: DICT_DISPID_ADD,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["Key".to_string(), "Item".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Remove".to_string(),
                token: DICT_DISPID_REMOVE,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["Key".to_string()],
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "RemoveAll".to_string(),
                token: DICT_DISPID_REMOVEALL,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Keys".to_string(),
                token: DICT_DISPID_KEYS,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
            TypeLibMemberMetadata {
                name: "Items".to_string(),
                token: DICT_DISPID_ITEMS,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                return_type: None,
            },
        ];
        let events = Vec::new();
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        (None, member_name_to_token, members, events)
    } else {
        // Fallback: try live typelib loading from registry
        match try_live_typelib_metadata(identity) {
            Some(blob) => return blob,
            None => (None, Vec::new(), Vec::new(), Vec::new()),
        }
    };

    TypeLibMetadataBlob {
        identity: identity.clone(),
        create_object_selector,
        member_name_to_token,
        members,
        events,
    }
}

#[cfg(target_os = "windows")]
fn try_live_typelib_metadata(identity: &TypeLibResolvedIdentity) -> Option<TypeLibMetadataBlob> {
    use crate::windows_typelib_loader;

    // Try loading by LIBID first
    if let Some(ref libid_str) = identity.libid {
        let guid = crate::windows_client::parse_guid_canonical(libid_str)?;
        let ptlib = windows_typelib_loader::load_typelib_from_registry(
            &guid,
            identity.major_version,
            identity.minor_version,
            identity.lcid.unwrap_or(0),
        )
        .ok()?;
        let blob =
            windows_typelib_loader::build_metadata_blob_from_typelib(ptlib, identity.clone()).ok();
        unsafe { windows_typelib_loader::release_typelib(ptlib) };
        return blob;
    }

    // Try loading by path
    let ptlib = windows_typelib_loader::load_typelib_from_path(&identity.importlib).ok()?;
    let blob =
        windows_typelib_loader::build_metadata_blob_from_typelib(ptlib, identity.clone()).ok();
    unsafe { windows_typelib_loader::release_typelib(ptlib) };
    blob
}

#[cfg(not(target_os = "windows"))]
fn try_live_typelib_metadata(_identity: &TypeLibResolvedIdentity) -> Option<TypeLibMetadataBlob> {
    None
}

pub fn create_object_selector_from_typelib_metadata(blob: &TypeLibMetadataBlob) -> Option<i32> {
    blob.create_object_selector
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

pub fn member_token_and_spec_from_typelib_metadata_name(
    blob: &TypeLibMetadataBlob,
    member_name: &str,
) -> Option<(ComMemberToken, ComMemberSpec)> {
    match resolve_member_token_and_spec_from_typelib_metadata_name(blob, member_name) {
        TypeLibMemberLookupResult::Resolved(token, spec) => Some((token, spec)),
        TypeLibMemberLookupResult::Missing | TypeLibMemberLookupResult::Ambiguous => None,
    }
}

pub fn resolve_member_token_and_spec_from_typelib_metadata_name(
    blob: &TypeLibMetadataBlob,
    member_name: &str,
) -> TypeLibMemberLookupResult {
    let mut matches = blob
        .members
        .iter()
        .filter(|candidate| candidate.name.eq_ignore_ascii_case(member_name));
    let Some(member) = matches.next() else {
        return TypeLibMemberLookupResult::Missing;
    };
    if matches.next().is_some() {
        return TypeLibMemberLookupResult::Ambiguous;
    }
    TypeLibMemberLookupResult::Resolved(
        ComMemberToken::new(member.token),
        map_member_metadata_to_spec(member),
    )
}

pub fn resolve_default_member_token_and_spec_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> TypeLibMemberLookupResult {
    let mut matches = blob
        .members
        .iter()
        .filter(|member| member.is_default_member);
    let Some(member) = matches.next() else {
        return TypeLibMemberLookupResult::Missing;
    };
    if matches.next().is_some() {
        return TypeLibMemberLookupResult::Ambiguous;
    }
    TypeLibMemberLookupResult::Resolved(
        ComMemberToken::new(member.token),
        map_member_metadata_to_spec(member),
    )
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
        TypeLibMemberLookupResult, build_typelib_metadata,
        create_object_selector_from_typelib_metadata, event_spec_from_typelib_metadata,
        known_typelib_identity_for_prog_id_name, member_spec_from_typelib_metadata,
        resolve_default_member_token_and_spec_from_typelib_metadata,
        resolve_member_token_and_spec_from_typelib_metadata_name,
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

    #[test]
    fn create_object_selector_is_catalog_driven() {
        let dispatch_identity =
            known_typelib_identity_for_prog_id_name("OxVba.TestDispatch").expect("identity");
        let dispatch_blob = build_typelib_metadata(&dispatch_identity);
        assert_eq!(
            create_object_selector_from_typelib_metadata(&dispatch_blob),
            Some(4)
        );

        let excel_identity =
            known_typelib_identity_for_prog_id_name("Excel.Application").expect("identity");
        let excel_blob = build_typelib_metadata(&excel_identity);
        assert_eq!(
            create_object_selector_from_typelib_metadata(&excel_blob),
            None
        );
    }

    #[test]
    fn default_member_lookup_reports_missing_when_catalog_has_no_default() {
        let identity = known_typelib_identity_for_prog_id_name("OxVba.TestDispatchNoDefault")
            .expect("identity");
        let blob = build_typelib_metadata(&identity);
        assert_eq!(
            resolve_default_member_token_and_spec_from_typelib_metadata(&blob),
            TypeLibMemberLookupResult::Missing
        );
    }

    #[test]
    fn default_member_lookup_reports_ambiguous_when_catalog_has_multiple_defaults() {
        let identity =
            known_typelib_identity_for_prog_id_name("OxVba.TestDispatchAmbiguousDefault")
                .expect("identity");
        let blob = build_typelib_metadata(&identity);
        assert_eq!(
            resolve_default_member_token_and_spec_from_typelib_metadata(&blob),
            TypeLibMemberLookupResult::Ambiguous
        );
    }

    #[test]
    fn named_member_lookup_reports_missing_for_unknown_imported_member() {
        let identity = known_typelib_identity_for_prog_id_name("OxVba.TestDispatch").unwrap();
        let blob = build_typelib_metadata(&identity);
        assert_eq!(
            resolve_member_token_and_spec_from_typelib_metadata_name(&blob, "UnknownMember"),
            TypeLibMemberLookupResult::Missing
        );
    }
}
