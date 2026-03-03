# WORKSET: PMR Follow-up Queue from P0-P10 Observations

Date: 2026-03-03
Status: parity-closed-backlog-active
Source: post-P10 observations and parity-gate follow-up request

## Observation Disposition

1. `PMR-FUP-001`: `compile_project` rewrite bridge (not full module-aware IR bind)
- Status: open backlog
- Disposition: documented + queued.
- Next step: move from source rewrite to module-aware binder/IR lowering.

2. `PMR-FUP-002`: cross-project execution unsupported (`PMR-E-REFERENCE-CROSS-PROJECT-UNSUPPORTED`)
- Status: open backlog
- Disposition: documented + queued.
- Next step: add executable cross-project bind/runtime path.

3. `PMR-FUP-003`: reference-order parity needed end-to-end
- Status: partial (host graph/oracle lane complete, compiler/runtime lane pending)
- Disposition: addressed in host graph subset + oracle validated for precedence behavior.
- Next step: unify precedence semantics through compiler/runtime execution path.

4. `PMR-FUP-004`: header parsing strictness and host-edge tolerance
- Status: partial
- Disposition: deterministic malformed-header diagnostics + oracle export evidence landed.
- Next step: add host-import tolerance matrix tests where safe.

5. `PMR-FUP-005`: `Option Private Module` boundary behavior nuance
- Status: partial
- Disposition: addressed for host-direct invocation lane (oracle-backed).
- Next step: separate host-direct callable vs reference-visible contracts explicitly in PMR model.

6. `PMR-FUP-006`: follow-up queue formalization and synchronization
- Status: active ongoing process
- Disposition: addressed.
- Next step: keep PMR oracle lane and divergence foldback synchronized with implementation backlog.

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

1. `PMR-FUP-001..003`: module-aware binder/IR and cross-project executable resolution path.
2. `PMR-FUP-005`: explicit split of host-direct callable vs reference-visible access semantics.
3. `PMR-FUP-004/006`: tolerance matrix + ongoing oracle/divergence synchronization.
