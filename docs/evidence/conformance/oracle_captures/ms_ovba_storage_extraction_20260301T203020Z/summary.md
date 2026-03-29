# MS-OVBA Storage Extraction Evidence

- Run ID: `specproc-20260301-203019`
- Captured UTC: `2026-03-01T20:30:20.1287302Z`
- Source manifest: `C:\Work\DnaCalc\Foundation\reference\runs\20260301-ms-ovba-pass01\outputs\run_manifest.json`
- Spec items: `C:\Work\DnaCalc\Foundation\reference\runs\20260301-ms-ovba-pass01\outputs\spec_items.jsonl`
- Conformance items: `C:\Work\DnaCalc\Foundation\reference\runs\20260301-ms-ovba-pass01\outputs\conformance_items.jsonl`
- Conformance excluded: `C:\Work\DnaCalc\Foundation\reference\runs\20260301-ms-ovba-pass01\outputs\conformance_excluded.jsonl`

## Evidence Summary

The Foundation MS-OVBA extraction run registered the storage authority surface for `PH-0010` but did not yield executable conformance candidates.

Run facts:
- `docs_processed`: `1`
- `segments`: `3`
- `sentences`: `7`
- `spec_items`: `6`
- `conformance_candidates`: `0`
- `conformance_excluded`: `0`
- `pending_items`: `0`

## Storage-Relevant Anchors

The extracted `spec_items.jsonl` surface only the high-level storage framing needed for bounded registration:
- the document specifies the Office VBA file format structure,
- it applies to VBA projects,
- it describes a storage that contains a VBA project,
- sections `1.7` and `2` are normative,
- the remaining sections and examples are informative.

## Registration Result

This artifact registers the oracle-evidence boundary for the MS-OVBA storage lane on the current validation-reset doctrine.
It supports the honest scope statement for `PH-0010` but does not widen the matrix to parity closure.

The supported project/container roundtrip subset remains separately evidenced by the local `.basproj` and VBP adapter tests already cited in `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`.
