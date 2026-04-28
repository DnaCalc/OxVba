# XLL Array Marshaling Implementation

Date: 2026-04-28
Beads: `bd-iyx4.2.2`, `bd-iyx4.2.3`
Workset: `docs/worksets/WORKSET_2026-04-28_OXIDE_XLL_ARRAY_APPLICATION_EXECUTION.md`

## Delivered

Generated XLL shims now include bounded `xltypeMulti` support.

Argument path:

- `xll_to_variant` recognizes `XL_TYPE_MULTI`.
- `xll_multi_to_variant` reads `XLArray12.rows`, `XLArray12.columns`, and
  `XLArray12.lparray`.
- Element values are decoded through the existing scalar XLOPER12 decoder.
- The result is a retained `Variant::ArrayVariant` backed by
  `SafeArray::from_variants_nd` with two dimensions and lower bounds of `1`.

Return path:

- `variant_to_xll` recognizes `Variant::as_safearray`.
- `safe_array_to_xll_multi` maps one-dimensional arrays to `rows = len`,
  `columns = 1`, and two-dimensional arrays to the first two bounds.
- Returned array elements are owned by the generated `XllOwnedXloper12`.
- Nested counted-wide strings used by array elements are retained in
  `_array_wide` until `xlAutoFree12` drops the owner.

## Validation

Command:

```powershell
cargo test -p oxvba-build --lib xll --quiet
```

Result:

```text
running 5 tests
.....
test result: ok. 5 passed; 0 failed
```

The slow compile test in that set also compiled the generated source to a
non-empty `.xll` artifact.

## Remaining Scope

This implementation is not yet Excel-host validated. The next bead
(`bd-iyx4.2.4`) owns an array fixture and worksheet-host proof. Nested arrays,
references, object-valued array elements, async arrays, and broad dynamic-array
spill behavior remain outside the current support boundary.
