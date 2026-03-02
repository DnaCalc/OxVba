# HAL Conformance Expansion Plan V196

Status: `execution-plan`  
Step: `v196`  
Date: 2026-03-02

## Objective

Map new/expanded HAL clauses to executable checks and evidence lanes so the next implementation blocks can run without ambiguity.

## Layer Model

1. HAL layer (`oxvba-hal`)
- descriptor/capability checks,
- direct trait probe checks,
- host-backed clause checks.

2. Host integration layer (`oxvba-host` + VM/compiler)
- compile-time unsupported gating,
- runtime error routing behavior,
- end-to-end intrinsic execution paths.

3. Deferred empirical layer
- real VBA/Office differential checks,
- non-blocking deferred-gate foldback.

## Clause-to-Check Mapping (new block focus)

| Domain | Clause family | Primary checks | Artifact lane |
|---|---|---|---|
| Profile taxonomy | `HAL-PROF-V1-*` | descriptor matrix checks | `hal-conformance` |
| UI | `HAL-UI-V1-*` | scripted/denied/unsupported probe set + host policy tests | `hal-conformance`, `oxvba-host` |
| DoEvents | `HAL-EVT-V1-*` | supported/unsupported deterministic probes | `hal-conformance`, `oxvba-host` |
| COM | `HAL-COM-V1-*` | profile support matrix + compile/runtime gate checks | `hal-conformance`, `oxvba-host` |
| Declare/dynlink | `HAL-DYN-V1-*` | profile policy checks + invocation failure shape tests | `hal-conformance`, `oxvba-host` |
| File I/O | `HAL-FS-V1-*` | state-machine/property checks + host integration fixtures | `oxvba-hal`, `oxvba-host` |
| WASM classes | `HAL-WASM-V1-*` | runtime-class matrix probe rows | `hal-conformance-wasm32` |
| Time | `HAL-TIME-V1-*` | deterministic/host-backed probes + integration tests | `hal-conformance`, `oxvba-host` |
| Runner bootstrap | `HAL-RUNNER-V1-*` | precedence and fingerprint determinism tests | host runner suite (planned) |

## Execution Commands (baseline)

```powershell
cargo test -p oxvba-hal
cargo test -p oxvba-host
scripts/run-hal-conformance.ps1
scripts/run-hal-conformance-wasm32.ps1
scripts/check-hal-clause-drift.ps1
```

## Deferred Gate Policy

- formal failures remain non-blocking unless unsoundness/corruption risk is found.
- unresolved items are logged in evidence backlog and carried forward.

## Completion Target for Block A

By v196 completion:
- each new domain in v187..v195 has a clause family and test-layer destination.
- no domain remains "spec only, no planned executable check."
