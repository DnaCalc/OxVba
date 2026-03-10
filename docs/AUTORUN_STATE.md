# AutoRun Control

Purpose: minimal machine-readable and operator-facing control surface for active ladder sync.

Mode: AutoRun
Intent: Continue implementing OxVBA against `MACH1000_PLAN.md` until project completion, using repeated build/test/docs/commit/push cycles.
Rule: The end of any cycle means continue immediately into the next cycle; do not pause for checkpoint-style stops.
Recovery rule: an accidental interim status reply is non-blocking and must be treated as a logging mistake, not a stop condition. Resume execution immediately.
Reply condition: only report back when the active ladder end gate is passed, or when all remaining progress is blocked by documented blockers.
Current user instruction: execute the active ladder continuously through terminal gate `v620` and apply blocker protocol from `AGENTS.md`.

Active ladders:
- `v467..v620` (`docs/worksets/PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md`)
Terminal gate: `v620`

Active worksets:
- `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE_V467_V620.md`
- `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`

Authoritative status surfaces:
- `AGENTS.md` for active execution doctrine and blocker protocol
- `docs/IMPLEMENTATION_LOG.md` for the execution history and tranche-by-tranche progress record
- `CURRENT_BLOCKERS.md` for active blockers and unblock requests
- `docs/profile-status/README.md` plus `docs/profile-status/PROFILE_STATUS_V*.md` for immutable historical gate records
- `docs/status-tours/` for narrative walkthroughs of completed slices

Resume protocol:
1. Read `AGENTS.md` and this file.
2. Run `./scripts/meta-check.ps1 -Fast`.
3. Continue active ladder execution until terminal gate `v620` is passed.
