# CORE-0 Terminal Truth and Queue Handoff

Date: 2026-07-11  
Bead: `bd-59co.2.1.3`

Status: accepted.

## Certified scope

CORE-0 is a support-only authority and control-plane epic. Its one canonical
row, `CORE-READINESS/CORE-AUTHORITY-CLEAN-SPEC-VBA`, is `verified`. That state
certifies the clean-room authority protocol and the repository's hierarchy of
contracts, active specifications, matrices, evidence, and bead truth. It does
not certify an Excel/VBA observation or advance any compiler, library, OxIR,
OxImage, VM3, JIT, host, Windows, or language-service capability.

The verified row is supported by all six required authority clauses:
`AUTH-CLEAN-001`, `AUTH-SPEC-001`, `AUTH-VBA-001`, `CONF-MATRIX-001`,
`DOC-AUTH-001`, and `DOC-TRACE-001`. Its evidence owner is
`bd-59co.2.1.2`; its residual disposition and residual owner are both empty.
All three CORE-0 leaves trace to the same row and have no residual owner.

## Imported control truth

CORE-0 consumes, and does not duplicate, the PROGRAM-0 control surfaces:

- the five Core matrices and their declared ownership in
  `IDEAL_MATRIX_OWNERSHIP_V1.csv`;
- the six clause dispositions above in
  `IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv`;
- the 42 reconciled legacy dispositions in
  `IDEAL_LEGACY_BEAD_MIGRATION_V1.csv`;
- all current bead-to-row routes in
  `IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv`;
- the environment roles in `IDEAL_ENVIRONMENT_MANIFEST_V1.csv`; and
- the generated profile totals in `IDEAL_PROGRAM_DERIVED_SUMMARY_LATEST.md`.

At this handoff the program contains 189 canonical rows, 226 trace
relationships, and 78 execution leaves. Core contains 55 rows: one authority
row is verified and the remaining 54 capability rows stay planned with active
downstream owners. Windows x64 contains 57 planned rows and IDE contains 77
planned rows. No capability credit is inferred from rollout, documentation,
or this terminal support bead.

## Environment handoff

- `win-x64-dev-oracle-2026-07` remains a characterized, noncertifying
  development/oracle host owned by `bd-59co.3.1.2`.
- `win-x64-cert-vm-pending-v1` remains a planned, blocking clean Windows x64
  and 64-bit Excel certification environment owned by `bd-59co.3.15.3`.
- `linux-x64-ci-pending-v1` remains a planned, blocking portable CI
  environment owned by `bd-59co.2.2`.

These handoffs preserve x64-only active Windows scope. None is treated as
release evidence by CORE-0.

## Observable axes

| Axis | Result |
|---|---|
| result | verified authority/control protocol only |
| full Err | not applicable; no VBA program is executed |
| side effects | verified: support evidence changes no capability state |
| lifecycle/event order | verified: PROGRAM-0 rollout precedes CORE-0 authority reconciliation, which precedes delivery rollout |
| transport | not applicable; no runtime, COM, native, or LSP transport is exercised |
| balance | verified: all CORE-0 traces terminate without a residual owner and all capability residuals retain downstream owners |

## Terminal checks and review

The terminal gate ran truth reconciliation, governance, path stability,
`br lint --json`, and dependency-cycle detection while `bd-59co.2.1.3` was the
sole active claim. The first pre-close run exposed a real test-harness defect:
synthetic negative-test graphs inherited the live repository's active claim.
The first post-close run showed the same fixtures also inherited the completed
CORE-0 rollout transition. The harness now reopens its stable synthetic CORE-0
anchor and clears every live active claim before each case selects the exact
closed and active states it is intended to exercise. All 24 negative/positive
cases and the full terminal gate then passed. `br lint` reported zero findings
and the dependency graph reported no cycles. The only compiler warning was the
pre-existing unused `projection_member_token_by_name` helper, which remains
owned by CORE-1's strict-clean-build lane and receives no credit here.

An independent non-author review confirmed the matrix/trace counts, all three
environment states and owners, the six observable axes, the support-only
boundary, and live residual owners for every planned Core capability row. Its
only initial finding was that this artifact had to record, rather than promise,
the post-close queue and boundary capacity snapshot; the following sections
resolve that finding.

## Accepted successor queue

After closing `bd-59co.2.1.3` and `bd-59co.2.1`, the authoritative command
`br ready -l ideal-2026-07 -t task --json` returned exactly these current-program
leaves, in priority/creation order:

1. `bd-59co.2.2.1` — roll out CORE-1 executable child beads.
2. `bd-59co.3.1.1` — roll out WIN-0 executable child beads.
3. `bd-59co.3.15.1` — roll out WIN-14 executable child beads.

No historical or non-program bead appeared. This is the intended three-worker
handoff: Core baseline, Windows x64 control/fixture establishment, and early
Windows x64 certification-environment planning.

## Three-agent capacity snapshot

The required epic-boundary refresh used
`bv --robot-capacity --agents 3 --capacity-label ideal-2026-07 -f json` at
`2026-07-11T02:48:00Z`. A normalized snapshot of the returned planning fields
is `capacity-3-agents.json` (source data hash `1e4aa5af57a19d59`, `bv` v0.15.2).

- Open issues: 120; estimated work: 91,051 minutes.
- Serial/parallel estimate: 10,337 / 80,714 minutes (88.647% parallelizable).
- Three-agent forecast: 77.585 days; critical-path length: 11.
- Critical path: `bd-59co` -> `bd-59co.2` -> `bd-59co.2.2` ->
  `bd-59co.2.3` -> `bd-59co.2.4` -> `bd-59co.3.10` ->
  `bd-59co.3.11` -> `bd-59co.3.11.1` -> `bd-9sed.17` ->
  `bd-59co.3.11.2` -> `bd-59co.3.11.4`.

The capacity projection is planning evidence only. Claims continue to come
exclusively from the certified `br ready` queue, never from `bv`'s actionable
projection.
