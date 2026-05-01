# Archive

Historical planning documents retained for provenance. These documents are **superseded** and should not be used for current project guidance.

| Document | Original location | Date | Superseded by | Notes |
|---|---|---|---|---|
| [PLAN_v1_20260226.md](PLAN_v1_20260226.md) | `PLAN.md` | 2026-02-26 | [`MACH1000_PLAN.md`](../../MACH1000_PLAN.md), then current architecture/worksets | Original OxVba project plan. Baseline for synthesis run `20260226-mach1000-synthesis`. |
| [BRAINSTORM_MACH1000_20260226.md](BRAINSTORM_MACH1000_20260226.md) | `BRAINSTORM.md` | 2026-02-26 | [`MACH1000_PLAN.md`](../../MACH1000_PLAN.md), then current architecture/worksets | MACH-1000 theoretical architectures brainstorm. Input to synthesis run `20260226-mach1000-synthesis`. |

| [REVIEW_20260309.md](REVIEW_20260309.md) | `docs/` | 2026-03-09 | Subsequent IP-03–IP-09 work | Point-in-time review output (82K). Archived 2026-03-22. |
| [REVIEW_20260309_PROCEED.md](REVIEW_20260309_PROCEED.md) | `docs/` | 2026-03-09 | All items executed | Approved items from the March 9 review. Archived 2026-03-22. |
| [REVIEW_20260309_DEFER.md](REVIEW_20260309_DEFER.md) | `docs/` | 2026-03-09 | Items done or tracked in worklist | Deferred items from the March 9 review. Archived 2026-03-22. |
| [REVIEW_20260309_TRIAGE_PLAN.md](REVIEW_20260309_TRIAGE_PLAN.md) | `docs/` | 2026-03-09 | Triage completed | Triage plan from the March 9 review. Archived 2026-03-22. |
| [REVIEW_20260309_FOLLOWUP.md](REVIEW_20260309_FOLLOWUP.md) | `docs/` | 2026-03-09 | Follow-ups completed | Follow-up items from the March 9 review. Archived 2026-03-22. |
| [PHASE12_STATUS.md](PHASE12_STATUS.md) | `docs/` | 2026-03-06 | Current v467+ state | Phase 12 gate for v146, long superseded. Archived 2026-03-22. |
| [IN_PROGRESS_FEATURE_EXECUTION_2026-03-10.md](IN_PROGRESS_FEATURE_EXECUTION_2026-03-10.md) | `docs/` | 2026-03-10 | Later worklist updates | Dated execution snapshot. Archived 2026-03-22. |
| [MACH1000_PLAN_REFINEMENT_20260226.md](MACH1000_PLAN_REFINEMENT_20260226.md) | `docs/` | 2026-02-26 | Absorbed into MACH1000_PLAN | Refinement notes. Archived 2026-03-22. |
| [status-tours/](status-tours/) | `docs/status-tours/` | 2026-02-27 – 2026-03-05 | N/A | Session snapshot tours (17 files). Archived 2026-03-22. |

## Relationship to Synthesis

These documents were the two inputs to the formal synthesis run documented in [`synthesis/runs/20260226-mach1000-synthesis/`](../../synthesis/runs/20260226-mach1000-synthesis/README.md). The synthesis decision log records how each suggestion from the brainstorm was evaluated (accept/adapt/defer/reject) and integrated into `MACH1000_PLAN.md`.

As of 2026-04-30, `MACH1000_PLAN.md` remains top-level historical synthesis and vision context rather than the definitive current implementation plan. Current truth lives in [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md), active worksets, status files, and evidence artifacts; the native-ready rebase umbrella is [`docs/worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`](../worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md).
