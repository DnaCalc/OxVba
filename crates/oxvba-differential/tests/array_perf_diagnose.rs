//! DIAGNOSTIC (ignored): measure per-element array-access scaling to locate the
//! residual O(N) the OxForms min-of-trials still shows after the first bd-us4v fix.
//! Compares a module-level dynamic array (the `array_get_fast`/`array_set_fast`
//! shape), a CLASS-INSTANCE-FIELD array (the OxForms shape), and a UDT fixed-array
//! field (`rec.arr(i)`). A rising per-element curve means that shape is cloning or
//! materializing the whole array per element.
//!
//! Run: cargo test -p oxvba-differential --test array_perf_diagnose -- --ignored --nocapture

use std::time::Instant;

use oxvba_differential::{Executor, run, run_modules};
use oxvba_symbol::manifest::ModuleKind;

fn module_level_src(n: usize) -> String {
    format!(
        "Public total As Long\n\
         Sub Main()\n\
         \u{20}   Dim a() As Long, i As Long, s As Long\n\
         \u{20}   ReDim a(0 To {n} - 1)\n\
         \u{20}   For i = 0 To {n} - 1\n\
         \u{20}       a(i) = i\n\
         \u{20}   Next i\n\
         \u{20}   For i = 0 To {n} - 1\n\
         \u{20}       s = s + a(i)\n\
         \u{20}   Next i\n\
         \u{20}   total = s\n\
         End Sub\n"
    )
}

fn class_field_main() -> &'static str {
    "Public total As Long\n\
     Sub Main()\n\
     \u{20}   Dim o As Thing\n\
     \u{20}   Set o = New Thing\n\
     \u{20}   o.Setup\n\
     \u{20}   total = o.SumAll()\n\
     End Sub\n"
}

fn class_field_cls(n: usize) -> String {
    format!(
        "Private m() As Long\n\
         Public Sub Setup()\n\
         \u{20}   Dim i As Long\n\
         \u{20}   ReDim m(0 To {n} - 1)\n\
         \u{20}   For i = 0 To {n} - 1\n\
         \u{20}       m(i) = i\n\
         \u{20}   Next i\n\
         End Sub\n\
         Public Function SumAll() As Long\n\
         \u{20}   Dim i As Long, s As Long\n\
         \u{20}   For i = 0 To {n} - 1\n\
         \u{20}       s = s + m(i)\n\
         \u{20}   Next i\n\
         \u{20}   SumAll = s\n\
         End Function\n"
    )
}

fn udt_field_src(n: usize) -> String {
    let upper = n - 1;
    format!(
        "Type T\n\
         \u{20}   arr(0 To {upper}) As Long\n\
         End Type\n\
         Public total As Long\n\
         Sub Main()\n\
         \u{20}   Dim rec As T\n\
         \u{20}   Dim i As Long\n\
         \u{20}   For i = 0 To {upper}\n\
         \u{20}       rec.arr(i) = i\n\
         \u{20}   Next i\n\
         \u{20}   For i = 0 To {upper}\n\
         \u{20}       total = total + rec.arr(i)\n\
         \u{20}   Next i\n\
         End Sub\n"
    )
}

fn time_ms(f: impl Fn()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    best
}

#[test]
#[ignore = "diagnostic: prints per-element scaling, not an assertion"]
fn array_access_scaling_module_vs_class_field() {
    // The per-element trend (doubling N ~doubles class-field per-element cost) is
    // unmistakable by N=1000; N=2000 is omitted to keep the manual run snappy.
    for &n in &[250usize, 500, 1000] {
        let mod_src = module_level_src(n);
        let mod_ms = time_ms(|| {
            let o = run(Executor::Vm3, &mod_src);
            assert!(o.unsupported.is_none() && o.result.is_ok());
        });

        let cls = class_field_cls(n);
        let modules = [
            ("Main", ModuleKind::Procedural, class_field_main()),
            ("Thing", ModuleKind::Class, cls.as_str()),
        ];
        let cls_ms = time_ms(|| {
            let o = run_modules(Executor::Vm3, &modules, "Bench");
            assert!(
                o.unsupported.is_none(),
                "class run unsupported: {:?}",
                o.unsupported
            );
            assert!(o.result.is_ok(), "class run failed: {:?}", o.result);
        });

        let udt_src = udt_field_src(n);
        let udt_ms = time_ms(|| {
            let o = run(Executor::Vm3, &udt_src);
            assert!(
                o.unsupported.is_none(),
                "UDT run unsupported: {:?}",
                o.unsupported
            );
            assert!(o.result.is_ok(), "UDT run failed: {:?}", o.result);
        });

        // 2N element-ops per run (fill + read). per-element microseconds.
        let per = |ms: f64| ms * 1000.0 / (2.0 * n as f64);
        println!(
            "N={n:>5}  module={mod_ms:>9.3}ms ({:>7.3}us/elem)   class-field={cls_ms:>9.3}ms ({:>7.3}us/elem)   udt-field={udt_ms:>9.3}ms ({:>7.3}us/elem)",
            per(mod_ms),
            per(cls_ms),
            per(udt_ms),
        );
    }
}
