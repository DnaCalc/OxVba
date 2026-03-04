# WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_SPEC_CLOSURE_V387_V392

## Scope

Close the formal/spec planning gap for late-bound COM client support before implementation-heavy C2 work.

Profiles covered: `v387..v392`
Terminal gate for this workset: `v392`

## Deliverables

1. Updated COM scope document with explicit late-bound C2 pre/post/failure contracts.
2. Updated COM conformance plan with C2 lanes and artifact schema.
3. HAL COM bridge scope update aligned with current native+fallback behavior and next-step obligations.
4. HAL clause-catalog uplift (`HAL-COM-005`, `HAL-COM-006`) with machine-readable sync.
5. HAL implementation-defined + uncertainty register updates for late-bound boundary behavior.
6. Profile status records `PROFILE_STATUS_V387..V392` and closure evidence note.

## Exit Criteria (`v392`)

- Spec surfaces are internally consistent and cross-linked.
- Clause markdown/csv drift check passes.
- Fast project checks pass.
- Docs index/control files reflect new active ladder and gate.

## Verification Commands

- `./scripts/check-hal-clause-drift.ps1`
- `./scripts/meta-check.ps1 -Fast`
