//! vm3 `Collection.Item`/`Remove` accept any numeric index subtype (Byte,
//! Single, unsigned, …), not just Integer/Long/LongLong/Double. The selector
//! used a partial as_i16/i32/i64/f64 chain, so `c.Item(CByte(1))` fell through
//! to index 0 and raised error 9.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn item_result(index_expr: &str) -> Canon {
    let body = format!(
        "Public r As Variant\nSub Main()\n    Dim c As New Collection\n    c.Add \"a\"\n    c.Add \"b\"\n    r = c.Item({index_expr})\nEnd Sub\n"
    );
    let outcome = run(Executor::Vm3, &body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|e| panic!("run failed for index {index_expr}: {e}"))
        .into_iter()
        .next()
        .expect("global r")
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text))
}

#[test]
fn collection_item_accepts_all_numeric_index_subtypes() {
    // Previously broken subtypes:
    assert_eq!(item_result("CByte(1)"), s("a"));
    assert_eq!(item_result("CByte(2)"), s("b"));
    assert_eq!(item_result("CSng(1)"), s("a"));
    assert_eq!(item_result("CSng(2)"), s("b"));
    // Regression guard — the already-working subtypes still work:
    assert_eq!(item_result("CInt(1)"), s("a"));
    assert_eq!(item_result("CLng(2)"), s("b"));
    assert_eq!(item_result("1"), s("a"));
}

#[test]
fn collection_remove_accepts_byte_index() {
    let body = "Public r As Variant\nSub Main()\n    Dim c As New Collection\n    c.Add \"a\"\n    c.Add \"b\"\n    c.Remove CByte(1)\n    r = c.Item(1)\nEnd Sub\n";
    let outcome = run(Executor::Vm3, body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let r = outcome
        .result
        .unwrap_or_else(|e| panic!("run failed: {e}"))
        .into_iter()
        .next()
        .expect("global r");
    // After removing item 1 ("a"), item 1 is now "b".
    assert_eq!(r, s("b"));
}
