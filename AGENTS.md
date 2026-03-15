# AGENTS.md

Primary guidance for automated contributors, in order:
1. `CHARTER.md`
2. `OPERATIONS.md`
3. `MACH1000_PLAN.md`

Use `docs/` and `synthesis/` for supporting context and provenance.

## Workset Status Doctrine
- Follow `OPERATIONS.md` section `3.1 Workset Completion Doctrine` as binding terminology and execution policy.
- Do not describe partial subsets as `implemented`, `closed`, or `closure`.
- If parity for the scoped work area is not complete, status remains `in-progress` and the remaining blocker/question must be documented explicitly.

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
- Only reply when one of the following is true:
  - The active umbrella workset is complete and its terminal gate is passed.
    - Current active umbrella workset:
      - `docs/worksets/WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md`
    - Current required terminal gate: `v620`
    - Current AutoRun exit gate for this run:
      - completion of the full scope of the active umbrella workset, including the remaining `IP-02`, `IP-03`, `IP-05`, `IP-06`, `IP-07` dependency slices, and `IP-08` work defined there
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
- Latest explicit continue instruction (2026-03-15): enable AutoRun against the full active umbrella workset, use the umbrella-workset completion condition as the terminal gate, and continue execution until that gate is passed or all remaining progress is blocked.
