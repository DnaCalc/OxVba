use oxvba_differential::{Executor, run, run_modules};
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};
use oxvba_oxir::{OxImage, OxProgram};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
};
use oxvba_symbol::surface::synthesize_export_surface_from_core_program;

fn status(label: &str, unsupported: Option<&str>, raised: bool, result_ok: bool) -> String {
    let state = if unsupported.is_some() {
        "declined"
    } else if raised {
        "raised"
    } else if result_ok {
        "compiled"
    } else {
        "failed"
    };
    match unsupported {
        Some(reason) => format!("{label}\t{state}\t{reason}"),
        None => format!("{label}\t{state}"),
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

fn jit_status_for_programs(label: &str, programs: &[OxProgram]) -> String {
    let refs: Vec<&OxProgram> = programs.iter().collect();
    let compiled = match oxvba_jit::JitEngine.compile_image(&refs) {
        Ok(compiled) => compiled,
        Err(err) if err.unsupported_message().is_some() => {
            return status(label, err.unsupported_message(), false, true);
        }
        Err(err) => return status(label, None, false, Err::<(), _>(err).is_ok()),
    };
    let host = Engine::new(HostConfig::vm3())
        .with_runtime_profile(RuntimeProfileId::WindowsHeadless)
        .host_services();
    match compiled.run(&*host) {
        Ok(outcome) => status(label, None, outcome.raised, !outcome.raised),
        Err(_) => status(label, None, false, false),
    }
}

fn bundle_only_reference_fixture_programs() -> Vec<OxProgram> {
    let lib = SymbolProjectManifest {
        project_name: "Lib".to_string(),
        project_kind: ProjectKind::Library,
        modules: vec![exposed_class_module(
            "Box",
            include_str!("../benches/fixtures/referenced_class_aggregates_box.cls"),
        )],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    };
    let lib_core = oxvba_bind::bind_program(&lib, &oxvba_symbol::CatalogTypeLibResolver)
        .expect("bundle reference bind");
    let surface = synthesize_export_surface_from_core_program(&lib_core);
    let lib_program =
        oxvba_oxir::elaborate::elaborate(&lib_core).expect("bundle reference elaborate");
    let bytes = OxImage::new(vec![lib_program])
        .to_bytes()
        .expect("reference image serialize");
    let mut image = OxImage::from_bytes(&bytes).expect("reference image load");
    let lib_program = image.programs.pop().expect("single reference program");

    let app = SymbolProjectManifest {
        project_name: "App".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module(
            "Main",
            Procedural,
            include_str!("../benches/fixtures/referenced_class_aggregates.bas"),
        )],
        references: vec![ProjectReference::Project {
            referenced_project_name: "Lib".to_string(),
        }],
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    };
    let app_core = oxvba_bind::bind_program_with_project_surfaces(
        &app,
        &oxvba_symbol::CatalogTypeLibResolver,
        &[surface],
    )
    .expect("app bind against bundle reference surface");
    let app_program = oxvba_oxir::elaborate::elaborate(&app_core).expect("app elaborate");
    vec![lib_program, app_program]
}

#[test]
fn linux_safe_jit_scope_snapshot() {
    let mut rows = Vec::new();
    for (label, source) in [
        (
            "scalar/checked_long_loop",
            "\
Public r As Long
Sub Main()
  Dim i As Long
  For i = 1 To 10
    r = r + i
  Next i
End Sub
",
        ),
        (
            "coercion/variant_string_long",
            "\
Public r As Long
Sub Main()
  Dim v As Variant
  v = CStr(41)
  r = CLng(v) + 1
End Sub
",
        ),
        (
            "arrays/dynamic_long_loop",
            "\
Public r As Long
Sub Main()
  Dim a() As Long
  Dim i As Long
  ReDim a(0 To 3)
  For i = 0 To 3
    a(i) = i + 1
    r = r + a(i)
  Next i
End Sub
",
        ),
        (
            "records/nested_udt_arrays",
            include_str!("../benches/fixtures/udt_nested_arrays.bas"),
        ),
        (
            "strings/mid_mutation_boundary",
            "\
Public r As Long
Sub Main()
  Dim s As String
  s = Space(3)
  Mid(s, 2, 1) = \"x\"
  r = Len(s)
End Sub
",
        ),
        (
            "error/resume_next_div_zero",
            "\
Public r As Long
Sub Main()
  On Error Resume Next
  r = 1 \\ 0
  r = Err.Number
End Sub
",
        ),
        (
            "unsupported/native_declare",
            "\
Public r As Long
Declare PtrSafe Function NativeGetTickCount Lib \"kernel32\" Alias \"GetTickCount\" () As Long
Sub Main()
  r = NativeGetTickCount()
End Sub
",
        ),
    ] {
        let outcome = run(Executor::Jit, source);
        rows.push(status(
            label,
            outcome.unsupported.as_deref(),
            outcome.raised,
            outcome.result.is_ok(),
        ));
    }

    let modules = [
        (
            "Main",
            Procedural,
            "\
Public r As Long
Sub Main()
  Dim c As Object
  Set c = New Calc
  r = CallByName(c, \"Add\", vbMethod, 2, 4)
End Sub
",
        ),
        (
            "Calc",
            Class,
            "\
Public Function Add(ByVal a As Long, ByVal b As Long) As Long
  Add = a + b
End Function
",
        ),
    ];
    let outcome = run_modules(Executor::Jit, &modules, "VBAProject");
    rows.push(status(
        "project_object/call_by_name_method",
        outcome.unsupported.as_deref(),
        outcome.raised,
        outcome.result.is_ok(),
    ));

    let modules = [
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
    let outcome = run_modules(Executor::Jit, &modules, "VBAProject");
    rows.push(status(
        "project_object/class_field_aggregates",
        outcome.unsupported.as_deref(),
        outcome.raised,
        outcome.result.is_ok(),
    ));

    let programs = bundle_only_reference_fixture_programs();
    rows.push(jit_status_for_programs(
        "project_reference/bundle_only_class_aggregates",
        &programs,
    ));

    let actual = format!("{}\n", rows.join("\n"));
    let expected = include_str!("../jit_linux_safe_scope.snap");
    assert_eq!(actual, expected);
}
