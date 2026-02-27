# PROFILE_STATUS_V3.md

## Profile
- ID: `mvp-formal-foundation-v3`
- Ladder step: `v3`

## Scope Summary
- Manifest-driven formal obligation execution (`obligations.csv`).
- Structured non-blocking formal reports (markdown + csv).
- Formal inventory and extended todo evidence discipline.

## Gate Artifacts
- Formal run report: `docs/evidence/formal/latest_run.md`
- Formal run csv: `docs/evidence/formal/latest_run.csv`
- Obligation index: `docs/evidence/formal/obligations.csv`
- Manifest: `docs/evidence/formal/MANIFEST.md`
- Inventory: `docs/evidence/formal/INVENTORY.md`

## Closure Signals
A `v3` profile run is considered complete when:
1. `scripts/run-formal.ps1` executes and writes both report formats.
2. Formal docs and index are synchronized with active obligations.
3. `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal` is green.
4. Tooling gaps are captured in `docs/evidence/formal/EXTENDED_TODO.md` under non-blocking policy.