# Spec Source Sets

This directory tracks local source-set manifests for external VBA specification references.

Important:
- These are local source maps and extraction manifests.
- They are not complete vendored copies of Microsoft specifications.
- Canonical normative text remains at the original Microsoft Open Specifications / VBA documentation URLs.

Source sets:
- [`VBA_LANGUAGE_SPEC_SOURCESET.md`](VBA_LANGUAGE_SPEC_SOURCESET.md)
- [`VBA_LIBRARY_SPEC_SOURCESET.md`](VBA_LIBRARY_SPEC_SOURCESET.md)

Local snapshot cache:
- `docs/spec/sources/local/LATEST.txt` points to the latest captured snapshot directory.
- Each snapshot directory contains:
  - `manifest.csv` (URL, local filename, checksum, fetch status)
  - raw HTML snapshot files
  - `README.md`

Refresh:
- `pwsh -NoLogo -NoProfile -File scripts/fetch-spec-sources.ps1 -IncludeTimestampDir`
