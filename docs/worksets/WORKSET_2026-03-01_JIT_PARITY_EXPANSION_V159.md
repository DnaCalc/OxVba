# WORKSET_2026-03-01_JIT_PARITY_EXPANSION_V159.md

## Objective

Execute profile scope `v159`: harden JIT parity for newly added runtime behavior by explicitly validating fallback routing and VM-equivalence when Cranelift does not support the bytecode surface.

## Scope

In scope for `v159`:
- add targeted JIT tests proving unsupported bytecode fallback remains active for:
  - financial tolerance/error-tag paths (`Rate`/`NPer` non-convergence),
  - `VarType`/`IsNumeric` sentinel-tag introspection paths;
- add VM-vs-JIT equivalence tests at host level for these source programs;
- publish profile evidence showing conformance parity across backends.

Out of scope:
- expanding Cranelift supported-op surface for these intrinsics (explicitly deferred);
- host-oracle parity tuning.

## Deliverables

- JIT/host updates:
  - `crates/oxvba-jit/src/lib.rs`
  - `crates/oxvba-host/src/engine.rs`
- Evidence/docs:
  - `docs/evidence/formal/obligations.csv`
  - `docs/evidence/language/COVERAGE_INDEX.csv`
  - profile gate artifacts under `docs/evidence/profiles/v159/`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V159.md`

## Closure Conditions

Profile `v159` is complete when:
1. fallback classification and VM/JIT parity are tested for the newly concrete runtime paths,
2. conformance VM/JIT lanes remain green with updated corpus,
3. profile status and obligations are updated with passing evidence.
