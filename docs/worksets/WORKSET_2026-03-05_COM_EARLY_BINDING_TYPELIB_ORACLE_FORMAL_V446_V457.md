# WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_ORACLE_FORMAL_V446_V457

## Scope

Execute oracle-prep/formal foldback tranche for COM early-binding/type-library series:
- oracle mapping and deferred-gate scaffolding,
- compatibility and regression matrix synchronization,
- CI lane integration/stabilization,
- runtime/JIT parity and HAL policy gating checks,
- formal second-pass run and foldback registration.

Profiles covered: `v446..v457`

## Deliverables

1. Excel/VBA oracle topics for COM early binding are explicitly mapped and tracked (`CCT-046..CCT-048`).
2. Oracle template generation script is executable and linked to deferred gate register entries.
3. CI-oriented COM-early conformance lane orchestration is integrated into `meta-check`.
4. Runtime/JIT parity for early-binding subset is explicitly checked and tracked.
5. HAL capability/policy compile-time vs runtime behavior is exercised for early-binding pathways.
6. Security/safety review for early-binding FFI boundaries is documented.
7. Formal obligations for `v438` and `v456` are tracked as non-blocking deferred gates when unresolved.

## Verification Commands

- `./scripts/run-com-early-oracle-template.ps1`
- `./scripts/run-com-early-conformance.ps1 -IncludeFormalLane`
- `cargo test -p oxvba-host --test com_early_project_end_to_end -- --nocapture`
- `./scripts/meta-check.ps1 -Fast -Conformance -Formal`
