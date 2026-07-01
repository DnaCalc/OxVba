//! vm3 integer literal carrier/introspection truth surface (`bd-4ktq.11.1`).
//!
//! Live Excel/VBA 7.1 oracle evidence is captured in:
//! `docs/evidence/conformance/vm3_integer_literal_oracle_20260701T1200Z/`.
//! These rows pin oracle-backed carrier and syntax behavior: Integer-width
//! decimal/radix literals surface as Integer, explicit `^` literals surface as
//! LongLong, unsuffixed decimal beyond Long is Double, and unsuffixed radix
//! beyond Long width is a syntax error in Excel.

use oxvba_differential::{Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_probe(expr: &str) -> RunOutcome {
    let source = format!(
        "Public result As String\n\
         Sub Main()\n\
             result = CStr(VarType({expr})) & \":\" & TypeName({expr}) & \":\" & CStr({expr})\n\
         End Sub\n"
    );
    run(Executor::Vm3, &source)
}

fn assert_probe(expr: &str, expected: &str) {
    let outcome = run_probe(expr);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined integer literal case `{expr}` as unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 integer literal case `{expr}` failed: {err}"));
    let want = canon(&Variant::from_string(expected));
    assert!(
        values.contains(&want),
        "probe `{expr}` produced {values:?}, expected to contain {expected:?}"
    );
}

fn assert_compile_rejected(expr: &str) {
    let outcome = run_probe(expr);
    assert!(
        outcome.unsupported.is_some() || outcome.result.is_err(),
        "expected vm3 to reject integer literal case `{expr}`, got {outcome:?}"
    );
}

#[test]
fn decimal_long_width_literals_are_long() {
    assert_probe("32768", "3:Long:32768");
    assert_probe("2147483647", "3:Long:2147483647");
    assert_probe("7&", "3:Long:7");
}

#[test]
fn radix_long_width_literals_are_long() {
    assert_probe("&H10000", "3:Long:65536");
    assert_probe("&HFFFF&", "3:Long:65535");
    assert_probe("&O200000", "3:Long:65536");
    assert_probe("&O177777&", "3:Long:65535");
}

#[test]
fn decimal_integer_width_literals_are_integer() {
    assert_probe("7", "2:Integer:7");
    assert_probe("32767", "2:Integer:32767");
    assert_probe("7%", "2:Integer:7");
}

#[test]
fn unsuffixed_decimal_beyond_long_is_double() {
    assert_probe("2147483648", "5:Double:2147483648");
}

#[test]
fn caret_decimal_literal_is_longlong() {
    assert_probe("7^", "20:LongLong:7");
}

#[test]
fn radix_integer_width_literals_are_integer() {
    assert_probe("&HFFFF", "2:Integer:-1");
    assert_probe("&O177777", "2:Integer:-1");
}

#[test]
fn caret_radix_literal_is_longlong() {
    assert_probe("&HFFFFFFFFFFFFFFFF^", "20:LongLong:-1");
}

#[test]
fn unsuffixed_radix_beyond_long_width_is_compile_error() {
    assert_compile_rejected("&H100000000");
    assert_compile_rejected("&O40000000000");
}
