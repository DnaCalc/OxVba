# PROFILE_STATUS_V41.md

## Profile
- ID: mvp-lang-on-error-goto-label-v41
- Ladder step: v41

## Scope Summary
- `On Error GoTo <label>` subset with handler-target validation and runtime label transfer behavior.

## Gate Artifacts
- conformance/tests/on_error_goto_label_resume.bas
- conformance/tests/on_error_goto_label_missing_label_error.bas
- conformance/tests/on_error_goto_label_then_goto_zero_error.bas
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v41 is complete when label-handler conformance fixtures are green and `FO-V41-*` obligations are recorded in formal reports.
