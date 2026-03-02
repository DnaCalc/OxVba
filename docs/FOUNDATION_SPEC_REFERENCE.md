# Foundation Spec Reference

Canonical VBA/OpenSpec source material is managed in the sibling Foundation repository:

- Root: `../Foundation/reference`
- Doctrine: `../Foundation/REFERENCE_SPEC_FORMAT_AND_CONFORMANCE.md`

OxVba does not vendor local mirrored VBA spec snapshots anymore.

## Core Indexes

- Seeds: `../Foundation/reference/spec_seeds.csv`
- Mirror index (CSV): `../Foundation/reference/index.csv`
- Mirror index (Markdown): `../Foundation/reference/index.md`
- Raw mirrored files: `../Foundation/reference/downloads/...`

## Primary Canonical Sources

- MS-VBAL (VBA language):
  - `../Foundation/reference/downloads/learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74.md`
  - `../Foundation/reference/downloads/officeprotocoldocs-f5hpbjgea6b8gneq.b02.azurefd.net/files/MS-VBAL/[MS-VBAL]-250520.docx`
  - `../Foundation/reference/downloads/officeprotocoldocs-f5hpbjgea6b8gneq.b02.azurefd.net/files/MS-VBAL/[MS-VBAL]-250218.pdf`
- MS-OVBA (VBA project/container format):
  - `../Foundation/reference/downloads/learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/b39ac32f-0ce1-4533-9297-2ff3ff62c9ec.md`
- MS-OAUT (OLE Automation):
  - `../Foundation/reference/downloads/learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/bbb05720-f724-45c7-8d17-f83c3d1a3961.md`
- MS-DTYP (Windows data types):
  - `../Foundation/reference/downloads/learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/cca27429-5689-4a16-b2b4-9325d93e4ba2.md`

## Extracted Conformance Sets

Use the normalized run outputs under `../Foundation/reference/runs/<run-id>/outputs/`:

- MS-VBAL: `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/`
- MS-OVBA: `../Foundation/reference/runs/20260301-ms-ovba-pass01/outputs/`
- MS-OAUT: `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/`
- MS-DTYP: `../Foundation/reference/runs/20260301-ms-dtyp-pass02/outputs/`

Each run output is expected to include:

- `run_manifest.json`
- `documents.csv`
- `selected_sources.csv`
- `spec_items.jsonl`
- `conformance_items.jsonl`
- `conformance_excluded.jsonl`

For extraction format and conformance doctrine requirements, use:
`../Foundation/REFERENCE_SPEC_FORMAT_AND_CONFORMANCE.md`.
