# AGENTS.md

Primary guidance for automated contributors, in order:
1. `CHARTER.md`
2. `OPERATIONS.md`
3. `MACH1000_PLAN.md`

Use `docs/` and `synthesis/` for supporting context and provenance.

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
- Execute the MACH1000 profile ladder continuously until completion.
- Only reply when one of the following is true:
  - The active profile ladder is complete and its final gate is passed.
    - Current active ladder:
      - `v387..v406` (`docs/worksets/PROFILE_LADDER_2026-03-04_MACH1000_V387_V406_COM_CLIENT_LATEBOUND_C2.md`)
    - Current required terminal gate: `v400`
  - Blockers are documented and no progress can be made on any remaining ladder task without unblocking.

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
