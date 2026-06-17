use crate::ComMemberToken;
use crate::runtime_state::{ComEventPath, ComEventSpec, ComMemberSpec};
use crate::typelib::{
    TypeLibEventMetadata, TypeLibInterfaceMetadata, TypeLibMemberMetadata, TypeLibMetadataBlob,
    TypeLibResolveRequest, TypeLibResolvedIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLibMemberLookupResult {
    Resolved(ComMemberToken, ComMemberSpec),
    Missing,
    Ambiguous,
}

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

#[cfg(any(test, feature = "fixture-typelibs"))]
fn is_fixture_typelib_identity(identity: &TypeLibResolvedIdentity) -> bool {
    let importlib = normalize_importlib_token(&identity.importlib);
    if importlib.starts_with("oxvba_test") || importlib.starts_with("oxvba.test") {
        return true;
    }

    identity.libid.as_deref().is_some_and(|libid| {
        let libid = normalize_guid_like(libid);
        libid.starts_with("11111111-2222-3333-4444-55555555555")
            || libid.starts_with("e2a30001-0001-0001-0001-000000000")
    })
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
    if let Some(identity) = crate::fixture_typelib_catalog::resolve_known_typelib_identity(request)
        && is_fixture_typelib_identity(&identity)
    {
        return Some(identity);
    }

    if let Some(coclass_name) = request.requested_coclass.as_deref() {
        let prog_id_name = format!("{}.{}", request.reference_name.trim(), coclass_name.trim());
        if let Ok(identity) =
            crate::windows_typelib_loader::resolve_typelib_identity_from_prog_id(&prog_id_name)
        {
            return Some(identity);
        }
    }

    crate::windows_typelib_loader::resolve_typelib_identity_from_registry(request).ok()
}

#[cfg(any(test, feature = "fixture-typelibs"))]
pub fn known_typelib_identity_for_prog_id_name(
    prog_id_name: &str,
) -> Option<TypeLibResolvedIdentity> {
    crate::fixture_typelib_catalog::known_typelib_identity_for_prog_id_name(prog_id_name)
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
    #[cfg(any(test, feature = "fixture-typelibs"))]
    if is_fixture_typelib_identity(identity) {
        return crate::fixture_typelib_catalog::build_typelib_metadata(identity);
    }

    match try_live_typelib_metadata(identity) {
        Some(blob) => blob,
        None => TypeLibMetadataBlob {
            identity: identity.clone(),
            activation_prog_id: None,
            member_name_to_token: Vec::new(),
            members: Vec::new(),
            events: Vec::new(),
            coclass_names: Vec::new(),
        },
    }
}

pub fn resolve_typelib_interface_metadata(
    request: &TypeLibResolveRequest,
    interface_name: &str,
) -> Option<TypeLibInterfaceMetadata> {
    let identity = resolve_known_typelib_identity(request)?;
    resolve_typelib_interface_metadata_from_identity(&identity, interface_name)
}

pub fn resolve_typelib_interface_metadata_from_identity(
    identity: &TypeLibResolvedIdentity,
    interface_name: &str,
) -> Option<TypeLibInterfaceMetadata> {
    #[cfg(target_os = "windows")]
    {
        resolve_live_typelib_interface_metadata(identity, interface_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (identity, interface_name);
        None
    }
}

#[cfg(target_os = "windows")]
fn resolve_live_typelib_interface_metadata(
    identity: &TypeLibResolvedIdentity,
    interface_name: &str,
) -> Option<TypeLibInterfaceMetadata> {
    use crate::windows_typelib_loader;

    let ptlib = if let Some(ref libid_str) = identity.libid {
        let guid = crate::windows_client::parse_guid_canonical(libid_str)?;
        windows_typelib_loader::load_typelib_from_registry(
            &guid,
            identity.major_version,
            identity.minor_version,
            identity.lcid.unwrap_or(0),
        )
        .ok()?
    } else {
        windows_typelib_loader::load_typelib_from_path(&identity.importlib).ok()?
    };

    let members =
        windows_typelib_loader::enumerate_typelib_members_for_interface(ptlib, interface_name).ok();
    // SAFETY: `ptlib` is the live ITypeLib* reference obtained from this function's
    // load call above. The enumeration only borrows it, and this is the single
    // owning Release for the successful load path.
    unsafe { windows_typelib_loader::release_typelib(ptlib) };

    let members = members?;
    if members.is_empty() {
        return None;
    }
    let iid = members.iter().find_map(|member| member.interface_iid);
    Some(TypeLibInterfaceMetadata {
        name: interface_name.to_string(),
        iid,
        members,
    })
}

#[cfg(target_os = "windows")]
fn try_live_typelib_metadata(identity: &TypeLibResolvedIdentity) -> Option<TypeLibMetadataBlob> {
    use crate::windows_typelib_loader;

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
        && spec.connection_point_iid.is_some()
}

pub(crate) fn map_member_metadata_to_spec(member: &TypeLibMemberMetadata) -> ComMemberSpec {
    ComMemberSpec {
        name: member.name.clone(),
        requires_argument: member.requires_argument,
        invoke_kind: member.invoke_kind,
        parameter_names: member.parameter_names.clone(),
        is_default_member: member.is_default_member,
        vtable_slot: member.vtable_slot,
        parameter_types: member.parameter_types.clone(),
        parameter_iids: member.parameter_iids.clone(),
        parameter_optional_defaults: member.parameter_optional_defaults.clone(),
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
            crate::typelib::TypeLibEventDispatchPath::Dispatch => ComEventPath::Dispatch,
            crate::typelib::TypeLibEventDispatchPath::SourceInterface => {
                ComEventPath::SourceInterface
            }
        },
        connection_point_iid: event.connection_point_iid.clone(),
        dispatch_member_id: event.dispatch_member_id,
    }
}
