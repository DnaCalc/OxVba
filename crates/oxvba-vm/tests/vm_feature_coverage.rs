//! Wide feature-coverage for the single package-descriptor VM.
//!
//! Compiles small VBA snippets and runs them through the package VM, asserting
//! the user-visible variant snapshot. Touches the core metadata shapes the VM
//! must run correctly: scalar type matrix, strings/BSTR, arrays (fixed/dynamic),
//! UDTs, control flow, and error handling.

use oxvba_compiler::{OxBundle, compile_with_runtime_metadata};
use oxvba_runtime::{Variant, bstr::BStr};
use oxvba_vm::execute_bundle_and_snapshot_variants;

/// Compile `source` (lightweight snippet path) and return the user-visible snapshot.
fn run(source: &str) -> Vec<Variant> {
    let (bytecode, metadata) =
        compile_with_runtime_metadata(source).expect("snippet should compile");
    let bundle = OxBundle::new(bytecode, metadata);
    execute_bundle_and_snapshot_variants(&bundle).expect("snippet should execute")
}

#[test]
fn scalar_long_arithmetic() {
    let snap = run("Sub Main()\nDim x As Long\nx = 2\nx = x * 3 + 4\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_i32(10)]);
}

#[test]
fn scalar_double_arithmetic() {
    let snap = run("Sub Main()\nDim d As Double\nd = 1.5\nd = d * 2.0\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_f64(3.0)]);
}

#[test]
fn integer_division_and_mod() {
    // `\` integer division and `Mod`.
    let snap = run("Sub Main()\nDim a As Long\nDim b As Long\na = 17 \\ 5\nb = 17 Mod 5\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_i32(3), Variant::from_i32(2)]);
}

#[test]
fn boolean_and_or_in_condition() {
    // VM evaluation of And/Or through the supported branch-predicate path.
    // (Logical operators as assignment rvalues are a separate compiler-lowering
    // gap tracked outside this VM coverage suite.)
    let snap = run(
        "Sub Main()\nDim a As Boolean\nDim b As Boolean\nDim andRes As Long\nDim orRes As Long\na = True\nb = False\nIf a And b Then\nandRes = 1\nElse\nandRes = 0\nEnd If\nIf a Or b Then\norRes = 1\nElse\norRes = 0\nEnd If\nEnd Sub",
    );
    // a And b = False -> andRes = 0 ; a Or b = True -> orRes = 1
    assert!(
        snap.contains(&Variant::from_i32(0)) && snap.contains(&Variant::from_i32(1)),
        "expected andRes=0 and orRes=1 in {snap:?}"
    );
}

#[test]
fn string_concat_and_len() {
    let snap =
        run("Sub Main()\nDim s As String\nDim n As Long\ns = \"ab\" & \"cd\"\nn = Len(s)\nEnd Sub");
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("abcd")),
            Variant::from_i32(4)
        ]
    );
}

#[test]
fn string_functions_left_mid_ucase() {
    let snap = run(
        "Sub Main()\nDim a As String\nDim b As String\na = Left$(\"hello\", 3)\nb = UCase$(Mid$(\"hello\", 2, 2))\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("hel")),
            Variant::from_string(BStr::from("EL")),
        ]
    );
}

#[test]
fn fixed_array_index_assign_read() {
    let snap = run(
        "Sub Main()\nDim a(1 To 3) As Long\nDim total As Long\na(1) = 10\na(2) = 20\na(3) = 30\ntotal = a(1) + a(2) + a(3)\nEnd Sub",
    );
    // total is the last user slot.
    assert_eq!(snap.last(), Some(&Variant::from_i32(60)));
}

#[test]
fn dynamic_array_redim_and_use() {
    let snap = run(
        "Sub Main()\nDim a() As Long\nDim v As Long\nReDim a(2)\na(0) = 7\na(2) = 5\nv = a(0) + a(2)\nEnd Sub",
    );
    assert_eq!(snap.last(), Some(&Variant::from_i32(12)));
}

#[test]
fn for_loop_accumulator() {
    let snap = run(
        "Sub Main()\nDim i As Long\nDim sum As Long\nsum = 0\nFor i = 1 To 5\nsum = sum + i\nNext i\nEnd Sub",
    );
    // sum = 15
    assert!(
        snap.contains(&Variant::from_i32(15)),
        "expected sum=15 in {snap:?}"
    );
}

#[test]
fn while_loop_countdown() {
    let snap = run(
        "Sub Main()\nDim n As Long\nDim steps As Long\nn = 3\nsteps = 0\nDo While n > 0\nn = n - 1\nsteps = steps + 1\nLoop\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_i32(3)),
        "expected steps=3 in {snap:?}"
    );
}

#[test]
fn if_elseif_else_branch() {
    let snap = run(
        "Sub Main()\nDim x As Long\nDim label As String\nx = 5\nIf x < 0 Then\nlabel = \"neg\"\nElseIf x = 0 Then\nlabel = \"zero\"\nElse\nlabel = \"pos\"\nEnd If\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_string(BStr::from("pos"))),
        "expected label=pos in {snap:?}"
    );
}

#[test]
fn udt_field_assign_and_read() {
    let snap = run("Type Point\nX As Long\nY As Long\nEnd Type\n\
         Sub Main()\nDim p As Point\nDim s As Long\np.X = 3\np.Y = 4\ns = p.X + p.Y\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i32(7)),
        "expected p.X+p.Y=7 in {snap:?}"
    );
}

#[test]
fn udt_whole_copy_independence() {
    // Copying a UDT must be by value: mutating the copy must not affect the source.
    let snap = run("Type Pair\nA As Long\nB As Long\nEnd Type\n\
         Sub Main()\nDim p As Pair\nDim q As Pair\nDim srcA As Long\np.A = 1\np.B = 2\nq = p\nq.A = 99\nsrcA = p.A\nEnd Sub");
    // p.A must remain 1 after q.A = 99.
    assert!(
        snap.contains(&Variant::from_i32(1)),
        "expected source p.A=1 preserved in {snap:?}"
    );
}

#[test]
fn on_error_resume_next_division_by_zero() {
    // Division by zero under On Error Resume Next: Err.Number set, execution continues.
    let snap = run(
        "Sub Main()\nDim r As Double\nDim afterErr As Long\nOn Error Resume Next\nr = 1 / 0\nafterErr = Err.Number\nEnd Sub",
    );
    // afterErr should be the division-by-zero error number (11).
    assert!(
        snap.contains(&Variant::from_i32(11)),
        "expected Err.Number=11 after div-by-zero in {snap:?}"
    );
}

#[test]
fn logical_operators_as_rvalues() {
    // And/Or/Not used as value-producing expressions (not just branch predicates).
    let snap = run(
        "Sub Main()\nDim a As Boolean\nDim b As Boolean\nDim andRes As Boolean\nDim orRes As Boolean\nDim notRes As Boolean\na = True\nb = False\nandRes = a And b\norRes = a Or b\nnotRes = Not a\nEnd Sub",
    );
    // a=True, b=False -> andRes=False, orRes=True, notRes=False
    assert_eq!(
        snap,
        vec![
            Variant::from_bool(true),
            Variant::from_bool(false),
            Variant::from_bool(false),
            Variant::from_bool(true),
            Variant::from_bool(false),
        ]
    );
}

#[test]
fn type_suffix_numeric_literals() {
    // VBA type-suffix literals: # Double, & Long.
    let snap = run("Sub Main()\nDim d As Double\nDim n As Long\nd = 2# * 1.5\nn = 100&\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_f64(3.0)),
        "expected 2# * 1.5 = 3.0 in {snap:?}"
    );
    assert!(
        snap.contains(&Variant::from_i32(100)),
        "expected 100& = 100 in {snap:?}"
    );
}

#[test]
fn logical_operator_precedence_with_comparison() {
    // Comparison binds tighter than And/Or: `x > 0 And x < 10`.
    let snap = run(
        "Sub Main()\nDim x As Long\nDim inRange As Boolean\nx = 5\ninRange = x > 0 And x < 10\nEnd Sub",
    );
    assert!(
        snap.contains(&Variant::from_bool(true)),
        "expected inRange=True for x=5 in {snap:?}"
    );
}
