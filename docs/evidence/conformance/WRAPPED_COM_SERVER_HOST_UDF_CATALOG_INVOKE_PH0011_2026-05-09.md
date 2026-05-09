# WrappedComServer host UDF catalog/invoke PH-0011 evidence

Date: 2026-05-09
Bead: `bd-wcs1.9.2`
Matrix row: `PH-0011`

## Scope

This evidence covers the first typed host UDF API over prepared project runtime
sessions. It proves that DnaOneCalc/OxIde-style hosts can enumerate public
procedural functions through a typed catalog and invoke a catalog entry with a
caller context through the existing VM procedure execution path.

This is not a full worksheet calculation harness, array/error return parity
claim, or thread-safe UDF claim.

## Commands

```powershell
cargo test -p oxvba-host --test invoke_procedure_tests host_udf --quiet
```

## Verified behavior

- `Engine::host_udf_catalog` returns a typed `HostUdfCatalog` containing public
  procedural functions only.
- Public procedural `Sub` exports are excluded from the UDF catalog.
- Private procedures and class-module functions are excluded from the UDF
  catalog by the existing project host-export rules.
- Catalog entries include stable host-call IDs, project/module/procedure
  identity, argument names, argument types, return type, conservative volatility
  and dependency policy, side-effect policy, thread-safety policy, and allowed
  host contexts.
- `Engine::invoke_host_udf_with_variants` resolves a stable host-call ID,
  rejects non-function host-call IDs, and routes function execution through the
  existing prepared-session `invoke_procedure_with_variants` path.
- `HostUdfCallContext` carries caller address, locale, dependency tokens, and a
  volatile-request sink flag into the host UDF invocation result shape.
- The focused test invokes `HostAdd(2, 5)` from caller `Sheet1!A1` and receives
  scalar result `7` while preserving caller, volatile-request, and dependency
  token metadata in `HostUdfInvokeResult`.

## Residual

`PH-0011` remains `in-progress`. This subset proves typed catalog enumeration
and scalar function invocation with caller/dependency/volatile context shape.
Remaining work includes richer scalar coercion rules, array and error returns,
explicit volatile/dependency behavior inside hosted worksheet semantics, and a
DnaOneCalc/OxIde-style host-context harness.
