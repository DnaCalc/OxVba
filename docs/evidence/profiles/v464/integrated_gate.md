# Integrated Gate v464

- Timestamp (UTC): 2026-03-05T19:51:23Z
- Gate: v464
- Status: pass
- Scope: COM early-binding/type-library integrated verification.

## Commands
- `cargo test -p oxvba-hal -p oxvba-compiler -p oxvba-host`
- `./scripts/run-com-early-conformance.ps1 -IncludeFormalLane`
- `./scripts/run-com-early-perf.ps1 -Iterations 3`
- `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal`

## Key run artifacts
- COM early conformance latest run: `20260305T194931Z`
- COM early perf latest run: `20260305T194128Z`
- Matrix run: `PASS (2/2 required cells green)`
- Formal run report timestamp: `2026-03-05T19:51:23Z`

## Result
- Integrated command set completed successfully under deferred-formal policy.
