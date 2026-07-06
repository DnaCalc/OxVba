use oxvba_differential::{Executor, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

#[test]
fn jit_project_class_new_declines_without_vm_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  r = w.Value\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private m As Long\nPrivate Sub Class_Initialize()\n  m = 42\nEnd Sub\nPublic Property Get Value() As Long\n  Value = m\nEnd Property\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert!(
        vm3.unsupported.is_none(),
        "vm3 should execute the project-object oracle case: {vm3:?}"
    );
    assert!(
        vm3.handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "vm3 handle imbalance: {:?}",
        vm3.handle_balance
    );
    assert_eq!(
        vm3.result.expect("vm3 should complete").first(),
        Some(&canon(&Variant::from_i32(42)))
    );

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    let unsupported = jit
        .unsupported
        .as_deref()
        .expect("JIT should decline project-object construction explicitly");
    assert!(
        unsupported.contains("NewObject") && unsupported.contains("VM3-only"),
        "unexpected JIT unsupported diagnostic: {unsupported}"
    );
    assert!(
        matches!(jit.result.as_ref(), Ok(values) if values.is_empty()),
        "JIT decline must not return the VM3 result: {jit:?}"
    );
    assert!(
        jit.handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "jit decline handle imbalance: {:?}",
        jit.handle_balance
    );
}
