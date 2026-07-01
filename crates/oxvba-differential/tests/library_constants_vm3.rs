//! Focused vm3 coverage for base-library constants that are folded at bind time.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn snapshot(body: &str) -> Vec<Canon> {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{body}"))
}

#[test]
fn form_modality_constants_resolve_as_values() {
    let snap = snapshot(
        "    Dim combined As Long\n\
             combined = vbModal * 10 + vbModeless",
    );
    assert!(
        snap.contains(&canon(&Variant::from_i32(10))),
        "vbModal * 10 + vbModeless = 10: {snap:?}"
    );
}
