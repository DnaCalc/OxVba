#[cfg(target_os = "windows")]
fn main() -> Result<(), String> {
    use oxvba_com::typelib::TypeLibResolveRequest;
    use oxvba_com::windows_typelib_loader::{audit_typelib_shapes, release_typelib};
    use oxvba_com::{build_typelib_metadata, resolve_known_typelib_identity};

    fn request(
        reference_name: &str,
        libid: &str,
        major: u16,
        minor: u16,
        lcid: u32,
    ) -> TypeLibResolveRequest {
        TypeLibResolveRequest {
            reference_name: reference_name.to_string(),
            requested_coclass: None,
            importlib_hint: None,
            libid_hint: Some(libid.to_string()),
            major_version_hint: Some(major),
            minor_version_hint: Some(minor),
            lcid_hint: Some(lcid),
        }
    }

    let requests = vec![
        request("Excel", "{00020813-0000-0000-C000-000000000046}", 1, 9, 0),
        request("Office", "{2DF8D04C-5BFA-101B-BDE5-00AA0044DE52}", 2, 8, 0),
        request("VBA", "{000204EF-0000-0000-C000-000000000046}", 6, 0, 9),
        request("VBIDE", "{0002E157-0000-0000-C000-000000000046}", 5, 3, 0),
    ];

    println!("kind,library,field1,field2,field3,field4,field5,field6");
    for req in requests {
        let Some(identity) =
            resolve_known_typelib_identity(&req).or_else(|| fallback_identity(&req))
        else {
            println!("missing,{},identity,,,,,", req.reference_name);
            continue;
        };
        let metadata = build_typelib_metadata(&identity);
        println!(
            "metadata,{},{},{},{},{},{},{}",
            identity.reference_name,
            metadata.members.len(),
            metadata.events.len(),
            metadata.member_name_to_token.len(),
            metadata.activation_prog_id.clone().unwrap_or_default(),
            identity.importlib,
            identity.cache_key,
        );

        let ptlib = load_identity(&identity)?;
        let audit = audit_typelib_shapes(ptlib)?;
        // SAFETY: `ptlib` is the live ITypeLib* reference returned by load_identity
        // (the loader module's registry/path helpers); audit_typelib_shapes only
        // borrows it, and this is its single owning Release.
        unsafe { release_typelib(ptlib) };
        for row in audit.csv_rows(&identity.reference_name) {
            println!("{row}");
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn fallback_identity(
    req: &oxvba_com::typelib::TypeLibResolveRequest,
) -> Option<oxvba_com::typelib::TypeLibResolvedIdentity> {
    Some(oxvba_com::typelib::TypeLibResolvedIdentity {
        reference_name: req.reference_name.clone(),
        requested_coclass: req.requested_coclass.clone(),
        importlib: req.reference_name.clone(),
        libid: req.libid_hint.clone(),
        major_version: req.major_version_hint?,
        minor_version: req.minor_version_hint?,
        lcid: req.lcid_hint,
        cache_key: format!(
            "audit:{}:{}:{}",
            req.libid_hint.as_deref().unwrap_or(&req.reference_name),
            req.major_version_hint?,
            req.minor_version_hint?
        ),
    })
}

#[cfg(target_os = "windows")]
fn load_identity(
    identity: &oxvba_com::typelib::TypeLibResolvedIdentity,
) -> Result<*mut core::ffi::c_void, String> {
    use oxvba_com::windows_client::parse_guid_canonical;
    use oxvba_com::windows_typelib_loader::{load_typelib_from_path, load_typelib_from_registry};
    if let Some(libid) = identity.libid.as_deref() {
        let guid = parse_guid_canonical(libid).ok_or_else(|| format!("invalid libid {libid}"))?;
        load_typelib_from_registry(
            &guid,
            identity.major_version,
            identity.minor_version,
            identity.lcid.unwrap_or(0),
        )
    } else {
        load_typelib_from_path(&identity.importlib)
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("typelib audit requires Windows");
}
