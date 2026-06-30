//! Class-instance-field array access on vm3 (bd-us4v round 2).
//!
//! Round 1 made arrays in module/local/temp SLOTS O(1) per element; this round
//! makes arrays held as CLASS-INSTANCE FIELDS (`Private mX()` in a `.cls`, the
//! OxForms shape) O(1) too, via the fused `FieldArrayGet`/`FieldArraySet`
//! instructions that read/write one element through the field's SAFEARRAY
//! descriptor instead of cloning the whole field array per access.
//!
//! These tests pin both correctness (fill via field-array writes, read back via
//! field-array reads, with the right values) and the performance regression
//! (a 2000-element class-field fill+read must be milliseconds, not the tens of
//! seconds the O(N²) defect took).

use std::time::{Duration, Instant};

use oxvba_differential::{Canon, Executor, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind;

fn run_thing(n: usize) -> (Vec<Canon>, Duration) {
    let main = "\
Public total As Long
Public first As Long
Public last As Long
Sub Main()
    Dim o As Thing
    Set o = New Thing
    o.Fill 2000
    total = o.Sum()
    first = o.At(0)
    last = o.At(1999)
End Sub
";
    let cls = format!(
        "Private m() As Long\n\
         Private cnt As Long\n\
         Public Sub Fill(nn As Long)\n\
         \u{20}   cnt = {n}\n\
         \u{20}   ReDim m(0 To cnt - 1)\n\
         \u{20}   Dim i As Long\n\
         \u{20}   For i = 0 To cnt - 1\n\
         \u{20}       m(i) = i * 2\n\
         \u{20}   Next i\n\
         End Sub\n\
         Public Function Sum() As Long\n\
         \u{20}   Dim i As Long, s As Long\n\
         \u{20}   For i = 0 To cnt - 1\n\
         \u{20}       s = s + m(i)\n\
         \u{20}   Next i\n\
         \u{20}   Sum = s\n\
         End Function\n\
         Public Function At(ix As Long) As Long\n\
         \u{20}   At = m(ix)\n\
         End Function\n"
    );
    let modules = [
        ("Main", ModuleKind::Procedural, main),
        ("Thing", ModuleKind::Class, cls.as_str()),
    ];
    let start = Instant::now();
    let outcome = run_modules(Executor::Vm3, &modules, "Bench");
    let elapsed = start.elapsed();
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    let snap = outcome.result.unwrap_or_else(|e| panic!("vm3 run failed: {e}"));
    (snap, elapsed)
}

#[test]
fn class_field_array_read_write_is_correct_and_o1() {
    let (snap, elapsed) = run_thing(2000);
    let has = |v: i32| snap.contains(&canon(&Variant::from_i32(v)));
    // m(i) = i*2 over 0..1999; Sum = 2 * (1999*2000/2) = 3_998_000.
    assert!(has(3_998_000), "Sum over the field array should be 3998000: {snap:?}");
    assert!(has(0), "first = m(0) = 0: {snap:?}");
    assert!(has(3_998), "last = m(1999) = 3998: {snap:?}");
    // O(1) field-element access → a 2000-element class-field fill+read is milliseconds;
    // the O(N²) defect this guards took tens of seconds.
    assert!(
        elapsed < Duration::from_secs(3),
        "class-field fill+read of 2000 elements took {elapsed:?}; field-array element access \
         must be O(1) (bd-us4v round 2) — a multi-second time means it regressed to O(N)"
    );
}
