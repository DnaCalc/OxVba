# PMR Project-Model Oracle Foldback (2026-03-03)

Scope: `CCT-037..CCT-041` (`ODG-035..ODG-039`)
Runner: `scripts/run-pmr-project-model-oracle.ps1`
Artifact sets:
- `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T070427Z/`
- `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T074118Z/` (dialog-guardian validated rerun)

## Result Snapshot

- Total cases: 8
- Matches: 6
- Mismatches: 2

Per-topic:

- `CCT-037`: matched (`3/3`)
- `CCT-038`: matched (`2/2`) for current host-direct invocation contract
- `CCT-039`: matched (`1/1`) for exported class-header defaults
- `CCT-040`: mismatch (`0/1`) -> divergence `DIV-0003`
- `CCT-041`: mismatch (`0/1`) -> divergence `DIV-0004`

## Gate Resolution

- `ODG-035`: closed
- `ODG-036`: closed
- `ODG-037`: closed
- `ODG-038`: closed with divergence (`DIV-0003`) and queued follow-up
- `ODG-039`: closed with divergence (`DIV-0004`) and queued follow-up

## Notes

- `CCT-038` behavior reflects host-direct invocation semantics (`Application.Run` explicit target) in Excel and corresponding OxVba host-export lane behavior.
- `CCT-040/041` remain implementation backlog items under:
  - `docs/worksets/WORKSET_2026-03-03_PMR_FOLLOWUP_QUEUE_FROM_OBSERVATIONS.md`
- Oracle runner operational caveat: occasional hidden Excel modal states can stall unattended reruns; when this occurs, keep the last successful capture as parity evidence and terminate the stalled `pwsh`/`EXCEL` pair before rerunning.
- Dialog-guardian validation: rerun `pmr_project_model_20260303T074118Z` confirmed unattended handling of macro trust prompts (`excel_dialog_guardian.log` shows handled dialog button `Enable Macros`).
