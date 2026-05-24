# COM And Future XLL Wrapper Substrate Alignment

Date: 2026-05-24
Bead: `bd-hjys.12`

## Implementation summary

Added substrate alignment helpers/tests in `crates/oxvba-build/src/wrapper_plan.rs`:

- `com_server_plan_from_bundle(...)`
- `future_xll_plan_from_reflection(...)`
- `FutureXllRegistrationPlaceholder`

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| COM wrapper generation uses descriptor inventory where available and avoids reparsing callable facts. | `com_server_plan_uses_bundle_descriptor_inventory` builds a plan from `OxBundle::project_reflection()` and checks descriptor fingerprints; `com_plan_reports_missing_descriptor_inventory` requires explicit inventory availability. |
| Future XLL plan can represent callable selection, registration metadata placeholders, and conversion lanes. | `future_xll_plan_represents_placeholders_and_defers_execution` checks `FutureXll`, explicit callable IDs, placeholder type text, and `TypedScalarFirstTier`. |
| XLL execution and Excel registration remain explicitly deferred to a future workset. | Future XLL placeholders carry `execution_deferred = true` and `excel_registration_deferred = true`; no Excel registration execution is added. |
| Suite H rows are covered by tests or documented checks. | The three `substrate_alignment_tests` cover COM descriptor inventory use, legacy inventory failure, and future XLL placeholder/deferred behavior. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-build substrate_alignment_tests -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- COM and future-XLL profiles now share the neutral wrapper plan substrate.
- Future XLL remains a representation/projection only; no XLL execution or Excel registration parity is claimed.
