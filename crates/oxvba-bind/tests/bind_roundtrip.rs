//! End-to-end: real VBA source → `oxvba_bind::bind_program` → `oxvba_bundle::linearize`
//! → run on `oxvba-vm2`. This is the "tie the path together" proof — the whole
//! clean pipeline exercised from source text.

use std::collections::BTreeMap;

use oxvba_bind::bind_program;
use oxvba_bundle::coreir::{CoreCallee, CoreProgram, CoreStmt, CoreValue};
use oxvba_bundle::native::NativeImplId;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_hal::HostPolicy;
use oxvba_symbol::manifest::{ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest};
use oxvba_symbol::provider::TypeLibResolver;

struct NullTypeLibs;
impl TypeLibResolver for NullTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        None
    }
}

fn manifest(source: &str) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Mod1".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Mod1"),
            source: source.into(),
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

fn bind(source: &str) -> CoreProgram {
    bind_program(&manifest(source), &NullTypeLibs).expect("bind_program")
}

/// Bind + linearize + run; read `Main`'s first local as a number.
fn run_main_local0(source: &str) -> Option<f64> {
    let program = bind(source);
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    let value = vm.slot(bundle.global_count)?;
    value
        .as_f64()
        .or_else(|| value.as_f32().map(f64::from))
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
        .or_else(|| value.as_currency_scaled_i64().map(|v| v as f64 / 10_000.0))
        .or_else(|| value.as_date_f64())
        .or_else(|| value.as_bool().map(|b| if b { -1.0 } else { 0.0 }))
}

/// Wrap a `Main` body that leaves its result in the first declared local.
fn main_sub(body: &str) -> String {
    format!("Sub Main()\n{body}End Sub\n")
}

// ── Arithmetic / coercion / operators ────────────────────────────────────────

#[test]
fn arithmetic_precedence() {
    assert_eq!(run_main_local0(&main_sub("    Dim r As Long\n    r = 1 + 2 * 3\n")), Some(7.0));
}

#[test]
fn integer_division() {
    assert_eq!(run_main_local0(&main_sub("    Dim r As Long\n    r = 7 \\ 2\n")), Some(3.0));
}

#[test]
fn xor_operator() {
    assert_eq!(run_main_local0(&main_sub("    Dim r As Long\n    r = 6 Xor 3\n")), Some(5.0));
}

#[test]
fn currency_coercion_on_assignment() {
    // 2.5 assigned to a Currency local narrows via the new Currency coercion target.
    assert_eq!(run_main_local0(&main_sub("    Dim r As Currency\n    r = 2.5\n")), Some(2.5));
}

// ── Control flow ─────────────────────────────────────────────────────────────

#[test]
fn if_elseif_else() {
    let body = "    Dim r As Long\n    r = 0\n    If 2 > 3 Then\n        r = 1\n    ElseIf 5 > 4 Then\n        r = 42\n    Else\n        r = 7\n    End If\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(42.0));
}

#[test]
fn for_with_exit_for() {
    let body = "    Dim total As Long\n    Dim i As Long\n    total = 0\n    For i = 1 To 10\n        If i > 5 Then Exit For\n        total = total + i\n    Next\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(15.0));
}

#[test]
fn for_each_over_array() {
    let body = "    Dim total As Long\n    Dim item\n    total = 0\n    For Each item In Array(10, 20, 30)\n        total = total + item\n    Next\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(60.0));
}

#[test]
fn do_while_loop() {
    let body = "    Dim n As Long\n    n = 0\n    Do While n < 10\n        n = n + 1\n    Loop\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(10.0));
}

#[test]
fn select_case_value_list_and_is() {
    let body = "    Dim r As Long\n    Dim x As Long\n    x = 2\n    Select Case x\n        Case 1\n            r = 10\n        Case 2, 3\n            r = 20\n        Case Is > 5\n            r = 30\n        Case Else\n            r = 99\n    End Select\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(20.0));
}

// ── Calls (project Sub ByRef + Function, native) ─────────────────────────────

#[test]
fn project_sub_by_ref_mutates_caller() {
    let src = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(105.0));
}

#[test]
fn project_function_returns_value() {
    let src = "Sub Main()\n    Dim r As Long\n    r = Add(2, 3)\nEnd Sub\n\nFunction Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn native_len() {
    assert_eq!(run_main_local0(&main_sub("    Dim r As Long\n    r = Len(\"hello\")\n")), Some(5.0));
}

// ── Arrays / error-state ─────────────────────────────────────────────────────

#[test]
fn redim_array_set_get() {
    let body = "    Dim r As Long\n    Dim v1\n    ReDim v1(0 To 2)\n    v1(1) = 77\n    r = v1(1)\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(77.0));
}

#[test]
fn on_error_resume_next_err_number() {
    let body = "    Dim r As Long\n    On Error Resume Next\n    Err.Raise 11\n    r = Err.Number\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(11.0));
}

// ── Regression tests for review findings ────────────────────────────────────

#[test]
fn recursive_function_call() {
    // `Fact(n-1)` inside `Fact` must lower to a recursive call, not an index into
    // the return pseudo-variable.
    let src = "Sub Main()\n    Dim r As Long\n    r = Fact(5)\nEnd Sub\n\nFunction Fact(n As Long) As Long\n    If n <= 1 Then\n        Fact = 1\n    Else\n        Fact = n * Fact(n - 1)\n    End If\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(120.0));
}

#[test]
fn named_arguments_reordered() {
    // Named args must be reordered into the parameter positions, not passed in
    // call-site order.
    let src = "Sub Main()\n    Dim r As Long\n    r = Diff(b:=2, a:=10)\nEnd Sub\n\nFunction Diff(a As Long, b As Long) As Long\n    Diff = a - b\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(8.0));
}

#[test]
fn keyword_named_proc_does_not_desync_frames() {
    // `Function Name()` has a keyword name (no plain Ident), but the scanner still
    // gives it a scope. The binder must agree on the proc set so `Compute`'s body
    // binds against its OWN frame (its local), not the keyword proc's frame.
    let src = "Sub Main()\n    Dim r As Long\n    r = Compute()\nEnd Sub\n\nFunction Name(p As Long) As String\n    Name = \"x\"\nEnd Function\n\nFunction Compute() As Long\n    Dim total As Long\n    total = 42\n    Compute = total\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(42.0));
}

#[test]
fn parenthesized_argument_forces_by_val() {
    // A parenthesized argument `Inc((r))` is forced ByVal → caller unchanged
    // (without the parens, `Inc r` would mutate r to 105 via ByRef).
    let src = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc ((r))\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn hex_literal() {
    assert_eq!(run_main_local0(&main_sub("    Dim r As Long\n    r = &H1F\n")), Some(31.0));
}

// ── Objects: classes, New, Me, fields, methods, properties, Set ──────────────

/// A two-module project: a procedural `Main` + one class module. The class
/// module is named so it sorts after `Main`.
fn class_manifest(main_src: &str, class_name: &str, class_src: &str) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![
            ModuleUnit {
                module_name: "Main".into(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes::named("Main"),
                source: main_src.into(),
            },
            ModuleUnit {
                module_name: class_name.into(),
                module_kind: ModuleKind::Class,
                attributes: ModuleAttributes::named(class_name),
                source: class_src.into(),
            },
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

/// Bind + linearize + run a class project; read `Main`'s first local as a number.
fn run_class_main_local0(main_src: &str, class_name: &str, class_src: &str) -> Option<f64> {
    let program = bind_program(&class_manifest(main_src, class_name, class_src), &NullTypeLibs)
        .expect("bind_program");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    let value = vm.slot(bundle.global_count)?;
    value
        .as_f64()
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
}

#[test]
fn new_initialize_field_and_method() {
    // New runs Class_Initialize (sets the instance field); a method reads it.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    r = w.GetValue()\nEnd Sub\n";
    let widget = "Private mValue As Long\n\n\
                  Private Sub Class_Initialize()\n    mValue = 42\nEnd Sub\n\n\
                  Public Function GetValue() As Long\n    GetValue = mValue\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(42.0));
}

#[test]
fn property_get_let_roundtrip() {
    // `w.Value = 10` routes to Property Let; `r = w.Value` to Property Get.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w.Value = 10\n    r = w.Value\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value() As Long\n    Value = mV\nEnd Property\n\n\
                  Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(10.0));
}

#[test]
fn method_mutates_instance_field_across_calls() {
    // Two `c.Inc` statement-calls mutate the same instance's field; Total() reads it.
    let main = "Sub Main()\n    Dim r As Long\n    Dim c As Counter\n    Set c = New Counter\n    c.Inc\n    c.Inc\n    r = c.Total()\nEnd Sub\n";
    let counter = "Private n As Long\n\n\
                   Public Sub Inc()\n    n = n + 1\nEnd Sub\n\n\
                   Public Function Total() As Long\n    Total = n\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Counter", counter), Some(2.0));
}

// ── Events: WithEvents + RaiseEvent routing ──────────────────────────────────

fn multi_manifest(modules: &[(&str, ModuleKind, &str)]) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: modules
            .iter()
            .map(|(name, kind, src)| ModuleUnit {
                module_name: (*name).into(),
                module_kind: *kind,
                attributes: ModuleAttributes::named(*name),
                source: (*src).into(),
            })
            .collect(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

fn run_multi_main_local0(modules: &[(&str, ModuleKind, &str)]) -> Option<f64> {
    let program = bind_program(&multi_manifest(modules), &NullTypeLibs).expect("bind_program");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    let value = vm.slot(bundle.global_count)?;
    value
        .as_f64()
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
}

#[test]
fn withevents_raise_event_routes_to_handler() {
    // `Set k.Watched = s` subscribes the sink; `s.Fire` raises the event, which
    // routes to `Watched_Fired` (run with the sink's Me) and sets `k.Got`.
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    s.Fire\n    r = k.Got\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\nPublic Got As Long\n\n\
                Private Sub Watched_Fired(ByVal v As Long)\n    Got = v\nEnd Sub\n";
    let source = "Public Event Fired(ByVal v As Long)\n\n\
                  Public Sub Fire()\n    RaiseEvent Fired(99)\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(99.0)
    );
}

// ── File I/O (structural — bind emits native ops; not run) ────────────────────

#[test]
fn file_io_lowers_to_native_calls() {
    let src = "Sub Main()\n    Dim f As Long\n    f = FreeFile\n    Open \"x.txt\" For Output As #1\n    Print #1, \"hi\"\n    Close #1\nEnd Sub\n";
    let program = bind(src);
    assert!(
        contains_native(&program, NativeImplId::FilePrint),
        "expected a FilePrint native call in the lowered program"
    );
    assert!(contains_native(&program, NativeImplId::FileOpen));
    assert!(contains_native(&program, NativeImplId::FileClose));
}

fn contains_native(program: &CoreProgram, id: NativeImplId) -> bool {
    program.procs.iter().any(|p| p.body.iter().any(|s| stmt_has_native(s, id)))
}

fn stmt_has_native(stmt: &CoreStmt, id: NativeImplId) -> bool {
    match stmt {
        CoreStmt::Eval(value) => value_has_native(value, id),
        _ => false,
    }
}

fn value_has_native(value: &CoreValue, id: NativeImplId) -> bool {
    matches!(value, CoreValue::Call { callee: CoreCallee::Native(n), .. } if *n == id)
}
