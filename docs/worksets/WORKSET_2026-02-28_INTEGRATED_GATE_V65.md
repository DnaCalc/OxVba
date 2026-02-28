# WORKSET_2026-02-28_INTEGRATED_GATE_V65

## Profile
- ID: `mvp-integrated-correctness-perf-gate-v65`
- Ladder step: `v65`

## Scope
- Add one-command integrated profile gate for formal, matrix, and benchmark lanes.
- Persist consolidated gate report with pass/fail rollup.

## Implementation Tasks
- Add integrated gate script to orchestrate formal/matrix/bench execution.
- Emit consolidated report and summary CSV in profile evidence directory.
- Add formal checks for integrated gate report structure.

## Gate Commands
- `./scripts/run-profile-gate.ps1 -ProfileScope mvp-integrated-correctness-perf-gate-v65 -OutputDir docs/evidence/profiles/v65`
- `cargo test -p oxvba-host --lib`
- `./scripts/run-formal.ps1 -ProfileScope mvp-integrated-correctness-perf-gate-v65`
