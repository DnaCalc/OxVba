# AGENTS.md

Primary guidance for automated contributors, in order:
1. `CHARTER.md`
2. `OPERATIONS.md`
3. `MACH1000_PLAN.md`

Use `docs/` and `synthesis/` for supporting context and provenance.

## GPT-5.5 Agent Operating Shape
- Treat this file as the startup contract for GPT-5.5/Codex-style agents: outcome first, constraints explicit, evidence before completion language.
- Keep startup context lean. Read the three primary guidance documents first, then open only the active workset, bead, blocker, or evidence files needed for the current outcome.
- Prefer bounded context gathering:
  - start broad enough to identify the exact files and truth surfaces,
  - stop gathering once the next concrete edit/check path is clear,
  - search again only when validation fails, signals conflict, or new unknowns appear.
- Use `medium` reasoning as the default balanced mode for this repo. Escalate effort only for hard architectural, formal, or parity decisions where extra reasoning is likely to improve correctness.
- For tool-heavy work, state the intended action briefly, execute it, then verify the outcome. Do not add long progress narratives that compete with the bead/workset truth surfaces.
- Before finalizing a cycle, check:
  - the requested outcome is satisfied or explicitly blocked,
  - compatibility/parity claims are backed by tests or evidence,
  - docs and bead state reflect the same truth,
  - no required follow-up is left only in chat.

## Workset Status Doctrine
- Follow `OPERATIONS.md` section `3.1 Workset Completion Doctrine` as binding terminology and execution policy.
- Do not describe partial subsets as `implemented`, `closed`, or `closure`.
- If parity for the scoped work area is not complete, status remains `in-progress` and the remaining blocker/question must be documented explicitly.
- For repo project-scope work executed through beads, capability lanes do not close on documentation, audit, rollout, or other support-only bead outcomes alone.
- If a support bead exposes unfinished capability work, it must leave behind the next delivery bead or delivery-ready path before the lane is considered clean.

## AutoRun Continuation Rule
- Active mode is `AutoRun`.
- End of a work cycle means immediately continue to the next cycle.
- Do not stop at checkpoints unless explicitly instructed by the user.
- Keep repeating: implement -> docs update -> checks -> commit -> push -> continue.
- Accidental interim chat responses are non-blocking events; they do not reset or pause AutoRun.
- If an interim response is sent by mistake, immediately resume execution without waiting for additional user confirmation.

## Current User Execution Constraint
- Continue implementation continuously.
- Do not stop for progress summaries, watchpoints, confirmations, checkpoints, or any other interim responses.
- Execute the active umbrella workset continuously until completion.
- Maintain legacy ladder state in sync while the umbrella workset remains the actual stop condition for this run.
- Only reply when one of the following is true:
  - `IP-08` is complete and the umbrella-workset terminal gate is therefore passed.
    - Current active umbrella workset:
      - `docs/worksets/WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md`
    - Current active ladder:
      - `v467..v620` (`docs/worksets/PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md`)
    - Current required terminal gate: `v620`
    - Current AutoRun exit gate for this run:
      - completion of `IP-08`, with AutoRun continuing through all prerequisite remaining umbrella-workset phases needed to reach that gate (`IP-03`, `IP-05`, `IP-06`, `IP-07`, and `IP-08`)
  - Blockers are documented and no progress can be made on any remaining umbrella-workset task without unblocking.

### Blocker Handling Protocol
- If blocked on a linear path, create or update `CURRENT_BLOCKERS.md` with the blocker entry.
- After documenting a blocker, continue with any other work that makes progress toward the final milestone.
- If all progress is blocked:
  - Add a structured summary in `CURRENT_BLOCKERS.md` with:
    - blocker IDs/titles,
    - impact by milestone/phase,
    - exact unblocking steps,
    - suggestions/questions for the user.
  - Then send a user request containing those details.

### Formal Verification Execution Policy (Current Ladder Run)
- Run formal verification lanes in every cycle where relevant changes are made.
- Formal verification failures are currently non-blocking.
- Apply moderate effort to fix formal failures in-cycle; if unresolved, track them in an extended to-do list/evidence backlog and continue ladder execution.

## Immediate Instruction Capture
- This file update records the latest instruction set.
- Do not start implementation work until the user explicitly asks to continue.
- Latest explicit continue instruction (2026-03-19): enable AutoRun against the full active umbrella workset, set the exit gate explicitly to `IP-08` completion, and continue execution until that gate is passed or all remaining progress is blocked.
