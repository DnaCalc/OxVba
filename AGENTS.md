# AGENTS.md

Primary guidance for automated contributors, in order:
1. `CHARTER.md`
2. `OPERATIONS.md`
3. `docs/ARCHITECTURE.md`
4. Active worksets, status files, and evidence artifacts

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
  - relevant checks have run,
  - fresh-eyes review has actively looked for blunders, mistakes, oversights, omissions, logical gaps, misconceptions, hidden assumptions, regressions, and bugs,
  - compatibility/parity claims are backed by tests or evidence,
  - docs and bead state reflect the same truth,
  - no required follow-up is left only in chat.

## Workset Status Doctrine
- Follow `OPERATIONS.md` section `3.1 Workset Completion Doctrine` as binding terminology and execution policy.
- Do not describe partial subsets as `implemented`, `closed`, or `closure`.
- If parity for the scoped work area is not complete, status remains `in-progress` and the remaining blocker/question must be documented explicitly.
- For repo project-scope work executed through beads, capability lanes do not close on documentation, audit, rollout, or other support-only bead outcomes alone.
- If a support bead exposes unfinished capability work, it must leave behind the next delivery bead or delivery-ready path before the lane is considered clean.

## VBA Compatibility Objective
- The goal is always to match real VBA compile-time and run-time behavior.
- Do not preserve legacy OxVBA behavior, fallbacks, or conveniences as compatibility
  targets. If such a path remains temporarily, document it as a gap and keep
  delivery status `in-progress` until it matches VBA or the residual scope is
  explicitly split into an open bead.
- When behavior is uncertain, use public specifications and reproducible
  Excel/VBA oracle observations to decide the target behavior before making
  compatibility claims.

## Excel/VBA Oracle Modal Handling
- When driving real Excel/VBA as an oracle, always be prepared to intercept modal
  compile/runtime dialogs with UI Automation before starting the run. Follow
  Govert's Excel/VBA agentic coding guide and Jun 27, 2026 follow-up comment:
  `https://gist.github.com/govert/2d3946830c35c74806df3f32b597eb72`.
- Do not rely on `Application.Run` as a compile check. If compile diagnostics are
  in scope, make the VBE visible, invoke Debug -> Compile VBAProject, then use
  UI Automation to read the modal text and the VBE selected token/line.
- Treat "Cannot run the macro ... may not be available" as ambiguous: macros may
  be disabled, the macro may genuinely be missing, or any procedure in the
  project/module may have failed to compile. If VBOM access is available, macros
  are enabled, and the macro exists, investigate it as a compile failure.
- If Excel is unresponsive after a second or two, inspect UIA windows scoped to
  the Excel/VBE process before assuming a hang. Capture dialog text, highlighted
  token, and full selected code line; then dismiss only owned/scoped dialogs.
- Compile errors can surface at a call site far away from the real defective
  declaration. Also check the called procedure declaration and intrinsic-name
  shadowing traps such as `Fix`, `Date`, `Time`, `Name`, `Error`, `Left`,
  `Right`, `Len`, `Val`, and `Format`.
- Keep all Excel cleanup PID-scoped. Never blanket-dismiss `#32770` dialogs or
  kill Excel processes not recorded as owned by the oracle run.

## AutoRun Continuation Rule
- Active mode is `AutoRun`.
- End of a work cycle means immediately continue to the next cycle.
- Do not stop at checkpoints unless explicitly instructed by the user.
- Keep repeating: implement -> docs update -> checks -> fresh-eyes review -> close bead when satisfied -> commit closure state -> push -> continue.
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
