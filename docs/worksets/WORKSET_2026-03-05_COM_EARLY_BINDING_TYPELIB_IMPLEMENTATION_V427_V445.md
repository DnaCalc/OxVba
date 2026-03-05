# WORKSET_2026-03-05_COM_EARLY_BINDING_TYPELIB_IMPLEMENTATION_V427_V445

## Scope

Execute implementation block II after initial early-binding bridge landing (`v417..v426`):
- dual-interface invocation strategy controls,
- diagnostics and taxonomy hardening,
- controlled test-server/type-library fixture lane,
- conformance lane execution (`E0..E5`) and formal-lane registration (`E6`),
- early performance/robustness passes and compatibility regression checks.

Profiles covered: `v427..v445`

## Deliverables

1. Runtime policy exposes explicit dispatch/vtable strategy selection with deterministic fallback behavior.
2. Early-binding diagnostics are stabilized and mapped into taxonomy artifacts.
3. Controlled COM test-server + typelib fixture lane is runnable in host tests.
4. Conformance lanes `E0..E5` are executable through script orchestration with deterministic artifacts.
5. Formal lane `E6` is registered and tracked under deferred-gate policy when Kani remains long-running.
6. Performance baseline and optimization deltas for early-binding hot paths are captured.
7. Robustness corpus expansion and deterministic negative/failure checks are published.

## Verification Commands

- `cargo test -p oxvba-hal -p oxvba-compiler -p oxvba-host`
- `./scripts/run-com-early-conformance.ps1 -IncludeFormalLane`
- `./scripts/run-com-early-perf.ps1 -Iterations 3`
- `./scripts/meta-check.ps1 -Fast -Conformance`
