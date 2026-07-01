//! vm3 `String(number, numericCharacter)` follows VBA's byte-code wrap rule:
//! numeric character codes above 255 become `character Mod 256`.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn eval_string(expr: &str) -> Canon {
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
fn numeric_character_codes_wrap_mod_256() {
    assert_eq!(
        eval_string("String(3, 321)"),
        canon(&Variant::from_string("AAA"))
    );
    assert_eq!(
        eval_string("String(2, 322&)"),
        canon(&Variant::from_string("BB"))
    );
}

#[test]
fn string_character_argument_uses_first_character() {
    assert_eq!(
        eval_string("String(4, \"321\")"),
        canon(&Variant::from_string("3333"))
    );
}
