# V462 COM Early Drift Check Results

## Executed checks

- `scripts/validate-gate-sync.ps1`
- `scripts/validate-active-ladder-sync.ps1`
- `scripts/check-hal-clause-drift.ps1`
- `scripts/check-pmr-clause-drift.ps1`

## Result

All drift/sync checks pass after gate updates to `v466`.
- `gate-sync: ok (v466)`
- `active-ladder-sync: ok (range=v407..v466 gate=v466 ...)`
- `hal clause catalog drift check passed (65 clause IDs)`
- `pmr clause catalog drift check passed (40 clause IDs)`
