use std::collections::BTreeMap;

use oxvba_differential::{Executor, RunOutcome, canon, run, run_modules, run_project_closure};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference,
    ReferencedProjectManifest, SymbolProjectManifest,
};

fn module(name: &str, kind: ModuleKind, src: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: kind,
        attributes: ModuleAttributes::named(name),
        source: src.to_string(),
    }
}

fn proc_module(name: &str, src: &str) -> ModuleUnit {
    module(name, Procedural, src)
}

fn exposed_class_module(name: &str, src: &str) -> ModuleUnit {
    let mut module = module(name, Class, src);
    module.attributes.vb_exposed = true;
    module.attributes.vb_creatable = true;
    module
}

fn referenced(project_name: &str, modules: Vec<ModuleUnit>) -> ReferencedProjectManifest {
    ReferencedProjectManifest {
        project_name: project_name.to_string(),
        project_kind: ProjectKind::Library,
        modules,
    }
}

fn library_project(reference: &ReferencedProjectManifest) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: reference.project_name.clone(),
        project_kind: ProjectKind::Library,
        modules: reference.modules.clone(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn project_with_refs(
    name: &str,
    modules: Vec<ModuleUnit>,
    refs: Vec<ReferencedProjectManifest>,
) -> SymbolProjectManifest {
    let references = refs
        .iter()
        .map(|reference| ProjectReference::Project {
            referenced_project_name: reference.project_name.clone(),
        })
        .collect();
    SymbolProjectManifest {
        project_name: name.to_string(),
        project_kind: ProjectKind::Source,
        modules,
        references,
        reference_projects: refs,
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn assert_match(label: &str, vm3: RunOutcome, jit: RunOutcome) {
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

fn assert_modules_match(label: &str, modules: &[(&str, ModuleKind, &str)]) {
    assert_match(
        label,
        run_modules(Executor::Vm3, modules, "VBAProject"),
        run_modules(Executor::Jit, modules, "VBAProject"),
    );
}

#[test]
fn jit_udt_fixed_array_field_bounds_and_elements_match_vm3() {
    for (label, source, expected) in [
        (
            "explicit one-based scalar field array",
            "\
Private Type State
    Buses(1 To 2) As Long
End Type
Public r As Variant
Sub Main()
    Dim s As State
    s.Buses(1) = 11
    s.Buses(2) = 22
    r = CStr(LBound(s.Buses)) & \":\" & CStr(UBound(s.Buses)) & \":\" & CStr(s.Buses(1)) & \":\" & CStr(s.Buses(2))
End Sub
",
            "1:2:11:22",
        ),
        (
            "option-base single-bound scalar field array",
            "\
Option Base 1
Private Type State
    Buses(2) As Long
End Type
Public r As Variant
Sub Main()
    Dim s As State
    s.Buses(1) = 11
    s.Buses(2) = 22
    r = CStr(LBound(s.Buses)) & \":\" & CStr(UBound(s.Buses)) & \":\" & CStr(s.Buses(1)) & \":\" & CStr(s.Buses(2))
End Sub
",
            "1:2:11:22",
        ),
        (
            "negative-bound scalar field array",
            "\
Private Type State
    Buses(-2 To 0) As Long
End Type
Public r As Variant
Sub Main()
    Dim s As State
    s.Buses(-2) = 7
    s.Buses(0) = 9
    r = CStr(LBound(s.Buses)) & \":\" & CStr(UBound(s.Buses)) & \":\" & CStr(s.Buses(-2)) & \":\" & CStr(s.Buses(0))
End Sub
",
            "-2:0:7:9",
        ),
        (
            "multidimensional scalar field array",
            "\
Private Type State
    Grid(1 To 2, 3 To 4) As Long
End Type
Public r As Variant
Sub Main()
    Dim s As State
    s.Grid(1, 3) = 13
    s.Grid(2, 4) = 24
    r = CStr(LBound(s.Grid, 1)) & \":\" & CStr(UBound(s.Grid, 1)) & \":\" & CStr(LBound(s.Grid, 2)) & \":\" & CStr(UBound(s.Grid, 2)) & \":\" & CStr(s.Grid(1, 3)) & \":\" & CStr(s.Grid(2, 4))
End Sub
",
            "1:2:3:4:13:24",
        ),
    ] {
        let vm3 = run(Executor::Vm3, source);
        let jit = run(Executor::Jit, source);
        assert_match(label, vm3, jit.clone());
        assert_eq!(
            jit.result.expect("JIT run should complete").first(),
            Some(&canon(&Variant::from_string(expected.to_string()))),
            "{label}"
        );
    }
}

#[test]
fn jit_nested_udt_and_fixed_array_elements_match_vm3() {
    assert_source_match(
        "nested scalar UDT field plus fixed-array of UDT elements",
        "\
Private Type Cell
    Value As Long
End Type
Private Type Row
    Primary As Cell
    Cells(1 To 2) As Cell
End Type
Public r As Variant
Sub Main()
    Dim row As Row
    row.Primary.Value = 5
    row.Cells(1).Value = row.Primary.Value + 7
    row.Cells(2).Value = row.Cells(1).Value + 11
    r = CStr(row.Primary.Value) & \":\" & CStr(row.Cells(1).Value) & \":\" & CStr(row.Cells(2).Value)
End Sub
",
    );
}

#[test]
fn jit_dynamic_arrays_of_udt_elements_match_vm3() {
    assert_source_match(
        "dynamic UDT array element field get/set",
        "\
Private Type Cell
    Value As Long
End Type
Public r As Variant
Sub Main()
    Dim cells() As Cell
    ReDim cells(0 To 2)
    cells(0).Value = 3
    cells(2).Value = cells(0).Value + 9
    r = CStr(cells(0).Value) & \":\" & CStr(cells(1).Value) & \":\" & CStr(cells(2).Value)
End Sub
",
    );
}

#[test]
fn jit_dynamic_udt_arrays_with_nested_fixed_arrays_match_vm3() {
    assert_source_match(
        "dynamic UDT array elements containing fixed-array fields",
        "\
Private Type Cell
    Values(1 To 2) As Long
End Type
Public r As Variant
Sub Main()
    Dim cells() As Cell
    ReDim cells(-1 To 1)
    cells(-1).Values(1) = 5
    cells(-1).Values(2) = cells(-1).Values(1) + 7
    cells(1).Values(1) = cells(-1).Values(2) + 11
    cells(1).Values(2) = cells(1).Values(1) + 13
    r = CStr(LBound(cells)) & \":\" & CStr(UBound(cells)) & \":\" & CStr(cells(-1).Values(1)) & \":\" & CStr(cells(-1).Values(2)) & \":\" & CStr(cells(1).Values(1)) & \":\" & CStr(cells(1).Values(2))
End Sub
",
    );
}

#[test]
fn jit_deeply_nested_udt_fixed_arrays_match_vm3() {
    assert_source_match(
        "nested fixed arrays of nested UDTs",
        "\
Private Type Leaf
    Value As Long
End Type
Private Type Branch
    Leaves(0 To 1) As Leaf
End Type
Private Type Root
    Branches(1 To 2) As Branch
End Type
Public r As Variant
Sub Main()
    Dim root As Root
    root.Branches(1).Leaves(0).Value = 3
    root.Branches(1).Leaves(1).Value = root.Branches(1).Leaves(0).Value + 4
    root.Branches(2).Leaves(0).Value = root.Branches(1).Leaves(1).Value + 5
    root.Branches(2).Leaves(1).Value = root.Branches(2).Leaves(0).Value + 6
    r = CStr(root.Branches(1).Leaves(0).Value) & \":\" & CStr(root.Branches(1).Leaves(1).Value) & \":\" & CStr(root.Branches(2).Leaves(0).Value) & \":\" & CStr(root.Branches(2).Leaves(1).Value)
End Sub
",
    );
}

#[test]
fn jit_compound_array_erase_and_redim_preserve_match_vm3() {
    assert_source_match(
        "UDT fixed-array field erase resets inline storage",
        "\
Private Type State
    Values(1 To 2) As Long
    Names(1 To 2) As String
End Type
Public r As Variant
Sub Main()
    Dim s As State
    s.Values(1) = 9
    s.Values(2) = 11
    s.Names(1) = \"left\"
    s.Names(2) = \"right\"
    Erase s.Values
    Erase s.Names
    r = CStr(s.Values(1)) & \":\" & CStr(s.Values(2)) & \":\" & s.Names(1) & \":\" & s.Names(2)
End Sub
",
    );

    assert_modules_match(
        "class field dynamic array preserve and erase",
        &[
            (
                "Main",
                Procedural,
                "\
Public r As Variant
Public erasedErr As Long
Sub Main()
    Dim box As Box
    Set box = New Box
    box.Fill
    r = box.Snapshot()
    erasedErr = box.ErasedReadErr()
End Sub
",
            ),
            (
                "Box",
                Class,
                "\
Private values() As Long
Public Sub Fill()
    ReDim values(0 To 1)
    values(0) = 5
    values(1) = 7
    ReDim Preserve values(0 To 2)
    values(2) = values(1) + 9
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(LBound(values)) & \":\" & CStr(UBound(values)) & \":\" & CStr(values(0)) & \":\" & CStr(values(1)) & \":\" & CStr(values(2))
End Function
Public Function ErasedReadErr() As Long
    Erase values
    On Error Resume Next
    Dim ignored As Long
    ignored = values(0)
    ErasedReadErr = Err.Number
End Function
",
            ),
        ],
    );
}

#[test]
fn jit_udt_lset_overlays_fixed_storage_match_vm3() {
    assert_source_match(
        "UDT LSet fixed string and integer overlay",
        "\
Private Type A
    X As String * 2
    N As Integer
End Type
Private Type B
    X As String * 2
    N As Integer
End Type
Public r As Variant
Sub Main()
    Dim a As A
    Dim b As B
    b.X = \"xy\"
    b.N = 513
    LSet a = b
    r = \"|\" & a.X & \"|:\" & CStr(a.N)
End Sub
",
    );
    assert_source_match(
        "UDT LSet fixed byte array overlay",
        "\
Private Type A
    B(0 To 3) As Byte
End Type
Private Type B
    L As Long
End Type
Public r As Variant
Sub Main()
    Dim a As A
    Dim b As B
    b.L = &H4030201
    LSet a = b
    r = CStr(a.B(0)) & \":\" & CStr(a.B(1)) & \":\" & CStr(a.B(2)) & \":\" & CStr(a.B(3))
End Sub
",
    );
}

#[test]
fn jit_class_fixed_array_field_and_with_receiver_match_vm3() {
    let main = "\
Public r As Variant
Sub Main()
    Dim box As Box
    Set box = New Box
    box.Fill
    With box
        r = .Snapshot()
    End With
End Sub
";
    let class = "\
Private values(1 To 2) As Long
Public Sub Fill()
    values(1) = 31
    values(2) = values(1) + 4
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(LBound(values)) & \":\" & CStr(UBound(values)) & \":\" & CStr(values(1)) & \":\" & CStr(values(2))
End Function
";
    assert_modules_match(
        "class fixed-array field access through With receiver",
        &[("Main", Procedural, main), ("Box", Class, class)],
    );
}

#[test]
fn jit_class_field_object_arrays_match_vm3() {
    let main = "\
Public r As Variant
Sub Main()
    Dim box As Box
    Set box = New Box
    box.Fill
    r = box.Snapshot()
End Sub
";
    let box_class = "\
Private kids() As Child
Public Sub Fill()
    ReDim kids(0 To 1)
    Set kids(0) = New Child
    Set kids(1) = New Child
    kids(0).Value = 17
    kids(1).Value = kids(0).Value + 19
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(LBound(kids)) & \":\" & CStr(UBound(kids)) & \":\" & CStr(kids(0).Value) & \":\" & CStr(kids(1).Value)
End Function
";
    let child_class = "\
Private m As Long
Public Property Get Value() As Long
    Value = m
End Property
Public Property Let Value(ByVal v As Long)
    m = v
End Property
";
    assert_modules_match(
        "class field dynamic object array",
        &[
            ("Main", Procedural, main),
            ("Box", Class, box_class),
            ("Child", Class, child_class),
        ],
    );
}

#[test]
fn jit_class_field_dynamic_array_elements_match_vm3() {
    let main = "\
Public r As Long
Public first As Long
Public last As Long
Sub Main()
    Dim o As Thing
    Set o = New Thing
    o.Fill 8
    r = o.Sum()
    first = o.At(0)
    last = o.At(7)
End Sub
";
    let class = "\
Private m() As Long
Private cnt As Long
Public Sub Fill(ByVal n As Long)
    cnt = n
    ReDim m(0 To cnt - 1)
    Dim i As Long
    For i = 0 To cnt - 1
        m(i) = i * 2
    Next i
End Sub
Public Function Sum() As Long
    Dim i As Long
    For i = 0 To cnt - 1
        Sum = Sum + m(i)
    Next i
End Function
Public Function At(ByVal i As Long) As Long
    At = m(i)
End Function
";
    assert_modules_match(
        "class field dynamic Long array",
        &[("Main", Procedural, main), ("Thing", Class, class)],
    );
}

#[test]
fn jit_class_field_udt_arrays_with_nested_fixed_arrays_match_vm3() {
    let main = "\
Public r As Variant
Sub Main()
    Dim box As Box
    Set box = New Box
    box.Fill
    r = box.Snapshot()
End Sub
";
    let class = "\
Private Type Cell
    Values(1 To 2) As Long
End Type
Private cells() As Cell
Public Sub Fill()
    ReDim cells(0 To 1)
    cells(0).Values(1) = 2
    cells(0).Values(2) = cells(0).Values(1) + 3
    cells(1).Values(1) = cells(0).Values(2) + 5
    cells(1).Values(2) = cells(1).Values(1) + 7
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(cells(0).Values(1)) & \":\" & CStr(cells(0).Values(2)) & \":\" & CStr(cells(1).Values(1)) & \":\" & CStr(cells(1).Values(2))
End Function
";
    assert_modules_match(
        "class field dynamic UDT array elements containing fixed arrays",
        &[("Main", Procedural, main), ("Box", Class, class)],
    );
}

#[test]
fn jit_class_field_udt_arrays_and_nested_records_match_vm3() {
    let main = "\
Public r As Variant
Sub Main()
    Dim box As Box
    Set box = New Box
    box.Fill
    r = box.Snapshot()
End Sub
";
    let class = "\
Private Type Cell
    Value As Long
End Type
Private Type State
    Seed As Cell
    Cells(0 To 1) As Cell
End Type
Private state As State
Private dynamicCells() As Cell
Public Sub Fill()
    state.Seed.Value = 10
    state.Cells(0).Value = state.Seed.Value + 1
    state.Cells(1).Value = state.Cells(0).Value + 1
    ReDim dynamicCells(0 To 1)
    dynamicCells(0).Value = state.Cells(1).Value + 1
    dynamicCells(1).Value = dynamicCells(0).Value + 1
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(state.Seed.Value) & \":\" & CStr(state.Cells(0).Value) & \":\" & CStr(state.Cells(1).Value) & \":\" & CStr(dynamicCells(0).Value) & \":\" & CStr(dynamicCells(1).Value)
End Function
";
    assert_modules_match(
        "class field nested UDT and dynamic UDT array",
        &[("Main", Procedural, main), ("Box", Class, class)],
    );
}

#[test]
fn jit_referenced_project_class_with_receiver_and_object_array_match_vm3() {
    let lib = referenced(
        "Lib",
        vec![
            exposed_class_module(
                "Box",
                "\
Private kids() As Child
Public Sub Fill(ByVal base As Long)
    ReDim kids(1 To 2)
    Set kids(1) = New Child
    Set kids(2) = New Child
    kids(1).Value = base
    kids(2).Value = kids(1).Value + 8
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(LBound(kids)) & \":\" & CStr(UBound(kids)) & \":\" & CStr(kids(1).Value) & \":\" & CStr(kids(2).Value)
End Function
",
            ),
            exposed_class_module(
                "Child",
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
    let app = project_with_refs(
        "App",
        vec![proc_module(
            "Main",
            "\
Public r As Variant
Sub Main()
    Dim box As Lib.Box
    Set box = New Lib.Box
    With box
        .Fill 90
        r = .Snapshot()
    End With
End Sub
",
        )],
        vec![lib.clone()],
    );
    let lib_project = library_project(&lib);

    assert_match(
        "referenced project class With receiver and object array",
        run_project_closure(Executor::Vm3, &[lib_project.clone(), app.clone()]),
        run_project_closure(Executor::Jit, &[lib_project, app]),
    );
}

#[test]
fn jit_referenced_project_class_aggregate_fields_match_vm3() {
    let lib = referenced(
        "Lib",
        vec![exposed_class_module(
            "Box",
            "\
Private Type Cell
    Value As Long
End Type
Private cells() As Cell
Public Sub Fill(ByVal base As Long)
    ReDim cells(1 To 2)
    cells(1).Value = base
    cells(2).Value = cells(1).Value + 5
End Sub
Public Function Snapshot() As String
    Snapshot = CStr(LBound(cells)) & \":\" & CStr(UBound(cells)) & \":\" & CStr(cells(1).Value) & \":\" & CStr(cells(2).Value)
End Function
",
        )],
    );
    let app = project_with_refs(
        "App",
        vec![proc_module(
            "Main",
            "\
Public r As Variant
Sub Main()
    Dim box As Lib.Box
    Set box = New Lib.Box
    box.Fill 40
    r = box.Snapshot()
End Sub
",
        )],
        vec![lib.clone()],
    );
    let lib_project = library_project(&lib);

    assert_match(
        "referenced project class aggregate fields",
        run_project_closure(Executor::Vm3, &[lib_project.clone(), app.clone()]),
        run_project_closure(Executor::Jit, &[lib_project, app]),
    );
}
