# V460 COM Early Plan-to-Implementation Trace

## Ladder trace

- Planning: `v407..v416`
- Implementation block-I: `v417..v426`
- Implementation block-II: `v427..v445`
- Oracle/formal and CI hardening: `v446..v457`
- Closure: `v458..v466`

## Requirement-to-artifact mapping

1. Strategy policy control
   - Plan: `v427`
   - Code: `crates/oxvba-hal/src/model.rs`, `crates/oxvba-hal/src/adapters/standard.rs`
2. Diagnostics hardening
   - Plan: `v428`
   - Artifacts: `docs/DIAGNOSTIC_TAXONOMY.md`, `docs/evidence/conformance/com_early/COM_EARLY_DIAGNOSTIC_TAXONOMY_V1.csv`
3. Conformance lanes E0..E6
   - Plan: `v430..v438`
   - Scripts: `scripts/run-com-early-conformance.ps1`, `scripts/run-com-early-lane.ps1`
   - Evidence: `docs/evidence/conformance/com_early/`
4. Perf baseline/iteration
   - Plan: `v441..v443`
   - Script/artifacts: `scripts/run-com-early-perf.ps1`, `docs/evidence/perf/com_early/`
5. Oracle/deferred policy
   - Plan: `v446..v447`
   - Register: `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
   - Template: `scripts/run-com-early-oracle-template.ps1`
6. Terminal gate
   - Plan: `v464..v466`
   - Artifacts: `docs/evidence/profiles/v464/integrated_gate.md`, `docs/evidence/profiles/v466/integrated_gate.md`

## Trace result

- Planned scope is represented in implementation and evidence artifacts with explicit deferred items for oracle/formal long-running lanes.
