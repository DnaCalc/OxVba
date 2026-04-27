# V0.2 Office COM Interop Corpus Rollout

- Bead: `bd-bqm8.7.1`
- Parent lane: `bd-bqm8.7`
- Date: 2026-04-27
- Status: rollout complete; lane remains in-progress

## Scope

`bd-bqm8.7` covers the bounded V0.2 Excel and Access/JET COM interop corpus.
The goal is not full Office automation parity. The goal is an explicit,
repeatable corpus with supported, unsupported, and environment-dependent rows
classified clearly.

## Child Beads

- `bd-bqm8.7.1`: audit and roll out Office COM corpus child beads.
- `bd-bqm8.7.2`: publish the Excel and Access/JET corpus matrix.
- `bd-bqm8.7.3`: add Excel late-bound/metadata-backed fixture coverage.
- `bd-bqm8.7.4`: add Access/JET fixture coverage or deterministic unsupported rows.
- `bd-bqm8.7.5`: refresh host, VM, JIT, and conformance evidence for corpus rows.
- `bd-bqm8.7.6`: run the final Office COM corpus checklist and close `bd-bqm8.7`
  only if active rows, unsupported rows, and environment prerequisites are
  explicit.

## Initial Corpus Dimensions

- Excel activation and object-root acquisition.
- Workbook/worksheet/range/property get and method invoke shapes.
- Default member and named-argument behavior where metadata is authoritative.
- Access/JET activation and minimal database/object interaction where available.
- Deterministic unsupported diagnostics where Office or JET providers are not
  available in the validation environment.
- VM/JIT/host parity for controlled supported rows.

## Next Step

The next ready bead is `bd-bqm8.7.2`, the explicit corpus matrix.

