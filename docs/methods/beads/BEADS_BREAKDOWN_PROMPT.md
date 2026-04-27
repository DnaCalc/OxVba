# Beads Breakdown Prompt

Use this prompt when you have:
- a scope document,
- an engineering spec,
- an architecture note,
- or an initial work-item list,

and you want to turn it into something that is ready for bead generation.

This prompt is for preparation.

It should not immediately generate a giant backlog.

For GPT-5.5/Codex use, keep the prompt outcome-first. Ask for the smallest
useful executable set, success criteria, dependencies, and blockers rather than
a rigid implementation script.

## Prompt

```text
Read the attached engineering spec, scope note, and any existing work-item breakdown.

Your job is to prepare this project for bead generation using the beads working method.

Do not generate a full bead backlog yet.

Instead, produce these sections:

1. Project intent
- Summarize what the project is trying to achieve.
- State what is in scope and out of scope.

2. First thin slice
- Identify the best first end-to-end slice to prove the design.
- Explain why this is the right starting slice.

3. Worksets
- Reduce the design into 3-7 major capability worksets.
- Each workset should be milestone-sized and outcome-oriented.
- Do not use vague themes or giant umbrella categories.

4. Candidate beads
- For each workset, list the likely executable beads that would later be created.
- Phrase them as reviewable outcomes, not activities.
- Keep them concrete, but not tiny.

5. Dependency structure
- Identify what depends on what.
- Call out the intended execution order.
- Identify anything that can proceed in parallel.

6. Risks and ambiguities
- Identify parts of the design that are still too vague to break down cleanly.
- Flag any assumptions that should be clarified before bead generation.

7. Recommended first bead set
- Propose the first parent bead, if one is useful.
- Propose the first 4-8 beads that should actually be created.
- Explain why these are the right first executable set.
- Do not emit CLI commands yet.

Constraints:
- Optimize for execution clarity, not project-management completeness.
- Prefer a small number of meaningful work units over a large backlog.
- Keep bead candidates outcome-oriented and dependency-aware.
- Distinguish clearly between worksets and beads.
- Avoid beads for distant speculative work unless they unblock the thin slice.
- State completion evidence for the first executable set.
```

## Optional Tightening Line

If the source material is very broad, add:

```text
If the design is too broad to elaborate fully in one pass, first propose a phased decomposition and only fully elaborate phase 1.
```

## Optional Bead-Quality Line

If the agent tends to produce weak task phrasing, add:

```text
For each candidate bead, phrase it as a reviewable engineering outcome rather than a generic activity.
```

## What a Good Result Looks Like

A good response to this prompt should:
- make the thin slice obvious,
- reduce the design into a small number of worksets,
- surface real dependencies,
- and make the first bead set feel natural rather than arbitrary.

If the result is a flat list of dozens of tasks, it is not ready yet.
