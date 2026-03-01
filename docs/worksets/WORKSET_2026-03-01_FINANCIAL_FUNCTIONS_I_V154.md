# WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_I_V154.md

## Objective

Execute profile scope `v154`: replace remaining projection placeholders for `NPV`, `IRR`, and `MIRR` with deterministic executable algorithmic implementations in the current non-HAL runtime subset.

## Scope

In scope for `v154`:
- add dedicated bytecode/runtime operations for `NPV`, `IRR`, and `MIRR`;
- replace emitter projection lowering (`copy`/`sum-only`) with algorithmic intrinsic lowering;
- provide deterministic numeric algorithms over current scalar-tag domain:
  - discounted-sum `NPV`,
  - Newton-iteration-backed `IRR` synthetic single-flow subset,
  - finance/reinvest-rate adjusted `MIRR` subset;
- refresh compiler/VM/host/conformance/formal evidence for these paths.

Out of scope:
- full VBA financial parity/tolerance model and convergence diagnostics for all edge cases (`v155+v156`);
- full array-cashflow parity with host `Variant` arrays.

## Deliverables

- Compiler/runtime updates:
  - `crates/oxvba-compiler/src/bytecode.rs`
  - `crates/oxvba-compiler/src/emit.rs`
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-compiler/src/lib.rs`
  - `crates/oxvba-host/src/engine.rs`
- Conformance:
  - updated `conformance/golden/smoke.csv` financial expansion expectations
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/language/NON_HAL_COMPLETION_BACKLOG_2026-03-01.md`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
  - profile gate artifacts under `docs/evidence/profiles/v154/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V154.md`

## Closure Conditions

Profile `v154` is complete when:
1. `NPV`/`IRR`/`MIRR` execute via explicit algorithmic runtime operations (not projection passthroughs),
2. VM/JIT conformance remains green with updated financial outputs,
3. formal/profile/evidence artifacts are updated for the new behavior.
