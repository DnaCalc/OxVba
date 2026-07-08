use oxvba_differential::{
    Executor, RunOutcome, canon, run, run_manifest, run_modules, run_ox_programs,
    run_project_closure,
};
use oxvba_oxir::{OxImage, OxProgram};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleUnit, ProjectKind, ProjectReference, ReferencedProjectManifest,
    SymbolProjectManifest,
};
use oxvba_symbol::surface::{ProjectExportSurface, synthesize_export_surface_from_core_program};

fn assert_match(label: &str, vm3: RunOutcome, jit: RunOutcome) {
    assert!(
        vm3.unsupported.is_none(),
        "{label}: VM3 declined Linux-safe case: {vm3:?}"
    );
    assert!(
        jit.unsupported.is_none(),
        "{label}: JIT declined accepted Linux-safe case: {jit:?}"
    );
    assert_eq!(jit.raised, vm3.raised, "{label}: raised mismatch");
    assert_eq!(jit.err, vm3.err, "{label}: Err mismatch");
    assert_eq!(jit.result, vm3.result, "{label}: snapshot mismatch");
    assert!(
        jit.handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "{label}: JIT handle imbalance {:?}",
        jit.handle_balance
    );
}

fn assert_source_match(label: &str, source: &str) {
    assert_match(
        label,
        run(Executor::Vm3, source),
        run(Executor::Jit, source),
    );
}

fn test_dispatch_typelib_ref() -> ProjectReference {
    let identity = oxvba_com::known_typelib_identity_for_prog_id_name("OxVba.TestDispatch")
        .expect("fixture typelib identity for OxVba.TestDispatch");
    ProjectReference::TypeLibrary {
        name: identity.reference_name,
        guid: identity.libid,
        version_major: Some(identity.major_version),
        version_minor: Some(identity.minor_version),
        lcid: identity.lcid,
        import_lib: Some(identity.importlib),
    }
}

fn com_fixture_manifest() -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "VBAProject".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".to_string(),
            module_kind: Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: include_str!("../benches/fixtures/com_late_vs_early.bas").to_string(),
        }],
        references: vec![test_dispatch_typelib_ref()],
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    }
}

fn module(name: &str, kind: oxvba_symbol::manifest::ModuleKind, source: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: kind,
        attributes: ModuleAttributes::named(name),
        source: source.to_string(),
    }
}

fn exposed_class_module(name: &str, source: &str) -> ModuleUnit {
    let mut module = module(name, Class, source);
    module.attributes.vb_exposed = true;
    module.attributes.vb_creatable = true;
    module
}

fn benchmark_reference_project() -> ReferencedProjectManifest {
    ReferencedProjectManifest {
        project_name: "Lib".to_string(),
        project_kind: ProjectKind::Library,
        modules: vec![exposed_class_module(
            "Box",
            include_str!("../benches/fixtures/referenced_class_aggregates_box.cls"),
        )],
    }
}

fn library_project(reference: &ReferencedProjectManifest) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: reference.project_name.clone(),
        project_kind: ProjectKind::Library,
        modules: reference.modules.clone(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    }
}

fn compiled_reference(manifest: &SymbolProjectManifest) -> (ProjectExportSurface, OxProgram) {
    let core = oxvba_bind::bind_program(manifest, &oxvba_symbol::CatalogTypeLibResolver)
        .expect("reference bind");
    let surface = synthesize_export_surface_from_core_program(&core);
    let ox = oxvba_oxir::elaborate::elaborate(&core).expect("reference elaborate");
    let bytes = OxImage::new(vec![ox])
        .to_bytes()
        .expect("reference image serialize");
    let mut image = OxImage::from_bytes(&bytes).expect("reference image load");
    (
        surface,
        image.programs.pop().expect("single reference program"),
    )
}

fn compile_bundle_only_app(
    app: &SymbolProjectManifest,
    surfaces: &[ProjectExportSurface],
) -> OxProgram {
    let core = oxvba_bind::bind_program_with_project_surfaces(
        app,
        &oxvba_symbol::CatalogTypeLibResolver,
        surfaces,
    )
    .expect("app bind against compiled reference surface");
    oxvba_oxir::elaborate::elaborate(&core).expect("app elaborate")
}

fn referenced_class_fixture_manifest(
    reference: ReferencedProjectManifest,
) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "App".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module(
            "Main",
            Procedural,
            include_str!("../benches/fixtures/referenced_class_aggregates.bas"),
        )],
        references: vec![ProjectReference::Project {
            referenced_project_name: reference.project_name.clone(),
        }],
        reference_projects: vec![reference],
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    }
}

#[test]
fn generated_scalar_arithmetic_and_coercions_match_vm3() {
    let mut seed = 0x5eed_1234_u64;
    for index in 0..32 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = (seed % 997) as i32 - 498;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b = ((seed % 91) as i32 + 1).max(1);
        let source = format!(
            "\
Public r As Long
Sub Main()
  Dim a As Long
  Dim b As Long
  Dim v As Variant
  a = {a}
  b = {b}
  v = CStr(a + b)
  r = CLng(v) + (a \\ b) - (a Mod b)
End Sub
"
        );
        assert_source_match(&format!("generated scalar/coercion {index}"), &source);
    }
}

#[test]
fn generated_loop_array_and_string_cases_match_vm3() {
    for len in [1_i32, 2, 7, 16, 31] {
        let source = format!(
            "\
Public r As Long
Sub Main()
  Dim a() As Long
  Dim i As Long
  Dim s As String
  ReDim a(0 To {len})
  For i = 0 To {len}
    a(i) = (i + 3) Mod 5
    s = CStr(a(i))
  Next i
  For i = 0 To {len}
    r = r + a(i)
  Next i
  r = r + Len(s)
End Sub
"
        );
        assert_source_match(&format!("generated loop/array/string {len}"), &source);
    }
}

#[test]
fn generated_mid_statement_mutation_cases_match_vm3() {
    for (label, args, expected) in [
        ("replace_to_end", "2", "aZZZ"),
        ("replace_count", "2, 2", "aZZd"),
        ("overlarge_count", "3, 10", "abZZ"),
    ] {
        let source = format!(
            "\
Public r As String
Sub Main()
  Dim s As String
  s = \"abcd\"
  Mid(s, {args}) = \"ZZZ\"
  r = s & \":\" & CStr(Len(s))
End Sub
"
        );
        let vm3 = run(Executor::Vm3, &source);
        let jit = run(Executor::Jit, &source);
        assert_match(&format!("generated MidStmt {label}"), vm3, jit.clone());
        assert_eq!(
            jit.result
                .expect("jit MidStmt case should complete")
                .first(),
            Some(&canon(&Variant::from_string(format!("{expected}:4"))))
        );
    }
}

#[test]
fn benchmark_string_concat_fixture_matches_vm3() {
    assert_source_match(
        "benchmark string_concat fixture",
        include_str!("../benches/fixtures/string_concat.bas"),
    );
}

#[test]
fn benchmark_variant_and_project_dispatch_fixtures_match_vm3() {
    assert_source_match(
        "benchmark variant_box_unbox fixture",
        include_str!("../benches/fixtures/variant_box_unbox.bas"),
    );

    let project_modules = [
        (
            "Main",
            Procedural,
            include_str!("../benches/fixtures/project_object_calls.bas"),
        ),
        (
            "Counter",
            Class,
            include_str!("../benches/fixtures/project_object_calls.cls"),
        ),
    ];
    assert_match(
        "benchmark project_object_calls fixture",
        run_modules(Executor::Vm3, &project_modules, "VBAProject"),
        run_modules(Executor::Jit, &project_modules, "VBAProject"),
    );

    let dynamic_modules = [
        (
            "Main",
            Procedural,
            include_str!("../benches/fixtures/dynamic_dispatch_helpers.bas"),
        ),
        (
            "Counter",
            Class,
            include_str!("../benches/fixtures/dynamic_dispatch_helpers.cls"),
        ),
    ];
    assert_match(
        "benchmark dynamic_dispatch_helpers fixture",
        run_modules(Executor::Vm3, &dynamic_modules, "VBAProject"),
        run_modules(Executor::Jit, &dynamic_modules, "VBAProject"),
    );
}

#[test]
fn benchmark_udt_and_collection_fixtures_match_vm3() {
    assert_source_match(
        "benchmark udt_fields fixture",
        include_str!("../benches/fixtures/udt_fields.bas"),
    );
    assert_source_match(
        "benchmark udt_nested_arrays fixture",
        include_str!("../benches/fixtures/udt_nested_arrays.bas"),
    );

    let class_modules = [
        (
            "Main",
            Procedural,
            include_str!("../benches/fixtures/class_field_aggregates.bas"),
        ),
        (
            "Box",
            Class,
            include_str!("../benches/fixtures/class_field_aggregates.cls"),
        ),
    ];
    assert_match(
        "benchmark class_field_aggregates fixture",
        run_modules(Executor::Vm3, &class_modules, "VBAProject"),
        run_modules(Executor::Jit, &class_modules, "VBAProject"),
    );

    let reference = benchmark_reference_project();
    let app = referenced_class_fixture_manifest(reference.clone());
    let lib = library_project(&reference);
    assert_match(
        "benchmark referenced_class_aggregates fixture",
        run_project_closure(Executor::Vm3, &[lib.clone(), app.clone()]),
        run_project_closure(Executor::Jit, &[lib, app]),
    );

    let reference = benchmark_reference_project();
    let lib = library_project(&reference);
    let (surface, lib_program) = compiled_reference(&lib);
    let mut app = referenced_class_fixture_manifest(reference);
    app.reference_projects.clear();
    let app_program = compile_bundle_only_app(&app, &[surface]);
    assert_match(
        "benchmark bundle_only_referenced_class_aggregates fixture",
        run_ox_programs(Executor::Vm3, &[lib_program.clone(), app_program.clone()]),
        run_ox_programs(Executor::Jit, &[lib_program, app_program]),
    );

    assert_source_match(
        "benchmark collection_ops fixture",
        include_str!("../benches/fixtures/collection_ops.bas"),
    );
}

#[test]
fn benchmark_fixture_backed_com_activation_matches_vm3() {
    let manifest = com_fixture_manifest();
    assert_match(
        "benchmark fixture-backed com_late_vs_early fixture",
        run_manifest(Executor::Vm3, &manifest),
        run_manifest(Executor::Jit, &manifest),
    );
}

#[test]
fn generated_simple_calls_and_error_routing_match_vm3() {
    for divisor in [0_i32, 1, 3, 9] {
        let source = format!(
            "\
Public r As Long
Sub Main()
  On Error Resume Next
  r = Twice(21) + CheckedDiv(9, {divisor})
  If Err.Number <> 0 Then
    r = Err.Number
    Err.Clear
  End If
End Sub

Private Function Twice(ByVal value As Long) As Long
  Twice = value * 2
End Function

Private Function CheckedDiv(ByVal left As Long, ByVal right As Long) As Long
  CheckedDiv = left \\ right
End Function
"
        );
        assert_source_match(&format!("generated call/error {divisor}"), &source);
    }
}

#[test]
fn generated_project_object_dispatch_cases_match_vm3() {
    for value in [3_i32, 7, 11] {
        let main = format!(
            "\
Public r As Long
Sub Main()
  Dim c As Object
  Set c = New Calc
  CallByName c, \"SetValue\", vbMethod, {value}
  r = c.Value + CallByName(c, \"Add\", vbMethod, 2, 5)
End Sub
"
        );
        let modules = [
            ("Main", Procedural, main.as_str()),
            (
                "Calc",
                Class,
                "\
Private m As Long
Public Sub SetValue(ByVal value As Long)
  m = value
End Sub
Public Function Add(ByVal a As Long, ByVal b As Long) As Long
  Add = a + b
End Function
Public Property Get Value() As Long
  Value = m
End Property
",
            ),
        ];
        assert_match(
            &format!("generated project-object dispatch {value}"),
            run_modules(Executor::Vm3, &modules, "VBAProject"),
            run_modules(Executor::Jit, &modules, "VBAProject"),
        );
    }
}

#[test]
fn unsupported_scope_declines_are_explicit() {
    let source = "\
Public r As Long
Declare PtrSafe Function NativeGetTickCount Lib \"kernel32\" Alias \"GetTickCount\" () As Long
Sub Main()
  r = NativeGetTickCount()
End Sub
";
    let jit = run(Executor::Jit, source);
    let unsupported = jit
        .unsupported
        .as_deref()
        .expect("native Declare should remain an explicit JIT decline on Linux");
    assert!(
        !unsupported.trim().is_empty(),
        "unsupported reason should be classified"
    );
}

#[test]
fn generated_case_expected_value_smoke() {
    let source = "\
Public r As Long
Sub Main()
  r = 40 + 2
End Sub
";
    let jit = run(Executor::Jit, source);
    assert_eq!(
        jit.result.expect("jit smoke should complete").first(),
        Some(&canon(&Variant::from_i32(42)))
    );
}
