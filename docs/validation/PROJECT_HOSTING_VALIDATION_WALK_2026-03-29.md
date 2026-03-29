# Project / Hosting Validation Walk - 2026-03-29

Status: `complete`
Scope: bead `bd-gm3.12.3`
Canonical matrix: `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`

## Purpose

Record the bounded verification pass for the project/hosting matrix rows without widening the canonical claims.

## Verified Rows

| Feature ID | Verified subset | Evidence checked | Result |
|---|---|---|---|
| `PH-0001` | executable startup ladder: explicit entrypoint, unique top-level mainline, unique `Sub Main` fallback | `crates/oxvba-host/tests/startup_entry_end_to_end.rs`, `docs/evidence/conformance/project_integration/PROJECT_INTEGRATION_SUITE_LATEST.md` | supported as `implemented-subset` |
| `PH-0002` | top-level executable mainline in program/script lanes | `crates/oxvba-host/tests/project_hosting_examples_end_to_end.rs`, `crates/oxvba-cli/src/main.rs` run-project unit tests | supported as `implemented-subset` |
| `PH-0003` | strict VBP-S0 adapter subset for executable startup and ordered reference handling | `crates/oxvba-host/tests/startup_entry_end_to_end.rs`, `crates/oxvba-cli/src/main.rs` run-project unit tests | supported as `implemented-subset` |

## Matrix Expansion

The canonical matrix now also names adjacent project/hosting lanes explicitly instead of collapsing them into the startup subset:
- `PH-0004` VBP-S0 adapter support
- `PH-0005` project references and cross-project accessibility
- `PH-0006` host project and extension-module behavior
- `PH-0007` imported default-property attribute runtime behavior
- `PH-0008` imported NewEnum attribute runtime behavior
- `PH-0009` host-sensitive policy surface
- `PH-0010` MS-OVBA storage roundtrip, now locally evidenced for the supported `.basproj` / VBP adapter subset while the full MS-OVBA corpus extraction remains open under the split oracle lane

## Bounded Outcome

The checked evidence supports the current matrix claims for `PH-0001` through `PH-0003`.
The matrix itself is now more complete because the adjacent lanes above are tracked as separate rows with their own subset boundaries and truth states instead of being left implicit.

The project-storage lane was also checked at the loader/generator level:
- `cargo test -p oxvba-project --test parse_tests round_trip -- --nocapture`
- `cargo test -p oxvba-project load_vbp_from_str_ -- --nocapture`

Those tests pass for the supported `.basproj` and VBP adapter roundtrip subset, but they do not close the MS-OVBA corpus/extraction gap tracked by `ODG-042`. The Foundation reference run `20260301-ms-ovba-pass01` is the extracted-corpus source run for this lane; it produced 6 spec items and 0 conformance candidates, so the remaining oracle-evidence work is split to `bd-gm3.12.8.1`.

Remaining broader work stays with the existing open records:
- `CCT-045` for broader startup/entrypoint oracle coverage.
- `CCT-049` and `CCT-050` for the imported attribute lanes.
- `ODG-043` for the remaining project startup and configuration breadth.

Follow-up beads created for the explicitly planned rows:
- `bd-gm3.12.6` project-hosting host-sensitive policy validation lane
- `bd-gm3.12.7` project-hosting MS-OVBA storage roundtrip validation lane
- `bd-gm3.12.8.1` project-hosting MS-OVBA storage oracle evidence lane
