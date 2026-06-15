#![allow(dead_code)]

use crate::ComMemberToken;
use crate::runtime_state::{ComEventPath, ComEventSpec, ComMemberSpec};
use crate::typelib::{
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};

#[cfg(any(test, feature = "fixture-typelibs"))]
const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
#[cfg(any(test, feature = "fixture-typelibs"))]
const OXVBA_TEST_DISPATCH_NO_DEFAULT_PROGID: &str = "OxVba.TestDispatchNoDefault";
#[cfg(any(test, feature = "fixture-typelibs"))]
const OXVBA_TEST_DISPATCH_AMBIGUOUS_DEFAULT_PROGID: &str = "OxVba.TestDispatchAmbiguousDefault";
#[cfg(any(test, feature = "fixture-typelibs"))]
const OXVBA_TEST_DISPATCH_DEFAULT_PUT_PROGID: &str = "OxVba.TestDispatchDefaultPut";
const OXVBA_TEST_EVENT_SERVER_PROGID: &str = "OxVba.TestEventServer";
const OXVBA_ALT_TEST_EVENT_SERVER_PROGID: &str = "OxVba.TestEventServerAlt";
const OXVBA_ALT2_TEST_EVENT_SERVER_PROGID: &str = "OxVba.TestEventServerAlt2";

const IID_OXVBA_TEST_DISPATCH_EVENTS_STR: &str = "11111112-2222-3333-4444-555555555556";
const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR: &str = "11111113-2222-3333-4444-555555555557";
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

fn normalize_importlib_token(input: &str) -> String {
    std::path::Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .map(normalize_ci_token)
        .unwrap_or_else(|| normalize_ci_token(input))
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
    let normalized_importlib = request
        .importlib_hint
        .as_deref()
        .map(normalize_importlib_token);
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
            requested_coclass: request.requested_coclass.clone(),
            importlib: "stdole2.tlb".to_string(),
            libid: Some("00020430-0000-0000-C000-000000000046".to_string()),
            major_version: 2,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:stdole2:2.0:0".to_string(),
        });
    }

    #[cfg(any(test, feature = "fixture-typelibs"))]
    {
        if normalized_importlib.as_deref().is_some_and(|value| {
            value == "oxvba_testdispatch.tlb" || value == "oxvba.testdispatch.tlb"
        }) || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "11111111-2222-3333-4444-555555555555")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: request.reference_name.clone(),
                requested_coclass: request.requested_coclass.clone(),
                importlib: "oxvba_testdispatch.tlb".to_string(),
                libid: Some("11111111-2222-3333-4444-555555555555".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:oxvba-testdispatch:1.0:0".to_string(),
            });
        }

        if normalized_importlib.as_deref().is_some_and(|value| {
            value == "oxvba_testeventserver.tlb" || value == "oxvba.testeventserver.tlb"
        }) || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "e2a30001-0001-0001-0001-000000000001")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: "OxVba_TestEventServer".to_string(),
                requested_coclass: request.requested_coclass.clone(),
                importlib: "oxvba_testeventserver.tlb".to_string(),
                libid: Some("E2A30001-0001-0001-0001-000000000001".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:oxvba-testeventserver:1.0:0".to_string(),
            });
        }

        if normalized_importlib.as_deref().is_some_and(|value| {
            value == "oxvba_testeventserveralt.tlb" || value == "oxvba.testeventserveralt.tlb"
        }) || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "e2a30001-0001-0001-0001-000000000101")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: "OxVba_TestEventServerAlt".to_string(),
                requested_coclass: request.requested_coclass.clone(),
                importlib: "oxvba_testeventserveralt.tlb".to_string(),
                libid: Some("E2A30001-0001-0001-0001-000000000101".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:oxvba-testeventserveralt:1.0:0".to_string(),
            });
        }

        if normalized_importlib.as_deref().is_some_and(|value| {
            value == "oxvba_testeventserveralt2.tlb" || value == "oxvba.testeventserveralt2.tlb"
        }) || normalized_libid
            .as_deref()
            .is_some_and(|value| value == "e2a30001-0001-0001-0001-000000000201")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: "OxVba_TestEventServerAlt2".to_string(),
                requested_coclass: request.requested_coclass.clone(),
                importlib: "oxvba_testeventserveralt2.tlb".to_string(),
                libid: Some("E2A30001-0001-0001-0001-000000000201".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:oxvba-testeventserveralt2:1.0:0".to_string(),
            });
        }
    }

    if let Some(coclass_name) = request.requested_coclass.as_deref() {
        let prog_id_name = format!("{}.{}", request.reference_name.trim(), coclass_name.trim());
        if let Ok(identity) =
            crate::windows_typelib_loader::resolve_typelib_identity_from_prog_id(&prog_id_name)
        {
            return Some(identity);
        }
    }

    // Fallback: try live registry-based typelib loading
    crate::windows_typelib_loader::resolve_typelib_identity_from_registry(request).ok()
}

#[cfg(any(test, feature = "fixture-typelibs"))]
pub fn known_typelib_identity_for_prog_id_name(
    prog_id_name: &str,
) -> Option<TypeLibResolvedIdentity> {
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_EVENT_SERVER_PROGID)
        || prog_id_name.eq_ignore_ascii_case("OxVba_TestEventServer")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba_TestEventServer".to_string(),
            requested_coclass: None,
            importlib: "oxvba_testeventserver.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000001".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserver:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case("OxVba_TestEventServer.TestEventServer") {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba_TestEventServer".to_string(),
            requested_coclass: None,
            importlib: "oxvba_testeventserver.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000001".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserver:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_ALT_TEST_EVENT_SERVER_PROGID)
        || prog_id_name.eq_ignore_ascii_case("OxVba_TestEventServerAlt")
        || prog_id_name.eq_ignore_ascii_case("OxVbaAlt.TestEventServer")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba_TestEventServerAlt".to_string(),
            requested_coclass: None,
            importlib: "oxvba_testeventserveralt.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000101".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserveralt:1.0:0".to_string(),
        });
    }

    if prog_id_name.eq_ignore_ascii_case("OxVba_TestEventServerAlt.TestEventServer") {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba_TestEventServerAlt".to_string(),
            requested_coclass: None,
            importlib: "oxvba_testeventserveralt.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000101".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserveralt:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_ALT2_TEST_EVENT_SERVER_PROGID)
        || prog_id_name.eq_ignore_ascii_case("OxVba_TestEventServerAlt2")
        || prog_id_name.eq_ignore_ascii_case("OxVbaAlt2.TestEventServer")
    {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba_TestEventServerAlt2".to_string(),
            requested_coclass: None,
            importlib: "oxvba_testeventserveralt2.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000201".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserveralt2:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case("OxVba_TestEventServerAlt2.TestEventServer") {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba_TestEventServerAlt2".to_string(),
            requested_coclass: None,
            importlib: "oxvba_testeventserveralt2.tlb".to_string(),
            libid: Some("E2A30001-0001-0001-0001-000000000201".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testeventserveralt2:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: "OxVba.TestDispatch".to_string(),
            requested_coclass: None,
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
            requested_coclass: None,
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
            requested_coclass: None,
            importlib: "oxvba_testdispatch_ambiguousdefault.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555557".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch-ambiguousdefault:1.0:0".to_string(),
        });
    }
    if prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_DEFAULT_PUT_PROGID) {
        return Some(TypeLibResolvedIdentity {
            reference_name: OXVBA_TEST_DISPATCH_DEFAULT_PUT_PROGID.to_string(),
            requested_coclass: None,
            importlib: "oxvba_testdispatch_defaultput.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555558".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch-defaultput:1.0:0".to_string(),
        });
    }
    None
}

#[cfg(not(any(test, feature = "fixture-typelibs")))]
pub fn known_typelib_identity_for_prog_id_name(
    _prog_id_name: &str,
) -> Option<TypeLibResolvedIdentity> {
    None
}

pub fn resolve_typelib_identity_for_prog_id_name(
    prog_id_name: &str,
) -> Option<TypeLibResolvedIdentity> {
    known_typelib_identity_for_prog_id_name(prog_id_name).or_else(|| {
        crate::windows_typelib_loader::resolve_typelib_identity_from_prog_id(prog_id_name).ok()
    })
}

pub fn build_typelib_metadata(identity: &TypeLibResolvedIdentity) -> TypeLibMetadataBlob {
    let mut effective_identity = identity.clone();
    let (activation_prog_id, member_name_to_token, members, events) = if identity
        .importlib
        .eq_ignore_ascii_case("oxvba_testdispatch.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_nodefault.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_ambiguousdefault.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_defaultput.tlb")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555555")
                || libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555556")
                || libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555557")
                || libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555558")
        }) {
        let mut members = vec![
            TypeLibMemberMetadata {
                name: "Count".to_string(),
                token: TEST_DISPID_COUNT,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "Exists".to_string(),
                token: TEST_DISPID_EXISTS,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "FireChanged".to_string(),
                token: TEST_DISPID_FIRE_CHANGED,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "FireChangedPair".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_PAIR,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "FireChangedSourceInterface".to_string(),
                token: TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_DISPID_PING,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "Lookup".to_string(),
                token: TEST_DISPID_LOOKUP,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SetValue".to_string(),
                token: TEST_DISPID_SET_VALUE,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SetValueRef".to_string(),
                token: TEST_DISPID_SET_VALUE_REF,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "Value".to_string(),
                token: TEST_DISPID_VALUE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SumPair".to_string(),
                token: TEST_DISPID_SUM_PAIR,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "LookupPair".to_string(),
                token: TEST_DISPID_LOOKUP_PAIR,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: vec!["lhs".to_string(), "rhs".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValue".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SetIndexedValueRef".to_string(),
                token: TEST_DISPID_SET_INDEXED_VALUE_REF,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                parameter_names: vec!["lhs".to_string(), "value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "EchoVariant".to_string(),
                token: TEST_DISPID_ECHO_VARIANT,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: true,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "RaiseException".to_string(),
                token: TEST_DISPID_RAISE_EXCEPTION,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "RaiseRichException".to_string(),
                token: TEST_DISPID_RAISE_RICH_EXCEPTION,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallInt".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedWord".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_WORD,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByte".to_string(),
                token: TEST_DISPID_RETURN_BYTE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSignedByte".to_string(),
                token: TEST_DISPID_RETURN_SIGNED_BYTE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformInt".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_INT,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformUInt".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_UINT,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnHyper".to_string(),
                token: TEST_DISPID_RETURN_HYPER,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedHyper".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_HYPER,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDouble".to_string(),
                token: TEST_DISPID_RETURN_DOUBLE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSingle".to_string(),
                token: TEST_DISPID_RETURN_SINGLE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDate".to_string(),
                token: TEST_DISPID_RETURN_DATE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnCurrency".to_string(),
                token: TEST_DISPID_RETURN_CURRENCY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDecimal".to_string(),
                token: TEST_DISPID_RETURN_DECIMAL,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnBool".to_string(),
                token: TEST_DISPID_RETURN_BOOL,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnString".to_string(),
                token: TEST_DISPID_RETURN_STRING,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnMissingMemberName".to_string(),
                token: TEST_DISPID_RETURN_MISSING_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPingMemberName".to_string(),
                token: TEST_DISPID_RETURN_PING_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLookupMemberName".to_string(),
                token: TEST_DISPID_RETURN_LOOKUP_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSumPairMemberName".to_string(),
                token: TEST_DISPID_RETURN_SUM_PAIR_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLookupPairMemberName".to_string(),
                token: TEST_DISPID_RETURN_LOOKUP_PAIR_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetValueMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_VALUE_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetValueRefMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_VALUE_REF_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetIndexedValueMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_INDEXED_VALUE_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSetIndexedValueRefMemberName".to_string(),
                token: TEST_DISPID_RETURN_SET_INDEXED_VALUE_REF_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnValueMemberName".to_string(),
                token: TEST_DISPID_RETURN_VALUE_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDefaultMemberName".to_string(),
                token: TEST_DISPID_RETURN_DEFAULT_MEMBER_NAME,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnEmpty".to_string(),
                token: TEST_DISPID_RETURN_EMPTY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnNull".to_string(),
                token: TEST_DISPID_RETURN_NULL,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnError".to_string(),
                token: TEST_DISPID_RETURN_ERROR,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByRefLong".to_string(),
                token: TEST_DISPID_RETURN_BYREF_LONG,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByRefLongArray".to_string(),
                token: TEST_DISPID_RETURN_BYREF_LONG_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideHyper".to_string(),
                token: TEST_DISPID_RETURN_WIDE_HYPER,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideHyperArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_HYPER_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedHyper".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedHyperArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnVariantMatrix".to_string(),
                token: TEST_DISPID_RETURN_VARIANT_MATRIX,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "NewEnum".to_string(),
                token: -4,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknownVariantArray".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN_VARIANT_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLong".to_string(),
                token: TEST_DISPID_RETURN_LONG,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedLong".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_LONG,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallIntArray".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnBoolArray".to_string(),
                token: TEST_DISPID_RETURN_BOOL_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnStringArray".to_string(),
                token: TEST_DISPID_RETURN_STRING_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSmallIntMatrix".to_string(),
                token: TEST_DISPID_RETURN_SMALLINT_MATRIX,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknown".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlainUnknownArray".to_string(),
                token: TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnByteArray".to_string(),
                token: TEST_DISPID_RETURN_BYTE_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSignedByteArray".to_string(),
                token: TEST_DISPID_RETURN_SIGNED_BYTE_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformIntArray".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_INT_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnPlatformUIntArray".to_string(),
                token: TEST_DISPID_RETURN_PLATFORM_UINT_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnHyperArray".to_string(),
                token: TEST_DISPID_RETURN_HYPER_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedHyperArray".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_HYPER_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDoubleArray".to_string(),
                token: TEST_DISPID_RETURN_DOUBLE_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSingleArray".to_string(),
                token: TEST_DISPID_RETURN_SINGLE_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDateArray".to_string(),
                token: TEST_DISPID_RETURN_DATE_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnCurrencyArray".to_string(),
                token: TEST_DISPID_RETURN_CURRENCY_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnDecimalArray".to_string(),
                token: TEST_DISPID_RETURN_DECIMAL_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedLong".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWideUnsignedLongArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWidePlatformUInt".to_string(),
                token: TEST_DISPID_RETURN_WIDE_PLATFORM_UINT,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnWidePlatformUIntArray".to_string(),
                token: TEST_DISPID_RETURN_WIDE_PLATFORM_UINT_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnLongArray".to_string(),
                token: TEST_DISPID_RETURN_LONG_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnUnsignedLongArray".to_string(),
                token: TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfDispatch".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SelfDispatch".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfUnknown".to_string(),
                token: TEST_DISPID_RETURN_SELF_UNKNOWN,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "SelfUnknown".to_string(),
                token: TEST_DISPID_RETURN_SELF_UNKNOWN,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ClassifyVariantArg".to_string(),
                token: TEST_DISPID_CLASSIFY_VARIANT_ARG,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ClassifyVariantArrayFirstElementArg".to_string(),
                token: TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfDispatchArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfTypedDispatchArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "ReturnSelfTypedUnknownArray".to_string(),
                token: TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
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
        } else if identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_defaultput.tlb")
        {
            for member in &mut members {
                member.is_default_member = member.name.eq_ignore_ascii_case("SetIndexedValue")
                    || member.name.eq_ignore_ascii_case("SetIndexedValueRef");
            }
        }
        let member_name_to_token = members
            .iter()
            .map(|entry| (entry.name.clone(), entry.token))
            .collect();
        let activation_prog_id = if identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_nodefault.tlb")
            || identity.libid.as_deref().is_some_and(|libid: &str| {
                libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555556")
            }) {
            "OxVba.TestDispatchNoDefault"
        } else if identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch_ambiguousdefault.tlb")
            || identity.libid.as_deref().is_some_and(|libid: &str| {
                libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555557")
            })
        {
            "OxVba.TestDispatchAmbiguousDefault"
        } else {
            "OxVba.TestDispatch"
        };
        (
            Some(activation_prog_id.to_string()),
            member_name_to_token,
            members,
            events,
        )
    } else if identity
        .importlib
        .eq_ignore_ascii_case("oxvba_testeventserver.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testeventserveralt.tlb")
        || identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testeventserveralt2.tlb")
        || identity.libid.as_deref().is_some_and(|libid: &str| {
            libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000001")
                || libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000101")
                || libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000201")
        })
    {
        let members = vec![
            TypeLibMemberMetadata {
                name: "FireSimpleEvent".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_SIMPLE,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "FireValueChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_VALUE_CHANGED,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "FirePairChanged".to_string(),
                token: TEST_EVENT_SERVER_DISPID_FIRE_PAIR_CHANGED,
                vtable_slot: None,
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["value".to_string()],
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
            TypeLibMemberMetadata {
                name: "Ping".to_string(),
                token: TEST_EVENT_SERVER_DISPID_PING,
                vtable_slot: None,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: Vec::new(),
                parameter_optional: Vec::new(),
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: None,
                callconv_is_stdcall: false,
                is_dual: false,
                interface_iid: None,
                source_typekind: None,
                vtable_slot_bound: None,
            },
        ];
        let events = vec![
            TypeLibEventMetadata {
                name: "OnSimpleEvent".to_string(),
                token: TEST_EVENT_SERVER_EVENT_SIMPLE,
                callback_arity: 0,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_EVENT_SERVER_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_SERVER_EVENT_SIMPLE),
            },
            TypeLibEventMetadata {
                name: "OnValueChanged".to_string(),
                token: TEST_EVENT_SERVER_EVENT_VALUE_CHANGED,
                callback_arity: 1,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_OXVBA_TEST_EVENT_SERVER_EVENTS_STR.to_string()),
                dispatch_member_id: Some(TEST_EVENT_SERVER_EVENT_VALUE_CHANGED),
            },
            TypeLibEventMetadata {
                name: "OnPairChanged".to_string(),
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
        (
            Some(
                if identity
                    .importlib
                    .eq_ignore_ascii_case("oxvba_testeventserveralt.tlb")
                    || identity.libid.as_deref().is_some_and(|libid: &str| {
                        libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000101")
                    })
                {
                    OXVBA_ALT_TEST_EVENT_SERVER_PROGID
                } else if identity
                    .importlib
                    .eq_ignore_ascii_case("oxvba_testeventserveralt2.tlb")
                    || identity.libid.as_deref().is_some_and(|libid: &str| {
                        libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000201")
                    })
                {
                    OXVBA_ALT2_TEST_EVENT_SERVER_PROGID
                } else {
                    OXVBA_TEST_EVENT_SERVER_PROGID
                }
                .to_string(),
            ),
            member_name_to_token,
            members,
            events,
        )
    } else {
        // Fallback: try live typelib loading from registry
        match try_live_typelib_metadata(identity) {
            Some(blob) => return blob,
            None => (None, Vec::new(), Vec::new(), Vec::new()),
        }
    };

    TypeLibMetadataBlob {
        identity: {
            if effective_identity.requested_coclass.is_none()
                && (effective_identity
                    .importlib
                    .eq_ignore_ascii_case("oxvba_testeventserver.tlb")
                    || effective_identity
                        .importlib
                        .eq_ignore_ascii_case("oxvba_testeventserveralt.tlb")
                    || effective_identity
                        .importlib
                        .eq_ignore_ascii_case("oxvba_testeventserveralt2.tlb")
                    || effective_identity
                        .libid
                        .as_deref()
                        .is_some_and(|libid: &str| {
                            libid.eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000001")
                                || libid
                                    .eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000101")
                                || libid
                                    .eq_ignore_ascii_case("E2A30001-0001-0001-0001-000000000201")
                        }))
            {
                effective_identity.requested_coclass = Some("TestEventServer".to_string());
            }
            effective_identity
        },
        activation_prog_id,
        member_name_to_token,
        members,
        events,
        coclass_names: Vec::new(),
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
        // SAFETY: `ptlib` is the live ITypeLib* reference handed to this caller by
        // load_typelib_from_registry above; build_metadata_blob_from_typelib only
        // borrows it, and this is its single owning Release on this path.
        unsafe { windows_typelib_loader::release_typelib(ptlib) };
        return blob;
    }

    // Try loading by path
    let ptlib = windows_typelib_loader::load_typelib_from_path(&identity.importlib).ok()?;
    let blob =
        windows_typelib_loader::build_metadata_blob_from_typelib(ptlib, identity.clone()).ok();
    // SAFETY: `ptlib` is the live ITypeLib* reference handed to this caller by
    // load_typelib_from_path above; build_metadata_blob_from_typelib only borrows
    // it, and this is its single owning Release on this path.
    unsafe { windows_typelib_loader::release_typelib(ptlib) };
    blob
}

#[cfg(not(target_os = "windows"))]
fn try_live_typelib_metadata(_identity: &TypeLibResolvedIdentity) -> Option<TypeLibMetadataBlob> {
    None
}

pub fn activation_prog_id_from_typelib_metadata(blob: &TypeLibMetadataBlob) -> Option<&str> {
    blob.activation_prog_id.as_deref()
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
        vtable_slot: member.vtable_slot,
        parameter_types: member.parameter_types.clone(),
        parameter_iids: member.parameter_iids.clone(),
        parameter_optional_defaults: Vec::new(),
        return_type: member.return_type,
        callconv_is_stdcall: member.callconv_is_stdcall,
        interface_iid: member.interface_iid,
        is_dual: member.is_dual,
        source_typekind: member.source_typekind,
        vtable_slot_bound: member.vtable_slot_bound,
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
        TypeLibMemberLookupResult, activation_prog_id_from_typelib_metadata,
        build_typelib_metadata, event_spec_from_typelib_metadata,
        known_typelib_identity_for_prog_id_name, member_spec_from_typelib_metadata,
        resolve_default_member_token_and_spec_from_typelib_metadata,
        resolve_known_typelib_identity, resolve_member_token_and_spec_from_typelib_metadata_name,
        resolve_typelib_identity_for_prog_id_name, source_interface_event_spec_supported,
    };
    use crate::{
        ComMemberToken, TEST_DISPID_EXISTS, TypeLibMemberInvokeKind, TypeLibResolveRequest,
    };

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
    fn activation_prog_id_is_catalog_driven() {
        let dispatch_identity =
            known_typelib_identity_for_prog_id_name("OxVba.TestDispatch").expect("identity");
        let dispatch_blob = build_typelib_metadata(&dispatch_identity);
        assert_eq!(
            activation_prog_id_from_typelib_metadata(&dispatch_blob),
            Some("OxVba.TestDispatch")
        );

        if let Some(excel_identity) = resolve_typelib_identity_for_prog_id_name("Excel.Application")
        {
            let excel_blob = build_typelib_metadata(&excel_identity);
            assert_eq!(
                activation_prog_id_from_typelib_metadata(&excel_blob),
                Some("Excel.Application")
            );
        }

        let alt_identity =
            known_typelib_identity_for_prog_id_name("OxVba.TestEventServerAlt").expect("identity");
        let alt_blob = build_typelib_metadata(&alt_identity);
        assert_eq!(
            activation_prog_id_from_typelib_metadata(&alt_blob),
            Some("OxVba.TestEventServerAlt")
        );

        let alt2_identity =
            known_typelib_identity_for_prog_id_name("OxVba.TestEventServerAlt2").expect("identity");
        let alt2_blob = build_typelib_metadata(&alt2_identity);
        assert_eq!(
            activation_prog_id_from_typelib_metadata(&alt2_blob),
            Some("OxVba.TestEventServerAlt2")
        );
    }

    #[test]
    fn resolve_known_typelib_identity_accepts_absolute_importlib_paths() {
        let request = TypeLibResolveRequest {
            reference_name: "OxVbaMissingBase".to_string(),
            requested_coclass: None,
            importlib_hint: Some(
                "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb".to_string(),
            ),
            libid_hint: None,
            major_version_hint: Some(1),
            minor_version_hint: Some(0),
            lcid_hint: Some(0),
        };
        let identity = resolve_known_typelib_identity(&request).expect("identity");
        assert_eq!(identity.reference_name, "OxVba_TestEventServer");
        assert_eq!(identity.importlib, "oxvba_testeventserver.tlb");
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
    fn default_put_fixture_marks_indexed_put_members_as_defaults() {
        let identity = known_typelib_identity_for_prog_id_name("OxVba.TestDispatchDefaultPut")
            .expect("identity");
        let blob = build_typelib_metadata(&identity);
        let put = member_spec_from_typelib_metadata(
            &blob,
            ComMemberToken::new(super::TEST_DISPID_SET_INDEXED_VALUE),
        )
        .expect("default property put spec");
        assert_eq!(put.invoke_kind, TypeLibMemberInvokeKind::PropertyPut);
        assert!(put.is_default_member);

        let putref = member_spec_from_typelib_metadata(
            &blob,
            ComMemberToken::new(super::TEST_DISPID_SET_INDEXED_VALUE_REF),
        )
        .expect("default property putref spec");
        assert_eq!(putref.invoke_kind, TypeLibMemberInvokeKind::PropertyPutRef);
        assert!(putref.is_default_member);
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

    #[test]
    fn catalog_stays_fixture_scoped_for_external_typelibs() {
        assert!(known_typelib_identity_for_prog_id_name("Scripting.FileSystemObject").is_none());
        assert!(known_typelib_identity_for_prog_id_name("Excel.Application").is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn scripting_filesystemobject_live_typelib_lookup_resolves_member_subset() {
        let identity = resolve_known_typelib_identity(&TypeLibResolveRequest {
            reference_name: "Scripting".to_string(),
            requested_coclass: Some("FileSystemObject".to_string()),
            importlib_hint: Some("scrrun.dll".to_string()),
            libid_hint: Some("420B2830-E718-11CF-893D-00A0C9054228".to_string()),
            major_version_hint: Some(1),
            minor_version_hint: Some(0),
            lcid_hint: Some(0),
        })
        .expect("identity");
        let blob = build_typelib_metadata(&identity);
        if blob.members.is_empty() {
            return;
        }

        assert_eq!(
            activation_prog_id_from_typelib_metadata(&blob),
            Some("Scripting.FileSystemObject")
        );
        match resolve_member_token_and_spec_from_typelib_metadata_name(&blob, "GetExtensionName") {
            TypeLibMemberLookupResult::Resolved(_, spec) => {
                assert_eq!(spec.invoke_kind, TypeLibMemberInvokeKind::Method);
                assert_eq!(spec.parameter_names.len(), 1);
            }
            other => panic!(
                "expected Scripting.FileSystemObject.GetExtensionName to resolve, got {:?}",
                other
            ),
        }
    }
}
