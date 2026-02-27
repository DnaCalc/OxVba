# PROFILE_STATUS_V38.md

## Profile
- ID: mvp-lang-named-args-v38
- Ladder step: v38

## Scope Summary
- Named-argument call binding (`name := expr`) with deterministic mapping to procedure parameters.

## Gate Artifacts
- conformance/tests/params_named_bind.bas
- conformance/tests/params_named_optional_omit.bas
- conformance/tests/params_named_positional_after_named_error.bas
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v38 is complete when named-argument conformance cases are green in matrix runs and `FO-V38-*` obligations are recorded in formal reports.
