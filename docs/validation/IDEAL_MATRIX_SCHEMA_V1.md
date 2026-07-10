# Ideal Program Matrix Schema V1

Date: 2026-07-10
Program: `bd-59co` / `ideal-2026-07`
Ownership manifest: `IDEAL_MATRIX_OWNERSHIP_V1.csv`

All 15 canonical program matrices use the same identity/authority prefix and closure tail. Matrix-specific columns describe the observable or component states owned by that matrix.

## Required vocabularies

- `truth_role`: `primary`, `projection`, `evidence`, or `quality`.
- component state: `n/a`, `planned`, `in-progress`, `implemented-subset`, `implemented-full`, or `verified`.
- `truth_state`: `planned`, `in-progress`, `implemented-subset`, `implemented-full`, `verified`, or `archived`.
- `residual_disposition`: `remaining-accepted-scope`, `intentional-boundary`, or `external-boundary`.

Bare `implemented` is invalid. A required terminal row closes only at `verified`. Before then, a required row uses `remaining-accepted-scope` and names an open/in-progress descendant of `bd-59co` in `residual_owner_bead`.

Projection and evidence rows name their primary `source_claim_key`. Contract fields contain exact clause IDs rather than wildcard families. Test/evidence anchors must resolve to current files or named external environments; deleted-stack and historical captures may seed provenance but cannot establish current verification.

Test/evidence fields use `;` or `|` between references. A plain reference is a repository file path (an optional `#anchor` or Rust `::test_name` suffix is ignored for the existence check). Typed `matrix:`, `workset:`, and `file:` references also resolve their repository path. Named assertions use a nonempty `br:`, `command:`, `cargo:`, `test:`, `oracle:`, `environment:`, `excel:`, `spec:`, `external:`, or `transcript:` value; a raw command without one of these prefixes is invalid.

`producer_dependencies` uses a semicolon delimiter and each item names a current-program epic or executable bead. Validators trim each dependency and do not treat commas as dependency separators because CSV quoting must remain unambiguous.

## Target policy

Windows matrices use `target_arch=x64`. `office_bitness` is `64` or `n/a`. x86/32-bit Office, WOW64, ARM64 and other Windows targets are outside the accepted program and must not appear as required rows.

## Traceability

`IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv` maps one bead/matrix/row relationship per record. Delivery beads advance existing rows; support rollout/scaffold beads may use a matrix-level relationship until rows are seeded. Any support bead that exposes remaining accepted capability work must leave a delivery bead owner before closing.

Allowed trace relationships are `owns`, `owns-planned-row`, `advances`, `evidences`, `projects`, and `matrix-scaffold`. `owns-planned-row` gives a rollout owner to a seeded planned row before a delivery successor exists. Only a support `matrix-scaffold` relationship may omit `row_id`; every delivery relationship names an existing row. Every executable profile leaf and every matrix row must have at least one current-program trace relationship.

The trace `profile` is the bead's execution profile. Cross-profile producer/consumer mappings are valid (for example, the Windows oracle-environment lane can advance the Core-owned Excel oracle matrix); matrix ownership remains authoritative for the row's profile.

Every trace clause must occur in the bead's own contract text. A row-level trace must cover every clause on its matrix row; it may additionally carry bead-level boundary clauses that do not belong to the narrower row.

## Named program artifacts

New generated evidence and status belong under `docs/evidence/programs/<program-id>/<profile>/` and `docs/program-status/<program-id>/<profile>/`, where the profile is `core`, `windows-x64`, or `ide` for this manifest. The historical `docs/evidence/profiles/vNNN/` and `docs/profile-status/PROFILE_STATUS_VNNN.md` trees are read-only provenance unless a command supplies an explicit historical version allow-list.

The legacy migration ledger's `status_after` records migration-time state. During directed PROGRAM-0 it must match `br` exactly. After PROGRAM-0 closes, imported delivery rows may advance monotonically from recorded `open` through `in_progress`/`blocked` to `closed`; retired and PROFILE-EXT rows retain their exact recorded terminal/deferred state.
