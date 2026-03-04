# COM Client Error Mapping v1

Status: `working-draft`  
Date: 2026-03-04

This table defines deterministic COM client error-label mapping used by the Windows late-bound client lane.

Primary machine-readable source:
- `docs/evidence/conformance/COM_CLIENT_ERROR_MAPPING_V1.csv`

## Intent

1. Keep HAL-level COM failure classification stable (`HAL-E-ADAPTER-FAULT`) while native details evolve.
2. Attach deterministic labels (for diagnostics and conformance evidence) for common COM HRESULT families.
3. Separate mapping-table stability from host-specific `Err.Number` parity until oracle foldback is complete.

## Policy

- `Err.Number` exact parity is currently tracked as `deferred-oracle`.
- Labels are expected to appear in adapter fault messages for C2 late-bound lanes.
- New native failure categories must be added to the CSV before broadening invoke coverage.
