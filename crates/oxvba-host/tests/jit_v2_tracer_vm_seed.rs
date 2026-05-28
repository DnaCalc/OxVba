use std::path::PathBuf;

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::HostVariantSnapshotWithPackageIdentity;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;

#[cfg(target_os = "windows")]
use oxvba_compiler::{ProjectReference, ReferenceKind};
#[cfg(target_os = "windows")]
use oxvba_hal::model::HostPolicy;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_source(file: &str) -> String {
    std::fs::read_to_string(
        repo_root()
            .join("conformance")
            .join("jit_v2")
            .join("tracer_bullets")
            .join(file),
    )
    .unwrap_or_else(|err| panic!("failed to read JIT v2 tracer fixture `{file}`: {err}"))
}

fn run_source_vm(source: &str) -> Vec<Variant> {
    Engine::new(HostConfig { enable_jit: false })
        .execute_source_with_variant_snapshot_phased(source)
        .expect("JIT v2 VM seed source should execute")
}

fn project_manifest_from_source(
    project_name: &str,
    module_name: &str,
    source: &str,
) -> ProjectManifest {
    ProjectManifest {
        project_name: project_name.to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(module_name, ModuleKind::Procedural, source)
                .expect("JIT v2 tracer module should parse"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    }
}

fn run_project_vm_with_package(
    manifest: &ProjectManifest,
) -> HostVariantSnapshotWithPackageIdentity {
    Engine::new(HostConfig { enable_jit: false })
        .execute_project_with_variant_snapshot_and_package_identity_phased(manifest)
        .expect("JIT v2 VM seed project should execute")
}

#[cfg(target_os = "windows")]
fn run_windows_hosted_source_vm_with_package(
    source: &str,
) -> HostVariantSnapshotWithPackageIdentity {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_and_package_identity_phased(source)
        .expect("JIT v2 Windows host-backed VM seed source should execute")
}

#[cfg(target_os = "windows")]
fn manifest_with_oxvba_typelib(source: &str) -> ProjectManifest {
    let main_module = module_unit_from_source("MainModule", ModuleKind::Procedural, source)
        .expect("JIT v2 early-bound tracer module should parse");
    ProjectManifest {
        project_name: "JitV2Tracer".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    }
}

#[cfg(target_os = "windows")]
fn run_windows_hosted_project_vm_with_package(
    manifest: &ProjectManifest,
) -> HostVariantSnapshotWithPackageIdentity {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_project_with_variant_snapshot_and_package_identity_phased(manifest)
        .expect("JIT v2 Windows host-backed VM seed project should execute")
}

fn interop_descriptor_observation_tokens(
    snapshot: &HostVariantSnapshotWithPackageIdentity,
) -> Vec<String> {
    snapshot
        .package_identity
        .interop_descriptor_evidence
        .iter()
        .flat_map(|descriptor| {
            descriptor
                .observations
                .iter()
                .map(move |observation| format!("{}:{observation}", descriptor.descriptor_id))
        })
        .collect()
}

fn error_descriptor_observation_tokens(
    snapshot: &HostVariantSnapshotWithPackageIdentity,
) -> Vec<String> {
    snapshot
        .package_identity
        .error_descriptor_evidence
        .iter()
        .flat_map(|descriptor| {
            descriptor
                .observations
                .iter()
                .map(move |observation| format!("{}:{observation}", descriptor.error_scope_id))
        })
        .collect()
}

fn deopt_snapshot_observation_tokens(
    snapshot: &HostVariantSnapshotWithPackageIdentity,
) -> Vec<String> {
    snapshot
        .package_identity
        .deopt_snapshot_evidence
        .iter()
        .flat_map(|descriptor| {
            descriptor
                .observations
                .iter()
                .map(move |observation| format!("{}:{observation}", descriptor.safepoint_id))
        })
        .collect()
}

fn assert_interop_observation(tokens: &[String], expected: &str) {
    assert!(
        tokens.iter().any(|token| token.contains(expected)),
        "expected interop descriptor evidence containing `{expected}`; got: {tokens:?}"
    );
}

fn assert_descriptor_observation(tokens: &[String], expected: &str) {
    assert!(
        tokens.iter().any(|token| token.contains(expected)),
        "expected descriptor evidence containing `{expected}`; got: {tokens:?}"
    );
}

#[test]
fn tb01_primitive_scalar_vm_seed_runs() {
    let out = run_source_vm(&fixture_source("tb01_primitive_scalar_loop.bas"));

    assert_eq!(out[0], Variant::from_i32(11), "loop index mismatch");
    assert_eq!(out[1], Variant::from_i32(30), "Long accumulator mismatch");
    assert_eq!(out[2], Variant::from_i32(3), "Long step mismatch");
    assert_eq!(out[3].as_f64(), Some(1.5), "Double scale mismatch");
    assert_eq!(out[4].as_f64(), Some(45.0), "Double product mismatch");
    assert_eq!(out[5], Variant::from_bool(true), "Boolean result mismatch");
}

#[test]
fn tb02_udt_struct_vm_seed_runs() {
    let out = run_source_vm(&fixture_source("tb02_udt_struct_fields.bas"));

    assert_eq!(out[1], Variant::from_i32(7), "source UDT X mismatch");
    assert_eq!(out[2], Variant::from_i32(9), "source UDT Y mismatch");
    assert_eq!(
        out[4],
        Variant::from_i32(10),
        "copied UDT X update mismatch"
    );
    assert_eq!(out[5], Variant::from_i32(9), "copied UDT Y mismatch");
    assert_eq!(out[6], Variant::from_i32(19), "UDT field total mismatch");
    assert_eq!(
        out[7],
        Variant::from_i32(17),
        "UDT cross-struct sum mismatch"
    );
}

#[test]
fn tb03_error_routing_vm_seed_runs() {
    let source = fixture_source("tb03_error_resume_next.bas");
    let out = run_source_vm(&source);

    assert_eq!(out[0], Variant::from_i32(0), "initial Err.Number mismatch");
    assert_eq!(
        out[1],
        Variant::from_i32(11),
        "Resume Next Err.Number mismatch"
    );
    assert_eq!(out[2], Variant::from_i32(10), "numerator slot mismatch");
    assert_eq!(out[3], Variant::from_i32(0), "denominator slot mismatch");
    assert_eq!(out[4], Variant::empty(), "failed result slot mismatch");

    let manifest = project_manifest_from_source("JitV2Tracer", "ErrorModule", &source);
    let snapshot = run_project_vm_with_package(&manifest);
    let error_tokens = error_descriptor_observation_tokens(&snapshot);
    assert_descriptor_observation(&error_tokens, "kind=on-error-resume-next");
    assert_descriptor_observation(&error_tokens, "state-transition=enable-resume-next");
    assert_descriptor_observation(&error_tokens, "kind=fallible-helper");
    assert_descriptor_observation(&error_tokens, "runtime-error=division-by-zero-11");
    assert_descriptor_observation(&error_tokens, "resume-next-consumable=true");

    let deopt_tokens = deopt_snapshot_observation_tokens(&snapshot);
    assert_descriptor_observation(&deopt_tokens, "operation=helper-div");
    assert_descriptor_observation(
        &deopt_tokens,
        "error-state=err-number-description-source-last-error-pc",
    );
    assert_descriptor_observation(&deopt_tokens, "cleanup-state=lifecycle-descriptor-refs");
    assert_descriptor_observation(&deopt_tokens, "live-carrier-map=carrier-layout-descriptors");
}

#[test]
fn tb04_bstr_lifetime_vm_seed_runs() {
    let out = run_source_vm(&fixture_source("tb04_bstr_lifetime_concat_len.bas"));

    assert_eq!(
        out[0].as_bstr(),
        Some("alpha".into()),
        "source string mismatch"
    );
    assert_eq!(
        out[1].as_bstr(),
        Some("alpha-beta".into()),
        "concat string mismatch"
    );
    assert_eq!(out[2], Variant::from_i32(10), "Len result mismatch");
}

#[test]
fn tb05_safearray_vm_seed_runs() {
    let out = run_source_vm(&fixture_source("tb05_safearray_foreach_bounds.bas"));

    assert_eq!(out[1], Variant::from_i32(2), "a(0) store mismatch");
    assert_eq!(out[2], Variant::from_i32(3), "a(1) store mismatch");
    assert_eq!(out[3], Variant::from_i32(5), "a(2) store mismatch");
    assert_eq!(out[6], Variant::from_i32(2), "index read a(0) mismatch");
    assert_eq!(out[7], Variant::from_i32(5), "index read a(2) mismatch");
    assert_eq!(out[8], Variant::from_i32(0), "LBound mismatch");
    assert_eq!(out[9], Variant::from_i32(2), "UBound mismatch");
    assert_eq!(out[10], Variant::from_i32(10), "For Each sum mismatch");
}

#[cfg(target_os = "windows")]
#[test]
fn tb06_late_bound_com_vm_seed_runs_with_hosted_controlled_com() {
    let snapshot = run_windows_hosted_source_vm_with_package(&fixture_source(
        "tb06_late_bound_com_resume_next.bas",
    ));
    let out = &snapshot.values;

    assert!(
        out[0]
            .as_object_ref()
            .is_some_and(|object| object.raw() >= 20_001),
        "CreateObject should return a retained ObjectRef, got {out:?}"
    );
    assert_eq!(out[1], Variant::from_i32(7), "Count mismatch");
    assert_eq!(out[2], Variant::from_bool(true), "Exists(42) mismatch");
    assert!(
        out[3].as_i32().is_some_and(|err| err != 0),
        "RaiseException under Resume Next should set Err.Number, got {out:?}"
    );

    let interop_tokens = interop_descriptor_observation_tokens(&snapshot);
    assert_interop_observation(&interop_tokens, "kind=com-createobject");
    assert_interop_observation(&interop_tokens, "boundary=host-com-activation");
    assert_interop_observation(&interop_tokens, "kind=com-dispatch-invoke");
    assert_interop_observation(&interop_tokens, "early-bound=false");
    assert_interop_observation(&interop_tokens, "selector=runtime-name-slot");
    assert_interop_observation(&interop_tokens, "hresult-excepinfo-argerr=runtime-owned");
}

#[cfg(target_os = "windows")]
#[test]
fn tb07_early_bound_com_vm_seed_runs_with_typelib_reference() {
    let source = fixture_source("tb07_early_bound_com_typelib.bas");
    let manifest = manifest_with_oxvba_typelib(&source);
    let snapshot = run_windows_hosted_project_vm_with_package(&manifest);
    let out = &snapshot.values;

    assert!(
        out[0]
            .as_object_ref()
            .is_some_and(|object| object.raw() >= 20_001),
        "typed New should return a retained ObjectRef, got {out:?}"
    );
    assert_eq!(out[1], Variant::from_i32(7), "typed Count mismatch");
    assert_eq!(
        out[2],
        Variant::from_bool(true),
        "typed Exists(42) mismatch"
    );

    let interop_tokens = interop_descriptor_observation_tokens(&snapshot);
    assert_interop_observation(&interop_tokens, "kind=com-dispatch-invoke");
    assert_interop_observation(&interop_tokens, "early-bound=true");
    assert_interop_observation(&interop_tokens, "boundary=host-com-dispatch");
    assert_interop_observation(&interop_tokens, "hresult-excepinfo-argerr=runtime-owned");
}

#[cfg(target_os = "windows")]
#[test]
fn tb08_native_declare_vm_seed_runs_on_current_windows_native_lane() {
    let snapshot = run_windows_hosted_source_vm_with_package(&fixture_source(
        "tb08_native_declare_shared_abi.bas",
    ));
    let out = &snapshot.values;

    assert_eq!(out[0], Variant::from_i32(5), "BSTR length mismatch");
    assert!(
        out[1].as_f64() == Some(123.0),
        "ByRef Double writeback mismatch: {out:?}"
    );
    assert_eq!(out[2], Variant::from_i32(0), "VarR8FromI4 status mismatch");
    assert_eq!(
        out[3],
        Variant::from_i32(2),
        "SAFEARRAY buffer length mismatch"
    );
    assert_eq!(
        out[4],
        Variant::from_bool(true),
        "Variant pointer exposure mismatch"
    );
    assert_eq!(out[5], Variant::from_i32(7), "aggregate result mismatch");

    let interop_tokens = interop_descriptor_observation_tokens(&snapshot);
    assert_interop_observation(&interop_tokens, "kind=native-declare");
    assert_interop_observation(&interop_tokens, "declared-name=lstrlenw");
    assert_interop_observation(&interop_tokens, "declared-name=varr8fromi4");
    assert_interop_observation(&interop_tokens, "library=kernel32");
    assert_interop_observation(&interop_tokens, "library=oleaut32");
    assert_interop_observation(&interop_tokens, "param:1:byref=true");
    assert_interop_observation(&interop_tokens, "kind=native-invoke");
    assert_interop_observation(&interop_tokens, "writeback:0:kind=byrefvalue");
}

#[test]
fn tb09_exported_callable_vm_seed_runs() {
    let source = fixture_source("tb09_exported_callable_projection.bas");
    let out = run_source_vm(&source);

    assert_eq!(out[0], Variant::from_i32(6), "ByRef writeback mismatch");
    assert_eq!(out[1], Variant::from_i32(13), "return projection mismatch");

    let manifest = project_manifest_from_source("JitV2Tracer", "ExportModule", &source);
    let snapshot = run_project_vm_with_package(&manifest);

    let interop_tokens = interop_descriptor_observation_tokens(&snapshot);
    assert_interop_observation(&interop_tokens, "kind=exported-callable");
    assert_interop_observation(&interop_tokens, "procedure=jitexportedadd");
    assert_interop_observation(
        &interop_tokens,
        "inbound-projection=variant-positional-to-procedure-slots",
    );
    assert_interop_observation(&interop_tokens, "param:1:writeback=export-boundary-byref");
    assert_interop_observation(
        &interop_tokens,
        "outbound-return-projection=return-slot-to-variant",
    );
    assert_interop_observation(
        &interop_tokens,
        "cleanup-policy=vm-frame-owned-slots-and-export-boundary-temporaries",
    );
    assert_interop_observation(
        &interop_tokens,
        "error-policy=runtime-error-projected-to-host-failure",
    );
}
