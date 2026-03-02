# HAL Conformance Suite

Status: `working-draft`  
Date: 2026-03-01

## 1. Purpose

Define a runnable, repeatable HAL verification suite with:
- adapter contract checks,
- capability declaration checks,
- deterministic unsupported behavior checks,
- compile-time/runtime unsupported-mode integration checks.

## 2. Suite Layers

1. Pre-engine layer (`oxvba-hal`):
- Trait/descriptor conformance and deterministic probe outcomes.
- Source: `crates/oxvba-hal/src/conformance.rs`

2. In-engine layer (`oxvba-host` + VM/JIT):
- Runtime wiring through bytecode host intrinsics.
- Compile-time unsupported preflight enforcement.
- Runtime unsupported error surfacing.

## 3. Commands

Fast lane:

```powershell
cargo test -p oxvba-hal
```

Full HAL suite artifacts:

```powershell
scripts/run-hal-conformance.ps1
```

Catalog drift guard only:

```powershell
scripts/check-hal-clause-drift.ps1
```

This command runs crate tests and emits artifacts under `docs/evidence/hal`:
- `HAL_CONFORMANCE_<timestamp>.md`
- `HAL_CONFORMANCE_<timestamp>.jsonl`

Current artifact schema also includes clause-coverage totals per profile/lane:
- `clause_count`
- `clause_pass_count`
- `failed_clauses`
- `governance_notice_count`
- `governance_notices`

Clause coverage is computed against the machine-readable catalog:
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv`

Integration lane:

```powershell
cargo test -p oxvba-host
```

## 4. Profile Coverage

The pre-engine harness probes all current profiles:
- `Windows`
- `Linux`
- `MacOs`
- `Wasm`
- `Null`

Each profile is executed in:
- `runtime` unsupported mode,
- `compile-time` unsupported mode.

## 5. Expected COM Result

Current conformance expectation:
- Windows: `ComActivationDispatch` supported.
- Linux/macOS/WASM/Null: `ComActivationDispatch` unsupported and deterministically reported.

This expectation is encoded in `oxvba-hal` tests.

## 6. Failure Policy

At current ladder stage:
- HAL/formal failures are non-blocking unless they indicate unsoundness/data corruption risk.
- Failures must still produce artifacts and be triaged into deferred/backlog evidence.

## 7. Next Hardening Steps

1. Promote candidate spec anchors to reviewed, behavior-specific conformance rows.
2. Add richer UI virtualization and event-pump deterministic model probes.
3. Add Office empirical differential checks for host-sensitive behavior classes on Windows.

## 8. Clause Mapping Baseline

Clause catalog baseline:
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md`

Phase-1 expectation:
- every new contract clause added to the catalog must include an explicit verification mapping field.
- clauses marked `implemented-partial` require dedicated test expansion in Phase 2.

Phase-2 progress:
- conformance reports now carry clause coverage aggregation mapped from descriptor checks and probe-to-clause assignments.

Phase-4 progress:
- conformance lane coverage is now catalog-scoped against the machine-readable CSV (drift-guarded against markdown IDs),
- governance notices are emitted as non-blocking evidence for maturity-policy issues.

Phase-3 progress:
- adapter suite includes side-effect/invariant checks and property checks for selected filesystem behaviors;
- host suite includes explicit runtime error-routing checks for HAL failures under `On Error Resume Next`.
