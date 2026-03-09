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
- `oxvba-com`: emerging shared COM transport/model crate; current extraction target for deeper COM cleanup
- `oxvba-host`: engine orchestration, host policy, project runtime sessions, event dispatch
- `oxvba-cli`: CLI bootstrap/run surface

## Current Dependency Shape

High-level execution path:
- source/project inputs enter through `oxvba-host` or `oxvba-cli`
- `oxvba-compiler` produces `Bytecode`
- `oxvba-vm` and `oxvba-jit` execute compiled subsets
- `oxvba-hal` provides profile/policy-governed host services
- COM-related shared transport/types now begin in `oxvba-com`, while deeper COM execution/state still largely lives in `oxvba-hal` pending extraction

## Important Current Truths

1. `oxvba-hal` is a real workspace crate and part of the active runtime boundary.
2. `oxvba-com` is no longer just historical scaffolding; it now owns shared COM transport primitives, but not yet the full COM bridge implementation.
3. `StandardHostServices` is currently the shared Windows/Linux/macOS adapter core.
4. Windows COM support is active and tested; non-Windows COM remains explicitly unsupported.
5. Host/runtime event ingress exists in two planes:
- project/runtime event routing in `oxvba-host`
- COM callback transport through HAL/adapter state, now with payload-based polling support

## Intended Near-Term Evolution

Near-term architectural direction remains:
- contract HAL back toward host/profile/policy/bootstrap concerns
- extract deeper COM transport/state/metadata ownership from `oxvba-hal` into `oxvba-com`
- keep compiler/VM/host semantics aligned while that extraction happens in staged slices

The implementation still follows `MACH1000_PLAN.md` sequencing, but this file should describe the current code truth first and the destination second.
