# PROGRAM-0.4 Graph Polish and AutoRun Acceptance Evidence

Initiated: 2026-07-10  
Completed: 2026-07-11  
Bead: `bd-59co.1.4`

Outcome: accepted. The directed review sequence exposed and repaired the stricter AutoRun transition defect, the Pass 9 owner/consumer routing defect, and the hidden certification-route defects. The final independent semantic and documentation rereads were clean.

Accepted post-repair state:

- Windows scope is x64 and actual 64-bit Excel only; excluded Windows targets have no active gate or successor;
- 42 execution epics and 42 rollout leaves exist beneath three independently closable profile roots;
- 15 matrices contain 189 required `planned` rows: Core 55, Windows x64 57, IDE 77;
- 224 exact relationships cover all 189 rows and all 76 execution leaves;
- all 30 LSP rows remain fail-closed with `capability_advertised=false`;
- 60 clauses, 42 legacy issues, and three execution environments have complete dispositions;
- resource admission permits no more than two Rust writers and serializes all named exclusive lanes;
- dependency cycles are zero;
- `br ready -l ideal-2026-07 -t task` is the only claim command and its writable database result initially returns only `bd-59co.2.1.1`; ancestor epic blockers make later rollout leaves non-ready even when a leaf has no direct blocker edge.

Checks passed to date:

- `br lint --json`;
- `br dep cycles`;
- `scripts/check-governance.ps1`;
- `scripts/run-truth-reconciliation.ps1`;
- `scripts/test-path-stability.ps1`;
- all 19 negative validator cases, including missing owner/consumer/matrix routes, reverse leaf parity and non-leaf trace rejection;
- independent Pass-6 and Pass-7 reviews;
- independent final Pass-9 semantic and documentation rereads, both clean;
- `git diff --check` with no whitespace error.

The full pass history, capacity snapshot, environment variance, and residual disposition are recorded in `PROGRAM-0-POLISH-LOG-2026-07-10.md`.

Residual state: no PROGRAM-0 blocker. All capability outcomes remain planned delivery work; the existing COM projection dead-code warning is owned by CORE-1. The certified initial delivery claim is `bd-59co.2.1.1`.
