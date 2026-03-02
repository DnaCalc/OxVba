# HAL Evidence Artifacts

This directory stores generated HAL conformance artifacts.

Generator:

```powershell
./scripts/run-hal-conformance.ps1
```

Wasm target generator:

```powershell
./scripts/run-hal-conformance-wasm32.ps1
```

Primary outputs:
- `HAL_CONFORMANCE_<timestamp>.md`: human-readable summary table by profile/lane.
- `HAL_CONFORMANCE_<timestamp>.jsonl`: machine-readable summary records.
- `HAL_CONFORMANCE_REMOTE_LINUX_<timestamp>.md|jsonl`: copied remote Linux conformance artifacts for host-verification evidence.

Current JSONL summary fields include:
- `profile`
- `runtime_class`
- `lane`
- `host_backed_eligible`
- `host_backed_active`
- `passed`
- `failure_count`
- `governance_notice_count`
- `probe_count`
- `probe_pass_count`
- `clause_count`
- `clause_pass_count`
- `failed_clauses`
- `governance_notices`
- `failures`

Current lane model:
- `runtime`: unsupported features surface at runtime.
- `compile-time`: unsupported/policy-denied host-sensitive features are preflighted in host diagnostics.
- `interactive-dev`: non-deterministic exploratory mode for host-backed lane evidence.

Wasm runtime classes:
- `wasi`
- `browser-sandbox`

Phase-1 formalization artifacts:
- `HAL_PHASE1_BASELINE_AUDIT_2026-03-02.md`
- `HAL_UNCERTAINTY_REGISTER.md`
- `HAL_IMPLEMENTATION_DEFINED.md`
- `HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md`

Phase-2 formalization artifact:
- `HAL_PHASE2_CONTRACT_CHECKS_2026-03-02.md`

Phase-3 formalization artifact:
- `HAL_PHASE3_ADAPTER_REFINEMENT_2026-03-02.md`

Remote host verification record:
- `HAL_REMOTE_LINUX_VERIFICATION_2026-03-02.md`

CI artifact mirrors:
- `docs/evidence/hal_ci_native/`
- `docs/evidence/hal_ci_wasm32/`
