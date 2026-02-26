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
