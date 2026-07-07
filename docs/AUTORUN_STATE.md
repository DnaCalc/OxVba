# AutoRun Control

Purpose: minimal machine-readable and operator-facing control surface for active ladder sync.

Mode: AutoRun
Intent: Continue implementing OxVBA against `MACH1000_PLAN.md` until project completion, using repeated build/test/docs/commit/push cycles.
Rule: The end of any cycle means continue immediately into the next cycle; do not pause for checkpoint-style stops.
Recovery rule: an accidental interim status reply is non-blocking and must be treated as a logging mistake, not a stop condition. Resume execution immediately.
Reply condition: only report back when the active ladder end gate is passed, or when all remaining progress is blocked by documented blockers.
Current user instruction (2026-07-07): take stock from the IP-08B detour, resume class-module
support in the JIT, and keep follow-up class-related beads ready in line with the current
class-module plan. `bd-h4oh.10.29`
(`M4-8 class support terminal docs and residual handoff`) is closed. Current terminal evidence is
`docs/evidence/class_m4_8_terminal_residual_handoff_20260707.md`: accepted project-class execution
rows are green for construction, `As New`, identity/type checks, member/property/default-member
dispatch, object-valued properties, predeclared singletons, lifecycle/termination timing,
cross-project classes, project events, unsupported diagnostics, and exposed handle-balance checks.
COM activation/dispatch, COM connection points, COM server/export, AOT COM vtables, and live
Windows COM interop remain later-lane residuals.
`bd-aprs.8.8` / IP-08B COM-host work is deferred from scheduler/ready output, not closed; its
remaining state is documented in `CURRENT_BLOCKERS.md` and
`docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`.
GPT-5.5 startup note: keep resume context bounded. Read the authoritative status surfaces below, then open only the active workset/bead/evidence files needed for the next outcome.

Active ladder/work bead:
- `bd-h4oh.10` remains open only because its legacy ladder dependency `bd-h4oh.9` is still open;
  all class-related child beads through `bd-h4oh.10.29` are closed.
Completed gate: `.10.29` reconciled terminal docs, unsupported diagnostics, and residual handoff
for the accepted M4-8 project-class execution subset.
Follow-up beads staged under `bd-h4oh.10`: `bd-h4oh.10.24` through `bd-h4oh.10.29`, chained in
execution order after `bd-h4oh.10.23`. `bd-h4oh.10.26-.29` cover the post-events class metadata
audit, binder/lowering residual suite, VM3/JIT parity sweep, and terminal docs/residual handoff.
Class-related COM export readiness is deferred to `bd-h4oh.15.1` under the later M4-13 COM/AOT
lane and must not block the current M4-8 class/JIT chain.

Active worksets:
- `docs/OXVBA_JIT_PLAN.md`
- `docs/worksets/WORKSET_2026-05-08_COM_SHAPED_INTERNAL_OBJECT_ABI.md`
- `docs/evidence/divergences/DIV-0005.md`

Authoritative status surfaces:
- `AGENTS.md` for active execution doctrine and blocker protocol
- `docs/IMPLEMENTATION_LOG.md` for the execution history and tranche-by-tranche progress record
- `CURRENT_BLOCKERS.md` for active blockers and unblock requests
- `docs/profile-status/README.md` plus `docs/profile-status/PROFILE_STATUS_V*.md` for immutable historical gate records
- `docs/status-tours/` for narrative walkthroughs of completed slices

Resume protocol:
1. Read `AGENTS.md` and this file.
2. Inspect `bd-h4oh.10`, `bd-h4oh.9`, and `docs/OXVBA_JIT_PLAN.md` M4-8/M4-7 before attempting
   parent-bead closure.
3. Do not resume IP-08B unless explicitly redirected.
