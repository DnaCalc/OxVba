use std::collections::BTreeMap;

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
    ReferencedProjectManifest, module_unit_from_source,
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, compat::RuntimeValueCompatEngineExt};
use oxvba_runtime::compat::RuntimeValue;

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

fn contains_value(snapshot: &[RuntimeValue], expected: RuntimeValue) {
    assert!(
        snapshot.contains(&expected),
        "expected snapshot to contain {:?}, got {:?}",
        expected,
        snapshot
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
        .execute_project_with_snapshot_phased(&manifest)
        .expect("edge array project should execute");
    contains_value(&snapshot, RuntimeValue::I32(26));
}

#[test]
fn e2e_edge_dynamic_multidim_runtime_redim_vm_jit_parity() {
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

    let vm = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let jit = Engine::new(HostConfig {
        enable_jit: true,
        root_object_name: None,
    });

    let vm_snapshot = vm
        .execute_project_with_snapshot_phased(&manifest)
        .expect("vm execution should succeed");
    let jit_snapshot = jit
        .execute_project_with_snapshot_phased(&manifest)
        .expect("jit execution should succeed");

    assert_eq!(
        vm_snapshot, jit_snapshot,
        "vm/jit snapshots diverged on dynamic multidimensional runtime arrays"
    );
    contains_value(&vm_snapshot, RuntimeValue::I32(26));
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
        .execute_project_with_snapshot_phased(&manifest)
        .expect("policy denial path should execute");
    contains_value(&snapshot, RuntimeValue::I32(91));
}

#[test]
fn e2e_scaling_pressure_large_linear_statement_block_vm_jit_parity() {
    let mut source = String::from("Option Explicit\nPublic Sub Main()\nDim x As Long\nx = 0\n");
    let iterations = 4_000usize;
    for _ in 0..iterations {
        source.push_str("x = x + 1\n");
    }
    source.push_str("End Sub\n");

    let manifest = source_project("ScaleLinear", vec![proc_module("MainModule", &source)]);
    let vm = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let jit = Engine::new(HostConfig {
        enable_jit: true,
        root_object_name: None,
    });

    let vm_snapshot = vm
        .execute_project_with_snapshot_phased(&manifest)
        .expect("vm execution should succeed");
    let jit_snapshot = jit
        .execute_project_with_snapshot_phased(&manifest)
        .expect("jit execution should succeed");

    assert_eq!(
        vm_snapshot, jit_snapshot,
        "vm/jit snapshots diverged under large linear pressure"
    );
    assert_eq!(
        vm_snapshot.first(),
        Some(&RuntimeValue::I32(iterations as i32)),
        "linear pressure case should converge to increment count"
    );
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
        .execute_project_with_snapshot_phased(&manifest)
        .expect("cross-project scaling case should execute");
    contains_value(&snapshot, RuntimeValue::I32(expected_marker));
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
        .execute_project_with_snapshot_phased(&manifest)
        .expect("branch-heavy scaling case should execute");
    contains_value(&snapshot, RuntimeValue::I32(1746));
}
