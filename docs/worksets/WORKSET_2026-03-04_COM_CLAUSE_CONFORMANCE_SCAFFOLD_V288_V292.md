# WORKSET_2026-03-04_COM_CLAUSE_CONFORMANCE_SCAFFOLD_V288_V292.md

## Objective

Execute `v288..v292`: build the formal contract and conformance skeleton for Windows COM client/server support.

## Scope

1. Define COM clause IDs and machine-readable catalog.
2. Extend diagnostic/error taxonomy with COM-specific deterministic mappings.
3. Formalize apartment policy and object lifetime invariants.
4. Finalize conformance lane structure and artifact schema.

## Deliverables

- COM clause catalog (`md` + `csv`).
- diagnostic taxonomy updates for COM boundary failures.
- policy/lifecycle contract docs.
- `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md` with runnable lane plan.

## Checks

- clause/catalog drift guards pass.
- docs/reference links resolve.
- no ambiguity in lane ownership (HAL vs host/runtime vs com crate).

## Closure Conditions

`v292` is complete when clause definitions, policy/lifetime contract, and conformance lane model are ready for immediate implementation work (`v293+`).

