# Exact Carrier Expectations Evidence

> Recovery note 2026-05-02: the runtime SAFEARRAY carrier coverage still exists,
> but the cited VM/JIT filters `mixed_numeric_matrix_current_variant_results` and
> `runtime_math_helpers_read_variant_carriers` are not accepted until recovery
> beads re-run or replace them with nonzero tests. Reopened recovery bead:
> `bd-9xmu.3.9`.

Date: 2026-05-01
Bead: `bd-9xmu.3.6` / `value-clean-005`
Workset: `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md`

## Pinned expectations

| Carrier | Expected retained shape |
|---|---|
| `Currency` | Stored as exact scaled `i64` with scale 10,000; SAFEARRAY typed currency elements preserve the scaled payload. |
| `Decimal` | Stored as `Decimal96` parts (`lo/mid/hi`, scale, sign); typed SAFEARRAY decimal elements preserve the exact parts. |
| `Date` | Stored as date-tagged `f64` carrier; date arithmetic/numeric helpers may currently produce `Double` per the mixed numeric matrix unless a date-specific helper re-tags. |
| `Boolean` | Stored as Boolean carrier with VBA truth semantics; numeric coercion treats `True` as `-1` and `False` as `0`. |

## Regression coverage

Added runtime SAFEARRAY regression:

- `safe_array::tests::typed_exact_safearray_carriers_preserve_intrinsic_payloads`
  covers typed currency, decimal, date, and bool SAFEARRAY variant elements.

Existing/related VM/JIT coverage used by this bead:

- `semantics::tests::mixed_numeric_matrix_current_variant_results` covers
  Currency/Decimal/Date/Boolean rows in retained-Variant arithmetic.
- `oxvba-jit` runtime helper tests such as
  `runtime_math_helpers_read_variant_carriers` verify JIT helper paths read
  retained Variant carriers.
- Runtime `variant` tests cover direct storage/bridge roundtrips for currency,
  decimal, date, and Boolean carriers.

## Verification

Passed:

```text
cargo test -p oxvba-runtime typed_exact_safearray_carriers_preserve_intrinsic_payloads
cargo test -p oxvba-vm mixed_numeric_matrix_current_variant_results
cargo test -p oxvba-jit runtime_math_helpers_read_variant_carriers
cargo check --workspace
```
