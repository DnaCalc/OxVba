# WORKSET: PMR Follow-up Queue from P0-P10 Observations

Date: 2026-03-03
Status: parity-closed-backlog-active
Source: post-P10 observations and parity-gate follow-up request

## Observation Disposition

1. `PMR-FUP-001`: `compile_project` rewrite bridge (not full module-aware IR bind)
- Status: addressed (active path advanced)
- Disposition: `compile_project` now defaults to module-aware bind-plan lowering with explicit bridge fallback (`OXVBA_PMR_LOWERING=rewrite-bridge`), including deterministic bare `Call` target rewrites.
- Next step: bridge retirement remains blocked by `DG-V287-001` host-harness timeouts (`FO-V287-001/002`); keep fallback until remediation + clean re-run.

2. `PMR-FUP-002`: cross-project execution unsupported (`PMR-E-REFERENCE-CROSS-PROJECT-UNSUPPORTED`)
- Status: partial (advanced)
- Disposition: executable subset now supports cross-project calls when referenced project source is loaded into the manifest (`reference_projects`).
- Next step: add richer bind/runtime path for host-resolved reference payloads (non-source-backed lanes).

3. `PMR-FUP-003`: reference-order parity needed end-to-end
- Status: addressed (current subset)
- Disposition: compiler/runtime executable path now follows declared reference order for unqualified call rewrites, aligned with host graph and oracle subset.
- Next step: preserve this ordering in future module-aware IR binder path.

4. `PMR-FUP-004`: header parsing strictness and host-edge tolerance
- Status: addressed (matrix-backed)
- Disposition: malformed-header diagnostics remain deterministic; host-edge tolerance/rejection policy is now explicit and test-anchored in `docs/evidence/conformance/PMR_HOST_IMPORT_TOLERANCE_MATRIX_V1.md`.
- Next step: expand matrix rows as additional host-import shapes are discovered.

5. `PMR-FUP-005`: `Option Private Module` boundary behavior nuance
- Status: addressed (current subset)
- Disposition: host-direct callable and reference-visible export surfaces are split explicitly (`host_exports` vs `reference_visible_exports`) in compiler and host models.
- Next step: fold this split into future host project catalog/reference APIs.

6. `PMR-FUP-006`: follow-up queue formalization and synchronization
- Status: addressed (guarded)
- Disposition: synchronization now has an executable guard (`scripts/validate-pmr-followup-sync.ps1`) wired into `scripts/meta-check.ps1`.
- Next step: keep the guard green as oracle/divergence records evolve.

## Parity Queue (`CCT-037..CCT-041`)

- Oracle artifacts:
  - `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T070427Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T070427Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T074118Z/results.csv` (dialog-guardian rerun)
  - `docs/evidence/conformance/oracle_captures/pmr_project_model_20260303T074118Z/summary.md`

### Closure state

- `CCT-037`: matched and closed (reference precedence/shadowing subset).
- `CCT-038`: matched and closed for current host-direct invocation contract.
- `CCT-039`: matched and closed for exported class-header defaults.
- `CCT-040`: mismatch recorded; class-interface coverage remains queued (`DIV-0003`).
- `CCT-041`: mismatch recorded; class event model remains queued (`DIV-0004`).

### Parity gate result

- Gate `PMR-PARITY-CCT-037-041`: `PASS` under deferred-divergence policy.
- Pass rule:
  - resolved topics (`CCT-037..CCT-039`) are oracle-matched and closed.
  - unresolved topics (`CCT-040..CCT-041`) are closed with explicit divergence records + queued implementation follow-up.

## Next Execution Block

1. Remediate host Kani timeout behavior for `FO-V287-001/002` and rerun `DG-V287-001`.
2. Retire rewrite-bridge fallback only after rerun foldback clears host PMR obligations (`PMR-FUP-001` closure).
3. Keep `PMR-FUP-004/006` synchronized with oracle/divergence foldback and tolerance-matrix expansion.
4. Continue deferring `PMR-FUP-002` (`CCT-043`/`ODG-041`) to the COM stabilization tranche.
