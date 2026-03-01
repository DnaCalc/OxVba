# WORKSET_2026-03-01_FORMAL_OBLIGATIONS_UPDATE_V162.md

## Objective

Execute profile scope `v162`: add formal/Kani obligations for newly introduced runtime paths in recent profiles, with non-blocking policy preserved.

## Scope

In scope for `v162`:
- add Kani harnesses for:
  - `Rate` non-convergence/invalid-path error-tag signaling,
  - `NPer` invalid-domain error-tag signaling,
  - `VarType` intrinsic output-domain constraints;
- register these as active formal obligations;
- add host-formal checks ensuring the new harnesses are present.

Out of scope:
- making Kani success blocking for profile completion;
- exhaustive oracle-level semantic proof closure.

## Deliverables

- Formal harness updates:
  - `crates/oxvba-vm/src/interpreter.rs`
- Formal obligation registry updates:
  - `docs/evidence/formal/obligations.csv`
- Host/formal checks:
  - `crates/oxvba-host/src/engine.rs`
- Profile artifacts:
  - `docs/profile-status/PROFILE_STATUS_V162.md`
  - `docs/evidence/profiles/v162/`

## Closure Conditions

Profile `v162` is complete when:
1. the new Kani harnesses are in-tree and referenced in obligations,
2. formal lane remains green under non-blocking policy,
3. profile evidence artifacts are published.
