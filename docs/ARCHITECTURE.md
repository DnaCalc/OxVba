# Architecture

## Current Workspace

Workspace crates and current roles:
- `oxvba-syntax`: lexer/parser and syntax-tree infrastructure
- `oxvba-runtime`: runtime value/tag/SAFEARRAY support
- `oxvba-ir`: HIR/MIR/CFG lowering structures
- `oxvba-compiler`: resolve/typecheck/project lowering/bytecode emission
- `oxvba-vm`: register-slot interpreter
- `oxvba-jit`: JIT backend subset
- `oxvba-hal`: host/profile/policy boundary plus current shared adapter core
- `oxvba-com`: emerging COM bridge crate; now owns shared COM transport and typelib data models and is the extraction target for deeper COM cleanup
- `oxvba-host`: engine orchestration, host policy, project runtime sessions, event dispatch
- `oxvba-cli`: CLI bootstrap/run surface

## Current Dependency Shape

High-level execution path:
- source/project inputs enter through `oxvba-host` or `oxvba-cli`
- `oxvba-compiler` produces `Bytecode`
- `oxvba-vm` and `oxvba-jit` execute compiled subsets
- `oxvba-hal` provides profile/policy-governed host services
- OxVba semantic values remain the canonical runtime value model for compiler/VM/host coordination
- `oxvba-com` is the COM boundary crate that should translate those values to and from COM wire representations (`VARIANT`, `BSTR`, `SAFEARRAY`, `IDispatch`, event payload transport)
- COM-related shared transport/types plus deterministic typelib catalog/metadata construction now begin in `oxvba-com`, while deeper COM execution/state and transitional cache ownership still largely live in `oxvba-hal` pending extraction
- the surviving HAL-facing COM seam is now narrower:
  - `ComHal` carries current activation/invoke/event/object-description/typelib hooks
  - the old parallel `TypeLibraryHal` surface has been collapsed away

## Important Current Truths

1. `oxvba-hal` is a real workspace crate and part of the active runtime boundary.
2. `oxvba-com` is no longer just historical scaffolding; it now owns shared COM transport primitives, object-descriptor types, and deterministic typelib catalog/build logic, but not yet the full COM bridge implementation.
3. `StandardHostServices` is currently the shared Windows/Linux/macOS adapter core.
4. Windows COM support is active and tested; non-Windows COM remains explicitly unsupported.
5. Host/runtime event ingress exists in two planes:
- project/runtime event routing in `oxvba-host`
- COM callback transport through HAL/adapter state, now with payload-based polling support
6. The current COM blocker is architectural as well as behavioral:
- the invoke path still leans on a lossy `i32` value lane,
- richer COM value transport therefore cannot close honestly until a canonical OxVba-side carrier replaces that lossy path,
- that carrier should stay semantic-value-oriented rather than becoming raw COM wire types inside the VM/compiler boundary.

## Intended Near-Term Evolution

Near-term architectural direction remains:
- contract HAL back toward host/profile/policy/bootstrap concerns
- extract deeper COM transport/state/metadata ownership from `oxvba-hal` into `oxvba-com`
- introduce a richer OxVba-side external value carrier so compiler/VM/host stay on semantic values while `oxvba-com` handles COM translation
- keep compiler/VM/host semantics aligned while that extraction happens in staged slices

The implementation still follows `MACH1000_PLAN.md` sequencing, but this file should describe the current code truth first and the destination second.
