//! End-to-end: VBA source → front-end bound HIR → clean bundle (this crate) →
//! run on oxvba-vm2.

use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;

use crate::lower_module;

/// Lower + run, then read the entry procedure's first frame-local (slot
/// `global_count`) as a number. FIDELITY: arithmetic results are `Double` and a
/// fixed-type (`As Long`) target is not yet narrowed by `CoerceNumeric`, so we
/// compare numerically rather than by exact VBA subtype.
fn run_first_local(src: &str) -> Option<f64> {
    let bundle = lower_module("Mod1", src).expect("lower");
    let first_local = bundle.global_count;
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let vm = oxvba_vm2::run(&bundle, &host).expect("run");
    let value = vm.slot(first_local)?;
    value
        .as_f64()
        .or_else(|| value.as_i32().map(f64::from))
        .or_else(|| value.as_i64().map(|v| v as f64))
}

#[test]
fn arithmetic_precedence() {
    let src = "Sub Main()\r\n    Dim x As Long\r\n    x = 1 + 2 * 3\r\nEnd Sub\r\n";
    assert_eq!(run_first_local(src), Some(7.0));
}

#[test]
fn for_loop_sum() {
    let src = "Sub Main()\r\n\
        \x20   Dim total As Long\r\n\
        \x20   Dim i As Long\r\n\
        \x20   total = 0\r\n\
        \x20   For i = 1 To 5\r\n\
        \x20       total = total + i\r\n\
        \x20   Next i\r\n\
        End Sub\r\n";
    assert_eq!(run_first_local(src), Some(15.0));
}

#[test]
fn if_branch() {
    let src = "Sub Main()\r\n\
        \x20   Dim x As Long\r\n\
        \x20   x = 0\r\n\
        \x20   If 2 > 1 Then\r\n\
        \x20       x = 42\r\n\
        \x20   Else\r\n\
        \x20       x = 7\r\n\
        \x20   End If\r\n\
        End Sub\r\n";
    assert_eq!(run_first_local(src), Some(42.0));
}

#[test]
fn do_while_loop() {
    let src = "Sub Main()\r\n\
        \x20   Dim n As Long\r\n\
        \x20   n = 0\r\n\
        \x20   Do While n < 10\r\n\
        \x20       n = n + 1\r\n\
        \x20   Loop\r\n\
        End Sub\r\n";
    assert_eq!(run_first_local(src), Some(10.0));
}
