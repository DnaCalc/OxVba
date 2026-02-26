# CONFORMANCE.md

## Purpose
Defines the current conformance loop for OxVBA MVP execution.

## Assets
- `conformance/tests/*.bas` — executable input corpus.
- `conformance/golden/*.csv` — expected outcomes.

## Commands
```powershell
./scripts/run-smoke.ps1
./scripts/run-conformance.ps1
```

## Current policy
At MVP stage, conformance compares execution success/failure status. As runtime semantics mature, this will expand to structured output comparison (values, error state, and object lifecycle signals).
