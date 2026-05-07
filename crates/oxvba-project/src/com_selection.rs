//! Typed COM reference discovery and active-selection models.
//!
//! This module is the canonical OxVba-side shape for COM reference selection.
//! Discovery backends may produce advisory candidates from the registry,
//! ProgID lookup, or explicit file browse flows, but durable project truth
//! remains the serialized `.basproj` `COMReference` list.

use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use oxvba_com::windows_typelib_loader::{
    discover_registered_typelib_identities_by_name, resolve_typelib_identity_from_registry,
};
use oxvba_com::{
    TypeLibResolveRequest, TypeLibResolvedIdentity, activation_prog_id_from_typelib_metadata,
    build_typelib_metadata, resolve_known_typelib_identity,
    resolve_typelib_identity_for_prog_id_name,
};
use oxvba_host::{
    DirectHostCapability, DirectHostCapabilityKind, DirectHostCommandStatus, DirectHostIssue,
    DirectHostIssueKind, DirectHostRetryability, TypeLibraryCatalogEntry,
};
use thiserror::Error;

use crate::{
    BasProjComReference, BasProjError, HostProjectEdit, HostProjectReferenceKind,
    HostWorkspaceTargetKind, inspect_workspace_target,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComSelectionIdentity {
    pub library_name: String,
    pub guid: Option<String>,
    pub version_major: Option<u16>,
    pub version_minor: Option<u16>,
    pub lcid: Option<u32>,
    pub import_lib: Option<String>,
    pub carrier_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComSelectionSourceKind {
    RegisteredLibrary,
    ProgIdLookup,
    FileBrowse,
    ProjectActiveReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComSelectionCarrierKind {
    TypeLibrary,
    DynamicLibrary,
    ActiveXControl,
    Executable,
    Xll,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComSelectionConfidence {
    Exact,
    Strong,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComSelectionCandidate {
    pub identity: ComSelectionIdentity,
    pub friendly_description: Option<String>,
    pub prog_ids: Vec<String>,
    pub source_kind: ComSelectionSourceKind,
    pub carrier_kind: ComSelectionCarrierKind,
    pub confidence: ComSelectionConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredComSelectionQuery {
    pub reference_name: String,
    pub requested_coclass: Option<String>,
    pub import_lib: Option<String>,
    pub guid: Option<String>,
    pub version_major: Option<u16>,
    pub version_minor: Option<u16>,
    pub lcid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBackedComSelectionQuery {
    pub carrier_path: PathBuf,
    pub reference_name: Option<String>,
    pub requested_coclass: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ComSelectionDiscoveryError {
    #[error("COM selection query requires a non-empty reference name")]
    EmptyReferenceName,
    #[error("COM selection query requires a non-empty ProgID")]
    EmptyProgId,
    #[error("COM selection query requires a non-empty carrier path")]
    EmptyCarrierPath,
    #[error("registered COM discovery failed for `{query}`: {message}")]
    RegisteredDiscoveryFailed { query: String, message: String },
    #[error("ProgID lookup failed for `{prog_id}`: {message}")]
    ProgIdLookupFailed { prog_id: String, message: String },
    #[error("file-backed COM discovery failed for `{path}`: {message}")]
    FileDiscoveryFailed { path: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComProjectSelection {
    pub reference: BasProjComReference,
    pub status: ComProjectSelectionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComProjectSelectionStatus {
    ResolvedUnique {
        candidate: ComSelectionCandidate,
    },
    Ambiguous {
        candidates: Vec<ComSelectionCandidate>,
    },
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComProjectEditPlanKind {
    Add,
    Replace,
    Repair,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComProjectEditPlan {
    pub kind: ComProjectEditPlanKind,
    pub include: String,
    pub resulting_reference: Option<BasProjComReference>,
    pub candidate: Option<ComSelectionCandidate>,
    pub edits: Vec<HostProjectEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostComProjectSelectionSurface {
    pub workspace_kind: HostWorkspaceTargetKind,
    pub workspace_target: PathBuf,
    pub project_file: Option<PathBuf>,
    pub project_name: String,
    pub active_references: Vec<BasProjComReference>,
    pub selections: Vec<ComProjectSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComHostPlatform {
    Windows,
    NonWindows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComRuntimeInvocationAvailability {
    pub platform: ComHostPlatform,
    pub command_status: DirectHostCommandStatus,
    pub requires_windows: bool,
    pub required_apartment: Option<String>,
    pub bitness_requirement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComCapabilityProfile {
    pub platform: ComHostPlatform,
    pub reference_discovery: DirectHostCapability,
    pub reference_editing: DirectHostCapability,
    pub runtime_invocation: DirectHostCapability,
    pub native_service: DirectHostCapability,
    pub runtime_availability: ComRuntimeInvocationAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComReferenceReorderIssueKind {
    DuplicateReference,
    MissingReference,
    OmittedReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComReferenceReorderIssue {
    pub kind: ComReferenceReorderIssueKind,
    pub include: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComReferenceReorderPlan {
    pub active_references: Vec<BasProjComReference>,
    pub requested_order: Vec<String>,
    pub resulting_references: Vec<BasProjComReference>,
    pub edits: Vec<HostProjectEdit>,
    pub can_apply: bool,
    pub issues: Vec<ComReferenceReorderIssue>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ComSelectionService;

pub fn candidate_from_catalog_entry(
    entry: &TypeLibraryCatalogEntry,
    source_kind: ComSelectionSourceKind,
) -> ComSelectionCandidate {
    let import_lib = non_empty(entry.importlib.as_str()).map(str::to_string);
    let carrier_path = import_lib.as_ref().map(PathBuf::from);
    let carrier_kind = carrier_kind_from_path(carrier_path.as_deref());

    ComSelectionCandidate {
        identity: ComSelectionIdentity {
            library_name: entry.library_name.clone(),
            guid: entry.libid.clone(),
            version_major: Some(entry.major_version),
            version_minor: Some(entry.minor_version),
            lcid: entry.lcid,
            import_lib,
            carrier_path,
        },
        friendly_description: Some(entry.library_name.clone()),
        prog_ids: Vec::new(),
        source_kind,
        carrier_kind,
        confidence: ComSelectionConfidence::Strong,
    }
}

pub fn candidate_from_project_reference(reference: &BasProjComReference) -> ComSelectionCandidate {
    let import_lib = reference
        .import_lib
        .clone()
        .filter(|value| !value.trim().is_empty());
    let carrier_path = import_lib.as_ref().map(PathBuf::from);
    let carrier_kind = carrier_kind_from_path(carrier_path.as_deref());

    ComSelectionCandidate {
        identity: ComSelectionIdentity {
            library_name: reference.include.clone(),
            guid: reference.guid.clone(),
            version_major: reference.version_major,
            version_minor: reference.version_minor,
            lcid: reference.lcid,
            import_lib,
            carrier_path,
        },
        friendly_description: Some(reference.include.clone()),
        prog_ids: Vec::new(),
        source_kind: ComSelectionSourceKind::ProjectActiveReference,
        carrier_kind,
        confidence: ComSelectionConfidence::Strong,
    }
}

pub fn candidate_from_resolved_identity(
    identity: &TypeLibResolvedIdentity,
    source_kind: ComSelectionSourceKind,
    prog_ids: Vec<String>,
) -> ComSelectionCandidate {
    let carrier_path = Some(PathBuf::from(identity.importlib.as_str()));
    let carrier_kind = carrier_kind_from_path(carrier_path.as_deref());

    ComSelectionCandidate {
        identity: ComSelectionIdentity {
            library_name: identity.reference_name.clone(),
            guid: identity.libid.clone(),
            version_major: Some(identity.major_version),
            version_minor: Some(identity.minor_version),
            lcid: identity.lcid,
            import_lib: Some(identity.importlib.clone()),
            carrier_path,
        },
        friendly_description: Some(identity.reference_name.clone()),
        prog_ids,
        source_kind,
        carrier_kind,
        confidence: ComSelectionConfidence::Strong,
    }
}

pub fn discover_registered_com_candidates(
    query: &RegisteredComSelectionQuery,
) -> Result<Vec<ComSelectionCandidate>, ComSelectionDiscoveryError> {
    let reference_name = query.reference_name.trim();
    if reference_name.is_empty() {
        return Err(ComSelectionDiscoveryError::EmptyReferenceName);
    }

    let request = TypeLibResolveRequest {
        reference_name: reference_name.to_string(),
        requested_coclass: query
            .requested_coclass
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        importlib_hint: query
            .import_lib
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        libid_hint: query
            .guid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        major_version_hint: query.version_major,
        minor_version_hint: query.version_minor,
        lcid_hint: query.lcid,
    };

    if let Some(identity) = resolve_known_typelib_identity(&request) {
        return Ok(vec![candidate_from_resolved_identity(
            &identity,
            ComSelectionSourceKind::RegisteredLibrary,
            Vec::new(),
        )]);
    }

    #[cfg(target_os = "windows")]
    {
        let identities =
            discover_registered_typelib_identities_by_name(reference_name).map_err(|message| {
                ComSelectionDiscoveryError::RegisteredDiscoveryFailed {
                    query: reference_name.to_string(),
                    message,
                }
            })?;
        let candidates = identities
            .into_iter()
            .filter(|identity| registered_identity_matches_query(identity, query))
            .map(|identity| {
                candidate_from_resolved_identity(
                    &identity,
                    ComSelectionSourceKind::RegisteredLibrary,
                    Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        Ok(candidates)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

pub fn discover_prog_id_com_candidates(
    prog_id_name: &str,
) -> Result<Vec<ComSelectionCandidate>, ComSelectionDiscoveryError> {
    let prog_id_name = prog_id_name.trim();
    if prog_id_name.is_empty() {
        return Err(ComSelectionDiscoveryError::EmptyProgId);
    }
    let Some(identity) = resolve_typelib_identity_for_prog_id_name(prog_id_name) else {
        return Err(ComSelectionDiscoveryError::ProgIdLookupFailed {
            prog_id: prog_id_name.to_string(),
            message: "no registered or known typelib identity resolved".to_string(),
        });
    };
    Ok(vec![candidate_from_resolved_identity(
        &identity,
        ComSelectionSourceKind::ProgIdLookup,
        vec![prog_id_name.to_string()],
    )])
}

pub fn discover_file_backed_com_candidates(
    query: &FileBackedComSelectionQuery,
) -> Result<Vec<ComSelectionCandidate>, ComSelectionDiscoveryError> {
    let carrier_path = query
        .carrier_path
        .as_os_str()
        .to_string_lossy()
        .trim()
        .to_string();
    if carrier_path.is_empty() {
        return Err(ComSelectionDiscoveryError::EmptyCarrierPath);
    }
    let reference_name = query
        .reference_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            query
                .carrier_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .ok_or(ComSelectionDiscoveryError::EmptyReferenceName)?;

    let request = TypeLibResolveRequest {
        reference_name: reference_name.clone(),
        requested_coclass: query
            .requested_coclass
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        importlib_hint: Some(carrier_path.clone()),
        libid_hint: None,
        major_version_hint: None,
        minor_version_hint: None,
        lcid_hint: None,
    };

    let identity = if let Some(identity) = resolve_known_typelib_identity(&request) {
        identity
    } else {
        #[cfg(target_os = "windows")]
        {
            resolve_typelib_identity_from_registry(&request).map_err(|message| {
                ComSelectionDiscoveryError::FileDiscoveryFailed {
                    path: carrier_path.clone(),
                    message,
                }
            })?
        }

        #[cfg(not(target_os = "windows"))]
        {
            return Err(ComSelectionDiscoveryError::FileDiscoveryFailed {
                path: carrier_path.clone(),
                message: "live file-backed typelib discovery is not available on this platform"
                    .to_string(),
            });
        }
    };

    let prog_ids = activation_prog_id_from_typelib_metadata(&build_typelib_metadata(&identity))
        .map(|value| vec![value.to_string()])
        .unwrap_or_default();
    let mut candidate =
        candidate_from_resolved_identity(&identity, ComSelectionSourceKind::FileBrowse, prog_ids);
    candidate.identity.carrier_path = Some(query.carrier_path.clone());
    candidate.carrier_kind = carrier_kind_from_path(Some(&query.carrier_path));
    Ok(vec![candidate])
}

pub fn assess_project_com_selections(
    references: &[BasProjComReference],
    candidates: &[ComSelectionCandidate],
) -> Vec<ComProjectSelection> {
    references
        .iter()
        .map(|reference| ComProjectSelection {
            reference: reference.clone(),
            status: selection_status_for_reference(reference, candidates),
        })
        .collect()
}

pub fn basproj_reference_from_candidate(
    candidate: &ComSelectionCandidate,
    include_override: Option<&str>,
) -> BasProjComReference {
    let include = include_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(candidate.identity.library_name.as_str())
        .to_string();
    BasProjComReference {
        include,
        guid: candidate.identity.guid.clone(),
        version_major: candidate.identity.version_major,
        version_minor: candidate.identity.version_minor,
        lcid: candidate.identity.lcid,
        import_lib: candidate.identity.import_lib.clone(),
    }
}

pub fn plan_add_com_candidate(
    candidate: &ComSelectionCandidate,
    include_override: Option<&str>,
) -> ComProjectEditPlan {
    let reference = basproj_reference_from_candidate(candidate, include_override);
    ComProjectEditPlan {
        kind: ComProjectEditPlanKind::Add,
        include: reference.include.clone(),
        resulting_reference: Some(reference.clone()),
        candidate: Some(candidate.clone()),
        edits: vec![HostProjectEdit::AddComReference(reference)],
    }
}

pub fn plan_remove_com_reference(reference: &BasProjComReference) -> ComProjectEditPlan {
    ComProjectEditPlan {
        kind: ComProjectEditPlanKind::Remove,
        include: reference.include.clone(),
        resulting_reference: None,
        candidate: None,
        edits: vec![HostProjectEdit::RemoveComReference {
            include: reference.include.clone(),
        }],
    }
}

pub fn plan_replace_com_reference(
    reference: &BasProjComReference,
    candidate: &ComSelectionCandidate,
) -> ComProjectEditPlan {
    let replacement = basproj_reference_from_candidate(candidate, Some(&reference.include));
    ComProjectEditPlan {
        kind: ComProjectEditPlanKind::Replace,
        include: reference.include.clone(),
        resulting_reference: Some(replacement.clone()),
        candidate: Some(candidate.clone()),
        edits: vec![
            HostProjectEdit::RemoveComReference {
                include: reference.include.clone(),
            },
            HostProjectEdit::AddComReference(replacement),
        ],
    }
}

pub fn plan_repair_project_selection(
    selection: &ComProjectSelection,
    candidate: &ComSelectionCandidate,
) -> ComProjectEditPlan {
    let kind = match selection.status {
        ComProjectSelectionStatus::Missing | ComProjectSelectionStatus::Ambiguous { .. } => {
            ComProjectEditPlanKind::Repair
        }
        ComProjectSelectionStatus::ResolvedUnique { .. } => ComProjectEditPlanKind::Replace,
    };
    let replacement =
        basproj_reference_from_candidate(candidate, Some(&selection.reference.include));
    ComProjectEditPlan {
        kind,
        include: selection.reference.include.clone(),
        resulting_reference: Some(replacement.clone()),
        candidate: Some(candidate.clone()),
        edits: vec![
            HostProjectEdit::RemoveComReference {
                include: selection.reference.include.clone(),
            },
            HostProjectEdit::AddComReference(replacement),
        ],
    }
}

pub fn com_runtime_invocation_availability() -> ComRuntimeInvocationAvailability {
    let platform = current_com_host_platform();
    let command_status = if platform == ComHostPlatform::Windows {
        DirectHostCommandStatus::available()
    } else {
        DirectHostCommandStatus::disabled(non_windows_com_issue(
            DirectHostIssueKind::ComRuntimeUnavailable,
            "COM runtime invocation requires a Windows native COM host",
        ))
    };

    ComRuntimeInvocationAvailability {
        platform,
        command_status,
        requires_windows: true,
        required_apartment: Some("STA or host-managed COM apartment".to_string()),
        bitness_requirement: Some(
            "host process bitness must match the target COM server".to_string(),
        ),
    }
}

pub fn com_capability_profile() -> ComCapabilityProfile {
    let platform = current_com_host_platform();
    let reference_discovery = if platform == ComHostPlatform::Windows {
        DirectHostCapability::available(DirectHostCapabilityKind::ComReferenceDiscovery)
    } else {
        DirectHostCapability::degraded(
            DirectHostCapabilityKind::ComReferenceDiscovery,
            non_windows_com_issue(
                DirectHostIssueKind::ComDiscoveryUnavailable,
                "known/catalog COM identity matching is available, but live registry and file-backed discovery require Windows",
            ),
        )
    };
    let reference_editing =
        DirectHostCapability::available(DirectHostCapabilityKind::ProjectAuthoring);
    let runtime_availability = com_runtime_invocation_availability();
    let runtime_invocation = match &runtime_availability.command_status {
        DirectHostCommandStatus::Available => {
            DirectHostCapability::available(DirectHostCapabilityKind::ComRuntimeInvocation)
        }
        DirectHostCommandStatus::Disabled { reason } => DirectHostCapability::unavailable(
            DirectHostCapabilityKind::ComRuntimeInvocation,
            reason.clone(),
        ),
    };
    let native_service = if platform == ComHostPlatform::Windows {
        DirectHostCapability::available(DirectHostCapabilityKind::NativeService)
    } else {
        DirectHostCapability::unavailable(
            DirectHostCapabilityKind::NativeService,
            non_windows_com_issue(
                DirectHostIssueKind::NonWindowsUnsupported,
                "native COM service boundary is only available on Windows",
            ),
        )
    };

    ComCapabilityProfile {
        platform,
        reference_discovery,
        reference_editing,
        runtime_invocation,
        native_service,
        runtime_availability,
    }
}

pub fn plan_reorder_com_references(
    active_references: &[BasProjComReference],
    ordered_includes: &[String],
) -> ComReferenceReorderPlan {
    let mut issues = Vec::new();
    let mut seen = Vec::<String>::new();
    for include in ordered_includes {
        let key = normalize_com_include_key(include);
        if seen.iter().any(|existing| existing == &key) {
            issues.push(ComReferenceReorderIssue {
                kind: ComReferenceReorderIssueKind::DuplicateReference,
                include: include.clone(),
                message: format!(
                    "COM reference `{include}` appears more than once in reorder request"
                ),
            });
        } else {
            seen.push(key);
        }
    }

    for include in ordered_includes {
        if !active_references
            .iter()
            .any(|reference| reference.include.eq_ignore_ascii_case(include))
        {
            issues.push(ComReferenceReorderIssue {
                kind: ComReferenceReorderIssueKind::MissingReference,
                include: include.clone(),
                message: format!("COM reference `{include}` is not active in the project"),
            });
        }
    }

    for reference in active_references {
        if !ordered_includes
            .iter()
            .any(|include| include.eq_ignore_ascii_case(&reference.include))
        {
            issues.push(ComReferenceReorderIssue {
                kind: ComReferenceReorderIssueKind::OmittedReference,
                include: reference.include.clone(),
                message: format!(
                    "COM reference `{}` is active but missing from reorder request",
                    reference.include
                ),
            });
        }
    }

    let can_apply = issues.is_empty();
    let resulting_references = if can_apply {
        ordered_includes
            .iter()
            .filter_map(|include| {
                active_references
                    .iter()
                    .find(|reference| reference.include.eq_ignore_ascii_case(include))
                    .cloned()
            })
            .collect::<Vec<_>>()
    } else {
        active_references.to_vec()
    };

    let edits = if can_apply {
        active_references
            .iter()
            .map(|reference| HostProjectEdit::RemoveComReference {
                include: reference.include.clone(),
            })
            .chain(
                resulting_references
                    .iter()
                    .cloned()
                    .map(HostProjectEdit::AddComReference),
            )
            .collect()
    } else {
        Vec::new()
    };

    ComReferenceReorderPlan {
        active_references: active_references.to_vec(),
        requested_order: ordered_includes.to_vec(),
        resulting_references,
        edits,
        can_apply,
        issues,
    }
}

fn current_com_host_platform() -> ComHostPlatform {
    if cfg!(target_os = "windows") {
        ComHostPlatform::Windows
    } else {
        ComHostPlatform::NonWindows
    }
}

fn non_windows_com_issue(kind: DirectHostIssueKind, detail: impl Into<String>) -> DirectHostIssue {
    DirectHostIssue::new(kind)
        .with_technical_detail(detail)
        .with_retryability(DirectHostRetryability::NotRetryable)
}

impl ComSelectionService {
    pub fn capability_profile(&self) -> ComCapabilityProfile {
        com_capability_profile()
    }

    pub fn runtime_invocation_availability(&self) -> ComRuntimeInvocationAvailability {
        com_runtime_invocation_availability()
    }

    pub fn plan_reorder_references(
        &self,
        active_references: &[BasProjComReference],
        ordered_includes: &[String],
    ) -> ComReferenceReorderPlan {
        plan_reorder_com_references(active_references, ordered_includes)
    }

    pub fn discover_registered_candidates(
        &self,
        query: &RegisteredComSelectionQuery,
    ) -> Result<Vec<ComSelectionCandidate>, ComSelectionDiscoveryError> {
        discover_registered_com_candidates(query)
    }

    pub fn discover_prog_id_candidates(
        &self,
        prog_id_name: &str,
    ) -> Result<Vec<ComSelectionCandidate>, ComSelectionDiscoveryError> {
        discover_prog_id_com_candidates(prog_id_name)
    }

    pub fn discover_file_backed_candidates(
        &self,
        query: &FileBackedComSelectionQuery,
    ) -> Result<Vec<ComSelectionCandidate>, ComSelectionDiscoveryError> {
        discover_file_backed_com_candidates(query)
    }

    pub fn inspect_workspace_project_state(
        &self,
        path: &Path,
        discovered: &[ComSelectionCandidate],
    ) -> Result<HostComProjectSelectionSurface, BasProjError> {
        inspect_workspace_com_project_state(path, discovered)
    }

    pub fn plan_add_candidate(
        &self,
        candidate: &ComSelectionCandidate,
        include_override: Option<&str>,
    ) -> ComProjectEditPlan {
        plan_add_com_candidate(candidate, include_override)
    }

    pub fn plan_replace_reference(
        &self,
        reference: &BasProjComReference,
        candidate: &ComSelectionCandidate,
    ) -> ComProjectEditPlan {
        plan_replace_com_reference(reference, candidate)
    }

    pub fn plan_repair_selection(
        &self,
        selection: &ComProjectSelection,
        candidate: &ComSelectionCandidate,
    ) -> ComProjectEditPlan {
        plan_repair_project_selection(selection, candidate)
    }

    pub fn plan_remove_reference(&self, reference: &BasProjComReference) -> ComProjectEditPlan {
        plan_remove_com_reference(reference)
    }
}

pub fn inspect_workspace_com_project_state(
    path: &Path,
    discovered: &[ComSelectionCandidate],
) -> Result<HostComProjectSelectionSurface, BasProjError> {
    let surface = inspect_workspace_target(path)?;
    let active_references = surface
        .references
        .iter()
        .filter(|reference| reference.kind == HostProjectReferenceKind::Com)
        .map(|reference| BasProjComReference {
            include: reference.include.clone(),
            guid: reference.guid.clone(),
            version_major: reference.version_major,
            version_minor: reference.version_minor,
            lcid: reference.lcid,
            import_lib: reference.import_lib.clone(),
        })
        .collect::<Vec<_>>();
    let selections = assess_project_com_selections(&active_references, discovered);

    Ok(HostComProjectSelectionSurface {
        workspace_kind: surface.workspace_kind,
        workspace_target: surface.workspace_target,
        project_file: surface.project_file,
        project_name: surface.project_name,
        active_references,
        selections,
    })
}

fn normalize_com_include_key(include: &str) -> String {
    include.trim().to_ascii_lowercase()
}

fn selection_status_for_reference(
    reference: &BasProjComReference,
    candidates: &[ComSelectionCandidate],
) -> ComProjectSelectionStatus {
    let mut exact = Vec::new();
    let mut strong = Vec::new();
    let mut weak = Vec::new();

    for candidate in candidates {
        match match_strength(reference, candidate) {
            Some(ComSelectionConfidence::Exact) => exact.push(candidate.clone()),
            Some(ComSelectionConfidence::Strong) => strong.push(candidate.clone()),
            Some(ComSelectionConfidence::Weak) => weak.push(candidate.clone()),
            None => {}
        }
    }

    sort_candidates_deterministically(&mut exact);
    sort_candidates_deterministically(&mut strong);
    sort_candidates_deterministically(&mut weak);

    if exact.len() == 1 {
        return ComProjectSelectionStatus::ResolvedUnique {
            candidate: exact.remove(0),
        };
    }
    if exact.len() > 1 {
        return ComProjectSelectionStatus::Ambiguous { candidates: exact };
    }
    if strong.len() == 1 {
        return ComProjectSelectionStatus::ResolvedUnique {
            candidate: strong.remove(0),
        };
    }
    if strong.len() > 1 {
        return ComProjectSelectionStatus::Ambiguous { candidates: strong };
    }
    if weak.len() == 1 {
        return ComProjectSelectionStatus::ResolvedUnique {
            candidate: weak.remove(0),
        };
    }
    if weak.len() > 1 {
        return ComProjectSelectionStatus::Ambiguous { candidates: weak };
    }

    ComProjectSelectionStatus::Missing
}

fn match_strength(
    reference: &BasProjComReference,
    candidate: &ComSelectionCandidate,
) -> Option<ComSelectionConfidence> {
    let reference_name = reference.include.trim();
    let candidate_name = candidate.identity.library_name.trim();

    let same_guid = equal_optional_case_insensitive(
        reference.guid.as_deref(),
        candidate.identity.guid.as_deref(),
    );
    let same_import_lib = equal_optional_case_insensitive(
        reference.import_lib.as_deref(),
        candidate.identity.import_lib.as_deref(),
    );
    let same_name = reference_name.eq_ignore_ascii_case(candidate_name);
    let same_version = equal_optional(reference.version_major, candidate.identity.version_major)
        && equal_optional(reference.version_minor, candidate.identity.version_minor);
    let same_lcid = equal_optional(reference.lcid, candidate.identity.lcid);

    if same_guid && same_version && same_lcid {
        return Some(ComSelectionConfidence::Exact);
    }
    if same_guid && same_import_lib {
        return Some(ComSelectionConfidence::Exact);
    }
    if same_guid && same_name {
        return Some(ComSelectionConfidence::Strong);
    }
    if same_import_lib && same_name {
        return Some(ComSelectionConfidence::Strong);
    }
    if same_name && same_version {
        return Some(ComSelectionConfidence::Strong);
    }
    if same_name {
        return Some(ComSelectionConfidence::Weak);
    }

    None
}

fn carrier_kind_from_path(path: Option<&Path>) -> ComSelectionCarrierKind {
    let Some(path) = path else {
        return ComSelectionCarrierKind::Unknown;
    };
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("tlb") || ext.eq_ignore_ascii_case("olb") => {
            ComSelectionCarrierKind::TypeLibrary
        }
        Some(ext) if ext.eq_ignore_ascii_case("dll") => ComSelectionCarrierKind::DynamicLibrary,
        Some(ext) if ext.eq_ignore_ascii_case("ocx") => ComSelectionCarrierKind::ActiveXControl,
        Some(ext) if ext.eq_ignore_ascii_case("exe") => ComSelectionCarrierKind::Executable,
        Some(ext) if ext.eq_ignore_ascii_case("xll") => ComSelectionCarrierKind::Xll,
        _ => ComSelectionCarrierKind::Unknown,
    }
}

fn equal_optional_case_insensitive(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

fn equal_optional<T: PartialEq>(left: Option<T>, right: Option<T>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn registered_identity_matches_query(
    identity: &TypeLibResolvedIdentity,
    query: &RegisteredComSelectionQuery,
) -> bool {
    if let Some(guid) = query.guid.as_deref()
        && !equal_optional_case_insensitive(Some(guid), identity.libid.as_deref())
    {
        return false;
    }
    if let Some(import_lib) = query.import_lib.as_deref()
        && !equal_optional_case_insensitive(Some(import_lib), Some(identity.importlib.as_str()))
    {
        return false;
    }
    if let Some(major) = query.version_major
        && identity.major_version != major
    {
        return false;
    }
    if let Some(minor) = query.version_minor
        && identity.minor_version != minor
    {
        return false;
    }
    if let Some(lcid) = query.lcid
        && identity.lcid != Some(lcid)
    {
        return false;
    }
    true
}

fn sort_candidates_deterministically(candidates: &mut [ComSelectionCandidate]) {
    candidates.sort_by(|left, right| {
        left.confidence
            .cmp(&right.confidence)
            .reverse()
            .then_with(|| {
                left.identity
                    .library_name
                    .to_ascii_lowercase()
                    .cmp(&right.identity.library_name.to_ascii_lowercase())
            })
            .then_with(|| {
                left.identity
                    .guid
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .cmp(
                        &right
                            .identity
                            .guid
                            .as_deref()
                            .unwrap_or("")
                            .to_ascii_lowercase(),
                    )
            })
            .then_with(|| {
                left.identity
                    .version_major
                    .cmp(&right.identity.version_major)
                    .then_with(|| {
                        left.identity
                            .version_minor
                            .cmp(&right.identity.version_minor)
                    })
            })
            .then_with(|| {
                left.identity
                    .import_lib
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .cmp(
                        &right
                            .identity
                            .import_lib
                            .as_deref()
                            .unwrap_or("")
                            .to_ascii_lowercase(),
                    )
            })
    });
}

#[cfg(test)]
mod tests {
    use super::{
        ComHostPlatform, ComProjectEditPlanKind, ComProjectSelection, ComProjectSelectionStatus,
        ComReferenceReorderIssueKind, ComSelectionCarrierKind, ComSelectionConfidence,
        ComSelectionService, ComSelectionSourceKind, FileBackedComSelectionQuery,
        RegisteredComSelectionQuery, assess_project_com_selections,
        basproj_reference_from_candidate, candidate_from_catalog_entry,
        candidate_from_project_reference, com_capability_profile,
        com_runtime_invocation_availability, discover_file_backed_com_candidates,
        discover_prog_id_com_candidates, discover_registered_com_candidates,
        inspect_workspace_com_project_state, plan_add_com_candidate, plan_reorder_com_references,
        plan_repair_project_selection, plan_replace_com_reference,
    };
    use crate::{BasProjComReference, HostProjectEdit};
    use oxvba_host::{
        DirectHostCapabilityKind, DirectHostCapabilityStatus, DirectHostCommandStatus,
        TypeLibraryCatalogEntry,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn com_capability_profile_reports_platform_specific_runtime_availability() {
        let availability = com_runtime_invocation_availability();
        let profile = com_capability_profile();
        assert_eq!(profile.runtime_availability, availability);
        assert_eq!(
            profile.reference_editing.kind,
            DirectHostCapabilityKind::ProjectAuthoring
        );
        assert_eq!(
            profile.reference_discovery.kind,
            DirectHostCapabilityKind::ComReferenceDiscovery
        );
        assert_eq!(
            profile.runtime_invocation.kind,
            DirectHostCapabilityKind::ComRuntimeInvocation
        );

        if cfg!(target_os = "windows") {
            assert_eq!(availability.platform, ComHostPlatform::Windows);
            assert!(matches!(
                availability.command_status,
                DirectHostCommandStatus::Available
            ));
            assert!(matches!(
                profile.runtime_invocation.status,
                DirectHostCapabilityStatus::Available
            ));
        } else {
            assert_eq!(availability.platform, ComHostPlatform::NonWindows);
            assert!(matches!(
                &availability.command_status,
                DirectHostCommandStatus::Disabled { reason }
                    if reason.stable_code == "DH-COM-RUNTIME-UNAVAILABLE"
            ));
            assert!(matches!(
                &profile.reference_discovery.status,
                DirectHostCapabilityStatus::Degraded { reason }
                    if reason.stable_code == "DH-COM-DISCOVERY-UNAVAILABLE"
            ));
            assert!(matches!(
                &profile.runtime_invocation.status,
                DirectHostCapabilityStatus::Unavailable { reason }
                    if reason.stable_code == "DH-COM-RUNTIME-UNAVAILABLE"
            ));
        }
    }

    #[test]
    fn plan_reorder_com_references_rewrites_only_valid_complete_orders() {
        let active = vec![
            BasProjComReference {
                include: "A".to_string(),
                guid: Some("{A}".to_string()),
                version_major: Some(1),
                version_minor: Some(0),
                lcid: Some(0),
                import_lib: Some("a.dll".to_string()),
            },
            BasProjComReference {
                include: "B".to_string(),
                guid: Some("{B}".to_string()),
                version_major: Some(1),
                version_minor: Some(0),
                lcid: Some(0),
                import_lib: Some("b.dll".to_string()),
            },
            BasProjComReference {
                include: "C".to_string(),
                guid: Some("{C}".to_string()),
                version_major: Some(1),
                version_minor: Some(0),
                lcid: Some(0),
                import_lib: Some("c.dll".to_string()),
            },
        ];

        let order = vec!["C".to_string(), "A".to_string(), "B".to_string()];
        let plan = plan_reorder_com_references(&active, &order);
        assert!(plan.can_apply);
        assert!(plan.issues.is_empty());
        assert_eq!(
            plan.resulting_references
                .iter()
                .map(|reference| reference.include.as_str())
                .collect::<Vec<_>>(),
            vec!["C", "A", "B"]
        );
        assert_eq!(plan.edits.len(), 6);
        assert!(matches!(
            &plan.edits[0],
            HostProjectEdit::RemoveComReference { include } if include == "A"
        ));
        assert!(matches!(
            &plan.edits[3],
            HostProjectEdit::AddComReference(reference) if reference.include == "C"
        ));

        let invalid = plan_reorder_com_references(
            &active,
            &["B".to_string(), "B".to_string(), "D".to_string()],
        );
        assert!(!invalid.can_apply);
        assert!(invalid.edits.is_empty());
        assert!(
            invalid
                .issues
                .iter()
                .any(|issue| issue.kind == ComReferenceReorderIssueKind::DuplicateReference)
        );
        assert!(
            invalid
                .issues
                .iter()
                .any(|issue| issue.kind == ComReferenceReorderIssueKind::MissingReference)
        );
        assert!(
            invalid
                .issues
                .iter()
                .any(|issue| issue.kind == ComReferenceReorderIssueKind::OmittedReference)
        );
    }

    #[test]
    fn candidate_from_catalog_entry_preserves_typelib_identity_and_carrier() {
        let candidate = candidate_from_catalog_entry(
            &TypeLibraryCatalogEntry {
                library_name: "Scripting".to_string(),
                importlib: "scrrun.dll".to_string(),
                libid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
            },
            ComSelectionSourceKind::RegisteredLibrary,
        );

        assert_eq!(candidate.identity.library_name, "Scripting");
        assert_eq!(
            candidate.identity.guid.as_deref(),
            Some("{420B2830-E718-11CF-893D-00A0C9054228}")
        );
        assert_eq!(candidate.identity.version_major, Some(1));
        assert_eq!(candidate.identity.version_minor, Some(0));
        assert_eq!(
            candidate.carrier_kind,
            ComSelectionCarrierKind::DynamicLibrary
        );
        assert_eq!(candidate.confidence, ComSelectionConfidence::Strong);
    }

    #[test]
    fn project_selection_prefers_exact_guid_and_version_match() {
        let reference = BasProjComReference {
            include: "Scripting".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: Some(0),
            import_lib: Some("scrrun.dll".to_string()),
        };
        let exact = candidate_from_catalog_entry(
            &TypeLibraryCatalogEntry {
                library_name: "Scripting".to_string(),
                importlib: "scrrun.dll".to_string(),
                libid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
            },
            ComSelectionSourceKind::RegisteredLibrary,
        );
        let weaker = candidate_from_catalog_entry(
            &TypeLibraryCatalogEntry {
                library_name: "Scripting".to_string(),
                importlib: "other.dll".to_string(),
                libid: None,
                major_version: 0,
                minor_version: 0,
                lcid: None,
            },
            ComSelectionSourceKind::RegisteredLibrary,
        );

        let selections = assess_project_com_selections(&[reference], &[weaker, exact.clone()]);
        match &selections[0].status {
            ComProjectSelectionStatus::ResolvedUnique { candidate } => {
                assert_eq!(candidate.identity.guid, exact.identity.guid);
                assert_eq!(candidate.identity.import_lib, exact.identity.import_lib);
            }
            other => panic!("expected resolved selection, got {other:?}"),
        }
    }

    #[test]
    fn project_selection_marks_multiple_same_name_candidates_as_ambiguous() {
        let reference = BasProjComReference {
            include: "Scripting".to_string(),
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
            import_lib: None,
        };
        let left = candidate_from_catalog_entry(
            &TypeLibraryCatalogEntry {
                library_name: "Scripting".to_string(),
                importlib: "a.dll".to_string(),
                libid: None,
                major_version: 0,
                minor_version: 0,
                lcid: None,
            },
            ComSelectionSourceKind::RegisteredLibrary,
        );
        let right = candidate_from_catalog_entry(
            &TypeLibraryCatalogEntry {
                library_name: "Scripting".to_string(),
                importlib: "b.dll".to_string(),
                libid: None,
                major_version: 0,
                minor_version: 0,
                lcid: None,
            },
            ComSelectionSourceKind::FileBrowse,
        );

        let selections = assess_project_com_selections(&[reference], &[left, right]);
        match &selections[0].status {
            ComProjectSelectionStatus::Ambiguous { candidates } => {
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].identity.import_lib.as_deref(), Some("a.dll"));
                assert_eq!(candidates[1].identity.import_lib.as_deref(), Some("b.dll"));
            }
            other => panic!("expected ambiguous selection, got {other:?}"),
        }
    }

    #[test]
    fn project_selection_marks_unmatched_reference_as_missing() {
        let reference = BasProjComReference {
            include: "UnknownLib".to_string(),
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
            import_lib: None,
        };
        let candidate = candidate_from_catalog_entry(
            &TypeLibraryCatalogEntry {
                library_name: "Scripting".to_string(),
                importlib: "scrrun.dll".to_string(),
                libid: None,
                major_version: 1,
                minor_version: 0,
                lcid: None,
            },
            ComSelectionSourceKind::RegisteredLibrary,
        );

        let selections = assess_project_com_selections(&[reference], &[candidate]);
        assert!(matches!(
            selections[0].status,
            ComProjectSelectionStatus::Missing
        ));
    }

    #[test]
    fn candidate_from_project_reference_keeps_project_active_source_kind() {
        let candidate = candidate_from_project_reference(&BasProjComReference {
            include: "WidgetLib".to_string(),
            guid: Some("{11111111-2222-3333-4444-555555555555}".to_string()),
            version_major: Some(2),
            version_minor: Some(5),
            lcid: Some(0),
            import_lib: Some("widget.tlb".to_string()),
        });

        assert_eq!(
            candidate.source_kind,
            ComSelectionSourceKind::ProjectActiveReference
        );
        assert_eq!(candidate.carrier_kind, ComSelectionCarrierKind::TypeLibrary);
    }

    #[test]
    fn registered_com_query_uses_known_typelib_identity() {
        let candidates = discover_registered_com_candidates(&RegisteredComSelectionQuery {
            reference_name: "StdOle".to_string(),
            requested_coclass: None,
            import_lib: Some("stdole2.tlb".to_string()),
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
        })
        .expect("known typelib discovery");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].source_kind,
            ComSelectionSourceKind::RegisteredLibrary
        );
        assert_eq!(
            candidates[0]
                .identity
                .guid
                .as_deref()
                .map(|value| value.trim_matches('{').trim_matches('}')),
            Some("00020430-0000-0000-C000-000000000046")
        );
    }

    #[test]
    fn prog_id_discovery_returns_typed_candidate() {
        let candidates =
            discover_prog_id_com_candidates("OxVba.TestDispatch").expect("fixture ProgID lookup");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].source_kind,
            ComSelectionSourceKind::ProgIdLookup
        );
        assert_eq!(
            candidates[0].prog_ids,
            vec!["OxVba.TestDispatch".to_string()]
        );
        assert_eq!(candidates[0].identity.library_name, "OxVba.TestDispatch");
    }

    #[test]
    fn file_backed_discovery_accepts_absolute_typelib_path_hint() {
        let candidates = discover_file_backed_com_candidates(&FileBackedComSelectionQuery {
            carrier_path: PathBuf::from(
                r"C:\Work\DnaCalc\OxVba\temp\missing\OxVba.TestEventServer.tlb",
            ),
            reference_name: Some("OxVbaMissingBase".to_string()),
            requested_coclass: None,
        })
        .expect("fixture file-backed lookup");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].source_kind,
            ComSelectionSourceKind::FileBrowse
        );
        assert_eq!(
            candidates[0].carrier_kind,
            ComSelectionCarrierKind::TypeLibrary
        );
        assert_eq!(
            candidates[0].identity.carrier_path.as_deref(),
            Some(Path::new(
                r"C:\Work\DnaCalc\OxVba\temp\missing\OxVba.TestEventServer.tlb"
            ))
        );
    }

    #[test]
    fn plan_add_candidate_emits_add_reference_edit() {
        let candidate = candidate_from_project_reference(&BasProjComReference {
            include: "Scripting".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: Some(0),
            import_lib: Some("scrrun.dll".to_string()),
        });

        let plan = plan_add_com_candidate(&candidate, None);
        assert_eq!(plan.kind, ComProjectEditPlanKind::Add);
        assert_eq!(plan.edits.len(), 1);
        assert!(matches!(plan.edits[0], HostProjectEdit::AddComReference(_)));
    }

    #[test]
    fn plan_replace_reference_preserves_include_and_updates_identity() {
        let reference = BasProjComReference {
            include: "Scripting".to_string(),
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
            import_lib: None,
        };
        let candidate = candidate_from_project_reference(&BasProjComReference {
            include: "Scripting Runtime".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: Some(0),
            import_lib: Some("scrrun.dll".to_string()),
        });

        let plan = plan_replace_com_reference(&reference, &candidate);
        assert_eq!(plan.kind, ComProjectEditPlanKind::Replace);
        assert_eq!(plan.include, "Scripting");
        assert_eq!(
            plan.resulting_reference
                .as_ref()
                .and_then(|value| value.guid.as_deref()),
            Some("{420B2830-E718-11CF-893D-00A0C9054228}")
        );
        assert_eq!(
            plan.resulting_reference
                .as_ref()
                .map(|value| value.include.as_str()),
            Some("Scripting")
        );
        assert_eq!(plan.edits.len(), 2);
    }

    #[test]
    fn plan_repair_missing_selection_reuses_original_include() {
        let selection = ComProjectSelection {
            reference: BasProjComReference {
                include: "Scripting".to_string(),
                guid: None,
                version_major: None,
                version_minor: None,
                lcid: None,
                import_lib: None,
            },
            status: ComProjectSelectionStatus::Missing,
        };
        let candidate = candidate_from_project_reference(&BasProjComReference {
            include: "Scripting Runtime".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: Some(0),
            import_lib: Some("scrrun.dll".to_string()),
        });

        let plan = plan_repair_project_selection(&selection, &candidate);
        assert_eq!(plan.kind, ComProjectEditPlanKind::Repair);
        assert_eq!(
            plan.resulting_reference
                .as_ref()
                .map(|value| value.include.as_str()),
            Some("Scripting")
        );
    }

    #[test]
    fn basproj_reference_from_candidate_can_override_include() {
        let candidate = candidate_from_project_reference(&BasProjComReference {
            include: "Scripting Runtime".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: Some(0),
            import_lib: Some("scrrun.dll".to_string()),
        });

        let reference = basproj_reference_from_candidate(&candidate, Some("Scripting"));
        assert_eq!(reference.include, "Scripting");
        assert_eq!(reference.import_lib.as_deref(), Some("scrrun.dll"));
    }

    #[test]
    fn inspect_workspace_com_project_state_reports_active_selection_status() {
        let temp_root = unique_temp_dir("oxvba_com_selection_surface");
        fs::create_dir_all(&temp_root).expect("temp dir");
        fs::write(
            temp_root.join("Widget.cls"),
            "Attribute VB_Name = \"Widget\"\nOption Explicit\n",
        )
        .expect("class module");
        fs::write(
            temp_root.join("App.basproj"),
            "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <ProjectName>App</ProjectName>\n  </PropertyGroup>\n  <ItemGroup>\n    <ClassModule Include=\"Widget.cls\" />\n    <COMReference Include=\"Scripting\">\n      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>\n      <VersionMajor>1</VersionMajor>\n      <VersionMinor>0</VersionMinor>\n      <ImportLib>scrrun.dll</ImportLib>\n    </COMReference>\n  </ItemGroup>\n</Project>\n",
        )
        .expect("basproj");

        let discovered = vec![candidate_from_project_reference(&BasProjComReference {
            include: "Scripting".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("scrrun.dll".to_string()),
        })];
        let state =
            inspect_workspace_com_project_state(&temp_root, &discovered).expect("state surface");

        assert_eq!(state.project_name, "App");
        assert_eq!(state.active_references.len(), 1);
        assert!(matches!(
            state.selections[0].status,
            ComProjectSelectionStatus::ResolvedUnique { .. }
        ));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn service_wraps_project_state_and_plans() {
        let service = ComSelectionService;
        let candidate = candidate_from_project_reference(&BasProjComReference {
            include: "Scripting Runtime".to_string(),
            guid: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            version_major: Some(1),
            version_minor: Some(0),
            lcid: Some(0),
            import_lib: Some("scrrun.dll".to_string()),
        });
        let add_plan = service.plan_add_candidate(&candidate, Some("Scripting"));
        assert_eq!(add_plan.kind, ComProjectEditPlanKind::Add);

        let remove_plan = service.plan_remove_reference(&BasProjComReference {
            include: "Scripting".to_string(),
            guid: None,
            version_major: None,
            version_minor: None,
            lcid: None,
            import_lib: None,
        });
        assert_eq!(remove_plan.kind, ComProjectEditPlanKind::Remove);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{nonce}"))
    }
}
