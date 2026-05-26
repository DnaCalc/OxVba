use std::collections::BTreeMap;

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
    ReferencedProjectManifest, module_unit_from_source,
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, engine::DiagnosticPhase};
use oxvba_runtime::Variant;

fn proc_module(name: &str, source: &str) -> oxvba_compiler::ModuleUnit {
    module_unit_from_source(name, ModuleKind::Procedural, source).expect("module should parse")
}

fn source_project(project_name: &str, modules: Vec<oxvba_compiler::ModuleUnit>) -> ProjectManifest {
    ProjectManifest {
        project_name: project_name.to_string(),
        project_kind: ProjectKind::Source,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

fn contains_value(snapshot: &[Variant], expected: Variant) {
    assert!(
        snapshot.contains(&expected),
        "expected snapshot to contain {:?}, got {:?}",
        expected,
        snapshot
    );
}

fn assert_jit_unavailable(manifest: &ProjectManifest) {
    let err = Engine::new(HostConfig { enable_jit: true })
        .execute_project_with_variant_snapshot_phased(manifest)
        .expect_err("JIT request should not silently fall back to VM execution");
    assert_eq!(err.phase(), DiagnosticPhase::Runtime);
    assert!(
        err.message().contains("JIT execution"),
        "unexpected JIT unavailable diagnostic: {err}"
    );
}

fn assert_vm_contains_and_jit_unavailable(manifest: &ProjectManifest, expected: Variant) {
    let engine = Engine::new(HostConfig { enable_jit: false });
    let snapshot = engine
        .execute_project_with_variant_snapshot_phased(manifest)
        .expect("VM execution should succeed");
    contains_value(&snapshot, expected);
    assert_jit_unavailable(manifest);
}

#[test]
fn nested_udt_field_access_integration() {
    let manifest = source_project(
        "NestedUdtFieldAccess",
        vec![proc_module(
            "MainModule",
            r#"
Option Explicit
Type Point
    X As Integer
    Y As Integer
End Type
Type Rect
    TopLeft As Point
    BottomRight As Point
End Type
Public Sub Main()
    Dim r As Rect
    Dim signature As Long
    r.TopLeft_X = 1
    r.TopLeft_Y = 2
    r.BottomRight_X = 3
    r.BottomRight_Y = 4
    signature = r.TopLeft_X + (r.TopLeft_Y * 10) + (r.BottomRight_X * 100) + (r.BottomRight_Y * 1000)
End Sub
"#,
        )],
    );

    assert_vm_contains_and_jit_unavailable(&manifest, Variant::from_i32(4_321));
}

#[test]
fn nested_udt_whole_assignment_copies_declared_field_slots() {
    let manifest = source_project(
        "NestedUdtWholeCopy",
        vec![proc_module(
            "MainModule",
            r#"
Option Explicit
Type Point
    X As Integer
    Y As Integer
End Type
Public Sub Main()
    Dim a As Point
    Dim b As Point
    Dim signature As Long
    a.X = 7
    a.Y = 9
    b = a
    signature = b.X + (b.Y * 10)
End Sub
"#,
        )],
    );

    assert_vm_contains_and_jit_unavailable(&manifest, Variant::from_i32(97));
}

#[test]
fn nested_udt_cross_type_rejection() {
    let manifest = source_project(
        "NestedUdtCrossTypeRejection",
        vec![proc_module(
            "MainModule",
            r#"
Option Explicit
Type PairA
    X As Integer
    Y As Integer
End Type
Type PairB
    X As Integer
    Y As Integer
End Type
Public Sub Main()
    Dim a As PairA
    Dim b As PairB
    b = a
End Sub
"#,
        )],
    );

    let err = Engine::new(HostConfig::default())
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect_err("same-shape cross-type UDT assignment should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().to_ascii_lowercase().contains("udt")
            || err.message().to_ascii_lowercase().contains("type"),
        "unexpected diagnostic: {err}"
    );
}

#[test]
fn e2e_edge_multidim_redim_with_non_default_lower_bounds() {
    let engine = Engine::new(HostConfig::default());
    let manifest = source_project(
        "EdgeArrayBounds",
        vec![proc_module(
            "MainModule",
            r#"
Option Explicit
Public Sub Main()
    Dim result As Long
    Dim grid() As Long
    ReDim grid(3 To 5, 1 To 2)
    grid(3, 1) = 9
    grid(5, 2) = 17
    If grid(3, 1) = 9 And grid(5, 2) = 17 Then
        result = 26
    Else
        result = -1
    End If
End Sub
"#,
        )],
    );

    let snapshot = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("edge array project should execute");
    contains_value(&snapshot, Variant::from_i32(26));
}

#[test]
fn e2e_edge_dynamic_multidim_runtime_redim_runs_on_vm_and_rejects_jit() {
    let manifest = source_project(
        "EdgeDynamicRuntimeArray",
        vec![proc_module(
            "MainModule",
            r#"
Option Explicit
Option Base 1
Public Sub Main()
    Dim result As Long
    Dim grid() As Long
    ReDim grid(2, 2)
    grid(1, 1) = 9
    ReDim Preserve grid(2, 3)
    If grid(1, 1) = 9 And LBound(grid) = 1 And UBound(grid) = 2 Then
        result = 26
    Else
        result = -1
    End If
End Sub
"#,
        )],
    );

    let vm_snapshot = Engine::new(HostConfig { enable_jit: false })
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("vm execution should succeed");

    contains_value(&vm_snapshot, Variant::from_i32(26));
    assert_jit_unavailable(&manifest);
}

#[test]
fn e2e_edge_runtime_policy_denial_routes_through_resume_next() {
    let mut engine = Engine::new(HostConfig::default());
    let mut policy = HostPolicy::deterministic_runtime();
    policy.allow_process_spawn = false;
    engine.set_host_policy(policy);
    let manifest = source_project(
        "EdgeErrRouting",
        vec![proc_module(
            "MainModule",
            r#"
Option Explicit
Public Sub Main()
    Dim x As Long
    Dim marker As Long
    On Error Resume Next
    x = Shell(1)
    If Err.Number <> 0 Then
        marker = 91
    End If
End Sub
"#,
        )],
    );

    let snapshot = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("policy denial path should execute");
    contains_value(&snapshot, Variant::from_i32(91));
}

#[test]
fn e2e_scaling_pressure_large_linear_statement_block_runs_on_vm_and_rejects_jit() {
    let mut source = String::from("Option Explicit\nPublic Sub Main()\nDim x As Long\nx = 0\n");
    let iterations = 4_000usize;
    for _ in 0..iterations {
        source.push_str("x = x + 1\n");
    }
    source.push_str("End Sub\n");

    let manifest = source_project("ScaleLinear", vec![proc_module("MainModule", &source)]);
    let vm_snapshot = Engine::new(HostConfig { enable_jit: false })
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("vm execution should succeed");

    assert_eq!(
        vm_snapshot.first(),
        Some(&Variant::from_i32(iterations as i32)),
        "linear pressure case should converge to increment count"
    );
    assert_jit_unavailable(&manifest);
}

#[test]
fn e2e_scaling_pressure_cross_project_many_modules() {
    let mut ref_modules = Vec::new();
    let mut main_source = String::from("Option Explicit\nPublic Sub Main()\nDim marker As Long\n");
    let module_count = 32usize;
    let mut expected_marker = 0i32;
    for index in 1..=module_count {
        let value = (index as i32) * 3;
        expected_marker = value;
        let module_name = format!("M{index:02}");
        ref_modules.push(proc_module(
            &module_name,
            &format!(
                "Option Explicit\nPublic Function Value() As Long\nValue = {value}\nEnd Function\n"
            ),
        ));
        main_source.push_str(&format!("marker = LibScale.{module_name}.Value()\n"));
    }
    main_source.push_str("End Sub\n");

    let manifest = ProjectManifest {
        project_name: "ScaleMain".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![proc_module("MainModule", &main_source)],
        references: vec![ProjectReference {
            referenced_project_name: "LibScale".to_string(),
            reference_kind: ReferenceKind::Project,
        }],
        reference_projects: vec![ReferencedProjectManifest {
            project_name: "LibScale".to_string(),
            modules: ref_modules,
        }],
        conditional_constants: BTreeMap::new(),
    };

    let snapshot = Engine::new(HostConfig::default())
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("cross-project scaling case should execute");
    contains_value(&snapshot, Variant::from_i32(expected_marker));
}

#[test]
fn e2e_scaling_pressure_many_branches_with_select_case() {
    let mut source = String::from(
        "Option Explicit\nPublic Sub Main()\nDim x As Long\nDim score As Long\nx = 0\nscore = 0\n",
    );
    for _ in 0..250 {
        source.push_str(
            "Select Case x\nCase 0\nscore = score + 3\nCase 1\nscore = score + 7\nCase Else\nscore = score + 11\nEnd Select\n",
        );
        source.push_str("x = x + 1\nIf x = 3 Then\nx = 0\nEnd If\n");
    }
    source.push_str("End Sub\n");

    let manifest = source_project("ScaleSelectCase", vec![proc_module("MainModule", &source)]);
    let snapshot = Engine::new(HostConfig::default())
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("branch-heavy scaling case should execute");
    contains_value(&snapshot, Variant::from_i32(1746));
}
