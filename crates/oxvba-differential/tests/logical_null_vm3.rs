//! vm3 three-valued (`Null`) logic for `And`/`Or`/`Imp`/`Xor`/`Eqv`.
//!
//! These previously returned `Null` whenever EITHER operand was `Null`. VBA uses three-valued
//! logic: a result bit determined by the known operand survives (`False And Null` = False,
//! `True Or Null` = True), and only a result still depending on the unknown operand is `Null`.
//! Closes `and-or-imp-null-three-valued`. The `Null` operand is held in a variable so the
//! expression is evaluated at run time, not constant-folded.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

/// Run `Main` with a `Null` variable `n` and the given assignments to `Public` Variants,
/// returning the snapshot (globals in declaration order).
fn run_logic(decls: &str, body: &str) -> Vec<Canon> {
    let source = format!("{decls}Sub Main()\n    Dim n As Variant\n    n = Null\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 run failed: {e}\n{source}"))
}

fn b(v: bool) -> Canon {
    canon(&Variant::from_bool(v))
}
fn null() -> Canon {
    canon(&Variant::null())
}

#[test]
fn and_with_null_is_three_valued() {
    // False And Null = False; True And Null = Null; Null And Null = Null.
    let snap = run_logic(
        "Public a As Variant\nPublic b As Variant\nPublic c As Variant\n",
        "    a = (False And n)\n    b = (True And n)\n    c = (n And n)\n",
    );
    assert_eq!(snap.first(), Some(&b(false)), "False And Null: {snap:?}");
    assert_eq!(snap.get(1), Some(&null()), "True And Null: {snap:?}");
    assert_eq!(snap.get(2), Some(&null()), "Null And Null: {snap:?}");
}

#[test]
fn or_with_null_is_three_valued() {
    // True Or Null = True; False Or Null = Null.
    let snap = run_logic(
        "Public a As Variant\nPublic b As Variant\n",
        "    a = (True Or n)\n    b = (False Or n)\n",
    );
    assert_eq!(snap.first(), Some(&b(true)), "True Or Null: {snap:?}");
    assert_eq!(snap.get(1), Some(&null()), "False Or Null: {snap:?}");
}

#[test]
fn imp_with_null_is_three_valued() {
    // False Imp Null = True; True Imp Null = Null; Null Imp True = True; Null Imp False = Null.
    let snap = run_logic(
        "Public a As Variant\nPublic b As Variant\nPublic c As Variant\nPublic d As Variant\n",
        "    a = (False Imp n)\n    b = (True Imp n)\n    c = (n Imp True)\n    d = (n Imp False)\n",
    );
    assert_eq!(snap.first(), Some(&b(true)), "False Imp Null: {snap:?}");
    assert_eq!(snap.get(1), Some(&null()), "True Imp Null: {snap:?}");
    assert_eq!(snap.get(2), Some(&b(true)), "Null Imp True: {snap:?}");
    assert_eq!(snap.get(3), Some(&null()), "Null Imp False: {snap:?}");
}

#[test]
fn xor_and_eqv_with_null_are_always_null() {
    // Xor/Eqv with Null can never be determined → always Null.
    let snap = run_logic(
        "Public a As Variant\nPublic b As Variant\n",
        "    a = (True Xor n)\n    b = (True Eqv n)\n",
    );
    assert_eq!(snap.first(), Some(&null()), "True Xor Null: {snap:?}");
    assert_eq!(snap.get(1), Some(&null()), "True Eqv Null: {snap:?}");
}
