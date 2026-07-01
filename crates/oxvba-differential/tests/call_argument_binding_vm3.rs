//! Call/argument binding fixtures for the `bd-4ktq.10` batch.
//!
//! Live Excel/VBA 7.1 oracle evidence is captured in:
//! `docs/evidence/conformance/vm3_call_argument_oracle_20260701T1040Z/`.
//! The passing tests pin legal baseline shapes. Ignored tests encode the
//! oracle-backed statement-parentheses and compile-time rejection gaps that
//! follow-on call-argument beads are expected to unignore and satisfy.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_call_case(source: &str) -> RunOutcome {
    run(Executor::Vm3, source)
}

fn assert_snapshot_contains(outcome: RunOutcome, expected: Canon) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined call-argument case as unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 call-argument case failed: {err}"));
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

fn assert_compile_rejected(outcome: RunOutcome) {
    assert!(
        outcome.unsupported.is_some() || outcome.result.is_err() || outcome.raised,
        "expected compile/bind rejection or failure, got {outcome:?}"
    );
}

#[test]
fn bare_statement_byref_argument_mutates_caller() {
    assert_snapshot_contains(
        run_call_case(
            "Public result As Variant\n\
             Sub Main()\n\
             Dim x As Long\n\
             x = 5\n\
             Inc x\n\
             result = x\n\
             End Sub\n\n\
             Private Sub Inc(ByRef n As Long)\n\
             n = n + 100\n\
             End Sub\n",
        ),
        canon(&Variant::from_i32(105)),
    );
}

#[test]
fn byval_parameter_does_not_mutate_caller() {
    assert_snapshot_contains(
        run_call_case(
            "Public result As Variant\n\
             Sub Main()\n\
             Dim x As Long\n\
             x = 5\n\
             Touch x\n\
             result = x\n\
             End Sub\n\n\
             Private Sub Touch(ByVal n As Long)\n\
             n = n + 100\n\
             End Sub\n",
        ),
        canon(&Variant::from_i32(5)),
    );
}

#[test]
fn statement_parenthesized_byref_argument_is_forced_byval() {
    assert_snapshot_contains(
        run_call_case(
            "Public result As Variant\n\
             Sub Main()\n\
             Dim x As Long\n\
             x = 5\n\
             Inc (x)\n\
             result = x\n\
             End Sub\n\n\
             Private Sub Inc(ByRef n As Long)\n\
             n = n + 100\n\
             End Sub\n",
        ),
        canon(&Variant::from_i32(5)),
    );
}

#[test]
fn call_form_parenthesized_byref_argument_mutates_caller() {
    assert_snapshot_contains(
        run_call_case(
            "Public result As Variant\n\
             Sub Main()\n\
             Dim x As Long\n\
             x = 5\n\
             Call Inc(x)\n\
             result = x\n\
             End Sub\n\n\
             Private Sub Inc(ByRef n As Long)\n\
             n = n + 100\n\
             End Sub\n",
        ),
        canon(&Variant::from_i32(105)),
    );
}

#[test]
fn missing_optional_argument_uses_default() {
    assert_snapshot_contains(
        run_call_case(
            "Public result As Variant\n\
             Sub Main()\n\
             result = AddOpt(5)\n\
             End Sub\n\n\
             Private Function AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long\n\
             AddOpt = n + bonus\n\
             End Function\n",
        ),
        canon(&Variant::from_i32(12)),
    );
}

#[test]
fn paramarray_accepts_extra_positional_arguments() {
    assert_snapshot_contains(
        run_call_case(
            "Public result As Variant\n\
             Sub Main()\n\
             result = SumAll(1, 2, 3)\n\
             End Sub\n\n\
             Private Function SumAll(ParamArray xs() As Variant) As Long\n\
             Dim i As Long\n\
             For i = LBound(xs) To UBound(xs)\n\
             SumAll = SumAll + CLng(xs(i))\n\
             Next i\n\
             End Function\n",
        ),
        canon(&Variant::from_f64(6.0)),
    );
}

#[test]
fn byref_type_mismatch_should_be_rejected() {
    assert_compile_rejected(run_call_case(
        "Public result As Variant\n\
         Sub Main()\n\
         Dim x As Integer\n\
         TakeLong x\n\
         result = x\n\
         End Sub\n\n\
         Private Sub TakeLong(ByRef n As Long)\n\
         n = 7\n\
         End Sub\n",
    ));
}

#[test]
#[ignore = "bd-4ktq.10.4: extra args are still accepted/dropped today"]
fn extra_argument_should_be_rejected() {
    assert_compile_rejected(run_call_case(
        "Public result As Variant\n\
         Sub Main()\n\
         TakeOne 1, 2\n\
         result = 1\n\
         End Sub\n\n\
         Private Sub TakeOne(ByVal n As Long)\n\
         End Sub\n",
    ));
}

#[test]
#[ignore = "bd-4ktq.10.4: missing required args still fail late today"]
fn missing_required_argument_should_be_rejected() {
    assert_compile_rejected(run_call_case(
        "Public result As Variant\n\
         Sub Main()\n\
         TakeTwo 1\n\
         result = 1\n\
         End Sub\n\n\
         Private Sub TakeTwo(ByVal a As Long, ByVal b As Long)\n\
         End Sub\n",
    ));
}
