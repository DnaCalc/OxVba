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
