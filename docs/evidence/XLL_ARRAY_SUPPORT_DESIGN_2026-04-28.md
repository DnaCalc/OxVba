# XLL Array Support Design

Date: 2026-04-28
Bead: `bd-iyx4.2.1`
Workset: `docs/worksets/WORKSET_2026-04-28_OXIDE_XLL_ARRAY_APPLICATION_EXECUTION.md`

## Supported Shape

The generated XLL shim will support Excel `xltypeMulti` values through the
existing retained OxVba `Variant` / `SafeArray` carrier.

Argument direction:

- `xltypeMulti` arguments decode to `Variant::ArrayVariant`.
- Elements decode through the same scalar XLOPER12 cases as scalar arguments:
  number, string, boolean, int, error, nil, and missing.
- The Excel row/column shape is preserved as a two-dimensional `SafeArray`
  with lower bounds of `1` for both dimensions.

Return direction:

- `Variant::ArrayVariant` returns marshal to owned `xltypeMulti`.
- One-dimensional `SafeArray` values return as `rows = len`, `columns = 1`.
- Two-dimensional `SafeArray` values return as `rows = bounds[0].count`,
  `columns = bounds[1].count`.
- Element order is row-major, matching the generated `XLArray12.lparray`
  layout used by the shim.

## Lifetime Rule

Microsoft documents `xlAutoFree12` as the callback Excel uses when an XLL
returns an XLOPER12 with the DLL-free flag, and its framework example frees
nested `xltypeMulti` elements before freeing the array buffer. The generated
shim therefore owns:

- the top-level returned `XLOPER12`,
- the top-level string buffer when returning a scalar string,
- the `xltypeMulti` element buffer,
- and any counted-wide string buffers referenced by array elements.

Dropping the generated owner in `xlAutoFree12` releases those owned Rust
buffers. Excel-allocated values remain outside this owner path.

Reference:
<https://learn.microsoft.com/en-us/office/client-developer/excel/freexlopert-freexloper12t>

## Explicit Boundaries

This bead family does not yet claim:

- nested arrays as element values,
- references (`xltypeRef` / `xltypeSRef`) unless later coerced to
  `xltypeMulti`,
- object values inside XLL arrays,
- asynchronous return arrays,
- or complete Excel dynamic-array spill behavior across all Excel versions.

These are residuals for future beads if they become required by the XLL product
surface.
