# AutoRun Control

Purpose: minimal machine-readable and operator-facing control surface for active ladder sync.

Mode: AutoRun
Intent: Continue implementing OxVBA against `MACH1000_PLAN.md` until project completion, using repeated build/test/docs/commit/push cycles.
Rule: The end of any cycle means continue immediately into the next cycle; do not pause for checkpoint-style stops.
Recovery rule: an accidental interim status reply is non-blocking and must be treated as a logging mistake, not a stop condition. Resume execution immediately.
Reply condition: only report back when the active ladder end gate is passed, or when all remaining progress is blocked by documented blockers.
Current user instruction (2026-07-07): take stock from the IP-08B detour, resume class-module
support in the JIT, and keep follow-up class-related beads ready in line with the current
class-module plan. `bd-h4oh.10.26`
(`M4-8 class metadata/package contract residual audit`) is closed; continue with
`bd-h4oh.10.27` (`M4-8 binder and lowering class semantics residual suite`). Current `.10.26`
evidence is `docs/evidence/class_metadata_package_contract_audit_20260707.md`: the class
descriptor path carries fields, `As New`, methods/properties, default members, `_NewEnum`,
lifecycle hooks, hidden `Me`, predeclared singletons, cross-project identity, implemented project
interfaces, and event routes without source reconstruction; no new package-contract field was
needed before binder/lowering residual work.
`bd-aprs.8.8` / IP-08B COM-host work is deferred from scheduler/ready output, not closed; its
remaining state is documented in `CURRENT_BLOCKERS.md` and
`docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`.
GPT-5.5 startup note: keep resume context bounded. Read the authoritative status surfaces below, then open only the active workset/bead/evidence files needed for the next outcome.

Active ladder/work bead:
- `bd-h4oh.10.27` under `bd-h4oh.10` / M4-8 objects, classes, lifecycle, after closing
  `bd-h4oh.10.26`.
Completed gate: `.10.26` reconciled the package/runtime class descriptor contract against the
class-module plan and routed remaining behavior work to `.10.27-.29` plus deferred COM export
handoff `bd-h4oh.15.1`.
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
2. Inspect `bd-h4oh.10.27`, `bd-h4oh.10`, and `docs/OXVBA_JIT_PLAN.md` M4-8.
3. Continue with the scoped class/JIT slice; do not resume IP-08B unless explicitly redirected.
