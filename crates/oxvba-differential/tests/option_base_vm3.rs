//! vm3 honours `Option Base` for single-bound array declarations and the
//! `Array()` function — verified against live Office VBA 7.1.
//!
//! `Option Base 1` makes `Dim a(3)` span 1..3 (not 0..3) and `Array(10,20,30)`
//! span 1..3 (not 0..2). An explicit `Dim b(2 To 5)` always overrides it, and a
//! `ParamArray` slot is always 0-based regardless of `Option Base`. Default
//! (no statement / `Option Base 0`) stays 0-based. Closes `option-base-1-ignored`.

use oxvba_differential::{canon, run, Executor};
use oxvba_runtime::Variant;

/// Run a full module and read the first snapshot value as a `String`.
fn assert_first_string(source: &str, expected: &str) {
    let outcome = run(Executor::Vm3, source);
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_string(expected.to_string()))),
        "source=\n{source}\nsnap={snap:?}"
    );
}

#[test]
fn default_base_is_zero() {
    assert_first_string(
        "Public r As String\n\
         Sub Main()\n\
         Dim a(3) As Long\n\
         r = LBound(a) & \"..\" & UBound(a)\n\
         End Sub\n",
        "0..3",
    );
}

#[test]
fn option_base_one_shifts_single_bound_dim() {
    assert_first_string(
        "Option Base 1\n\
         Public r As String\n\
         Sub Main()\n\
         Dim a(3) As Long\n\
         r = LBound(a) & \"..\" & UBound(a)\n\
         End Sub\n",
        "1..3",
    );
}

#[test]
fn explicit_lower_bound_overrides_option_base() {
    assert_first_string(
        "Option Base 1\n\
         Public r As String\n\
         Sub Main()\n\
         Dim b(2 To 5) As Long\n\
         r = LBound(b) & \"..\" & UBound(b)\n\
         End Sub\n",
        "2..5",
    );
}

#[test]
fn array_function_respects_base_zero() {
    assert_first_string(
        "Public r As String\n\
         Sub Main()\n\
         Dim v\n\
         v = Array(10, 20, 30)\n\
         r = LBound(v) & \"..\" & UBound(v)\n\
         End Sub\n",
        "0..2",
    );
}

#[test]
fn array_function_respects_base_one() {
    assert_first_string(
        "Option Base 1\n\
         Public r As String\n\
         Sub Main()\n\
         Dim v\n\
         v = Array(10, 20, 30)\n\
         r = LBound(v) & \"..\" & UBound(v)\n\
         End Sub\n",
        "1..3",
    );
}

#[test]
fn paramarray_stays_zero_based_under_option_base_one() {
    assert_first_string(
        "Option Base 1\n\
         Public r As String\n\
         Sub Main()\n\
         r = Probe(10, 20, 30)\n\
         End Sub\n\
         Function Probe(ParamArray xs() As Variant) As String\n\
         Probe = LBound(xs) & \"..\" & UBound(xs)\n\
         End Function\n",
        "0..2",
    );
}
