# WORKSET_2026-03-01_FINANCIAL_HOTPATH_PERF_V169.md

## Objective

Execute profile scope `v169`: optimize financial intrinsic hot-path execution while preserving deterministic tolerance/error signaling behavior.

## Scope

In scope for `v169`:
- improve `Rate` solver hot path by replacing per-iteration numeric finite-difference derivative with analytic derivative + stable near-zero fallback;
- preserve bounded iteration and deterministic error-tag behavior;
- add formal checks for derivative-path presence.

Out of scope:
- changing financial surface semantics;
- widening financial domain beyond current subset.

## Deliverables

- VM optimization:
  - `crates/oxvba-vm/src/interpreter.rs`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V169.md`

## Closure Conditions

Profile `v169` is complete when `Rate` derivative computation avoids repeated finite-difference evaluation on normal paths and all lanes stay green.
