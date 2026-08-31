//! WIN-2 first-slice VM3/JIT interop-plan fixtures (`bd-59co.3.3.4`).
//!
//! Fail-closed comparison of VM3 and JIT on result, full Err, raised, and
//! handle-balance for one late IDispatch call and one x64 Declare call that
//! both execute the same verified plan. Excel rows stay planned under WIN-14.

use oxvba_differential::{Executor, run};
use oxvba_runtime::HandleBalance;

struct Case {
    family: &'static str,
    label: &'static str,
    source: &'static str,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            family: "late-com",
            label: "testdispatch_count",
            source: "\
Public r As Long
Sub Main()
  Dim obj As Variant
  Set obj = CreateObject(\"OxVba.TestDispatch\")
  r = obj.Count
End Sub
",
        },
        Case {
            family: "declare",
            label: "kernel32_gettickcount",
            source: "\
Public r As Long
Declare PtrSafe Function NativeGetTickCount Lib \"kernel32\" Alias \"GetTickCount\" () As Long
Sub Main()
  r = NativeGetTickCount()
End Sub
",
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
        "{id}: VM3 declined a first-slice Windows fixture: {:?}",
        vm3.unsupported
    );
    assert!(
        jit.unsupported.is_none(),
        "{id}: JIT declined a first-slice Windows fixture: {:?}",
        jit.unsupported
    );
    assert_eq!(jit.raised, vm3.raised, "{id}: raised");
    assert_eq!(jit.err, vm3.err, "{id}: full Err");
    assert_eq!(jit.result, vm3.result, "{id}: result");
    assert_zero_balance(&id, "vm3", vm3.handle_balance);
    assert_zero_balance(&id, "jit", jit.handle_balance);
}

#[test]
fn windows_plan_corpus_is_classified() {
    let cases = cases();
    assert!(
        cases.iter().any(|case| case.family == "late-com"),
        "missing late IDispatch fixture"
    );
    assert!(
        cases.iter().any(|case| case.family == "declare"),
        "missing x64 Declare fixture"
    );
}

#[test]
fn windows_plan_fixtures_match_vm3() {
    for case in cases() {
        run_case(&case);
    }
}
