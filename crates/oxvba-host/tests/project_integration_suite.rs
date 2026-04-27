use std::fs;
use std::path::{Path, PathBuf};

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
    ReferencedProjectManifest, module_unit_from_source,
};
use oxvba_hal::model::{
    HostPolicy, HostPolicyPreset, UiVirtualizationMode, UnsupportedFeatureMode,
};
use oxvba_host::engine::DiagnosticPhase;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};
use oxvba_runtime::Variant;
const CATALOG_REL_PATH: &str = "conformance/integration/catalog.psv";
const CASES_REL_PATH: &str = "conformance/integration/projects";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseStatus {
    Active,
    ActiveLimit,
    Deferred,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendMode {
    Vm,
    Jit,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedPhase {
    Any,
    CompileTime,
    Runtime,
}

#[derive(Debug, Clone)]
struct IntegrationCase {
    case_id: String,
    level: String,
    title: String,
    status: CaseStatus,
    backend: BackendMode,
    runtime_profile: RuntimeProfileId,
    policy_preset: HostPolicyPreset,
    policy_overrides: Vec<(String, String)>,
    unsupported_mode: Option<UnsupportedFeatureMode>,
    expected_status: ExpectedStatus,
    expected_phase: ExpectedPhase,
    expected_compat_slots: Vec<i32>,
    expected_error_contains: Vec<String>,
    reference_order: Vec<String>,
    deferred_gate: String,
    topic_refs: Vec<String>,
    project_name: String,
    notes: String,
}

impl BackendMode {
    fn backends(self) -> &'static [bool] {
        match self {
            Self::Vm => &[false],
            Self::Jit => &[true],
            Self::Both => &[false, true],
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn project_variants_to_expected_compat_slots(values: &[Variant]) -> Result<Vec<i32>, String> {
    values
        .iter()
        .map(|value| {
            value.project_compat_slot_i32().map_err(|err| {
                format!(
                    "variant {:?} cannot be projected into legacy expectation slot: {err}",
                    value
                )
            })
        })
        .collect()
}

fn parse_case_status(raw: &str) -> Result<CaseStatus, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "active" => Ok(CaseStatus::Active),
        "active-limit" => Ok(CaseStatus::ActiveLimit),
        "deferred" => Ok(CaseStatus::Deferred),
        "planned" => Ok(CaseStatus::Planned),
        other => Err(format!("invalid status `{other}`")),
    }
}

fn parse_backend_mode(raw: &str) -> Result<BackendMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "vm" => Ok(BackendMode::Vm),
        "jit" => Ok(BackendMode::Jit),
        "both" => Ok(BackendMode::Both),
        other => Err(format!("invalid backend `{other}`")),
    }
}

fn parse_runtime_profile(raw: &str) -> Result<RuntimeProfileId, String> {
    RuntimeProfileId::parse(raw)
}

fn parse_policy_preset(raw: &str) -> Result<HostPolicyPreset, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "strict-ci" => Ok(HostPolicyPreset::StrictCi),
        "deterministic-runtime" => Ok(HostPolicyPreset::DeterministicRuntime),
        "deterministic-compile-time" => Ok(HostPolicyPreset::DeterministicCompileTime),
        "interactive-dev" => Ok(HostPolicyPreset::InteractiveDev),
        other => Err(format!("invalid policy preset `{other}`")),
    }
}

fn parse_unsupported_mode(raw: &str) -> Result<Option<UnsupportedFeatureMode>, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "inherit" => Ok(None),
        "compile-time" => Ok(Some(UnsupportedFeatureMode::CompileTime)),
        "runtime" => Ok(Some(UnsupportedFeatureMode::Runtime)),
        other => Err(format!("invalid unsupported_mode `{other}`")),
    }
}

fn parse_expected_status(raw: &str) -> Result<ExpectedStatus, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "ok" => Ok(ExpectedStatus::Ok),
        "error" => Ok(ExpectedStatus::Error),
        other => Err(format!("invalid expect_status `{other}`")),
    }
}

fn parse_expected_phase(raw: &str) -> Result<ExpectedPhase, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "any" => Ok(ExpectedPhase::Any),
        "compile-time" => Ok(ExpectedPhase::CompileTime),
        "runtime" => Ok(ExpectedPhase::Runtime),
        other => Err(format!("invalid expect_phase `{other}`")),
    }
}

fn parse_list(raw: &str, separator: char) -> Vec<String> {
    raw.split(separator)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_policy_overrides(raw: &str) -> Result<Vec<(String, String)>, String> {
    let mut parsed = Vec::new();
    for item in parse_list(raw, ';') {
        let Some((key, value)) = item.split_once('=') else {
            return Err(format!("invalid policy override `{item}`"));
        };
        parsed.push((
            key.trim().to_ascii_lowercase(),
            value.trim().to_ascii_lowercase(),
        ));
    }
    Ok(parsed)
}

fn parse_slots(raw: &str) -> Result<Vec<i32>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    parse_list(raw, ',')
        .into_iter()
        .map(|value| {
            value
                .parse::<i32>()
                .map_err(|err| format!("invalid expected slot `{value}`: {err}"))
        })
        .collect()
}

fn load_catalog() -> Result<Vec<IntegrationCase>, String> {
    let root = workspace_root();
    let catalog_path = root.join(CATALOG_REL_PATH);
    let text = fs::read_to_string(&catalog_path)
        .map_err(|err| format!("failed to read {}: {err}", catalog_path.display()))?;

    let mut cases = Vec::new();
    let mut header_seen = false;
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if !header_seen {
            header_seen = true;
            continue;
        }

        let parts = line.split('|').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 18 {
            return Err(format!(
                "catalog line {} expected 18 columns but got {}",
                index + 1,
                parts.len()
            ));
        }

        let case = IntegrationCase {
            case_id: parts[0].to_string(),
            level: parts[1].to_string(),
            title: parts[2].to_string(),
            status: parse_case_status(parts[3])?,
            backend: parse_backend_mode(parts[4])?,
            runtime_profile: parse_runtime_profile(parts[5])?,
            policy_preset: parse_policy_preset(parts[6])?,
            policy_overrides: parse_policy_overrides(parts[7])?,
            unsupported_mode: parse_unsupported_mode(parts[8])?,
            expected_status: parse_expected_status(parts[9])?,
            expected_phase: parse_expected_phase(parts[10])?,
            expected_compat_slots: parse_slots(parts[11])?,
            expected_error_contains: parse_list(parts[12], ';'),
            reference_order: parse_list(parts[13], ';'),
            deferred_gate: parts[14].to_string(),
            topic_refs: parse_list(parts[15], ';'),
            project_name: parts[16].to_string(),
            notes: parts[17].to_string(),
        };
        cases.push(case);
    }

    if !header_seen {
        return Err("catalog header missing".to_string());
    }

    if cases.is_empty() {
        return Err("catalog has no cases".to_string());
    }

    Ok(cases)
}

fn parse_module_kind(raw: &str) -> Result<ModuleKind, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "proc" => Ok(ModuleKind::Procedural),
        "class" => Ok(ModuleKind::Class),
        "document" => Ok(ModuleKind::Document),
        "form" => Ok(ModuleKind::Form),
        "extension" => Ok(ModuleKind::Extension),
        other => Err(format!("invalid module kind `{other}`")),
    }
}

fn load_module_dir(dir: &Path) -> Result<Vec<oxvba_compiler::ModuleUnit>, String> {
    if !dir.exists() {
        return Err(format!("missing module dir: {}", dir.display()));
    }

    let mut entries = fs::read_dir(dir)
        .map_err(|err| format!("failed to read {}: {err}", dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut units = Vec::new();
    for entry in entries {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid utf-8 filename: {}", path.display()))?;
        if !file_name.ends_with(".bas") {
            continue;
        }

        let stem = file_name.trim_end_matches(".bas");
        let Some((module_name, kind_raw)) = stem.rsplit_once('.') else {
            return Err(format!(
                "module filename must be <ModuleName>.<kind>.bas: {}",
                path.display()
            ));
        };
        let kind = parse_module_kind(kind_raw)?;
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let unit = module_unit_from_source(module_name, kind, source).map_err(|err| {
            format!(
                "failed to parse module {} in {}: {}",
                module_name,
                path.display(),
                err
            )
        })?;
        units.push(unit);
    }

    if units.is_empty() {
        return Err(format!("no .bas modules found in {}", dir.display()));
    }

    Ok(units)
}

fn build_manifest(case: &IntegrationCase) -> Result<ProjectManifest, String> {
    let root = workspace_root().join(CASES_REL_PATH).join(&case.case_id);
    if !root.exists() {
        return Err(format!("missing case directory: {}", root.display()));
    }

    let modules = load_module_dir(&root.join("main"))?;

    let references_root = root.join("references");
    let mut available_refs = if references_root.exists() {
        fs::read_dir(&references_root)
            .map_err(|err| format!("failed to read {}: {err}", references_root.display()))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        format!(
                            "invalid utf-8 reference name in {}",
                            references_root.display()
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    available_refs.sort();

    let reference_names = if case.reference_order.is_empty() {
        available_refs.clone()
    } else {
        case.reference_order.clone()
    };

    let mut references = Vec::new();
    let mut reference_projects = Vec::new();
    for name in &reference_names {
        let project_dir = references_root.join(name);
        if !project_dir.exists() {
            return Err(format!(
                "case {} references `{}` but directory is missing",
                case.case_id, name
            ));
        }
        references.push(ProjectReference {
            referenced_project_name: name.clone(),
            reference_kind: ReferenceKind::Project,
        });
        reference_projects.push(ReferencedProjectManifest {
            project_name: name.clone(),
            modules: load_module_dir(&project_dir)?,
        });
    }

    Ok(ProjectManifest {
        project_name: case.project_name.clone(),
        project_kind: ProjectKind::Source,
        modules,
        references,
        reference_projects,
        conditional_constants: std::collections::BTreeMap::new(),
    })
}

fn parse_bool(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!("invalid boolean `{other}`")),
    }
}

fn parse_ui_virtualization(raw: &str) -> Result<UiVirtualizationMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "disabled" => Ok(UiVirtualizationMode::Disabled),
        "scripted-responses" => Ok(UiVirtualizationMode::ScriptedResponses),
        "fail-on-prompt" => Ok(UiVirtualizationMode::FailOnPrompt),
        other => Err(format!("invalid ui_virtualization `{other}`")),
    }
}

fn apply_policy_overrides(policy: &mut HostPolicy, case: &IntegrationCase) -> Result<(), String> {
    for (key, value) in &case.policy_overrides {
        match key.as_str() {
            "allow_interaction" => policy.allow_interaction = parse_bool(value)?,
            "allow_process_spawn" => policy.allow_process_spawn = parse_bool(value)?,
            "allow_filesystem_mutation" => policy.allow_filesystem_mutation = parse_bool(value)?,
            "allow_dynamic_link" => policy.allow_dynamic_link = parse_bool(value)?,
            "allow_com_activation" => policy.allow_com_activation = parse_bool(value)?,
            "deterministic_mode" => policy.deterministic_mode = parse_bool(value)?,
            "ui_virtualization" => policy.ui_virtualization = parse_ui_virtualization(value)?,
            "unsupported_feature_mode" => {
                policy.unsupported_feature_mode = match value.as_str() {
                    "compile-time" => UnsupportedFeatureMode::CompileTime,
                    "runtime" => UnsupportedFeatureMode::Runtime,
                    other => {
                        return Err(format!(
                            "invalid unsupported_feature_mode override `{other}`"
                        ));
                    }
                }
            }
            other => {
                return Err(format!("unsupported policy override key `{other}`"));
            }
        }
    }
    Ok(())
}

fn run_case(case: &IntegrationCase, enable_jit: bool) -> Result<(), String> {
    let manifest = build_manifest(case)?;

    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: Some("Application".to_string()),
    });
    engine.set_runtime_profile(case.runtime_profile);

    let mut policy = HostPolicy::for_preset(case.policy_preset);
    policy.runtime_class = Some(case.runtime_profile.runtime_class());
    apply_policy_overrides(&mut policy, case)?;
    engine.set_host_policy(policy);

    if let Some(mode) = case.unsupported_mode {
        engine.set_unsupported_feature_mode(mode);
    }

    let result = engine.execute_project_with_variant_snapshot_phased(&manifest);

    match (&case.expected_status, result) {
        (ExpectedStatus::Ok, Ok(values)) => {
            if case.expected_compat_slots.is_empty() {
                return Ok(());
            }
            let observed_slots = project_variants_to_expected_compat_slots(&values)?;
            if observed_slots != case.expected_compat_slots {
                return Err(format!(
                    "compat slot mismatch: expected {:?}, got {:?} from values {:?}",
                    case.expected_compat_slots, observed_slots, values
                ));
            }
        }
        (ExpectedStatus::Ok, Err(err)) => {
            return Err(format!("expected success but got error: {}", err.message()));
        }
        (ExpectedStatus::Error, Ok(values)) => {
            return Err(format!("expected error but got values {:?}", values));
        }
        (ExpectedStatus::Error, Err(err)) => {
            let expected_phase = match case.expected_phase {
                ExpectedPhase::Any => None,
                ExpectedPhase::CompileTime => Some(DiagnosticPhase::CompileTime),
                ExpectedPhase::Runtime => Some(DiagnosticPhase::Runtime),
            };
            if let Some(phase) = expected_phase
                && err.phase() != phase
            {
                return Err(format!(
                    "phase mismatch: expected {:?}, got {:?}",
                    phase,
                    err.phase()
                ));
            }
            for token in &case.expected_error_contains {
                if !err.message().contains(token) {
                    return Err(format!(
                        "error message missing token `{}`: {}",
                        token,
                        err.message()
                    ));
                }
            }
        }
    }

    Ok(())
}

#[test]
fn project_integration_catalog_is_well_formed_and_tracked() {
    let cases = load_catalog().expect("catalog should parse");
    let mut ids = std::collections::BTreeSet::new();
    let mut active_count = 0usize;
    let mut deferred_count = 0usize;

    for case in &cases {
        assert!(
            ids.insert(case.case_id.clone()),
            "duplicate case_id: {}",
            case.case_id
        );
        assert!(!case.level.is_empty(), "level missing for {}", case.case_id);
        assert!(!case.title.is_empty(), "title missing for {}", case.case_id);
        assert!(
            !case.project_name.is_empty(),
            "project_name missing for {}",
            case.case_id
        );
        match case.status {
            CaseStatus::Active | CaseStatus::ActiveLimit => {
                active_count += 1;
                assert!(
                    case.deferred_gate.is_empty(),
                    "active case {} should not set deferred_gate",
                    case.case_id
                );
                assert!(
                    !case.notes.is_empty(),
                    "active case {} should include notes",
                    case.case_id
                );
            }
            CaseStatus::Deferred | CaseStatus::Planned => {
                deferred_count += 1;
                assert!(
                    !case.deferred_gate.is_empty() || !case.topic_refs.is_empty(),
                    "deferred/planned case {} should reference deferred gate or topic",
                    case.case_id
                );
            }
        }
    }

    assert!(active_count > 0, "catalog must include active cases");
    assert!(
        deferred_count > 0,
        "catalog must include deferred/planned cases"
    );
}

#[test]
fn project_integration_suite_executes_active_cases() {
    let cases = load_catalog().expect("catalog should parse");
    let filter = std::env::var("OXVBA_PROJECT_INTEGRATION_FILTER").ok();
    let mut executed = 0usize;

    for case in &cases {
        if !matches!(case.status, CaseStatus::Active | CaseStatus::ActiveLimit) {
            continue;
        }
        if let Some(filter_value) = filter.as_ref()
            && !case
                .case_id
                .to_ascii_lowercase()
                .contains(&filter_value.to_ascii_lowercase())
        {
            continue;
        }

        for enable_jit in case.backend.backends() {
            let backend_name = if *enable_jit { "jit" } else { "vm" };
            run_case(case, *enable_jit).unwrap_or_else(|err| {
                panic!(
                    "{} ({}) failed: {} [{}]",
                    case.case_id, case.title, err, backend_name
                )
            });
            executed += 1;
        }
    }

    assert!(
        executed > 0,
        "no active integration cases executed (filter={:?})",
        filter
    );
}
