//! vm3 `Trim`/`LTrim`/`RTrim` remove space characters only, not every Unicode
//! whitespace character.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn eval(expr: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

#[test]
fn trim_family_strips_spaces_but_preserves_tabs() {
    assert_eq!(
        eval("Trim(\" \" & Chr(9) & \"x\" & Chr(9) & \" \")"),
        canon(&Variant::from_string("\tx\t"))
    );
    assert_eq!(
        eval("LTrim(\" \" & Chr(9) & \"x \")"),
        canon(&Variant::from_string("\tx "))
    );
    assert_eq!(
        eval("RTrim(\" x\" & Chr(9) & \" \")"),
        canon(&Variant::from_string(" x\t"))
    );
}
