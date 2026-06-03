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
fn scalar_longlong_const_carrier_executes() {
    let snap = run(
        "Const CTotal As LongLong = 5000000000\nSub Main()\nDim x As LongLong\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i64(5_000_000_000)]);
}

#[test]
fn scalar_typed_integer_const_expressions_execute() {
    let snap = run(
        "Const CByte As Byte = 1 + 2\nConst CInteger As Integer = 32767 - 1\nConst CLong As Long = 2 ^ 3 \\ 2 Mod 3 + 4\nSub Main()\nDim b As Byte\nDim i As Integer\nDim l As Long\nb = CByte\ni = CInteger\nl = CLong\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_i32(3),
            Variant::from_i32(32_766),
            Variant::from_i32(5),
        ]
    );
}

#[test]
fn scalar_untyped_integer_const_expression_executes() {
    let snap = run(
        "Const CBase = 1 + 2\nConst COffset = -1 + 2\nConst CTotal = CBase + COffset\nSub Main()\nDim x\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_i32(4)]);
}

#[test]
fn scalar_double_const_carrier_executes() {
    let snap = run(
        "Const CBase As Long = 1\nConst CTotal As Double = CBase + 0.5\nSub Main()\nDim x As Double\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_f64(1.5)]);
}

#[test]
fn scalar_single_const_carrier_executes() {
    let snap = run(
        "Const CBase As Double = 1.25\nConst CTotal As Single = CBase + 0.25\nSub Main()\nDim x As Single\nx = CTotal\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_f32(1.5)]);
}

#[test]
fn scalar_type_char_const_carriers_execute() {
    let snap = run(
        "Const CInteger% = 7\nConst CLong& = 8\nConst CLongLong^ = 5000000000\nConst CTotal! = 1.5\nConst CDouble# = 2\nConst CAmount@ = 1.25\nConst CText$ = \"ok\"\nSub Main()\nDim i As Integer\nDim l As Long\nDim ll As LongLong\nDim x As Single\nDim d As Double\nDim amount As Currency\nDim s As String\ni = CInteger\nl = CLong\nll = CLongLong\nx = CTotal\nd = CDouble\namount = CAmount\ns = CText\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_i32(7),
            Variant::from_i32(8),
            Variant::from_i64(5_000_000_000),
            Variant::from_f32(1.5),
            Variant::from_f64(2.0),
            Variant::from_currency_scaled_i64(12_500),
            Variant::from_string(BStr::from("ok")),
        ]
    );
}

#[test]
fn scalar_boolean_const_expression_executes() {
    let snap = run(
        "Const Prefix As String = \"re\"\nConst Enabled As Boolean = True\nConst CFlag As Boolean = Enabled = Not False And 2 > 1 And Prefix & \"ady\" = \"ready\"\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn scalar_boolean_xor_const_expression_executes() {
    let snap = run(
        "Const Enabled As Boolean = True\nConst CFlag As Boolean = Enabled Xor True\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(false)]);
}

#[test]
fn scalar_boolean_eqv_imp_const_expressions_execute() {
    let snap = run(
        "Const Enabled As Boolean = True\nConst CEqv As Boolean = Enabled Eqv False\nConst CImp As Boolean = Enabled Imp False\nSub Main()\nDim sameFlag As Boolean\nDim impliesFlag As Boolean\nsameFlag = CEqv\nimpliesFlag = CImp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![Variant::from_bool(false), Variant::from_bool(false)]
    );
}

#[test]
fn scalar_option_compare_text_boolean_const_expression_executes() {
    let snap = run(
        "Option Compare Text\nConst CFlag As Boolean = \"a\" = \"A\"\nSub Main()\nDim flag As Boolean\nflag = CFlag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn scalar_string_const_expression_executes() {
    let snap = run(
        "Const Prefix As String = \"re\"\nConst CText As String = Prefix & \"ady\"\nSub Main()\nDim text As String\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("ready"))]);
}

#[test]
fn scalar_string_const_scalar_concat_expression_executes() {
    let snap = run(
        "Const Prefix As String = \"v\"\nConst CNumber As Long = 7\nConst CFlag As Boolean = True\nConst CText As String = Prefix & CNumber & CFlag\nSub Main()\nDim text As String\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("v7True"))]);
}

#[test]
fn scalar_untyped_string_const_expression_executes() {
    let snap = run(
        "Const Prefix = \"re\"\nConst CText = Prefix & \"ady\"\nSub Main()\nDim text\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("ready"))]);
}

#[test]
fn scalar_untyped_string_const_scalar_concat_expression_executes() {
    let snap = run(
        "Const Prefix = \"v\"\nConst CNumber = 7\nConst CFlag = True\nConst CText = Prefix & CNumber & CFlag\nSub Main()\nDim text\ntext = CText\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("v7True"))]);
}

#[test]
fn scalar_currency_date_const_carriers_execute() {
    let snap = run(
        "Const CAmount As Currency = 1.25@\nConst CStamp As Date = #2026-02-28#\nSub Main()\nDim amount As Currency\nDim stamp As Date\namount = CAmount\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_currency_scaled_i64(12_500),
            Variant::from_date_f64(46_081.0),
        ]
    );
}

#[test]
fn scalar_month_name_date_const_carrier_executes() {
    let snap = run(
        "Const CStamp As Date = #February 28, 2026#, CNext As Date = CStamp + 1\nSub Main()\nDim stamp As Date\nDim nextStamp As Date\nstamp = CStamp\nnextStamp = CNext\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_date_f64(46_081.0),
            Variant::from_date_f64(46_082.0),
        ]
    );
}

#[test]
fn scalar_numeric_month_day_date_const_carrier_executes() {
    let snap = run(
        "Const CStamp As Date = #2/28/2026#\nSub Main()\nDim stamp As Date\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_date_f64(46_081.0)]);
}

#[test]
fn scalar_untyped_date_const_carrier_executes() {
    let snap =
        run("Const CStamp = #2026-02-28#\nSub Main()\nDim stamp As Date\nstamp = CStamp\nEnd Sub");
    assert_eq!(snap, vec![Variant::from_date_f64(46_081.0)]);
}

#[test]
fn scalar_currency_date_const_expression_carriers_execute() {
    let snap = run(
        "Const CAmount As Currency = 1.25@ * 2@ - 1.0@\nConst CStamp As Date = #2026-02-28# + 1\nSub Main()\nDim amount As Currency\nDim stamp As Date\namount = CAmount\nstamp = CStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_currency_scaled_i64(15_000),
            Variant::from_date_f64(46_082.0),
        ]
    );
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
fn optional_string_boolean_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim s As String\nDim b As Boolean\nCall Fill(s, b)\nEnd Sub\nSub Fill(ByRef target As String, ByRef flagTarget As Boolean, Optional ByVal text As String = \"ready\", Optional ByVal flag As Boolean = True)\ntarget = text\nflagTarget = flag\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("ready")),
            Variant::from_bool(true),
        ]
    );
}

#[test]
fn optional_string_boolean_module_constant_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const CText = \"ready\"\nConst CFlag = True\nSub Main()\nDim s As String\nDim b As Boolean\nCall Fill(s, b)\nEnd Sub\nSub Fill(ByRef target As String, ByRef flagTarget As Boolean, Optional ByVal text As String = CText, Optional ByVal flag As Boolean = CFlag)\ntarget = text\nflagTarget = flag\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("ready")),
            Variant::from_bool(true),
        ]
    );
}

#[test]
fn optional_string_concat_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Prefix = \"re\"\nSub Main()\nDim s As String\nCall Fill(s)\nEnd Sub\nSub Fill(ByRef target As String, Optional ByVal text As String = Prefix & \"ady\")\ntarget = text\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_string(BStr::from("ready"))]);
}

#[test]
fn optional_boolean_expression_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Const Enabled = True\nSub Main()\nDim b As Boolean\nCall Fill(b)\nEnd Sub\nSub Fill(ByRef target As Boolean, Optional ByVal flag As Boolean = Enabled = Not False And 2 > 1)\ntarget = flag\nEnd Sub",
    );
    assert_eq!(snap, vec![Variant::from_bool(true)]);
}

#[test]
fn optional_typed_declared_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim s As String\nDim b As Boolean\nDim n As Long\nCall Fill(s, b, n)\nEnd Sub\nSub Fill(ByRef target As String, ByRef flagTarget As Boolean, ByRef numberTarget As Long, Optional ByVal text As String, Optional ByVal flag As Boolean, Optional ByVal value As Long)\ntarget = text\nflagTarget = flag\nnumberTarget = value\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_string(BStr::from("")),
            Variant::from_bool(false),
            Variant::from_i32(0),
        ]
    );
}

#[test]
fn optional_date_currency_defaults_are_bound_for_omitted_args() {
    let snap = run(
        "Sub Main()\nDim amount As Variant\nDim stamp As Variant\nDim literalStamp As Variant\nDim numericStamp As Variant\nDim blankAmount As Variant\nDim blankStamp As Variant\nCall Fill(amount, stamp, literalStamp, numericStamp, blankAmount, blankStamp)\nEnd Sub\nSub Fill(ByRef amountTarget As Variant, ByRef stampTarget As Variant, ByRef literalStampTarget As Variant, ByRef numericStampTarget As Variant, ByRef blankAmountTarget As Variant, ByRef blankStampTarget As Variant, Optional ByVal amount As Currency = 1.25@ * 2@ - 1.0@, Optional ByVal stamp As Date = (2.0 + 3.0) / 2.0, Optional ByVal literalStamp As Date = #2026-02-28#, Optional ByVal numericStamp As Date = #2/28/2026#, Optional ByVal blankAmount As Currency, Optional ByVal blankStamp As Date)\namountTarget = amount\nstampTarget = stamp\nliteralStampTarget = literalStamp\nnumericStampTarget = numericStamp\nblankAmountTarget = blankAmount\nblankStampTarget = blankStamp\nEnd Sub",
    );
    assert_eq!(
        snap,
        vec![
            Variant::from_currency_scaled_i64(15_000),
            Variant::from_date_f64(2.5),
            Variant::from_date_f64(46_081.0),
            Variant::from_date_f64(46_081.0),
            Variant::from_currency_scaled_i64(0),
            Variant::from_date_f64(0.0),
        ]
    );
}

#[test]
fn indexed_property_get_executes_through_package_vm() {
    let snap = run(
        "Sub Main()\nDim value As Long\nvalue = Item(4)\nEnd Sub\nProperty Get Item(ByVal index As Long) As Long\nItem = index + 3\nEnd Property",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
}

#[test]
fn indexed_property_let_executes_through_package_vm() {
    let snap = run(
        "Sub Main()\nDim value As Long\nItem(value) = 3\nEnd Sub\nProperty Let Item(ByRef target As Long, ByVal newValue As Long)\ntarget = newValue + 4\nEnd Property",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
}

#[test]
fn named_indexed_property_let_executes_through_package_vm() {
    let snap = run(
        "Sub Main()\nDim value As Long\nItem(target := value) = 3\nEnd Sub\nProperty Let Item(ByRef target As Long, ByVal newValue As Long)\ntarget = newValue + 4\nEnd Property",
    );
    assert_eq!(snap, vec![Variant::from_i32(7)]);
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

/// Runs a snippet and returns the execution result (for asserting overflow errors).
fn run_result(source: &str) -> Result<Vec<Variant>, String> {
    let (bytecode, metadata) =
        compile_with_runtime_metadata(source).expect("snippet should compile");
    execute_bundle_and_snapshot_variants(&OxBundle::new(bytecode, metadata))
}

/// Asserts a snippet raises VBA run-time error 6 ("Overflow"), per the Excel oracle (bd-0d1y).
fn expect_overflow(source: &str) {
    let err = run_result(source).expect_err("expected VBA overflow error 6");
    assert!(
        err.contains("runtime error: 6"),
        "expected overflow error 6, got: {err}"
    );
}

#[test]
fn overflow_fixed_integer_assignment_raises_error_6() {
    // Overflow into a declared fixed-integer target is error 6 (Excel-oracle confirmed).
    expect_overflow("Sub Main()\nDim x As Long\nx = 2000000000\nx = x + 2000000000\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Integer\nx = 32767\nx = x + 1\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Integer\nx = -32768\nx = x - 1\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Byte\nx = 200\nx = x + 100\nEnd Sub");
    expect_overflow("Sub Main()\nDim x As Long\nx = 50000\nx = x * 50000\nEnd Sub");
}

#[test]
fn overflow_fixed_integer_expression_raises_error_6() {
    // Fixed-type arithmetic overflow errors at the operation even when the result flows into a
    // Variant (no widening), including intermediate overflow inside a larger expression.
    expect_overflow("Sub Main()\nDim ai As Integer\nDim r\nai = 32767\nr = ai + 1\nEnd Sub");
    expect_overflow("Sub Main()\nDim ai As Integer\nDim r\nai = -32768\nr = -ai\nEnd Sub");
    expect_overflow("Sub Main()\nDim al As Long\nDim r\nal = 2000000000\nr = al + al\nEnd Sub");
    expect_overflow(
        "Sub Main()\nDim al As Long\nDim r\nal = 2000000000\nr = (al + al) Mod 7\nEnd Sub",
    );
}

#[test]
fn overflow_variant_operands_widen_instead_of_erroring() {
    // Variant operands widen on overflow (Integer->Long->Double); no error.
    let snap = run("Sub Main()\nDim v\nDim r\nv = 2000000000\nr = v + v\nEnd Sub");
    assert!(
        snap.iter().any(|x| x.as_f64() == Some(4_000_000_000.0)),
        "expected widened Double 4e9 in {snap:?}"
    );
    let snap = run("Sub Main()\nDim v\nDim r\nv = 50000\nr = v * 50000\nEnd Sub");
    assert!(
        snap.iter().any(|x| x.as_f64() == Some(2_500_000_000.0)),
        "expected widened Double 2.5e9 in {snap:?}"
    );
}

#[test]
fn fixed_integer_arithmetic_in_range_does_not_error() {
    // In-range fixed-integer arithmetic is not flagged: the value is correct and no spurious
    // overflow is raised. (The result subtype tag — Integer vs Long — is a separate fidelity
    // concern outside the overflow gate; the value is what matters here.)
    let snap = run("Sub Main()\nDim ai As Integer\nDim r\nai = 100\nr = ai + 1\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i32(101)),
        "expected 101 in {snap:?}"
    );
    // Byte + Integer literal promotes to Integer (300 fits), so this does not overflow.
    let snap = run("Sub Main()\nDim ab As Byte\nDim r\nab = 200\nr = ab + 100\nEnd Sub");
    assert!(
        snap.contains(&Variant::from_i32(300)),
        "expected 300 (Byte+Integer promotes, in range) in {snap:?}"
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
