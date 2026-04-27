# V0.2 Late-Bound IDispatch Parity Rollout

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.3.1`
Status: complete

## Scope Matrix

This rollout splits `bd-bqm8.3` into concrete delivery and validation beads.
The V0.2 goal is not full Automation parity; it is an explicit supported
matrix with green evidence for the in-scope rows and named unsupported rows for
the rest.

V0.2 supported rows:

- member-name resolution for mapped native/controlled COM members;
- token-backed late-bound invocation for existing deterministic projection
  rows;
- positional method/property-get invocation for scalar, object, string, array,
  and wide numeric result payloads already covered by controlled COM fixtures;
- named-argument invocation where authoritative metadata supplies DISPIDs;
- default-member dispatch where typelib/import metadata supplies the default
  member identity;
- event callback payload projection for controlled connection-point lanes;
- deterministic unsupported diagnostics for ambiguous, metadata-missing, or
  unsupported invocation shapes.

Explicitly out of scope for this V0.2 lane:

- full Office-wide behavioral parity for every `IDispatch` implementation;
- untyped natural syntax that lacks authoritative default-member identity;
- arbitrary optional-argument/missing-argument synthesis without metadata;
- general property-put/property-set parity beyond rows proven by fixtures;
- non-Windows COM parity.

## Child Beads

- `bd-bqm8.3.1`: audit and roll out late-bound `IDispatch` child beads.
- `bd-bqm8.3.2`: publish the supported/unsupported late-bound matrix in docs
  and conformance evidence.
- `bd-bqm8.3.3`: deliver in-scope member-resolution, default-member, and
  named/missing-argument behavior where metadata is authoritative.
- `bd-bqm8.3.4`: extend controlled COM and host VM/JIT evidence for the
  supported rows.
- `bd-bqm8.3.5`: run the final late-bound `IDispatch` parity checklist and
  close `bd-bqm8.3` only if supported rows are green and residual rows are
  explicit.

## Verification

Passed:

- `rg -n "bd-bqm8\\.3|late-bound|IDispatch" .beads/issues.jsonl docs/worksets/WORKSET_2026-04-06_V0_2_SCOPE_ROUNDOUT_EXECUTION.md docs -g "*.md"`
