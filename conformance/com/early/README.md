# COM Early-Binding Fixture Pack

Status: `active-subset` (`v466` closure target)

Fixture families:
- `typelib_ingest/`: type-library identity/load/cache semantics fixtures.
- `binder/`: compile-time early-bind declaration/member diagnostics fixtures.
- `runtime/`: runtime execution/error-path fixtures for constrained early-bind subset.
- `cache/`: cache invalidation and deterministic replay fixtures.
- `end_to_end/`: mixed early/late and project-level scenarios.

Current execution policy:
- Lanes are executed via `scripts/run-com-early-conformance.ps1`.
- Formal lane (`E6`) is non-blocking and can report `deferred` for Kani/tooling constraints.
