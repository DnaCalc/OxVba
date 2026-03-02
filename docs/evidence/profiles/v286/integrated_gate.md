# v286 Integrated Gate

- Step: v286
- Scope: Terminal integrated closure gate for v227..v286 (Declare/marshaling full scope).
- Status: PASS
- Artifacts:
  - docs/worksets/PROFILE_LADDER_2026-03-02_MACH1000_V227_V286_DECLARE_MARSHAL_FULL_SCOPE.md
  - docs/worksets/WORKSET_2026-03-02_TERMINAL_INTEGRATED_CLOSURE_GATE_V286.md
  - docs/spec/HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md
  - docs/spec/HAL_DECLARE_ABI_SPEC_V1.md
  - docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md
  - docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md
  - docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv
  - docs/evidence/hal/HAL_DECLARE_MARSHAL_BATCH_V227_V286_2026-03-02.md
  - docs/profile-status/PROFILE_STATUS_V286.md

## Checks

- `./scripts/check-hal-clause-drift.ps1` : PASS
- `cargo test -p oxvba-hal` : PASS
- `cargo test -p oxvba-compiler compile_declare_` : PASS
- `cargo test -p oxvba-vm declare_invoke_` : PASS
- `cargo test -p oxvba-host hal_runtime_host_backed_declare_` : PASS
- `cargo test --workspace --no-run` : PASS
- `./scripts/run-formal.ps1 -ProfileScope mvp-profile-v286` : PASS (non-blocking mode; Kani deferred to WSL async lane)
- `./scripts/meta-check.ps1 -Fast` : PASS

## Notes

- Formal/Kani obligations remain non-blocking and continue under deferred-gate policy.
- Marshaling breadth beyond current deterministic subset is tracked as implemented-partial with explicit clause coverage and deferred items.
