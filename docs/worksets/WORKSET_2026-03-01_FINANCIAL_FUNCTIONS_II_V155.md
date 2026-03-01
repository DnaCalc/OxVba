# WORKSET_2026-03-01_FINANCIAL_FUNCTIONS_II_V155.md

## Objective

Execute profile scope `v155`: replace `Rate`/`NPer` projection placeholders with deterministic algorithmic runtime implementations.

## Scope

In scope for `v155`:
- add dedicated bytecode/runtime operations for `Rate` and `NPer`;
- replace emitter passthrough lowering (`copy pv`) with intrinsic lowering that consumes full argument sets;
- implement solver/math formulas in VM for deterministic subset behavior:
  - Newton-based `Rate` solving with bounded iteration and numeric derivative,
  - logarithmic-form `NPer` solving with guarded domains.

Out of scope:
- full VBA tolerance/convergence diagnostic model (continued in `v156`);
- full parity for all sign-convention edge cases versus Excel oracle.

## Deliverables

- Compiler/runtime updates:
  - `crates/oxvba-compiler/src/bytecode.rs`
  - `crates/oxvba-compiler/src/emit.rs`
  - `crates/oxvba-vm/src/interpreter.rs`
  - `crates/oxvba-host/src/engine.rs`
  - `crates/oxvba-compiler/src/lib.rs`
- Conformance:
  - updated `conformance/golden/smoke.csv` expected outputs for financial expansion fixture
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`
  - profile gate artifacts under `docs/evidence/profiles/v155/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V155.md`

## Closure Conditions

Profile `v155` is complete when:
1. `Rate`/`NPer` execute through explicit algorithmic runtime paths (no projection fallback),
2. VM/JIT conformance stays green with updated fixture outputs,
3. formal/profile/evidence artifacts are updated for the new behavior.
