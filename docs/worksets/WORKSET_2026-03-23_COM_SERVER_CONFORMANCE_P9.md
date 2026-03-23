# WORKSET: Phase 9 — COM Server Conformance and Out-of-Process Support

**Date:** 2026-03-23
**Phase:** 9
**Status:** Planned
**Depends on:** Phase 7 (COM server DLL generation), Phase 5 (Engine::create_class_instance, invoke_member_on_object)

---

## Objective

Validate COM server output against VB6 ActiveX DLL behavioral parity. Add out-of-process COM server support (`OutputType=ComExe` — VB6 `Type=OleExe`). Harden threading, lifecycle, and error semantics.

---

## Deliverables

### 1. COM server conformance test suite

- Create a VB6-equivalent test project with 2-3 creatable classes
- Build as COM server DLL via `oxvba build`
- Register and instantiate from an external COM client (PowerShell `New-Object` or C# test harness)
- Verify: object creation, method invocation, property get/let, error propagation as HRESULT, cleanup on release

### 2. Out-of-process COM server (`OutputType=ComExe`)

VB6 `Type=OleExe` — runs as separate process, communication via COM marshaling:

- Generates `.exe` with `-Embedding` command-line support
- Implements `CoRegisterClassObject` at startup
- Uses standard COM marshaling (no custom proxy/stub — `IDispatch`-only is auto-marshaled)
- Adds `LocalServer32` registry key instead of `InprocServer32`

### 3. Threading model hardening

- Verify STA enforcement: calls from MTA threads are marshaled correctly
- `CoInitializeEx(COINIT_APARTMENTTHREADED)` in generated shims
- Message pump integration for out-of-process servers

### 4. Lifecycle parity

- `Class_Initialize` / `Class_Terminate` fire correctly on COM create/release
- Global state cleanup when last reference released (`DllCanUnloadNow` returns `S_OK`)
- Process exit when last object released (out-of-process server)

### 5. Error propagation

- VBA `Err.Raise` → `HRESULT` + `IErrorInfo` on the COM boundary
- Map OxVba error codes to well-known HRESULTs where applicable
- `ISupportErrorInfo` implementation for rich error info

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-build/src/comserver_exe.rs` (new) | Out-of-process COM server EXE generation |
| `crates/oxvba-build/src/comserver.rs` | Add ISupportErrorInfo, IErrorInfo propagation |
| `crates/oxvba-build/src/registration.rs` | Add LocalServer32 for out-of-process |
| `crates/oxvba-project/src/model.rs` | Add `OutputType::ComExe` |
| `crates/oxvba-project/src/parse.rs` | Parse `OutputType=ComExe` |
| `tests/com_server_conformance/` (new) | End-to-end COM server tests |

---

## Execution Steps

1. Add `OutputType::ComExe` to model and parser
2. Implement `generate_com_exe_shim` — EXE with `-Embedding` flag, `CoRegisterClassObject`, message pump
3. Add `LocalServer32` registry key generation to `registration.rs`
4. Add `ISupportErrorInfo` and `IErrorInfo` implementation to COM server shim generation
5. Implement VBA error → HRESULT mapping at COM boundary
6. Implement `Class_Terminate` invocation on COM Release reaching zero
7. Add `DllCanUnloadNow` tracking via global reference count
8. Create conformance test project: 2-3 classes with methods, properties, error paths
9. Write conformance tests: PowerShell or C# test harness creates objects, invokes methods, verifies results
10. Threading tests: verify STA enforcement, MTA call marshaling

---

## Closure Conditions

1. In-process COM server: external client can create objects and invoke methods
2. Out-of-process COM server: external client can activate and use objects across process boundary
3. `Class_Initialize`/`Class_Terminate` fire at correct lifecycle points
4. VBA errors propagate as HRESULT + IErrorInfo to COM clients
5. STA threading model enforced — MTA calls are properly marshaled
6. Reference counting correct — DLL unloads / process exits when last reference released

---

## Dependency Graph

```
Phase 1 (DONE) ──► Phase 2 ──────► Phase 6 (CLI: run-project, import-vbp)
                      │
Phase 1 (DONE) ──► Phase 3 ──────► Phase 6 (CLI: build)
                      │
Phase 1 (DONE) ──► Phase 4 ──┐
                              ├──► Phase 7 ──┬──► Phase 8 (XLL/Addin)
Phase 2 ──────────► Phase 5 ──┘              │
                                             └──► Phase 9 (COM Server Conformance + OOP)
```

**Parallelism:** Phases 2, 3, 4 can proceed in parallel (independent code areas). Phase 5 depends on compiled project availability (Phase 2 provides reference resolution, but basic invoke works without it). Phase 6 can start CLI scaffolding in parallel with 2–5 but needs them for full functionality. Phase 7 is the gateway to both 8 (XLL) and 9 (COM server conformance), which can run in parallel with each other.

**COM server work distribution:**
- Phase 1 addendum (in Phase 2): `OutputType::ComServer`, `Instancing` enum, ProgId metadata (project model)
- Phase 3: `ComClassExportEntry` in OxBundle ExportInventory
- Phase 4: `validate_com_class_exports`, `ComClassExportDescriptor` with IDispatch member inventory
- Phase 5: `Engine::create_class_instance`, `Engine::invoke_member_on_object`
- Phase 7: COM server DLL generation (IClassFactory, IDispatch, registration, manifest, IDL/TLB)
- Phase 9: Conformance testing, out-of-process server, threading hardening, error propagation
