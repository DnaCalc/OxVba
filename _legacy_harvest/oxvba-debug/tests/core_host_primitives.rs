#[path = "support_core/mod.rs"]
mod support_core;

#[test]
fn host_debug_runtime_primitives_cover_core_needs() {
    let manifest = support_core::call_manifest();
    let mut runtime = support_core::prepared_runtime(&manifest);
    assert!(!runtime.compiled().bytecode.instructions.is_empty());
    assert!(runtime.debug_vm().debug_snapshot().is_none());
    runtime.debug_vm_mut().debug_set_breakpoints(Vec::new());
}
