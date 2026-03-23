# WORKSET: Phase 5 — Engine::invoke_procedure

**Date:** 2026-03-23
**Phase:** 5
**Status:** Planned
**Depends on:** Phase 2 (reference resolution)

---

## Objective

Add a public `Engine::invoke_procedure` method that invokes a single named procedure with `RuntimeValue` arguments and returns the result. This enables the DLL wrapper use case where external callers invoke individual VBA procedures. Add `Engine::create_class_instance` and `Engine::invoke_member_on_object` for COM server class instantiation and dispatch.

---

## Deliverables

### 1. `Engine::invoke_procedure` in `crates/oxvba-host/src/engine.rs`

```rust
impl Engine {
    pub fn invoke_procedure(
        &self,
        session: &mut ProjectRuntimeSession,
        module: &str,
        procedure: &str,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, PhaseDiagnostic>;
}
```

- Looks up `ProcedureRuntimeMetadata` by lowered key `"module.procedure"`
- Validates argument count matches `metadata.param_slots.len()`
- Calls `session.vm.invoke_procedure_with_values(bytecode, entry_pc, param_slots, args)`
- Reads return value from `metadata.return_slot` (or `RuntimeValue::Empty` for Sub)

### 2. `Engine::compile_and_prepare_session`

Public method to create a `ProjectRuntimeSession` without executing:

```rust
pub fn compile_and_prepare_session(
    &self,
    manifest: &ProjectManifest,
) -> Result<ProjectRuntimeSession, PhaseDiagnostic>;
```

- Compiles project, applies event bindings, preflights, creates VM
- Does NOT execute — allows caller to invoke procedures individually

### 3. Make `ProjectRuntimeSession` fields accessible for invoke

- `session.compiled()` → `&CompiledProject`
- `session.read_slot(&self, slot: usize) → RuntimeValue`

### 4. `Engine::create_class_instance`

For COM server use case, instantiate a VBA class module:

```rust
pub fn create_class_instance(
    &self,
    session: &mut ProjectRuntimeSession,
    class_name: &str,
) -> Result<ObjectHandle, PhaseDiagnostic>;
```

- Allocates an object handle for the class
- Runs `Class_Initialize` if present
- Returns `ObjectHandle` that the COM wrapper can associate with an IDispatch pointer
- This is the engine-side equivalent of `CoCreateInstance` → `IClassFactory::CreateInstance`

### 5. `Engine::invoke_member_on_object`

Dispatch a method/property call on a class instance:

```rust
pub fn invoke_member_on_object(
    &self,
    session: &mut ProjectRuntimeSession,
    object: ObjectHandle,
    member: &str,
    args: &[RuntimeValue],
) -> Result<RuntimeValue, PhaseDiagnostic>;
```

- Looks up member in `ProjectDynamicObjectRoute` for the class
- Routes to the correct procedure entry point
- This is the engine-side equivalent of `IDispatch::Invoke`

---

## Key Existing Code

- `crates/oxvba-host/src/engine.rs:325-391` — `dispatch_host_event_into_runtime` already does per-procedure invocation via `vm.invoke_procedure_with_values(bytecode, entry_pc, param_slots, args)`
- `crates/oxvba-vm/src/interpreter.rs:228-250` — `Vm::invoke_procedure_with_values` — validates arity, writes args to slots, calls `execute_loop(bytecode, entry_pc, ..., return_halts_when_stack_empty=true)`
- `crates/oxvba-host/src/engine.rs:717-753` — `resolve_runtime_handler_metadata` — looks up ProcedureRuntimeMetadata by lowered symbol name
- `crates/oxvba-host/src/engine.rs:498-518` — `start_project_runtime_session` — existing session creation pattern

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-host/src/engine.rs` | Add `invoke_procedure`, `compile_and_prepare_session`, `create_class_instance`, `invoke_member_on_object`, accessor methods on `ProjectRuntimeSession` |
| `crates/oxvba-host/src/lib.rs` | Re-export new methods (already re-exports Engine/ProjectRuntimeSession) |
| `crates/oxvba-host/tests/invoke_procedure_tests.rs` (new) | Per-procedure and per-object invocation tests |

---

## Execution Steps

1. Add `compile_and_prepare_session` by extracting the compile+setup portion of `execute_project_with_snapshot_phased` (without the final `vm.execute` call)
2. Add `invoke_procedure` that resolves metadata, validates args, delegates to `vm.invoke_procedure_with_values`, reads return slot
3. Add `ProjectRuntimeSession::compiled()` and `read_slot()` accessors
4. Add `create_class_instance` — allocate object handle, look up Class_Initialize entry point, invoke if present
5. Add `invoke_member_on_object` — look up member in `ProjectDynamicObjectRoute`, resolve to entry point, invoke with args
6. Write tests: invoke a Sub (no return), invoke a Function (return value), invoke with wrong arity (error), invoke missing procedure (error), invoke multiple procedures sequentially on same session, create class instance and call method on it

---

## Closure Conditions

1. `invoke_procedure` successfully calls a compiled VBA procedure and returns its value
2. Multiple invocations on the same session share VM state (slots persist)
3. Arity mismatch and missing-procedure produce clear diagnostics
4. `create_class_instance` allocates an object and runs `Class_Initialize`
5. `invoke_member_on_object` dispatches method/property calls on a class instance
6. Existing engine tests still pass
