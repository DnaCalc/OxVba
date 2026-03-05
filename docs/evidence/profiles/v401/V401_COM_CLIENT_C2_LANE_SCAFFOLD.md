# V401 COM Client C2 Lane Scaffold

## Scope
- Ladder: `v387..v406`
- Step: `v401`
- Workset: `WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_EXECUTION_V401_V406.md`

## Outputs
- Published dedicated C2 COM lane scripts:
  - `scripts/run-com-conformance.ps1`
  - `scripts/run-com-registrationless.ps1`
  - `scripts/run-com-registered.ps1`
- Added fixture lint guard to integration suite entrypoint:
  - `scripts/lint-integration-fixtures.ps1`
  - `scripts/run-project-integration-suite.ps1`
- Standardized lane artifact schema under:
  - `docs/evidence/conformance/com/COM_CONFORMANCE_RUN_<timestamp>.{md,csv}`
  - `docs/evidence/conformance/com/COM_LANE_L2B_*`
  - `docs/evidence/conformance/com/COM_LANE_L2_*`

## Gate Signal
- `v401` scaffold contract is complete; step is ready for executable lane runs.
