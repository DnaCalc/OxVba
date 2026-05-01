# Numeric Stress Cases Evidence

Date: 2026-05-01
Bead: `bd-9xmu.4.3` / `stress-002`
Workset: `WORKSET_2026-04-30_CORRECTNESS_CORPUS_AND_ORACLE_STRESS.md`

## Outcome

Added numeric stress regression `semantics::tests::numeric_stress_rounding_overflow_truncation_edges`.

Covered tripwire rows:

- `Round(19, -1)` tens rounding -> `20`.
- `Long * Long` overflow current behavior -> implementation-defined wrapped
  `Long` tripwire (`50_000 * 50_000 == -1_794_967_296`).
- integer division truncation -> `7 \ 2 == 3`.
- `Mod` -> `7 Mod 2 == 1`.
- exponentiation -> `2 ^ 10 == 1024.0`.
- unary negation of `Null` propagates `Null`.
- `Single + const` uses retained numeric helper and produces a `Double` carrier.

Matrix row `NR-NUM-002` was updated in
`CORRECTNESS_CORPUS_MATRIX_2026-05-01.md`.

## Verification

Passed:

```text
cargo test -p oxvba-vm numeric_stress_rounding_overflow_truncation_edges
cargo check --workspace
```
