# Bead Quality Contract

Status: `active`

This note defines the minimum quality bar for executable beads.

## Purpose

A bead is only useful if someone else can:
1. understand what it is supposed to produce,
2. execute it without hidden scope,
3. review whether it is actually done.

This contract exists to keep bead state trustworthy.

## Required Elements

Every executable bead should identify:
1. one reviewable outcome,
2. completion evidence,
3. its parent epic,
4. any required dependency relationship,
5. for validation/conformance work, the canonical matrix or matrix rows it advances.
6. whether it is primarily a `delivery` bead or a `support` bead.

## Outcome Rule

Good:
- one capability now exists,
- one audit result now exists,
- one rewrite/demotion now exists,
- one matrix walk now exists.

Bad:
- vague activity,
- ongoing theme,
- open-ended exploration without a bounded output.

## Completion Evidence Rule

Each bead should say what makes it closeable.

Examples:
- files created or updated,
- checks run,
- tests added or passing,
- matrix rows updated,
- audit classifications written,
- archive/demotion action completed.

If the evidence is not visible, the bead is not ready to close.

Effect rule:
- `delivery` beads change or prove real behavior in the capability lane.
- `support` beads improve the path, truth, or organization around the lane.
- Both are valid bead types, but only `delivery` beads count toward actual capability completion.

## Traceability Rule

For validation work, record the canonical matrix touched.

Preferred forms:
1. bead description names the matrix file and row ids,
2. bead external ref points to the matrix-relevant artifact,
3. `docs/validation/MATRIX_BEAD_TRACEABILITY_2026-03-29.csv` maps the bead to matrix files or rows.

## Rollout Rule

When an epic is new or has drifted:
1. create a rollout bead,
2. use that bead to create or refresh the executable child set,
3. do not claim the epic is ready until the next believable path exists.

## Closure Rule

At bead finish, exactly one of these should be true:
1. the stated outcome and evidence are satisfied, so the bead closes,
2. the bead uncovered required follow-up or blocking work, and that work is added as new beads before any closure claim.

Additional closure guard:
- If a `support` bead reveals remaining capability work, it must leave behind the next `delivery` bead or a believable delivery-ready path.
- Do not let a capability lane terminate on truth repair, documentation cleanup, or rollout alone.

## Anti-Patterns

Do not:
- close a bead because “enough progress” happened,
- leave required follow-up work only in chat or commit messages,
- silently widen a bead until it becomes a mini-workset,
- use a rollout bead as a substitute for real child beads.
- use support-bead closure as a substitute for delivered behavior.
