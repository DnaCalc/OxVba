//! vm3 should parse bare `End` as a whole-program halt.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(source: &str) -> Canon {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text.to_string()))
}

#[test]
fn end_statement_skips_following_statements() {
    assert_eq!(
        value(
            "Public r As Variant\n\
             Sub Main()\n\
                 r = \"before\"\n\
                 End\n\
                 r = \"after\"\n\
             End Sub\n"
        ),
        s("before")
    );
}

#[test]
fn end_statement_in_callee_stops_whole_program() {
    assert_eq!(
        value(
            "Public r As Variant\n\
             Sub Main()\n\
                 r = \"main\"\n\
                 Stopper\n\
                 r = \"after-call\"\n\
             End Sub\n\
             Sub Stopper()\n\
                 r = \"callee\"\n\
                 End\n\
                 r = \"after-end\"\n\
             End Sub\n"
        ),
        s("callee")
    );
}

#[test]
fn end_statement_before_end_if_keyword_stays_bare_end() {
    assert_eq!(
        value(
            "Public r As Variant\n\
             Sub Main()\n\
                 r = \"before\"\n\
                 End\n\
                 If True Then\n\
                     r = \"after-if\"\n\
                 End If\n\
             End Sub\n"
        ),
        s("before")
    );
}
