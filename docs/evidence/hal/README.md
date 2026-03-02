# HAL Evidence Artifacts

This directory stores generated HAL conformance artifacts.

Generator:

```powershell
./scripts/run-hal-conformance.ps1
```

Primary outputs:
- `HAL_CONFORMANCE_<timestamp>.md`: human-readable summary table by profile/lane.
- `HAL_CONFORMANCE_<timestamp>.jsonl`: machine-readable summary records.

Current lane model:
- `runtime`: unsupported features surface at runtime.
- `compile-time`: unsupported/policy-denied host-sensitive features are preflighted in host diagnostics.

Phase-1 formalization artifacts:
- `HAL_PHASE1_BASELINE_AUDIT_2026-03-02.md`
- `HAL_UNCERTAINTY_REGISTER.md`
- `HAL_IMPLEMENTATION_DEFINED.md`
