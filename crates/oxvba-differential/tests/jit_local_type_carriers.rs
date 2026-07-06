use oxvba_differential::{Executor, run};

fn assert_jit_matches_vm3(source: &str) {
    let vm3 = run(Executor::Vm3, source);
    assert!(
        vm3.unsupported.is_none(),
        "vm3 declined carrier case: {:?}\n{source}",
        vm3.unsupported
    );
    let jit = run(Executor::Jit, source);
    assert!(
        jit.unsupported.is_none(),
        "jit declined carrier case: {:?}\n{source}",
        jit.unsupported
    );
    assert_eq!(jit.raised, vm3.raised, "raised mismatch\n{source}");
    assert_eq!(jit.err, vm3.err, "Err mismatch\n{source}");
    assert_eq!(jit.result, vm3.result, "snapshot mismatch\n{source}");
}

#[test]
fn jit_object_local_nothing_matches_vm3() {
    assert_jit_matches_vm3(
        r#"
Public r As Variant

Sub Main()
    Dim o As Object
    Set o = Nothing
    r = (o Is Nothing)
End Sub
"#,
    );
}

#[test]
fn jit_udt_local_copy_return_matches_vm3() {
    assert_jit_matches_vm3(
        r#"
Private Type T
    X As Long
End Type

Public r As Variant

Function Echo(ByVal value As T) As T
    Echo = value
End Function

Sub Main()
    Dim a As T
    Dim b As T
    b = Echo(a)
    r = 1
End Sub
"#,
    );
}

#[test]
fn jit_scalar_dynamic_and_fixed_arrays_match_vm3() {
    assert_jit_matches_vm3(
        r#"
Public r As Long

Sub Main()
    Dim a() As Long
    Dim b(1 To 3) As Long
    ReDim a(0 To 2)
    a(1) = 7
    r = a(1) + UBound(b)
End Sub
"#,
    );
}

#[test]
fn jit_object_array_allocation_matches_vm3() {
    assert_jit_matches_vm3(
        r#"
Public r As Long

Sub Main()
    Dim a() As Object
    ReDim a(1 To 2)
    r = UBound(a)
End Sub
"#,
    );
}

#[test]
fn jit_udt_array_allocation_matches_vm3() {
    assert_jit_matches_vm3(
        r#"
Private Type T
    X As Long
End Type

Public r As Long

Sub Main()
    Dim a() As T
    ReDim a(1 To 2)
    r = UBound(a)
End Sub
"#,
    );
}
