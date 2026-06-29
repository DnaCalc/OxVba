//! vm3 `Abs`/`Int`/`Fix` preserve the argument's numeric subtype and `Sgn`
//! returns Integer — verified against live Office VBA 7.1.
//!
//! Previously all four coerced to Double. VBA: `Abs`/`Int`/`Fix` return the
//! argument's own subtype (Integer→Integer, Long→Long, Currency→Currency,
//! Date→Date, …); `Abs` *promotes* on overflow (`Abs(CInt(-32768))`=Long 32768,
//! `Abs(CLng(-2147483648))`=Double); a Boolean → Integer, a numeric String →
//! Double, Empty → Integer 0, Null → Null (but `Sgn(Null)` raises 94 since the
//! result is Integer). `Sgn` always returns Integer (-1/0/1). Closes
//! `abs-int-fix-sgn-return-double`.

use oxvba_differential::{canon, run, Canon, Executor, RunOutcome};
use oxvba_runtime::Variant;

fn run_main(body: &str) -> RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn assert_value(body: &str, expected: &Canon) {
    let outcome = run_main(body);
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed for `{body}`: {e}"));
    assert_eq!(snap.first(), Some(expected), "body=`{body}` snap={snap:?}");
}

fn assert_raises(body: &str, number: i32) {
    let outcome = run_main(body);
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    assert!(outcome.result.is_err(), "expected err {number}, completed: {:?}", outcome.result);
    assert_eq!(outcome.err.number, number, "body=`{body}` err={:?}", outcome.err);
}

#[test]
fn abs_preserves_subtype_and_promotes_on_overflow() {
    assert_value("    r = Abs(CInt(-5))\n", &canon(&Variant::from_i16(5)));
    assert_value("    r = Abs(CLng(-5))\n", &canon(&Variant::from_i32(5)));
    assert_value("    r = Abs(CDbl(-5.7))\n", &canon(&Variant::from_f64(5.7)));
    // Integer.MIN promotes to Long; Long.MIN promotes to Double.
    assert_value("    r = Abs(CInt(-32768))\n", &canon(&Variant::from_i32(32768)));
    assert_value("    r = Abs(CLng(-2147483648#))\n", &canon(&Variant::from_f64(2147483648.0)));
}

#[test]
fn abs_of_currency_keeps_currency() {
    // -5.70 scaled by 10_000.
    assert_value(
        "    r = Abs(CCur(-5.7))\n",
        &canon(&Variant::from_currency_scaled_i64(57_000)),
    );
}

#[test]
fn int_floors_fix_truncates_keeping_subtype() {
    // Negative non-integers: Int floors toward -inf, Fix truncates toward zero.
    assert_value("    r = Int(CDbl(-5.7))\n", &canon(&Variant::from_f64(-6.0)));
    assert_value("    r = Fix(CDbl(-5.7))\n", &canon(&Variant::from_f64(-5.0)));
    assert_value("    r = Int(CCur(-5.7))\n", &canon(&Variant::from_currency_scaled_i64(-60_000)));
    assert_value("    r = Fix(CCur(-5.7))\n", &canon(&Variant::from_currency_scaled_i64(-50_000)));
    // Integer types are returned unchanged.
    assert_value("    r = Int(CLng(-5))\n", &canon(&Variant::from_i32(-5)));
    assert_value("    r = Fix(CInt(-5))\n", &canon(&Variant::from_i16(-5)));
}

#[test]
fn int_fix_of_date_keep_date() {
    // Int/Fix of a Date drop the time-of-day, keeping the Date subtype.
    // Serial 43845.75 == 2020-01-15 18:00 → Int floors to the date-only serial.
    assert_value(
        "    r = Int(CDate(43845.75))\n",
        &canon(&Variant::from_date_f64(43845.0)),
    );
    assert_value(
        "    r = Fix(CDate(43845.75))\n",
        &canon(&Variant::from_date_f64(43845.0)),
    );
}

#[test]
fn boolean_and_string_and_empty() {
    // Boolean → Integer; Abs(True=-1)=1, Int(True)=-1.
    assert_value("    r = Abs(CBool(True))\n", &canon(&Variant::from_i16(1)));
    assert_value("    r = Int(CBool(True))\n", &canon(&Variant::from_i16(-1)));
    // A numeric String → Double.
    assert_value("    r = Abs(\"-5.7\")\n", &canon(&Variant::from_f64(5.7)));
    // Empty → Integer 0.
    assert_value("    r = Abs(Empty)\n", &canon(&Variant::from_i16(0)));
}

#[test]
fn sgn_returns_integer_and_rejects_null() {
    assert_value("    r = Sgn(CDbl(-5.7))\n", &canon(&Variant::from_i16(-1)));
    assert_value("    r = Sgn(CLng(42))\n", &canon(&Variant::from_i16(1)));
    assert_value("    r = Sgn(0)\n", &canon(&Variant::from_i16(0)));
    assert_value("    r = Sgn(Empty)\n", &canon(&Variant::from_i16(0)));
    // Sgn's result is Integer, which cannot hold Null → error 94.
    assert_raises("    r = Sgn(Null)\n", 94);
}

#[test]
fn abs_int_fix_propagate_null() {
    assert_value("    r = Abs(Null)\n", &canon(&Variant::null()));
    assert_value("    r = Int(Null)\n", &canon(&Variant::null()));
    assert_value("    r = Fix(Null)\n", &canon(&Variant::null()));
}
