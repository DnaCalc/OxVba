# WORKSET_2026-03-04_COM_WINDOWS_TEST_COMPONENTS_V293_V300.md

## Objective

Execute `v293..v300`: build first executable Windows COM scaffolding for both client and server verification lanes.

## Scope

1. Rust COM test component scaffolding (expandable, deterministic).
2. Test scripts for registration-free and registered lanes.
3. Initial client/server fixture packs.
4. Native Windows client activation + scalar invoke path.
5. Scalar variant/bstr marshaling subset.

## Deliverables

- COM test component code under Rust crates/tests.
- scripts for COM lane orchestration.
- conformance fixtures under `conformance/com/*`.
- first native client path implementation in COM adapter layer.

## Checks

- local Windows test lane passes for initial smoke fixtures.
- deterministic failure behavior verified for unsupported/denied modes.
- artifacts generated in planned conformance output locations.

## Closure Conditions

`v300` is complete when native client path and fixture/scaffold baseline are running and reproducible for subsequent error-mapping and integration steps.

