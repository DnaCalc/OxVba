# COM Early Binding Open Questions V416

Status: ctive
Date: 2026-03-05

## Purpose

Record explicit planning-stage open questions for the 407..v416 early-binding workset closure.

## Open Questions

1. Exact Office parity for typelib version fallback around LoadRegTypeLib when nearest compatible versions exist.
2. Policy default for dual-interface fallback under partially-valid vtable metadata (strict reject vs controlled dispatch fallback).
3. Cache invalidation trigger shape for host-injected reference maps when host-level references mutate without source edits.
4. Deterministic normalization rules for importlib/path identity across case/canonicalization differences.

## Tracking

- HAL implementation-defined / uncertainty registers:
  - docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md
  - docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md
- Deferred oracle tracker:
  - docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv

## Closure Criteria

Questions can move to
esolved when:
- a normative source anchor is added, or
- an implementation-defined policy is approved and documented with executable conformance expectations.
