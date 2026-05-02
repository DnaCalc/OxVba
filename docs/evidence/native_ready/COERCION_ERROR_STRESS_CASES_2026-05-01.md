# Coercion And Error-State Stress Cases Evidence

> Recovery note 2026-05-02: this evidence is current again after
> `bd-9xmu.4.7`; the cited filter now runs one executable test and passes. See
> `CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md`.

Date: 2026-05-01
Bead: `bd-9xmu.4.4` / `stress-003`
Workset: `WORKSET_2026-04-30_CORRECTNESS_CORPUS_AND_ORACLE_STRESS.md`

## Outcome

Added stress regression
`semantics::tests::coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing`.

Covered rows:

- blank numeric string coerces to zero in arithmetic;
- `Empty` coerces to zero in arithmetic;
- `Null` division propagates `Null` without divide-by-zero error;
- `CVErr` arithmetic reports type mismatch;
- `Null` comparison is deterministic false for the tested equality predicate;
- `Let` assignment into Object target reports assignment-timing error.

Matrix row `NR-COERCE-001` was updated in
`CORRECTNESS_CORPUS_MATRIX_2026-05-01.md`.

## Verification

Passed:

```text
cargo test -p oxvba-vm coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing
cargo check --workspace
```
