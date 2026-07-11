# CORE-0 Rollout Acceptance

Date: 2026-07-11  
Bead: `bd-59co.2.1.1`

Status: accepted.

CORE-0 is a support-only authority/control epic. Its rollout leaves two exact
support outcomes, while compiler/runtime capability delivery remains in the
other Core epics:

- `bd-59co.2.1.2` repairs and verifies semantic-authority and clean-room
  guidance for `CORE-READINESS/CORE-AUTHORITY-CLEAN-SPEC-VBA`;
- `bd-59co.2.1.3` independently certifies the canonical truth surfaces and
  queue handoff after the authority work.

The authority row's planned evidence and residual ownership has moved from the
rollout scaffold to `bd-59co.2.1.2`. The rollout trace is now non-owning
evidence. The two successors are siblings under CORE-0, are bounded to 360 and
240 minutes, carry exact clauses and resource metadata, and form the dependency
chain `rollout -> authority hygiene -> terminal handoff`.

PROGRAM-0's five Core matrices, clause disposition, environment manifest,
legacy migration, trace ledger and generated summary are reused as control
evidence. Their 54 non-authority Core capability rows remain planned with their
existing delivery owners; this rollout awards no implementation credit.

Acceptance checks:

- `./scripts/validate-workset-rollout.ps1`;
- `./scripts/run-truth-reconciliation.ps1`;
- `./scripts/test-ideal-program-validator-negative-cases.ps1`;
- `br lint --json` and `br dep cycles`.

Final results:

- truth reconciliation and governance passed at 189 rows, 226 exact trace
  relationships and 78 execution leaves;
- the queue validator passed with `ready=0 active=1`, proving that the claimed
  rollout is live and unblocked;
- the expanded fail-closed suite passed 24 cases plus positive guards for an
  unblocked active claim and manifest-declared support-only rollout;
- manifest effects prevent mutable labels from weakening delivery closure;
- active claims traverse their own and ancestor blockers and are capped at the
  three-worker limit;
- dependency cycles and bead lint findings are zero;
- three independent fresh-eyes reviews returned clean after their findings were
  repaired.

Observable classification: result is the exact executable successor path;
full Err, runtime side effects, lifecycle order, transport and runtime balance
are not applicable to rollout control. Any capability gap discovered later must
remain with or receive an exact downstream delivery owner.
