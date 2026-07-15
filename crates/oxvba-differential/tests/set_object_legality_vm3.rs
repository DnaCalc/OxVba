//! vm3 `Set` legality: a non-object source raises "Object required" (424).
//! `Set o = 0` used to silently succeed because is_nothing() treated numeric 0
//! as Nothing, so the object check passed and a scalar was stored.

use oxvba_differential::{Executor, run, run_modules};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn err_number(body: &str) -> i32 {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected a VBA error, got {:?}",
        outcome.result
    );
    outcome.err.number
}

fn runs_ok(body: &str) {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_ok(),
        "expected success, got {:?}",
        outcome.result
    );
}

#[test]
fn set_scalar_into_object_raises_object_required() {
    assert_eq!(err_number("    Dim o As Object\n    Set o = 0"), 424);
    assert_eq!(err_number("    Dim o As Object\n    Set o = 5"), 424);
}

#[test]
fn set_nothing_is_still_allowed() {
    runs_ok("    Dim o As Object\n    Set o = Nothing");
}

#[test]
fn let_scalar_into_object_var_is_object_required() {
    // Letting (no Set) a scalar into an Object variable is also "Object required".
    assert_eq!(err_number("    Dim o As Object\n    o = 0"), 424);
}

#[test]
fn set_real_object_still_works() {
    let main = "Public result As Long\n\
                Sub Main()\n\
                \x20   Dim w As Widget\n\
                \x20   Set w = New Widget\n\
                \x20   result = w.V\n\
                \x20   Set w = Nothing\n\
                End Sub\n";
    let widget = "Public Property Get V() As Long\n\
                  \x20   V = 7\n\
                  End Property\n";
    let outcome = run_modules(
        Executor::Vm3,
        &[("Main", Procedural, main), ("Widget", Class, widget)],
        "VBAProject",
    );
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_ok(),
        "Set of a real object must succeed: {:?}",
        outcome.result
    );
}
