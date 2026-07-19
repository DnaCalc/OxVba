//! JIT lowers `Debug.Print` (CallNative DebugPrint) through the same runtime
//! helper as vm3. Before this lowering, any program using `Debug.Print` was
//! declined with `RUN-E-JIT-UNSUPPORTED` (M4-4). The web-sourced repro that
//! surfaced the gap was the Rosetta 100-doors sample (bd-ivaha.37).

use oxvba_differential::{Executor, run};

fn assert_jit_matches_vm3(source: &str) {
    let vm3 = run(Executor::Vm3, source);
    assert!(
        vm3.unsupported.is_none(),
        "vm3 declined: {:?}\n{source}",
        vm3.unsupported
    );
    let jit = run(Executor::Jit, source);
    assert!(
        jit.unsupported.is_none(),
        "jit declined: {:?}\n{source}",
        jit.unsupported
    );
    assert_eq!(jit.raised, vm3.raised, "raised mismatch\n{source}");
    assert_eq!(jit.err, vm3.err, "Err mismatch\n{source}");
    assert_eq!(jit.result, vm3.result, "snapshot mismatch\n{source}");
}

#[test]
fn debug_print_concatenated_variant_local() {
    assert_jit_matches_vm3(
        r#"
Dim OpenCount As Long
Sub Main()
    Dim i As Long
    Dim Label As String
    OpenCount = 0
    For i = 1 To 10
        Label = "Closed"
        If i = 4 Then
            Label = "Open"
            OpenCount = OpenCount + 1
        End If
        Debug.Print "Door " & i & " is " & Label
    Next i
End Sub
"#,
    );
}

#[test]
fn debug_print_scalar_constants() {
    assert_jit_matches_vm3(
        r#"
Dim Done As Long
Sub Main()
    Debug.Print "plain string"
    Debug.Print 42
    Debug.Print 3.5
    Debug.Print True
    Done = 1
End Sub
"#,
    );
}

#[test]
fn debug_print_blank_line_and_semicolon_args() {
    assert_jit_matches_vm3(
        r#"
Dim Count As Long
Sub Main()
    Debug.Print
    Debug.Print "a"; "b"
    Debug.Print "x"; 7; "y"
    Count = 2
End Sub
"#,
    );
}

#[test]
fn debug_print_does_not_write_a_destination() {
    assert_jit_matches_vm3(
        r#"
Dim Value As Long
Sub Main()
    Value = 41
    Debug.Print "value is " & Value
    Value = Value + 1
End Sub
"#,
    );
}
