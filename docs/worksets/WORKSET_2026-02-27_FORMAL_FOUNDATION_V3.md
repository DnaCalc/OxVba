# WORKSET_2026-02-27_FORMAL_FOUNDATION_V3.md

## Profile
- ID: `mvp-formal-foundation-v3`
- Ladder step: `v3`
- Formal level target: `F1`

## Purpose
Establish repeatable formal-obligation execution/reporting infrastructure so each subsequent profile can attach obligations without redesigning tooling.

## Scope
1. Manifest-driven formal runner and obligation index.
2. Structured formal reports (markdown + csv).
3. Formal evidence inventory and extended todo format.
4. CI/script integration of non-blocking formal lane.

## Out Of Scope
1. Making formal lane hard-blocking.
2. Expanding semantics surface (handled in other profiles).
3. Installing environment tooling in this profile when unavailable.

## Exit Gate
1. `scripts/run-formal.ps1` consumes an obligation index file.
2. Formal run generates:
   - `docs/evidence/formal/latest_run.md`
   - `docs/evidence/formal/latest_run.csv`
3. Formal manifest and inventory docs exist and are synchronized with obligations.
4. `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal` is green.
5. Missing formal tooling is recorded as non-blocking evidence/todo.

## Required Artifacts
- `docs/evidence/formal/obligations.csv`
- `docs/evidence/formal/MANIFEST.md`
- `docs/evidence/formal/INVENTORY.md`
- `docs/evidence/formal/latest_run.md`
- `docs/evidence/formal/latest_run.csv`
- `docs/evidence/formal/EXTENDED_TODO.md`

## Verification Commands
```powershell
./scripts/run-formal.ps1 -ProfileScope mvp-formal-foundation-v3
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```