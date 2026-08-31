# WIN-2 Executable Rollout Acceptance

Date: 2026-08-23
Rollout bead: `bd-59co.3.3.1`
Status: accepted.

## Outcome and truth boundary

WIN-2 now has an executable path for a first late-IDispatch + x64 Declare
shared plan slice, then remaining session/cache/broader plan kinds. This is a
user-directed insertion: Gate A client/`Declare` rows do not wait for CORE-7
typed primary entries or CORE-8 cache. Those still block only the residual
leaf.

The owned row remains `planned`:

| Row | Delivery path | Current residual owner |
|---|---|---|
| `WAC-VERIFIED-INTEROP-PLAN` | `.2` through `.4`, then `.5` | `bd-59co.3.3.5` |

This support rollout awards no COM, Declare, serving, or native-output credit.

## Executable graph

| ID | Effect | Bounded outcome | Primary acceptance/evidence |
|---|---|---|---|
| `.3.3.2` | delivery | late IDispatch + x64 Declare plan types/verifier | `cargo test -p oxvba-runtime interop_plan`; `verified-interop-plan-types.md` |
| `.3.3.3` | delivery | migrate one VM3 late COM and one VM3 Declare path | VM3 interop_plan tests; `vm3-plan-migration.md` |
| `.3.3.4` | delivery | JIT helper adapter + two fail-closed fixtures | `jit_windows_vm3_parity`; `jit-plan-adapter.md` |
| `.3.3.5` | delivery | remaining session/cache/early/event/serving plans | blocked on WIN-1 and CORE-8; `remaining-session-attachment.md` |

WIN-3 first scalar late COM is `bd-59co.3.4.2` (blocked by `.3.3.4`). WIN-9
first x64 scalar Declare is `bd-59co.3.10.2` (blocked by `.3.3.4`). Remaining
WIN-3/WIN-9 rows stay on `.3.4.3` / `.3.10.3`.

## Graph repair incorporated

WIN-2's epic-level `blocks` on WIN-1 (`bd-59co.3.2`) moved onto
`bd-59co.3.3.5`. WIN-3 and WIN-9 epic-level waits on WIN-1/WIN-2/CORE-3 moved
onto their residual children so first scalar rows can follow the plan adapter.

## Observable and closure policy

Applicable leaves record result, full Err including LastDllError, side effects,
lifecycle/order, transport and balance. No snapshot may be blessed implicitly.
`bd-59co.2.9.9` and genuine WIN-12 stay parked.

WIN-2 can close only after the owned row is verified and no remaining accepted
architecture residual exists.
