# UDT Semantic And Layout Non-Claim Stress Evidence

> Recovery note 2026-05-02: this evidence is not currently accepted as an
> executable gate. The cited `nested_udt` host test filter currently runs zero
> tests. Reopened recovery bead: `bd-9xmu.4.7`.

Date: 2026-05-01
Bead: `bd-9xmu.4.5` / `stress-004`
Workset: `WORKSET_2026-04-30_CORRECTNESS_CORPUS_AND_ORACLE_STRESS.md`

## Outcome

Matrix row `NR-UDT-001` now records the active UDT semantic stress lane:

- `nested_udt_field_access_integration` covers nested UDT field access through
  the current flattened-field subset.
- `nested_udt_cross_type_rejection` covers same-shape/different-declared-type
  rejection behavior.
- existing formal whole-copy fixtures cover same-declared-type field-copy
  lowering.

## Claim boundary

These cases prove the bounded non-boundary semantic subset only:

- UDT declarations are parsed;
- nested field aliases execute through current compiler lowering;
- same-declared-type whole copy is allowed;
- same-shape cross-type copy does not imply compatibility.

They do **not** claim native memory layout, packing/alignment parity, UDT ByRef
`Declare` marshaling, COM record transport, or arbitrary struct overlay parity.
Those residuals are classified in
`UDT_NATIVE_ABI_RESIDUAL_CLASSIFICATION_2026-05-01.md`.

## Verification

Passed:

```text
cargo test -p oxvba-host nested_udt
cargo check --workspace
```
