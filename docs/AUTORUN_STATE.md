# Execution Control

Purpose: the sole volatile control surface for current execution mode, accepted worksets, terminal condition and resume context.

Mode: Directed
AutoRun terminal gate: inactive
Current task: execute `bd-59co.1` (`PROGRAM-0`) to reconcile the x64-only active target, roll out and polish the three-workset bead graph, migrate legacy open beads and prove that the ready queue is safe for AutoRun.
Terminal condition: `PROGRAM-0` is closed with current authority consistent, every execution epic rolled out, every legacy non-closed bead explicitly dispositioned, graph/validators green, and two consecutive polish passes finding no material change.
Last completed task: establish the OxVba system contract/current architecture, deprecate misleading historical guidance, and rewrite the three 2026-07-10 readiness worksets around the ideal architecture.

## Accepted umbrella and worksets under directed rollout

- umbrella bead: `bd-59co`

- `docs/worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md`
- `docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md`
- `docs/worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`

## Current truth surfaces

- `CHARTER.md`
- `OPERATIONS.md`
- `docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md`
- `docs/ARCHITECTURE.md`
- `docs/OXVBA_POST_JIT_STATUS_REVIEW_2026-07-10.md`

Historical ladders, IP-08/IP-08B plans, MACH-1000 profiles and earlier JIT/frontend/language-service worksets remain provenance only unless PROGRAM-0 explicitly imports a residual into `bd-59co`.

## Resume rule

Read the current truth surfaces, inspect `bd-59co.1` and `br ready`, and continue the directed PROGRAM-0 terminal condition. Do not enable AutoRun or execute an unrelated ready bead until PROGRAM-0 has certified the new queue.
