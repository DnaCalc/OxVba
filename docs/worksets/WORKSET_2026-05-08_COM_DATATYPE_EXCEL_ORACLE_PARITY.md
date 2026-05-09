# Workset: COM datatype Excel-oracle parity

Date: 2026-05-08
Status: complete

## Scope

Verify that newly accepted COM/typelib datatype shapes are not merely loaded as metadata, but match behavior observable from real Excel/VBA code.

## Fixture and harness

- Extended `tools/OxVba.TestEventServer` with datatype probe members for:
  - scalar `VT_I1`, `VT_UI2`, `VT_UI4`, `VT_UI8`, and `VT_DECIMAL`
  - typed SAFEARRAY returns for signed byte, unsigned word, unsigned long, unsigned hyper, and Decimal
  - COM object returns and `VT_RECORD` scalar/SAFEARRAY returns for Variant object/record behavior
- Added oracle runner:
  - `scripts/run-com-datatype-oracle.ps1`
  - It registers the fixture, runs matching Excel/VBA probes, runs OxVba probes through `oxvba-cli`, and emits side-by-side CSV/Markdown evidence.

## Implemented parity fixes

- Internal retained `Variant` now has exact scalar Automation carriers for:
  - `VT_I1` (`SignedByte`)
  - `VT_UI2` (`UnsignedInteger`)
  - `VT_UI4` (`UnsignedLong`)
  - `VT_UI8` (`UnsignedLongLong`)
  - `VT_UINT` (`UnsignedInt`)
- Windows COM scalar result translation preserves these carriers instead of normalizing them to signed `Long`/`LongLong`.
- `VarType` and `CStr`-style text conversion now match Excel for the scalar oracle rows.
- SAFEARRAY `VarType` now uses the array element VARTYPE where retained SAFEARRAY metadata is available.
- Dynamic Variant-held SAFEARRAY indexing now supports `v(LBound(v))` / `v(UBound(v))` for the oracle-shaped runtime-bound array expressions, covering Decimal SAFEARRAY element parity.
- Unsupported signed-byte/unsigned Automation typed SAFEARRAYs exposed to VBA code now surface Excel-compatible runtime error 458.
- `VT_RECORD` scalar and record SAFEARRAY returns now surface Excel-compatible runtime error 13, while ordinary returned COM objects remain usable as object Variants.
- VM and JIT host error routing preserve embedded runtime error codes from COM conversion failures instead of remapping them to generic HAL adapter faults.

## Evidence

Latest evidence:

- `docs/evidence/conformance/oracle_captures/com_datatype_oracle_datatype-objects-records/results.csv`
- `docs/evidence/conformance/oracle_captures/com_datatype_oracle_datatype-objects-records/summary.md`

Current oracle status:

- Scalar rows match Excel/VBA:
  - `VT_I1`: `16:-5`
  - `VT_UI2`: `18:65000`
  - `VT_UI4`: `19:4000000000`
  - `VT_UI8`: `21:9000000000`
  - `VT_DECIMAL`: `14:-123.45`
- Decimal SAFEARRAY row matches Excel/VBA:
  - `VT_ARRAY|VT_DECIMAL`: `8206:0:2:123.45:321`
- Unsupported typed SAFEARRAY rows match Excel/VBA runtime errors:
  - `VT_ARRAY|VT_I1`: `ERR:458`
  - `VT_ARRAY|VT_UI2`: `ERR:458`
  - `VT_ARRAY|VT_UI4`: `ERR:458`
  - `VT_ARRAY|VT_UI8`: `ERR:458`
- Object and record Variant rows match Excel/VBA:
  - returned object: `9:42`
  - `VT_RECORD`: `ERR:13`
  - `VT_ARRAY|VT_RECORD`: `ERR:13`

## Completion

`pwsh -NoProfile -File scripts/run-com-datatype-oracle.ps1 -RunId datatype-objects-records` completed without mismatches. The scoped Excel/VBA oracle parity work for COM Variant scalar, array, object, and record return behavior is complete.
