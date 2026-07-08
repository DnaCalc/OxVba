use std::collections::BTreeMap;

use oxvba_differential::{Executor, run_ox_programs};
use oxvba_oxir::{OxImage, OxProgram};
use oxvba_symbol::CatalogTypeLibResolver;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
};
use oxvba_symbol::surface::{ProjectExportSurface, synthesize_export_surface_from_core_program};

fn module(name: &str, kind: ModuleKind, source: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: kind,
        attributes: ModuleAttributes::named(name),
        source: source.to_string(),
    }
}

fn exposed_class_module(name: &str, source: &str) -> ModuleUnit {
    let mut module = module(name, ModuleKind::Class, source);
    module.attributes.vb_exposed = true;
    module.attributes.vb_creatable = true;
    module
}

fn project(name: &str, modules: Vec<ModuleUnit>) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: name.to_string(),
        project_kind: ProjectKind::Source,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn app_project(source: &str) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "App".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module("Main", ModuleKind::Procedural, source)],
        references: vec![ProjectReference::Project {
            referenced_project_name: "Lib".to_string(),
        }],
        // This is the point of the test: the referenced project's source is not
        // present in the app manifest. Binding uses only the compiled surface.
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn compiled_reference(manifest: &SymbolProjectManifest) -> (ProjectExportSurface, OxProgram) {
    let core = oxvba_bind::bind_program(manifest, &CatalogTypeLibResolver).expect("reference bind");
    let surface = synthesize_export_surface_from_core_program(&core);
    let ox = oxvba_oxir::elaborate::elaborate(&core).expect("reference elaborate");
    let bytes = OxImage::new(vec![ox])
        .to_bytes()
        .expect("serialize ox image");
    let mut image = OxImage::from_bytes(&bytes).expect("deserialize ox image");
    let program = image.programs.pop().expect("single referenced program");
    (surface, program)
}

fn compile_bundle_only_app(
    app: &SymbolProjectManifest,
    surfaces: &[ProjectExportSurface],
) -> OxProgram {
    let core =
        oxvba_bind::bind_program_with_project_surfaces(app, &CatalogTypeLibResolver, surfaces)
            .expect("app bind against compiled reference surfaces");
    oxvba_oxir::elaborate::elaborate(&core).expect("app elaborate")
}

fn assert_bundle_only_match(label: &str, programs: &[OxProgram]) {
    let vm3 = run_ox_programs(Executor::Vm3, programs);
    let jit = run_ox_programs(Executor::Jit, programs);
    assert!(
        vm3.unsupported.is_none(),
        "{label}: VM3 declined case: {vm3:?}"
    );
    assert!(
        jit.unsupported.is_none(),
        "{label}: JIT declined case: {jit:?}"
    );
    assert_eq!(jit.raised, vm3.raised, "{label}: raised mismatch");
    assert_eq!(jit.err, vm3.err, "{label}: Err mismatch");
    assert_eq!(jit.result, vm3.result, "{label}: snapshot mismatch");
    assert!(
        vm3.handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "{label}: VM3 handle imbalance {:?}",
        vm3.handle_balance
    );
    assert!(
        jit.handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "{label}: JIT handle imbalance {:?}",
        jit.handle_balance
    );
}

#[test]
fn jit_bundle_only_referenced_class_aggregates_match_vm3() {
    let lib = project(
        "Lib",
        vec![
            exposed_class_module(
                "Box",
                "\
Private Type Cell
    Values(1 To 2) As Long
End Type
Private cells() As Cell
Private kids() As Child
Public Sub Fill(ByVal seed As Long)
    ReDim cells(0 To 1)
    cells(0).Values(1) = seed
    cells(0).Values(2) = cells(0).Values(1) + 3
    cells(1).Values(1) = cells(0).Values(2) + 5
    cells(1).Values(2) = cells(1).Values(1) + 7
    ReDim kids(1 To 2)
    Set kids(1) = New Child
    Set kids(2) = New Child
    kids(1).Value = cells(1).Values(2) + 11
    kids(2).Value = kids(1).Value + 13
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(cells(0).Values(1)) & \":\" & CStr(cells(0).Values(2)) & \":\" & CStr(cells(1).Values(1)) & \":\" & CStr(cells(1).Values(2)) & \":\" & CStr(kids(1).Value) & \":\" & CStr(kids(2).Value)
End Function
",
            ),
            module(
                "Child",
                ModuleKind::Class,
                "\
Private m As Long
Public Property Get Value() As Long
    Value = m
End Property
Public Property Let Value(ByVal v As Long)
    m = v
End Property
",
            ),
        ],
    );
    let (surface, lib_program) = compiled_reference(&lib);
    let app = app_project(
        "\
Public r As Variant
Sub Main()
    Dim box As Lib.Box
    Set box = New Lib.Box
    box.Fill 10
    r = box.Snapshot()
End Sub
",
    );
    assert!(
        app.reference_projects.is_empty(),
        "app fixture must not carry referenced source"
    );
    let app_program = compile_bundle_only_app(&app, &[surface]);
    assert_bundle_only_match(
        "bundle-only referenced class aggregate",
        &[lib_program, app_program],
    );
}

#[test]
fn jit_bundle_only_referenced_module_function_matches_vm3() {
    let lib = project(
        "Lib",
        vec![module(
            "LibMath",
            ModuleKind::Procedural,
            "\
Public Function Accum(ByVal n As Long) As Long
    Dim i As Long
    For i = 1 To n
        Accum = Accum + i
    Next i
End Function
",
        )],
    );
    let (surface, lib_program) = compiled_reference(&lib);
    let app = app_project(
        "\
Public r As Long
Sub Main()
    r = Lib.LibMath.Accum(8)
End Sub
",
    );
    let app_program = compile_bundle_only_app(&app, &[surface]);
    assert_bundle_only_match(
        "bundle-only referenced module function",
        &[lib_program, app_program],
    );
}
