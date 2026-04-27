# V0.2 Test/Doc Compat-Slot Migration Progress

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.2.5`
Status: in-progress

## Change

This slice started migrating tests, conformance notes, and product docs away
from presenting slot-shaped projection as ordinary execution truth.

Implemented:

- Renamed the project integration catalog expectation header from
  `expect_slots` to `expect_compat_slots`.
- Renamed the project integration test helper and mismatch diagnostic so
  retained `Variant` snapshots are projected to legacy slots only through an
  explicit compatibility assertion path.
- Updated conformance docs to classify CLI `SLOTS:` output and project
  integration `expect_compat_slots` as compatibility-slot observations rather
  than the primary result model.
- Reworded historical implementation log and runtime fact-pack language that
  described the old `RuntimeValue::I32` and `Variant` slot lane as current
  carrier truth.
- Follow-up scan reclassified additional active runtime fact-pack and value
  migration planning references so the 16-byte retained `Variant` carrier and
  remaining compatibility-slot adapters are not described as the product
  execution model.
- Runtime/Variant compatibility projection error strings now say `legacy
  compat slot` instead of `current compat slot`, and matching unit-test
  expectations were updated.
- Consolidation/architecture docs and downstream V0.2 evidence now describe
  remaining slot lanes as legacy adapter seams rather than current carrier
  truth.

This does not close `bd-bqm8.2.5`; remaining scans still need to classify or
update additional architecture, historical evidence, and helper-test wording.

## Verification

Passed:

- `cargo test -p oxvba-host --test project_integration_suite`
- `cargo test -p oxvba-runtime compat_slot --lib`
- `./scripts/validate-project-integration-catalog.ps1`
- `cargo fmt --check`
