# Workset: COM datatype Excel-oracle parity

Date: 2026-05-08
Status: in-progress

## Scope

Verify that newly accepted COM/typelib datatype shapes are not merely loaded as metadata, but match behavior observable from real Excel/VBA code.

## Fixture and harness

- Extended `tools/OxVba.TestEventServer` with datatype probe members for:
  - scalar `VT_I1`, `VT_UI2`, `VT_UI4`, `VT_UI8`, and `VT_DECIMAL`
  - typed SAFEARRAY returns for signed byte, unsigned word, unsigned long, unsigned hyper, and Decimal
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

## Evidence

Latest evidence:

- `docs/evidence/conformance/oracle_captures/com_datatype_oracle_datatype-after-scalar-vartype/results.csv`
- `docs/evidence/conformance/oracle_captures/com_datatype_oracle_datatype-after-scalar-vartype/summary.md`

Current oracle status:

- Scalar rows match Excel/VBA:
  - `VT_I1`: `16:-5`
  - `VT_UI2`: `18:65000`
  - `VT_UI4`: `19:4000000000`
  - `VT_UI8`: `21:9000000000`
  - `VT_DECIMAL`: `14:-123.45`
- Remaining mismatches are typed SAFEARRAY rows:
  - Excel/VBA raises runtime error 458 for signed-byte and unsigned typed SAFEARRAYs returned through Automation.
  - OxVba currently accepts those typed SAFEARRAYs as retained `SafeArray` values.
  - Excel/VBA accepts Decimal SAFEARRAYs and permits element access; OxVba now reports `VarType`/bounds correctly, but the current dynamic array-index expression path does not yet cover `v(LBound(v))` for Variant-held SAFEARRAYs in the oracle probe.

## Remaining required work

1. Decide and implement Excel-compatible policy for unsupported Automation typed SAFEARRAY returns (`VT_I1`, `VT_UI2`, `VT_UI4`, `VT_UI8`, and related unsigned element types): they should surface Excel-compatible error 458 when exposed to VBA code.
2. Extend dynamic Variant-held SAFEARRAY indexing so `v(LBound(v))` and `v(UBound(v))` work for COM-returned SAFEARRAYs, including Decimal arrays.
3. Rerun `scripts/run-com-datatype-oracle.ps1` without `-AllowMismatch`; this workset is complete only when the oracle CSV has no mismatches.
