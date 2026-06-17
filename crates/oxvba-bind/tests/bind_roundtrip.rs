//! End-to-end: real VBA source → `oxvba_bind::bind_program` → `oxvba_bundle::linearize`
//! → run on `oxvba-vm2`. This is the "tie the path together" proof — the whole
//! clean pipeline exercised from source text.

use std::collections::BTreeMap;

use oxvba_bind::bind_program;
use oxvba_bundle::coreir::{CoreArg, CoreCallee, CoreProgram, CoreStmt, CoreValue};
use oxvba_bundle::native::NativeImplId;
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
};
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

fn bind_error(source: &str) -> String {
    format!(
        "{:?}",
        bind_program(&manifest(source), &NullTypeLibs).expect_err("bind should fail")
    )
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
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = 1 + 2 * 3\n")),
        Some(7.0)
    );
}

fn run_main_local0_string(source: &str) -> Option<String> {
    let program = bind(source);
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    vm.slot(bundle.global_count)?.as_bstr().map(|b| b.as_str())
}

#[test]
fn fixed_length_string_pads_on_assignment() {
    // `Dim s As String * 5` pads a shorter assignment with spaces.
    assert_eq!(
        run_main_local0_string("Sub Main()\n    Dim s As String * 5\n    s = \"ab\"\nEnd Sub\n"),
        Some("ab   ".to_string())
    );
}

#[test]
fn fixed_length_string_truncates_on_assignment() {
    assert_eq!(
        run_main_local0_string("Sub Main()\n    Dim s As String * 2\n    s = \"abcd\"\nEnd Sub\n"),
        Some("ab".to_string())
    );
}

#[test]
fn date_literal_assigns_serial() {
    // 2020-01-01 is OLE automation serial 43831.
    assert_eq!(
        run_main_local0(&main_sub("    Dim d As Date\n    d = #1/1/2020#\n")),
        Some(43831.0)
    );
}

#[test]
fn iif_selects_branch() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = IIf(2 > 1, 10, 20)\n")),
        Some(10.0)
    );
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = IIf(1 > 2, 10, 20)\n")),
        Some(20.0)
    );
}

#[test]
fn choose_is_one_based() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = Choose(2, 100, 200, 300)\n")),
        Some(200.0)
    );
}

#[test]
fn switch_returns_first_true() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r\n    r = Switch(False, 1, True, 42)\n")),
        Some(42.0)
    );
}

#[test]
fn const_references_const() {
    let body = "    Dim r As Long\n    Const A As Long = 1\n    Const B As Long = A + 1\n    Const C As Long = B * 2\n    r = C\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(4.0));
}

#[test]
fn const_forward_reference_resolves() {
    // C depends on B depends on A, all declared after the use site.
    let body = "    Dim r As Long\n    Const C As Long = B * 2\n    Const B As Long = A + 1\n    Const A As Long = 1\n    r = C\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(4.0));
}

#[test]
fn const_cycle_is_error() {
    let body = "    Const A As Long = B\n    Const B As Long = A\n    Dim r As Long\n    r = A\n";
    let src = main_sub(body);
    assert!(bind_program(&manifest(&src), &NullTypeLibs).is_err());
}

#[test]
fn module_qualified_call() {
    // `Mod1.SetIt` — a standard-module-qualified call (the form the project startup
    // shim emits as `Call Module.Proc()`). The module name is a namespace qualifier,
    // not a value; resolve the member as a qualified project call.
    let src = "Sub Main()\n    Dim r As Long\n    Call Mod1.SetIt\n    r = total\nEnd Sub\n\
               Public total As Long\n\
               Sub SetIt()\n    total = 7\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(7.0));
}

#[test]
fn module_qualified_const_and_variable() {
    // `Mod1.K` (a qualified Public Const) and `Mod1.gShared` (a qualified module
    // variable) resolve as a value / a place — not a call (regression for the
    // qualified-member path that previously routed everything through a call).
    let src = "Sub Main()\n    Dim r As Long\n    gShared = 5\n    r = Mod1.K + Mod1.gShared\nEnd Sub\n\
               Public Const K As Long = 10\n\
               Public gShared As Long\n";
    assert_eq!(run_main_local0(src), Some(15.0));
}

#[test]
fn integer_division() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = 7 \\ 2\n")),
        Some(3.0)
    );
}

#[test]
fn xor_operator() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = 6 Xor 3\n")),
        Some(5.0)
    );
}

#[test]
fn currency_coercion_on_assignment() {
    // 2.5 assigned to a Currency local narrows via the new Currency coercion target.
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Currency\n    r = 2.5\n")),
        Some(2.5)
    );
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
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = Len(\"hello\")\n")),
        Some(5.0)
    );
}

// ── Arrays / error-state ─────────────────────────────────────────────────────

#[test]
fn paramarray_sums_trailing_args() {
    let src = "Sub Main()\n    Dim r As Long\n    r = SumAll(10, 20, 30)\nEnd Sub\n\n\
               Function SumAll(ParamArray xs() As Variant) As Long\n\
               Dim i As Long\n    Dim t As Long\n    t = 0\n\
               For i = LBound(xs) To UBound(xs)\n        t = t + xs(i)\n    Next\n\
               SumAll = t\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(60.0));
}

#[test]
fn paramarray_empty_is_zero_length() {
    let src = "Sub Main()\n    Dim r As Long\n    r = CountAll()\nEnd Sub\n\n\
               Function CountAll(ParamArray xs() As Variant) As Long\n\
               CountAll = UBound(xs) - LBound(xs) + 1\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(0.0));
}

#[test]
fn paramarray_mixed_fixed_and_variadic() {
    let src = "Sub Main()\n    Dim r As Long\n    r = AddBase(100, 1, 2, 3)\nEnd Sub\n\n\
               Function AddBase(seed As Long, ParamArray xs() As Variant) As Long\n\
               Dim i As Long\n    Dim t As Long\n    t = seed\n\
               For i = LBound(xs) To UBound(xs)\n        t = t + xs(i)\n    Next\n\
               AddBase = t\nEnd Function\n";
    assert_eq!(run_main_local0(src), Some(106.0));
}

#[test]
fn random_access_file_statements_bind_and_lower() {
    // Get/Put/Seek/Width/Lock/Unlock/Name previously errored in the binder; they
    // now route to native impls (Get lowers as an assignment of the read value).
    let src = "Sub Main()\n\
        Dim num As Integer\n    num = 1\n\
        Dim v As Long\n    v = 7\n\
        Put #num, 1, v\n\
        Get #num, 1, v\n\
        Seek #num, 1\n\
        Width #num, 80\n\
        Lock #num\n\
        Unlock #num\n\
        Name \"a\" As \"b\"\n\
    End Sub\n";
    let program = bind(src);
    assert!(oxvba_bundle::linearize(&program).is_ok());
}

#[test]
fn declare_byval_string_lvalue_binds_byref_for_ansi_writeback() {
    // VBA marshals a `Declare` String argument through a system-codepage ANSI
    // buffer and converts the (possibly callee-mutated) buffer back into the
    // variable after the call — even when the parameter is declared `ByVal` (the
    // pre-sized-buffer idiom). The call site must therefore bind a String-typed
    // l-value argument ByRef so the marshaled-back value reaches the variable,
    // while an r-value (here a literal) stays ByVal with no write-back target.
    let program = bind(
        "Private Declare PtrSafe Function lstrcpyA Lib \"kernel32\" (ByVal dst As String, ByVal src As String) As LongPtr\n\
         Sub Main()\n    Dim buffer As String\n    lstrcpyA buffer, \"alpha\"\nEnd Sub\n",
    );
    let entry = program.entry.expect("entry");
    let args = program.procs[entry.0]
        .body
        .iter()
        .find_map(|stmt| match stmt {
            CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::Declare { .. },
                args,
            }) => Some(args.clone()),
            _ => None,
        })
        .expect("the Declare call should lower in Main's body");
    assert!(
        matches!(args[0], CoreArg::ByRef(_)),
        "a String l-value to a ByVal String param must bind ByRef (ANSI write-back), got {:?}",
        args[0]
    );
    assert!(
        matches!(args[1], CoreArg::ByVal(_)),
        "a literal String argument must stay ByVal, got {:?}",
        args[1]
    );
}

#[test]
fn kill_statement_routes_to_vba_filesystem() {
    // `Kill pathname` is not a lexer keyword (unlike Open/Close/Print#/Name/…), so it
    // parses as an ordinary statement-call and resolves by name. Since P4 it routes
    // cross-bundle to the `VBA` unit's `FileSystem.Kill` member (an `ExternProc` call),
    // exactly like the by-name file functions — not the bespoke `Native` route.
    let program = bind("Sub Main()\n    Kill \"scratch.tmp\"\nEnd Sub\n");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    assert!(
        imports_vba_filesystem(&bundle, "Kill"),
        "`Kill` should import VBA/FileSystem.Kill: {:?}",
        bundle.imports
    );
}

/// True if the linearized `bundle` imports a `VBA`/`FileSystem` `ModuleFunc` named
/// `member` (the cross-bundle link a `FileSystem` call lowers to).
fn imports_vba_filesystem(bundle: &oxvba_bundle::Bundle, member: &str) -> bool {
    bundle.imports.iter().any(|imp| {
        imp.unit.eq_ignore_ascii_case("VBA")
            && matches!(
                &imp.token,
                oxvba_bundle::ExportToken::ModuleFunc { module, member: m, .. }
                    if module.eq_ignore_ascii_case("FileSystem")
                        && m.eq_ignore_ascii_case(member)
            )
    })
}

#[test]
fn random_file_put_get_round_trips_through_vm() {
    // End-to-end: Open Random with Len=8, Put a Long at record 2, Get it back —
    // exercising the mode + Len plumbing through the standard host's in-memory file.
    let src = "Sub Main()\n\
        Dim r As Long\n    Dim v As Long\n    v = 222\n\
        Open \"rec.dat\" For Random As #1 Len = 8\n\
        Put #1, 2, v\n\
        Get #1, 2, r\n\
        Close #1\n\
    End Sub\n";
    let program = bind(src);
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = oxvba_hal::adapters::builder::HostBuilder::new()
        .profile(oxvba_hal::HalProfileId::Windows)
        .policy(oxvba_hal::HostPolicy {
            allow_filesystem_mutation: true,
            ..oxvba_hal::HostPolicy::default()
        })
        .build();
    let vm = oxvba_vm2::run(&bundle, host.as_ref()).expect("run");
    assert_eq!(
        vm.slot(bundle.global_count).and_then(|v| v.as_i32()),
        Some(222)
    );
}

/// Run a single-module source on the standard (in-memory) host; read `Main`'s
/// first local as a string.
fn run_main_local0_string_std(src: &str) -> Option<String> {
    let program = bind(src);
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = oxvba_hal::adapters::builder::HostBuilder::new()
        .profile(oxvba_hal::HalProfileId::Windows)
        .policy(oxvba_hal::HostPolicy {
            allow_filesystem_mutation: true,
            ..oxvba_hal::HostPolicy::default()
        })
        .build();
    let vm = oxvba_vm2::run(&bundle, host.as_ref()).expect("run");
    vm.slot(bundle.global_count)?.as_bstr().map(|b| b.as_str())
}

#[test]
fn fixed_length_string_random_record_round_trips() {
    // String * 4 is written raw (no prefix, padded to 4) and read back as 4 chars.
    let src = "Sub Main()\n\
        Dim r As String * 4\n    Dim v As String * 4\n    v = \"ab\"\n\
        Open \"f.dat\" For Random As #1 Len = 4\n\
        Put #1, 1, v\n    Get #1, 1, r\n    Close #1\n\
    End Sub\n";
    assert_eq!(run_main_local0_string_std(src), Some("ab  ".to_string()));
}

#[test]
fn binary_variable_string_reads_current_length() {
    // Binary `Get` reads `Len(r)` bytes; r is pre-sized to 5 spaces, so it reads
    // the 5 bytes previously written.
    let src = "Sub Main()\n\
        Dim r As String\n\
        Open \"f.dat\" For Binary As #1\n\
        Put #1, 1, \"hello\"\n    r = \"     \"\n    Get #1, 1, r\n    Close #1\n\
    End Sub\n";
    assert_eq!(run_main_local0_string_std(src), Some("hello".to_string()));
}

#[test]
fn addressof_binds_to_proc_ref() {
    let src = "Sub Main()\n    Dim p As Long\n    p = AddressOf Helper\nEnd Sub\n\nSub Helper()\nEnd Sub\n";
    let program = bind(src);
    let main = program
        .procs
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("Main"))
        .unwrap();
    // The proc-ref is stored into a `Long`, so the store coerces it to `Long`
    // (a no-op for an integer pointer); look through that `Coerce` wrapper.
    fn is_addressof(v: &CoreValue) -> bool {
        match v {
            CoreValue::AddressOf(_) => true,
            CoreValue::Coerce { value, .. } => is_addressof(value),
            _ => false,
        }
    }
    assert!(
        main.body
            .iter()
            .any(|s| matches!(s, CoreStmt::Assign { value, .. } if is_addressof(value)))
    );
}

#[test]
fn addressof_runs_as_integer() {
    // The proc reference materializes as an integer (round-trips through a slot).
    let src = "Sub Main()\n    Dim r As Long\n    r = AddressOf Helper\nEnd Sub\n\nSub Helper()\nEnd Sub\n";
    assert!(run_main_local0(src).is_some());
}

#[test]
fn addressof_unknown_is_error() {
    let src = "Sub Main()\n    Dim p As Long\n    p = AddressOf Nope\nEnd Sub\n";
    assert!(bind_program(&manifest(src), &NullTypeLibs).is_err());
}

#[test]
fn ubound_lbound_of_local_array() {
    let body = "    Dim r As Long\n    Dim v\n    ReDim v(2 To 9)\n    r = UBound(v) - LBound(v)\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(7.0));
}

#[test]
fn redim_array_set_get() {
    let body =
        "    Dim r As Long\n    Dim v1\n    ReDim v1(0 To 2)\n    v1(1) = 77\n    r = v1(1)\n";
    assert_eq!(run_main_local0(&main_sub(body)), Some(77.0));
}

#[test]
fn on_error_resume_next_err_number() {
    let body =
        "    Dim r As Long\n    On Error Resume Next\n    Err.Raise 11\n    r = Err.Number\n";
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
fn duplicate_named_argument_is_bind_error() {
    let src = "Sub Main()\n    Dim x As Long\n    x = 1\n    Call Fill(x, target := x)\nEnd Sub\n\nSub Fill(ByRef target As Long, ByVal value As Long)\n    target = value\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("duplicate argument for parameter target"),
        "unexpected error: {err}"
    );
}

#[test]
fn positional_after_named_argument_is_bind_error() {
    let src = "Sub Main()\n    Dim x As Long\n    x = 1\n    Call Fill(value := 9, x)\nEnd Sub\n\nSub Fill(ByRef target As Long, ByVal value As Long)\n    target = value\nEnd Sub\n";
    let err = bind_error(src);
    assert!(
        err.contains("positional argument cannot follow named argument"),
        "unexpected error: {err}"
    );
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
fn call_site_byval_forces_by_val_over_byref_param() {
    // A call-site `ByVal r` overrides the callee's declared `ByRef`, so the
    // mutation must NOT reach the caller's `r`. (The parser accepted the
    // `ByVal`/`ByRef` modifier but dropped it, so this used to mutate r to 105.)
    let src = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc ByVal r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(5.0));
}

#[test]
fn call_site_byref_keeps_write_back() {
    // The control: `Inc r` (no modifier) and an explicit `Inc ByRef r` both
    // write back through the ByRef param.
    let bare = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    let explicit = "Sub Main()\n    Dim r As Long\n    r = 5\n    Inc ByRef r\nEnd Sub\n\nSub Inc(ByRef n As Long)\n    n = n + 100\nEnd Sub\n";
    assert_eq!(run_main_local0(bare), Some(105.0));
    assert_eq!(run_main_local0(explicit), Some(105.0));
}

#[test]
fn hex_literal() {
    assert_eq!(
        run_main_local0(&main_sub("    Dim r As Long\n    r = &H1F\n")),
        Some(31.0)
    );
}

// ── Long tail ────────────────────────────────────────────────────────────────

#[test]
fn module_and_proc_consts_fold() {
    // A module-level Const and a proc-level Const both substitute their values.
    let src = "Const FACTOR As Long = 10\n\n\
               Sub Main()\n    Dim r As Long\n    Const ADD = 5\n    r = FACTOR * 2 + ADD\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(25.0));
}

#[test]
fn option_compare_text_is_case_insensitive() {
    // Under `Option Compare Text`, `"A" = "a"` is True (-1); Binary would give 0.
    let src = "Option Compare Text\n\nSub Main()\n    Dim r As Boolean\n    r = (\"A\" = \"a\")\nEnd Sub\n";
    assert_eq!(run_main_local0(src), Some(-1.0));
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
    let program = bind_program(
        &class_manifest(main_src, class_name, class_src),
        &NullTypeLibs,
    )
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
fn indexed_property_get_let_roundtrip() {
    // `w.Value(3) = 10` is an indexed Property Let, not an array-element write
    // through a synthesized helper. The setter receives index args followed by
    // the assigned value, and the getter remains an indexed Property Get.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w.Value(3) = 10\n    r = w.Value(2)\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value(ByVal i As Long) As Long\n    Value = mV + i\nEnd Property\n\n\
                  Public Property Let Value(ByVal i As Long, ByVal v As Long)\n    mV = v + i\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(15.0));
}

#[test]
fn named_indexed_property_let_roundtrip() {
    // Named index arguments on an indexed Property Let are reordered by the
    // accessor signature before the assigned value is placed in the trailing slot.
    let main = "Sub Main()\n    Dim r As Long\n    Dim w As Widget\n    Set w = New Widget\n    w.Value(i := 3) = 10\n    r = w.Value(2)\nEnd Sub\n";
    let widget = "Private mV As Long\n\n\
                  Public Property Get Value(ByVal i As Long) As Long\n    Value = mV + i\nEnd Property\n\n\
                  Public Property Let Value(ByVal i As Long, ByVal v As Long)\n    mV = v + i\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Widget", widget), Some(15.0));
}

#[test]
fn indexed_property_set_roundtrip() {
    // `Set b.Item(3) = t` must select the indexed Property Set accessor, carrying
    // the index argument before the object value argument.
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Dim t As Thing\n    Dim got As Thing\n    Set b = New Box\n    Set t = New Thing\n    Set b.Item(3) = t\n    Set got = b.Item(2)\n    r = got.GetVal()\nEnd Sub\n";
    let box_cls = "Private stored As Thing\n\n\
                   Public Property Get Item(ByVal i As Long) As Thing\n    Set Item = stored\nEnd Property\n\n\
                   Public Property Set Item(ByVal i As Long, ByVal v As Thing)\n    Set stored = v\nEnd Property\n";
    let thing = "Public Function GetVal() As Long\n    GetVal = 23\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, box_cls),
            ("Thing", ModuleKind::Class, thing),
        ]),
        Some(23.0)
    );
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

#[test]
fn project_method_byref_mutates_caller() {
    // o.Inc r passes r ByRef to a method; the method's write propagates back.
    // (The headline fix: method dispatch used to force ByVal.)
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As C\n    r = 5\n    Set o = New C\n    o.Inc r\nEnd Sub\n";
    let class_c = "Public Sub Inc(ByRef x As Long)\n    x = x + 100\nEnd Sub\n";
    assert_eq!(run_class_main_local0(main, "C", class_c), Some(105.0));
}

#[test]
fn byref_object_reassign_through_proc() {
    // `Set p = New Thing` through a ByRef object param rewrites the caller's
    // variable (release-old/retain-new), so `o` is a live Thing afterwards.
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As Thing\n    MakeThing o\n    r = o.GetVal()\nEnd Sub\n\n\
                Sub MakeThing(ByRef p As Thing)\n    Set p = New Thing\nEnd Sub\n";
    let thing = "Private mVal As Long\n\n\
                 Private Sub Class_Initialize()\n    mVal = 7\nEnd Sub\n\n\
                 Public Function GetVal() As Long\n    GetVal = mVal\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Thing", thing), Some(7.0));
}

#[test]
fn redim_member_array_then_use() {
    // ReDim a class instance's array field through a dotted target, then write and
    // read an element of it (the resized array must be stored back into the field).
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Set b = New Box\n    ReDim b.arr(5)\n    b.arr(2) = 7\n    r = b.arr(2)\nEnd Sub\n";
    let box_cls = "Public arr() As Long\n";
    assert_eq!(run_class_main_local0(main, "Box", box_cls), Some(7.0));
}

#[test]
fn callbyname_invokes_method() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As Calc\n    Set o = New Calc\n    r = CallByName(o, \"Add\", vbMethod, 2, 3)\nEnd Sub\n";
    let calc = "Public Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
    assert_eq!(run_class_main_local0(main, "Calc", calc), Some(5.0));
}

#[test]
fn callbyname_property_let_then_get() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim ignore\n    Dim o As Box\n    Set o = New Box\n    ignore = CallByName(o, \"Value\", vbLet, 42)\n    r = CallByName(o, \"Value\", vbGet)\nEnd Sub\n";
    let box_cls = "Private mV As Long\n\n\
                   Public Property Get Value() As Long\n    Value = mV\nEnd Property\n\n\
                   Public Property Let Value(ByVal v As Long)\n    mV = v\nEnd Property\n";
    assert_eq!(run_class_main_local0(main, "Box", box_cls), Some(42.0));
}

#[test]
fn callbyname_unknown_member_errors() {
    let main = "Sub Main()\n    Dim r\n    Dim o As Calc\n    Set o = New Calc\n    r = CallByName(o, \"Nope\", vbMethod)\nEnd Sub\n";
    let calc = "Public Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
    let program = bind_program(&class_manifest(main, "Calc", calc), &NullTypeLibs).expect("bind");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    assert!(oxvba_vm2::run(&bundle, &host).is_err());
}

// ── Implements: interface dispatch + TypeOf + strict Set ─────────────────────

const IANIMAL: &str = "Public Function Speak() As Long\nEnd Function\n";
const CDOG: &str = "Implements IAnimal\n\nPrivate Function IAnimal_Speak() As Long\n    IAnimal_Speak = 42\nEnd Function\n";

#[test]
fn implements_dispatch_through_interface_var() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As IAnimal\n    Set a = New CDog\n    r = a.Speak()\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CDog", ModuleKind::Class, CDOG),
        ]),
        Some(42.0)
    );
}

#[test]
fn implements_typeof_true_for_implementer() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim a As IAnimal\n    Set a = New CDog\n    If TypeOf a Is IAnimal Then\n        r = 1\n    Else\n        r = 0\n    End If\nEnd Sub\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CDog", ModuleKind::Class, CDOG),
        ]),
        Some(1.0)
    );
}

#[test]
fn as_new_auto_instantiates_a_user_class() {
    // `Dim t As New Thing` auto-instantiates `t` at scope entry — not Collection-
    // specific: any project class works. Without the fix `t` was `Nothing` (and was
    // mis-typed `Object("new")` by the parser-absorbs-`New` bug), so `t.GetVal()`
    // would fault.
    let main =
        "Sub Main()\n    Dim r As Long\n    Dim t As New Thing\n    r = t.GetVal()\nEnd Sub\n";
    let thing = "Public Function GetVal() As Long\n    GetVal = 7\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Thing", ModuleKind::Class, thing),
        ]),
        Some(7.0)
    );
}

#[test]
fn implements_typeof_false_for_non_implementer() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim o As Object\n    Set o = New CRock\n    If TypeOf o Is IAnimal Then\n        r = 1\n    Else\n        r = 0\n    End If\nEnd Sub\n";
    let crock = "Public Function Foo() As Long\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CRock", ModuleKind::Class, crock),
        ]),
        Some(0.0)
    );
}

#[test]
fn implements_set_type_mismatch_errors() {
    // Set into an IAnimal-typed var with a class that does not implement it → error 13.
    let main = "Sub Main()\n    Dim a As IAnimal\n    Set a = New CRock\nEnd Sub\n";
    let crock = "Public Function Foo() As Long\nEnd Function\n";
    let program = bind_program(
        &multi_manifest(&[
            ("Main", ModuleKind::Procedural, main),
            ("IAnimal", ModuleKind::Class, IANIMAL),
            ("CRock", ModuleKind::Class, crock),
        ]),
        &NullTypeLibs,
    )
    .expect("bind");
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    assert!(oxvba_vm2::run(&bundle, &host).is_err());
}

#[test]
fn implements_missing_member_is_bind_error() {
    // CDog declares Implements IAnimal but omits IAnimal_Speak → bind error.
    let main = "Sub Main()\nEnd Sub\n";
    let cdog = "Implements IAnimal\n";
    assert!(
        bind_program(
            &multi_manifest(&[
                ("Main", ModuleKind::Procedural, main),
                ("IAnimal", ModuleKind::Class, IANIMAL),
                ("CDog", ModuleKind::Class, cdog),
            ]),
            &NullTypeLibs,
        )
        .is_err()
    );
}

#[test]
fn implements_property_through_interface_var() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim s As IShape\n    Set s = New CBox\n    s.Size = 10\n    r = s.Size\nEnd Sub\n";
    let ishape = "Public Property Get Size() As Long\nEnd Property\n\n\
                  Public Property Let Size(ByVal v As Long)\nEnd Property\n";
    let cbox = "Implements IShape\n\nPrivate mS As Long\n\n\
                Private Property Get IShape_Size() As Long\n    IShape_Size = mS\nEnd Property\n\n\
                Private Property Let IShape_Size(ByVal v As Long)\n    mS = v\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IShape", ModuleKind::Class, ishape),
            ("CBox", ModuleKind::Class, cbox),
        ]),
        Some(10.0)
    );
}

#[test]
fn implements_indexed_property_through_interface_var() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim s As IShape\n    Set s = New CBox\n    s.Size(3) = 10\n    r = s.Size(2)\nEnd Sub\n";
    let ishape = "Public Property Get Size(ByVal i As Long) As Long\nEnd Property\n\n\
                  Public Property Let Size(ByVal i As Long, ByVal v As Long)\nEnd Property\n";
    let cbox = "Implements IShape\n\nPrivate mS As Long\n\n\
                Private Property Get IShape_Size(ByVal i As Long) As Long\n    IShape_Size = mS + i\nEnd Property\n\n\
                Private Property Let IShape_Size(ByVal i As Long, ByVal v As Long)\n    mS = v + i\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("IShape", ModuleKind::Class, ishape),
            ("CBox", ModuleKind::Class, cbox),
        ]),
        Some(15.0)
    );
}

// ── New of a COM coclass (lowers to CreateObject activation) ─────────────────

struct WidgetTypeLibs;
impl TypeLibResolver for WidgetTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        Some(oxvba_com::TypeLibMetadataBlob {
            identity: oxvba_com::TypeLibResolvedIdentity {
                reference_name: "Widget".into(),
                requested_coclass: None,
                importlib: "widget".into(),
                libid: None,
                major_version: 1,
                minor_version: 0,
                lcid: None,
                cache_key: "widget".into(),
            },
            activation_prog_id: Some("Widget.Thing".into()),
            member_name_to_token: Vec::new(),
            members: Vec::new(),
            events: Vec::new(),
            coclass_names: Vec::new(),
        })
    }
}

#[test]
fn new_com_coclass_lowers_to_create_object() {
    let main = "Sub Main()\n    Dim x As Widget\n    Set x = New Widget\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "Widget".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: None,
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let program = bind_program(&manifest, &WidgetTypeLibs).expect("bind_program");
    // `New Widget` resolves the coclass to its ProgID and lowers to CreateObject.
    assert!(format!("{program:?}").contains("CreateObject"));
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

#[test]
fn raise_event_byref_param_writes_back_to_raiser() {
    // VBA event parameters default to ByRef, and RaiseEvent is synchronous, so a
    // handler that assigns to a ByRef parameter mutates the raiser's variable
    // after RaiseEvent returns. (RaiseEvent args used to bind without the event
    // signature, defaulting every argument to ByVal and dropping the write-back.)
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    r = s.FireWith(7)\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\n\n\
                Private Sub Watched_Poked(v As Long)\n    v = 1234\nEnd Sub\n";
    let source = "Public Event Poked(v As Long)\n\n\
                  Public Function FireWith(ByVal start As Long) As Long\n    Dim x As Long\n    x = start\n    RaiseEvent Poked(x)\n    FireWith = x\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(1234.0),
        "the handler's write to its ByRef parameter must reach the raiser's variable"
    );
}

#[test]
fn raise_event_byval_param_does_not_write_back() {
    // The event declares its parameter ByVal, so a handler that assigns to it
    // must mutate a copy — never the raiser's variable. (RaiseEvent args used
    // to bind without the event's signature, so an l-value argument took the
    // ByRef default and the handler's write corrupted the raiser's local.)
    let main = "Sub Main()\n    Dim r As Long\n    Dim k As Sink\n    Dim s As Source\n    Set s = New Source\n    Set k = New Sink\n    Set k.Watched = s\n    r = s.FireWith(7)\nEnd Sub\n";
    let sink = "Public WithEvents Watched As Source\n\n\
                Private Sub Watched_Poked(ByVal v As Long)\n    v = 1234\nEnd Sub\n";
    let source = "Public Event Poked(ByVal v As Long)\n\n\
                  Public Function FireWith(ByVal start As Long) As Long\n    Dim x As Long\n    x = start\n    RaiseEvent Poked(x)\n    FireWith = x\nEnd Function\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Sink", ModuleKind::Class, sink),
            ("Source", ModuleKind::Class, source),
        ]),
        Some(7.0),
        "the handler's write to its ByVal parameter must not reach the raiser"
    );
}

// ── Class-module UDT fields: per-instance default record-init ────────────────

#[test]
fn class_udt_field_initializes_and_round_trips() {
    // A class module with a scalar UDT field (`Private p As Point`) must bind
    // without faulting `"is not a variable"` (the field resolves through `Me`,
    // unreachable from the entry proc's frame — its default record-init is
    // emitted per-instance into `Class_Initialize`). At runtime the field is a
    // default record, so writing/reading a sub-field round-trips.
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Set b = New Box\n\
                \x20   b.SetX 42\n    r = b.GetX\nEnd Sub\n";
    let class = "Private Type Point\n  X As Long\n  Y As Long\nEnd Type\n\
                 Private p As Point\n\n\
                 Private Sub Class_Initialize()\n    p.Y = 0\nEnd Sub\n\n\
                 Public Sub SetX(ByVal v As Long)\n    p.X = v\nEnd Sub\n\n\
                 Public Property Get GetX() As Long\n    GetX = p.X\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, class),
        ]),
        Some(42.0),
        "a class scalar-UDT field is a default record, so its sub-field round-trips"
    );
}

#[test]
fn class_with_udt_field_and_no_class_initialize_binds() {
    // The same shape with *no* `Class_Initialize`: binding must still SKIP the
    // class field as a bundle global (no `"is not a variable"`). This guards the
    // bind half — the runtime materialization is the gap below.
    let main = "Sub Main()\n    Dim b As Box\n    Set b = New Box\nEnd Sub\n";
    let class = "Private Type Point\n  X As Long\nEnd Type\n\
                 Private p As Point\n\n\
                 Public Sub SetX(ByVal v As Long)\n    p.X = v\nEnd Sub\n";
    // Binds + links + runs (Main never touches the field) without panicking.
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, class),
        ]),
        None
    );
}

// KNOWN GAP: a class scalar-UDT field whose class has NO `Class_Initialize` is
// never default-record-initialized (the per-instance record-init is emitted
// into the `Class_Initialize` prologue; with no such proc there is nowhere to
// emit it). Writing a sub-field then faults type 13 ("record expected"). The
// general fix is to SYNTHESIZE a `Class_Initialize` (a new ProcId wired as the
// class's initialize) when a class with UDT fields has none — a class-build-seam
// change deferred here. ChibiEx HAS a `Class_Initialize`, so this gap does not
// affect the acceptance test.
#[test]
#[ignore = "no-Class_Initialize UDT-field record-init: synthesize-prologue path deferred"]
fn class_udt_field_without_class_initialize_round_trips() {
    let main = "Sub Main()\n    Dim r As Long\n    Dim b As Box\n    Set b = New Box\n\
                \x20   b.SetX 7\n    r = b.GetX\nEnd Sub\n";
    let class = "Private Type Point\n  X As Long\nEnd Type\n\
                 Private p As Point\n\n\
                 Public Sub SetX(ByVal v As Long)\n    p.X = v\nEnd Sub\n\n\
                 Public Property Get GetX() As Long\n    GetX = p.X\nEnd Property\n";
    assert_eq!(
        run_multi_main_local0(&[
            ("Main", ModuleKind::Procedural, main),
            ("Box", ModuleKind::Class, class),
        ]),
        Some(7.0)
    );
}

// ── COM-source WithEvents: binder emits EventRoutes for a typelib coclass ────

/// Resolve the fixture `OxVba.TestEventServer` typelib through the real fixture
/// catalog (enabled by the `fixture-typelibs` dev-dep), so the bind test exercises
/// the genuine event classification (OnSimpleEvent/OnValueChanged/OnPairChanged,
/// all `Dispatch` path) rather than a hand-rolled blob.
struct EventServerTypeLibs;
impl TypeLibResolver for EventServerTypeLibs {
    fn resolve(
        &self,
        request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        let identity = oxvba_com::resolve_known_typelib_identity(request)?;
        Some(oxvba_com::build_typelib_metadata(&identity))
    }
}

#[test]
fn withevents_com_source_emits_event_route() {
    // A `WithEvents` field typed as a referenced COM coclass plus a matching
    // `<field>_<EventName>` handler must produce an `EventRoute` bound to that
    // handler, keyed on the event's dispid/token (the runtime subscribe key).
    let main = "Private WithEvents x As OxVba.TestEventServer\n\n\
                Private Sub x_OnValueChanged(ByVal v As Long)\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Class,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "OxVba_TestEventServer".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("oxvba_testeventserver.tlb".into()),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let program = bind_program(&manifest, &EventServerTypeLibs).expect("bind_program");

    // OnValueChanged is event token 2 in the fixture typelib; exactly one route is
    // emitted (only OnValueChanged has a matching handler) and it targets the
    // handler proc, keyed on that token.
    assert_eq!(
        program.event_routes.len(),
        1,
        "exactly one EventRoute (only OnValueChanged has a handler): {:?}",
        program.event_routes
    );
    let route = &program.event_routes[0];
    assert_eq!(
        route.event, 2,
        "route keyed on the OnValueChanged dispid/token"
    );
    let handler = &program.procs[route.handler];
    assert!(
        handler.name.eq_ignore_ascii_case("x_OnValueChanged"),
        "route targets the x_OnValueChanged handler, got: {}",
        handler.name
    );
}

#[test]
fn withevents_com_source_without_handler_emits_no_route() {
    // A COM-source `WithEvents` field with no matching handler emits no route
    // (nothing to subscribe).
    let main = "Private WithEvents x As OxVba.TestEventServer\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Class,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "OxVba_TestEventServer".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("oxvba_testeventserver.tlb".into()),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let program = bind_program(&manifest, &EventServerTypeLibs).expect("bind_program");
    assert!(
        program.event_routes.is_empty(),
        "no handler ⇒ no EventRoute: {:?}",
        program.event_routes
    );
}

// ── COM early binding: typed receiver lowers a member call to EarlyCom{dispid} ──

#[test]
fn typed_com_receiver_member_call_lowers_to_early_com() {
    // A receiver typed as a referenced COM coclass (`Dim s As OxVba.TestEventServer`)
    // resolves its member against the real fixture typelib, so `s.Ping()` lowers to an
    // early-bound dispatch keyed on Ping's dispid (104 in the fixture) — NOT a
    // by-name LateDispatch (the untyped-Object default). This proves the whole
    // early-bind lowering against a genuine typelib blob, headlessly.
    let main = "Sub Main()\n    Dim s As OxVba.TestEventServer\n    Dim r As Long\n    r = s.Ping()\nEnd Sub\n";
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: main.into(),
        }],
        references: vec![ProjectReference::TypeLibrary {
            name: "OxVba_TestEventServer".into(),
            guid: None,
            version_major: Some(1),
            version_minor: Some(0),
            lcid: None,
            import_lib: Some("oxvba_testeventserver.tlb".into()),
        }],
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    };
    let program = bind_program(&manifest, &EventServerTypeLibs).expect("bind_program");

    // Exactly one early-bound COM call, on Ping's dispid; and no late dispatch to Ping
    // (that would mean the typed receiver was treated as an untyped Object).
    let callees = top_level_callees(&program);
    assert!(
        callees
            .iter()
            .any(|c| matches!(c, CoreCallee::EarlyCom { dispid: 104, .. })),
        "expected an EarlyCom on Ping's dispid (104), got: {callees:?}"
    );
    assert!(
        !callees.iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch { name, .. } if name.eq_ignore_ascii_case("Ping")
        )),
        "Ping on a typed receiver must NOT lower to LateDispatch: {callees:?}"
    );
}

// ── COM late dispatch + Declare (structural — emit-correct, not run) ─────────

/// The callee of a value, unwrapping a `Coerce` wrapper (an assignment to a typed
/// target coerces the call result).
fn callee_of(value: &CoreValue) -> Option<&CoreCallee> {
    match value {
        CoreValue::Call { callee, .. } => Some(callee),
        CoreValue::Coerce { value, .. } => callee_of(value),
        _ => None,
    }
}

/// The callees of top-level `Assign`/`Eval` calls in every procedure.
fn top_level_callees(program: &CoreProgram) -> Vec<&CoreCallee> {
    let mut out = Vec::new();
    for proc in &program.procs {
        for stmt in &proc.body {
            let value = match stmt {
                CoreStmt::Assign { value, .. } => Some(value),
                CoreStmt::Eval(value) => Some(value),
                _ => None,
            };
            if let Some(callee) = value.and_then(callee_of) {
                out.push(callee);
            }
        }
    }
    out
}

#[test]
fn late_bound_member_call_on_object() {
    // A member call on an untyped `Object` (no typelib) becomes a by-name late
    // dispatch — the VBA late-binding default.
    let src = "Sub Main()\n    Dim o As Object\n    Dim r\n    r = o.Compute(1)\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program).iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch { name, .. } if name.eq_ignore_ascii_case("Compute")
        )),
        "expected a LateDispatch to Compute"
    );
}

#[test]
fn late_bound_property_put_on_object() {
    // `o.Value = 5` on an untyped Object becomes a late-bound Property Let put.
    let src = "Sub Main()\n    Dim o As Object\n    o.Value = 5\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program).iter().any(|c| matches!(
            c,
            CoreCallee::LateDispatch { name, kind: Some(oxvba_bundle::ProjectMemberKind::PropertyLet) }
                if name.eq_ignore_ascii_case("Value")
        )),
        "expected a late PropertyLet put to Value"
    );
}

#[test]
fn declare_lib_emits_external_call_descriptor() {
    // `Declare` lowers calls to CoreCallee::Declare and records a descriptor with
    // the source-declared library/name/params (runtime fields are defaults).
    let src = "Declare Function GetTickCount Lib \"kernel32\" () As Long\n\n\
               Sub Main()\n    Dim r As Long\n    r = GetTickCount()\nEnd Sub\n";
    let program = bind(src);
    assert!(
        top_level_callees(&program)
            .iter()
            .any(|c| matches!(c, CoreCallee::Declare { .. })),
        "expected a Declare callee"
    );
    let desc = program
        .external_calls
        .iter()
        .find(|d| d.declared_name.eq_ignore_ascii_case("GetTickCount"))
        .expect("expected a GetTickCount external-call descriptor");
    assert_eq!(desc.library, "kernel32");
    assert!(desc.return_type.is_some());
}

#[test]
fn declare_byref_arg_emits_byref() {
    // A ByRef `Declare` param called with an l-value lowers to CoreArg::ByRef (the
    // write-back target). Exercises bind_args_byref — the same path COM uses.
    let src = "Declare Sub Bump Lib \"k\" (ByRef n As Long)\n\n\
               Sub Main()\n    Dim r As Long\n    r = 5\n    Bump r\nEnd Sub\n";
    let program = bind(src);
    let args = program
        .procs
        .iter()
        .flat_map(|p| &p.body)
        .find_map(|s| match s {
            CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::Declare { .. },
                args,
            }) => Some(args),
            _ => None,
        })
        .expect("a Declare call");
    assert!(
        matches!(args.first(), Some(CoreArg::ByRef(_))),
        "ByRef Declare arg should be CoreArg::ByRef, got {:?}",
        args.first()
    );
}

// ── File I/O (structural — bind emits cross-bundle ExternProc calls; not run) ──

#[test]
fn file_io_lowers_to_vba_filesystem_externs() {
    // Since P4 the funny-syntax file statements lower to cross-bundle `ExternProc`
    // calls into the `VBA` bundle's `FileSystem` module (internal member names
    // `Open`/`Print`/`Close`/…) rather than `CoreCallee::Native(File*)`. We assert the
    // entry bundle imports each member; the special arg-shaping is unchanged.
    let src = "Sub Main()\n    Dim f As Long\n    f = FreeFile\n    Open \"x.txt\" For Output As #1\n    Print #1, \"hi\"\n    Close #1\nEnd Sub\n";
    let program = bind(src);
    let bundle = oxvba_bundle::linearize(&program).expect("linearize");
    for member in ["Open", "Print", "Close"] {
        assert!(
            imports_vba_filesystem(&bundle, member),
            "expected a VBA/FileSystem import for {member}: {:?}",
            bundle.imports
        );
    }
    // No file statement remains on the bespoke `CoreCallee::Native` route.
    assert!(
        !contains_file_native(&program),
        "no file statement may lower to CoreCallee::Native after P4"
    );
}

/// True if any statement still lowers a *migrated* file id to `CoreCallee::Native`.
fn contains_file_native(program: &CoreProgram) -> bool {
    fn value_native(v: &CoreValue) -> Option<NativeImplId> {
        match v {
            CoreValue::Call {
                callee: CoreCallee::Native(n),
                ..
            } => Some(*n),
            CoreValue::Unary { expr, .. } => value_native(expr),
            _ => None,
        }
    }
    program.procs.iter().flat_map(|p| &p.body).any(|s| {
        let id = match s {
            CoreStmt::Eval(v) => value_native(v),
            CoreStmt::Assign { value, .. } => value_native(value),
            _ => None,
        };
        id.is_some_and(|id| {
            id.library_member().is_some() || id.library_statement_member().is_some()
        })
    })
}
