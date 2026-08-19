# CORE-7 Executable Rollout Acceptance

Date: 2026-08-18
Rollout bead: `bd-59co.2.9.1`
Status: accepted.

## Outcome and truth boundary

CORE-7 now has an executable path for portable VM3/JIT basics first, then the
remaining ideal architecture. This is a user-directed insertion: matching the
interpreter does not wait for CORE-3/CORE-4/CORE-5 to close. Those epics still
block only the later architecture leaf.

The owned rows remain `planned`:

| Row | Delivery path | Current residual owner |
|---|---|---|
| `CORE-JIT-LOWERING` | `.2` through `.8`, then `.9` | `bd-59co.2.9.9` |
| `OXIR-JIT-DISPOSITION` | `.2` through `.8`, then `.9` | `bd-59co.2.9.9` |

This support rollout awards no JIT capability credit.

## Executable graph

| ID | Effect | Bounded outcome | Primary acceptance/evidence |
|---|---|---|---|
| `.2.9.2` | delivery | structural VM3/JIT portable-basics harness | `cargo test -p oxvba-differential --test jit_portable_vm3_parity`; `portable-vm3-jit-parity-harness.md` |
| `.2.9.3` | delivery | line/Erl/Err write and Resume seating | focused error/Erl tests; `error-erl-parity.md` |
| `.2.9.4` | delivery | remaining portable OxIR declines, including unused-Declare whole-image rejection | JIT plus harness; `portable-instruction-declines.md` |
| `.2.9.5` | delivery | portable calls, ByRef, Optional, ParamArray | harness call slice; `call-byref-parity.md` |
| `.2.9.6` | delivery | portable arrays, records, strings, project objects | harness aggregate slice; `aggregate-object-parity.md` |
| `.2.9.7` | delivery | portable VBA library routes | harness library slice; `library-route-parity.md` |
| `.2.9.8` | support | portable-basics pause gate | full harness plus truth reconciliation; `portable-basics-pause-gate.md` |
| `.2.9.9` | delivery | remaining ideal architecture after pause | blocked on `.8` and CORE-3/4/5; `remaining-architecture.md` |

Every leaf is at most 480 minutes and has explicit resource/risk/model labels,
contract clauses, exact traces, typed commands, an artifact, observable axes
and residual behavior. Windows COM, Declare execution, pointers, JIT
sessions/cache and native packaging stay outside this tranche.

## Graph repair incorporated

CORE-7's epic-level `blocks` on CORE-3 (`bd-59co.2.4`), CORE-4 (`bd-59co.2.6`)
and CORE-5 (`bd-59co.2.7`) moved onto `bd-59co.2.9.9`. Portable-basics delivery
does not consume sealed `VerifiedOxImage`, `AnalysisResultV1` or the versioned
helper catalog. The epic still cannot close until `.9` closes.

## Current characterization incorporated

- JIT is a 35,454-line Cranelift backend with a universal dynamic ABI.
- VM3 executes the full portable OxInst vocabulary.
- `SetLineNumber` is a no-op; `ErlGet` and `ErrFieldSet` are not lowered.
- Whole-image admission declines any program with unused Declare/COM tables.
- M4-era subset declines remain for omitted Optional, ParamArray, some
  arith/coerce lanes, library `NewExtern`, and `TypeOfIs`.
- Linux-safe snapshot compile/decline status is not structural parity.

These are planned delivery inputs, not completion evidence.

## Observable and closure policy

Applicable leaves record result, full Err including Erl, side effects,
lifecycle/order, transport and balance. No snapshot may be blessed implicitly.
`bd-59co.2.9.8` is the pause gate: after it, stop for the Windows/COM/packaging
discussion. `.9` remains the residual owner and does not become the next
user-directed task until that discussion.

CORE-7 can close only after both owned rows are verified and no remaining
accepted architecture residual exists.

## Acceptance record

Checks:

- `./scripts/validate-workset-rollout.ps1`
- `./scripts/run-truth-reconciliation.ps1`
- `br lint --json`
- `br dep cycles`

Expected observable: bounded delivery/support children exist with exact row
traces, producer dependencies, typed acceptance commands and residual owners;
no capability closes on this support bead.
