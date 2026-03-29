# Conformance Evidence Layout

This directory contains both active governance surfaces and historical run artifacts.

## Active Governance Surfaces

These files are expected to remain synchronized with current implementation semantics and are validated in governance checks:

- `CONFORMANCE_CHECK_TOPICS.csv` - topic/oracle/gate register; implementation truth is owned by the validation matrices under `docs/validation/`
- `DEFERRED_ORACLE_GATES.csv`
- `PROJECT_INTEGRATION_DEFERRED_UNCERTAINTIES_V1.md`
- `IMPLEMENTATION_DEFINED.md`

PMR/event diagnostic governance for active surfaces is validated by:
- `scripts/validate-pmr-event-diagnostic-sync.ps1`
- canonical manifest: `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`

## Historical Capture Areas

These paths preserve older oracle outputs and are intentionally excluded from active-drift checks by default:

- `oracle_captures/`
- timestamped run artifacts under `project_integration/` and other run-output lanes

Historical captures may contain legacy diagnostic IDs that were superseded in active semantics.
