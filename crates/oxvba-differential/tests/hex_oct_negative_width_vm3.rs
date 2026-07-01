//! vm3 `Hex`/`Oct` preserve the two's-complement width of negative fixed-width
//! integer subtypes instead of widening every negative value to `LongLong`.

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
fn hex_negative_width_follows_input_subtype() {
    assert_eq!(eval("Hex(CInt(-1))"), canon(&Variant::from_string("FFFF")));
    assert_eq!(
        eval("Hex(CLng(-1))"),
        canon(&Variant::from_string("FFFFFFFF"))
    );
    assert_eq!(
        eval("Hex(CLngLng(-1))"),
        canon(&Variant::from_string("FFFFFFFFFFFFFFFF"))
    );
}

#[test]
fn oct_negative_width_follows_input_subtype() {
    assert_eq!(
        eval("Oct(CInt(-1))"),
        canon(&Variant::from_string("177777"))
    );
    assert_eq!(
        eval("Oct(CLng(-1))"),
        canon(&Variant::from_string("37777777777"))
    );
    assert_eq!(
        eval("Oct(CLngLng(-1))"),
        canon(&Variant::from_string("1777777777777777777777"))
    );
}

#[test]
fn positive_values_stay_unpadded() {
    assert_eq!(eval("Hex(CInt(255))"), canon(&Variant::from_string("FF")));
    assert_eq!(eval("Oct(CLng(8))"), canon(&Variant::from_string("10")));
}
