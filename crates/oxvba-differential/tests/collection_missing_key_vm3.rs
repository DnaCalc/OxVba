//! vm3 built-in `Collection` missing entries should raise error 9.

use oxvba_differential::{run, Executor};

fn error_number(body: &str) -> i32 {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(outcome.raised, "expected error, got {:?}", outcome.result);
    outcome.err.number
}

#[test]
fn collection_item_missing_key_raises_9() {
    assert_eq!(
        error_number(
            "    Dim c As New Collection\n    c.Add 10, \"present\"\n    r = c.Item(\"missing\")\n"
        ),
        9
    );
}

#[test]
fn collection_default_member_missing_key_raises_9() {
    assert_eq!(
        error_number(
            "    Dim c As New Collection\n    c.Add 10, \"present\"\n    r = c(\"missing\")\n"
        ),
        9
    );
}

#[test]
fn collection_remove_missing_key_raises_9() {
    assert_eq!(
        error_number(
            "    Dim c As New Collection\n    c.Add 10, \"present\"\n    c.Remove \"missing\"\n"
        ),
        9
    );
}
