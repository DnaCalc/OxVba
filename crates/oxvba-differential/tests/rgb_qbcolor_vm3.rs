//! `RGB` / `QBColor` colour intrinsics on vm3 (gap: rgb-qbcolor-absent).
//!
//! All expected values are live-verified against VBA 7.1:
//! `RGB(255,255,255)=16777215`, `RGB(0,0,1)=65536`, components clamp high to 255
//! (`RGB(256,300,1000)=16777215`), and the 16-entry `QBColor` palette
//! (`QBColor(1)=8388608`, `QBColor(12)=255`, …). An out-of-range `QBColor` index
//! raises "Invalid procedure call or argument" (error 5).

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
        .unwrap_or_else(|e| panic!("vm3 run failed: {e}\n{body}"))
}

fn has_long(snap: &[Canon], value: i32) -> bool {
    snap.contains(&canon(&Variant::from_i32(value)))
}

#[test]
fn rgb_packs_and_clamps_components() {
    let snap = snapshot(
        "    Dim white As Long, blue As Long, clamped As Long, mid As Long\n\
         \u{20}   white = RGB(255, 255, 255)\n\
         \u{20}   blue = RGB(0, 0, 1)\n\
         \u{20}   clamped = RGB(256, 300, 1000)\n\
         \u{20}   mid = RGB(128, 128, 128)",
    );
    assert!(
        has_long(&snap, 16_777_215),
        "RGB(255,255,255)=16777215: {snap:?}"
    );
    assert!(has_long(&snap, 65_536), "RGB(0,0,1)=65536: {snap:?}");
    // clamped is also 16777215 (covered above); the distinct mid value:
    assert!(
        has_long(&snap, 8_421_504),
        "RGB(128,128,128)=8421504: {snap:?}"
    );
}

#[test]
fn qbcolor_returns_the_legacy_palette() {
    let snap = snapshot(
        "    Dim c1 As Long, c7 As Long, c12 As Long, c15 As Long\n\
         \u{20}   c1 = QBColor(1)\n\
         \u{20}   c7 = QBColor(7)\n\
         \u{20}   c12 = QBColor(12)\n\
         \u{20}   c15 = QBColor(15)",
    );
    assert!(has_long(&snap, 8_388_608), "QBColor(1)=8388608: {snap:?}");
    assert!(has_long(&snap, 12_632_256), "QBColor(7)=12632256: {snap:?}");
    assert!(has_long(&snap, 255), "QBColor(12)=255: {snap:?}");
    assert!(
        has_long(&snap, 16_777_215),
        "QBColor(15)=16777215: {snap:?}"
    );
}

#[test]
fn qbcolor_out_of_range_raises_5() {
    let outcome = run(
        Executor::Vm3,
        "Sub Main()\n    Dim x As Long\n    x = QBColor(99)\nEnd Sub\n",
    );
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "QBColor(99) should raise, got {:?}",
        outcome.result
    );
    assert_eq!(
        outcome.err.number, 5,
        "QBColor out-of-range → 5; err={:?}",
        outcome.err
    );
}
