# V386 COM Series Terminal Gate

- Step: `v386`
- Series: `v287..v386` (`PROFILE_SERIES_2026-03-04_MACH1000_COM_WINDOWS_CLIENT_SERVER`)
- Final status: `PASS`

## Executed Checks

- `./scripts/check-hal-clause-drift.ps1` : PASS
- `cargo test -p oxvba-hal` : PASS
- `./scripts/run-conformance.ps1 -Backend vm` : PASS (`192` files)
- `./scripts/run-conformance.ps1 -Backend jit` : PASS (`192` files)
- `./scripts/run-profile-gate.ps1 -ProfileScope mvp-profile-v386 -OutputDir docs/evidence/profiles/v386 -SkipBench` : PASS
- `./scripts/meta-check.ps1 -Fast` : PASS

## Primary Artifacts

- `docs/evidence/profiles/v386/integrated_gate.md`
- `docs/evidence/profiles/v386/gate_report.md`
- `docs/evidence/profiles/v386/matrix_latest.csv`
- `docs/evidence/formal/latest_run.md`
- `docs/profile-status/PROFILE_STATUS_V386.md`

## Notes

- Formal/Kani remains non-blocking in this lane and is tracked through deferred-gate policy.
- Windows native COM client execution is now available in host-backed mode for mapped tokens (`Scripting.Dictionary` baseline), with deterministic projection fallback preserved for compatibility.
- This artifact records successful completion of the `v287..v386` COM planning/native-client/stabilization tranche only.
- It does not claim broad COM client/server parity completion; later COM completion work remains active.
