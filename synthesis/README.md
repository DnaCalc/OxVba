# OxVba Synthesis

Synthesis runs convert brainstorm ideas, prompt-run outputs, and research findings into disciplined, traceable edits to source-of-truth project documents.

This process follows the methodology established in the DNA Calc Foundation repository.

## Source-of-Truth Precedence

When suggestions conflict with existing doctrine, the hierarchy is:

1. `PLAN.md` Section 1 (Charter — mission/values/scope)
2. `PLAN.md` Section 2 (Architecture)
3. `PLAN.md` Section 3 (Formal Approach)
4. `PLAN.md` Sections 4–8 (Testing, Research, Brainstorming, Structure, Sequencing)
5. `BRAINSTORM.md` (supporting context / suggestions)

## Decision Actions

| Action   | Meaning |
|----------|---------|
| `accept` | Suggestion incorporated as-is |
| `adapt`  | Suggestion valuable but modified to fit context/precedence |
| `defer`  | Suggestion valid but timing is wrong or dependencies unresolved |
| `reject` | Suggestion contradicts existing doctrine or is infeasible |

## Workflow

1. Freeze source inputs by hash
2. Extract suggestions from source documents
3. Classify suggestions by target section
4. Decide each suggestion with explicit action and rationale
5. Apply accepted/adapted edits to produce output document
6. Emit synthesis report and manifest
