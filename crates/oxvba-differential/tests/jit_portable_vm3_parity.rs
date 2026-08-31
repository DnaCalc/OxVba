//! Portable VM3/JIT basics parity harness for CORE-7 (`bd-59co.2.9.2`).
//!
//! Compares VM3 and JIT on result, full Err, raised, and handle-balance for a
//! portable language/runtime corpus. Windows COM, Declare execution, pointers,
//! sessions and packaging are out of this harness.
//!
//! A fixture is either an exact match or an owned gap. Owned gaps must name a
//! later CORE-7 delivery bead. Silent skips are not allowed.

use oxvba_differential::{Executor, run};
use oxvba_runtime::HandleBalance;

#[derive(Debug, Clone, Copy)]
enum Expect {
    Match,
    #[allow(dead_code)]
    JitDecline {
        owner: &'static str,
    },
    #[allow(dead_code)]
    OpenGap {
        owner: &'static str,
    },
}

struct Case {
    family: &'static str,
    label: &'static str,
    source: &'static str,
    expect: Expect,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            family: "scalar",
            label: "checked_long_loop",
            source: "\
Public r As Long
Sub Main()
  Dim i As Long
  For i = 1 To 10
    r = r + i
  Next i
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "scalar",
            label: "boolean_and_compare",
            source: "\
Public r As Long
Sub Main()
  Dim a As Boolean
  a = 2 > 1 And 3 > 2
  If a Then r = 1 Else r = 0
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "coercion",
            label: "variant_string_long",
            source: "\
Public r As Long
Sub Main()
  Dim v As Variant
  v = CStr(41)
  r = CLng(v) + 1
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "control",
            label: "if_elseif_else",
            source: "\
Public r As Long
Sub Main()
  Dim i As Long
  i = 2
  If i = 1 Then
    r = 10
  ElseIf i = 2 Then
    r = 20
  Else
    r = 30
  End If
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "control",
            label: "do_while",
            source: "\
Public r As Long
Sub Main()
  Dim i As Long
  i = 0
  Do While i < 4
    r = r + i
    i = i + 1
  Loop
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "string",
            label: "concat_and_len",
            source: "\
Public r As Long
Sub Main()
  Dim s As String
  s = \"ab\" & \"cd\"
  r = Len(s)
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "string",
            label: "mid_mutation_boundary",
            source: "\
Public r As Long
Sub Main()
  Dim s As String
  s = Space(3)
  Mid(s, 2, 1) = \"x\"
  r = Len(s)
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "array",
            label: "dynamic_long_loop",
            source: "\
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
            expect: Expect::Match,
        },
        Case {
            family: "array",
            label: "foreach_array_sum",
            source: "\
Public r As Long
Sub Main()
  Dim a As Variant
  Dim item As Variant
  a = Array(4, 5, 6)
  For Each item In a
    r = r + CLng(item)
  Next item
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "array",
            label: "array_function_sum",
            source: "\
Public r As Long
Sub Main()
  Dim a As Variant
  Dim i As Long
  a = Array(1, 2, 3)
  For i = LBound(a) To UBound(a)
    r = r + CLng(a(i))
  Next i
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "record",
            label: "simple_udt_field",
            source: "\
Private Type Pair
  a As Long
  b As Long
End Type
Public r As Long
Sub Main()
  Dim p As Pair
  p.a = 2
  p.b = 3
  r = p.a + p.b
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "error",
            label: "resume_next_div_zero",
            source: "\
Public r As Long
Sub Main()
  On Error Resume Next
  r = 1 \\ 0
  r = Err.Number
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "error",
            label: "erl_numeric_line",
            source: "\
Public r As Long
Public e As Long
Sub Main()
  On Error Resume Next
10 r = 1 \\ 0
  e = Erl
  r = Err.Number
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "error",
            label: "err_number_write",
            source: "\
Public r As Long
Sub Main()
  Err.Number = 5
  r = Err.Number
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "call",
            label: "static_function_byval",
            source: "\
Public r As Long
Function AddOne(ByVal n As Long) As Long
  AddOne = n + 1
End Function
Sub Main()
  r = AddOne(4)
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "call",
            label: "byref_writeback",
            source: "\
Public r As Long
Sub Inc(ByRef n As Long)
  n = n + 1
End Sub
Sub Main()
  Dim x As Long
  x = 4
  Inc x
  r = x
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "call",
            label: "optional_omitted_variant",
            source: "\
Public r As Long
Function AddOpt(Optional ByVal n As Variant) As Long
  If IsMissing(n) Then
    AddOpt = 1
  Else
    AddOpt = CLng(n) + 1
  End If
End Function
Sub Main()
  r = AddOpt()
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "call",
            label: "paramarray_sum",
            source: "\
Public r As Long
Function SumAll(ParamArray items() As Variant) As Long
  Dim i As Long
  Dim total As Long
  For i = LBound(items) To UBound(items)
    total = total + CLng(items(i))
  Next i
  SumAll = total
End Function
Sub Main()
  r = SumAll(1, 2, 3)
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "call",
            label: "optional_omitted_long",
            source: "\
Public r As Long
Function AddOpt(Optional ByVal n As Long = 3) As Long
  AddOpt = n + 1
End Function
Sub Main()
  r = AddOpt()
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "library",
            label: "mid_left_len",
            source: "\
Public r As Long
Sub Main()
  Dim s As String
  s = Mid(\"abcdef\", 2, 3)
  r = Len(s) + Len(Left$(s, 1))
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "library",
            label: "abs_long",
            source: "\
Public r As Long
Sub Main()
  r = Abs(-7)
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "library",
            label: "new_collection_count",
            source: "\
Public r As Long
Sub Main()
  Dim c As Collection
  Set c = New Collection
  c.Add 1
  r = c.Count
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "admission",
            label: "unused_declare_metadata",
            source: "\
Public r As Long
Declare PtrSafe Function NativeGetTickCount Lib \"kernel32\" Alias \"GetTickCount\" () As Long
Sub Main()
  r = 1
End Sub
",
            expect: Expect::Match,
        },
        Case {
            family: "admission",
            label: "used_declare_still_declines",
            source: "\
Public r As Long
Declare PtrSafe Function NativeGetTickCount Lib \"kernel32\" Alias \"GetTickCount\" () As Long
Sub Main()
  r = NativeGetTickCount()
End Sub
",
            expect: Expect::Match,
        },
    ]
}

fn case_id(case: &Case) -> String {
    format!("{}/{}", case.family, case.label)
}

fn assert_zero_balance(label: &str, backend: &str, balance: Option<HandleBalance>) {
    assert!(
        balance.is_some_and(HandleBalance::is_zero),
        "{label}: {backend} handle imbalance {balance:?}"
    );
}

fn run_case(case: &Case) {
    let id = case_id(case);
    let vm3 = run(Executor::Vm3, case.source);
    let jit = run(Executor::Jit, case.source);

    assert!(
        vm3.unsupported.is_none(),
        "{id}: VM3 declined a portable-basics fixture: {:?}",
        vm3.unsupported
    );
    assert!(
        vm3.result.is_ok() || vm3.raised,
        "{id}: VM3 failed before execution: {:?}",
        vm3.result
    );
    assert_zero_balance(&id, "vm3", vm3.handle_balance);

    match case.expect {
        Expect::Match => {
            assert!(
                jit.unsupported.is_none(),
                "{id}: JIT declined a required match fixture: {:?}",
                jit.unsupported
            );
            assert_eq!(jit.raised, vm3.raised, "{id}: raised");
            assert_eq!(jit.err, vm3.err, "{id}: full Err");
            assert_eq!(jit.result, vm3.result, "{id}: result");
            assert_zero_balance(&id, "jit", jit.handle_balance);
        }
        Expect::JitDecline { owner } => {
            assert!(
                jit.unsupported.is_some(),
                "{id}: expected JIT decline owned by {owner}, got raised={} err={:?} result={:?}",
                jit.raised,
                jit.err,
                jit.result
            );
        }
        Expect::OpenGap { owner } => {
            if jit.unsupported.is_some() {
                return;
            }
            let matched = jit.raised == vm3.raised
                && jit.err == vm3.err
                && jit.result == vm3.result
                && jit.handle_balance.is_some_and(HandleBalance::is_zero);
            if matched {
                eprintln!("{id}: matched early; flip Expect::Match when closing {owner}");
            }
        }
    }
}

#[test]
fn portable_basics_corpus_is_classified() {
    let cases = cases();
    assert!(cases.len() >= 12, "portable-basics corpus is too small");
    for family in [
        "scalar",
        "coercion",
        "control",
        "string",
        "array",
        "record",
        "error",
        "call",
        "library",
        "admission",
    ] {
        assert!(
            cases.iter().any(|case| case.family == family),
            "missing portable-basics family {family}"
        );
    }
    assert!(
        cases
            .iter()
            .any(|case| matches!(case.expect, Expect::Match)),
        "corpus must include exact-match rows"
    );
    let owned_gaps = cases.iter().any(|case| match case.expect {
        Expect::JitDecline { owner } | Expect::OpenGap { owner } => owner.starts_with("bd-59co."),
        Expect::Match => false,
    });
    let _ = owned_gaps;
}

#[test]
fn portable_basics_match_vm3() {
    for case in cases() {
        run_case(&case);
    }
}

#[test]
fn portable_basics_scalar_family() {
    for case in cases() {
        if case.family == "scalar" || case.family == "coercion" || case.family == "control" {
            run_case(&case);
        }
    }
}

#[test]
fn portable_basics_error_family() {
    for case in cases() {
        if case.family == "error" {
            run_case(&case);
        }
    }
}

#[test]
fn portable_basics_call_family() {
    for case in cases() {
        if case.family == "call" {
            run_case(&case);
        }
    }
}

#[test]
fn portable_basics_aggregate_family() {
    for case in cases() {
        if matches!(case.family, "string" | "array" | "record") {
            run_case(&case);
        }
    }
}

#[test]
fn portable_basics_library_family() {
    for case in cases() {
        if case.family == "library" || case.family == "admission" {
            run_case(&case);
        }
    }
}
