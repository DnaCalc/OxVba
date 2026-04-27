# GPT-5.5 Documentation Update

## What Changed

The repo startup and operations docs were updated for smoother GPT-5.5/Codex
execution while preserving the existing OxVba workset and bead method.

The main change is a shift toward outcome-first startup guidance:

- state the intended outcome, constraints, evidence, and stop condition
- keep startup context bounded to the active workset, bead, blocker, and
  evidence surfaces
- let the model choose the efficient implementation path from repo context
- keep validation and evidence requirements explicit before any completion
  language
- avoid long procedural scripts unless the exact sequence is part of the
  product or verification requirement

## Files Updated

- `AGENTS.md` now includes GPT-5.5/Codex startup guidance for outcome-first
  execution, bounded context gathering, reasoning-effort defaults, concise
  tool-heavy work updates, and final-cycle verification.
- `OPERATIONS.md` now frames durable operational guidance as outcome-first
  contracts while keeping the workset/bead doctrine unchanged.
- `docs/AUTORUN_STATE.md` now adds a startup note to keep resume context
  bounded to authoritative status surfaces and the next concrete outcome.
- `docs/methods/beads/BEADS_WORKING_METHOD.md` now describes GPT-5.5-friendly
  bead records as outcome contracts rather than rigid step-by-step recipes.
- `docs/methods/beads/BEAD_QUALITY_CONTRACT.md` now calls for compact,
  outcome-first bead records and locally verifiable completion evidence.
- `docs/methods/beads/BEADS_BREAKDOWN_PROMPT.md` now asks for the smallest
  useful executable set, dependencies, blockers, and completion evidence.
- `docs/methods/beads/BEADS_BREAKDOWN_EXAMPLE.md` now uses neutral AutoRun
  execution language.
- `docs/templates/WORKSET_EPIC_BEAD_ROLLOUT_TEMPLATE.md` now asks rollout
  authors to keep terminal conditions, evidence, dependencies, and uncovered
  follow-up behavior explicit.

## What Did Not Change

- The workset hierarchy remains `workset -> epic -> bead`.
- Beads remain the unit of executable progress and must still close only on
  stated outcome plus completion evidence.
- Capability lanes still cannot close on support/documentation work alone.
- The AutoRun loop, blocker protocol, validation discipline, and commit/push
  cadence remain intact.
- `CHARTER.md` and `MACH1000_PLAN.md` were not materially changed; their
  mission, scope, architecture, and sequencing remain authoritative.

## Why This Helps

The OpenAI GPT-5.5 guidance emphasizes smaller prompt stacks, outcome-first
instructions, explicit success criteria, bounded reasoning/tool use, and clear
stopping rules. These updates make OxVba startup docs more compatible with that
style without weakening the repo's existing execution doctrine.

Expected effects:

- less context loading at startup
- fewer duplicated process instructions
- clearer distinction between durable doctrine and volatile run state
- better bead records for long-running coding sessions
- more consistent validation before completion claims

## Sources

- `https://developers.openai.com/api/docs/guides/latest-model`
- `https://developers.openai.com/api/docs/guides/prompt-guidance`
