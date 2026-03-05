# V463 COM Early Integrated Gate Rehearsal

## Rehearsal command set

- `cargo test -p oxvba-hal -p oxvba-compiler -p oxvba-host`
- `./scripts/run-com-early-conformance.ps1 -IncludeFormalLane`
- `./scripts/run-com-early-perf.ps1 -Iterations 3`
- `./scripts/meta-check.ps1 -Fast -Conformance -Formal`

## Outcome

Rehearsal completed with no blocking failures. Deferred Kani obligations remained non-blocking and tracked.
