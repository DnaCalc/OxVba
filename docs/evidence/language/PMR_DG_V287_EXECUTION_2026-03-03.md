# PMR Deferred Formal Lane Execution (DG-V287-001) - 2026-03-03

Status: `completed-fail` (foldback captured)

## Objective
Execute and fold back `FO-V287-001..003` so rewrite-bridge retirement can proceed on evidence, not setup-only registration.

## Required obligations
- `FO-V287-001`: `cargo kani -p oxvba-host --harness pmr_typelib_resolution_transitions_typelib_refs_out_of_unbound`
- `FO-V287-002`: `cargo kani -p oxvba-host --harness pmr_active_resolution_prefers_local_symbol_before_reference_symbol`
- `FO-V287-003`: `cargo kani -p oxvba-hal --harness dynlink_contract_rejects_mismatched_selection_policy`

## Execution log

1. Remote deferred dispatch attempted:
- command: `./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredMode exact -DeferredStrategy dedup -DeferredVersions "287" -DeferredConcurrency 1`
- dispatch: `20260303T191906Z_deferred_dispatch`
- lane: `v287-kani`
- result: `pass` with `selected_count=0`
- interpretation: remote commit (`c71ad818d75b25fa8d4b89e14be93273537c965d`) does not currently select `FO-V287-*` obligations.

2. Local strict lane was briefly initiated for actual execution against current workspace:
- runner: `./scripts/run-formal-kani-async.ps1`
- lane name: `dg-v287-local`
- command: `wsl bash /mnt/c/Work/DnaCalc/OxVba/temp/kani_v287_run.sh`

3. First local attempt failed due command quoting (`$HOME` expansion mismatch).

4. Second local attempt exposed a real Kani compile blocker in workspace:
- `crates/oxvba-compiler/src/optimize.rs` Kani fixture `BoundModule` initializer missing fields
  - `resolution_diagnostics`
  - `external_declarations`
- Patch applied in the workspace; lane relaunched.

5. Final state:
- `dg-v287-local` was explicitly stopped per policy (`no local Kani runs`).
- Remote runner semantics now classify zero-selection exact lanes as explicit `no-op` (commit-obligation mismatch warning), not silent pass.
- Completed pinned lane:
  - job: `20260303T201117Z_manual-v287-kani-pinned-fast`
  - lane: `v287-kani-pinned-fast`
  - repository: `/home/ubuntu/.dnacalc_remote/work/OxVba_v287_pinned`
  - selection: `selected_count=3` (`FO-V287-001..003`)
  - result: `fail` (`FO-V287-001/002` timeout; `FO-V287-003` pass)
  - timeout policy: `600s`, `retries=0` (bounded foldback run)

## Evidence paths
- remote lane tail evidence: `./scripts/run-formal-kani-remote.ps1 -Action Tail -Lane v287-kani`
- async lane directory: `temp/async/formal-kani/dg-v287-local/`
- active logs:
  - `temp/async/formal-kani/dg-v287-local/stdout.log`
  - `temp/async/formal-kani/dg-v287-local/stderr.log`
  - `temp/async/formal-kani/dg-v287-local/status_snapshot.json`
- foldback report: `docs/evidence/language/PMR_DG_V287_FOLDBACK_2026-03-03.md`

## Follow-up
1. Keep rewrite-bridge fallback enabled (no retirement on this foldback).
2. Remediate host harness timeout behavior for `FO-V287-001/002`.
3. Re-run remote `v287` lane after harness remediation and re-evaluate retirement gate.
