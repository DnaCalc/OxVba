# Beads Utilities Cheat Sheet

This note summarizes the main utilities used in the beads-based working method.

It focuses on what each utility is for, not on advanced orchestration.

## Core Utilities

### `br`

`br` is the primary beads tool.

Use it to:
- inspect ready work,
- create beads,
- update bead status,
- close completed beads,
- and define dependencies.

Typical commands:

```bash
br ready
br show <id>
br create "Title" -t task -p 2
br update <id> --status in_progress
br close <id> --reason "Completed"
br dep add <parent-or-dependent> <other-id>
```

Use `br` whenever you are working with the bead graph itself.

### `bv`

`bv` is the bead-graph analysis tool.

Use it to:
- inspect dependency structure,
- find critical paths,
- identify cycles,
- and get graph-aware triage rather than reading raw issue data by hand.

`bv` is not the first tool you need for the basic loop, but it becomes useful once the graph grows.

### `git`

`git` is part of the bead method because bead state lives in the repo.

Use it to:
- review bead-state changes,
- commit `.beads/` with code changes,
- and preserve the history of planning and execution together.

The important idea is:
- bead state and code state should travel together

## Storage and State

### `.beads/`

This is the authoritative local bead state.

It contains the tracked issue graph and related metadata.

Important rule:
- do not edit `.beads` files directly
- use `br`

## Minimal Working Loop

The minimal bead loop uses just:
- `br`
- `git`
- your shell

Example:

```bash
br ready
br show <id>
br update <id> --status in_progress
# do the work
br close <id> --reason "Completed"
git status
```

## Optional Supporting Utilities

These are useful, but they are not the core bead method.

### `ntm`

Use `ntm` when you want agents to work beads.

It helps with:
- session management,
- agent panes,
- and assignment workflows.

But it is not required for the bead method itself.

### Agent Mail

Use Agent Mail when multiple agents need coordination.

It helps with:
- reservations,
- identities,
- and coordination state.

It is not the bead tracker.

### `cass` / `cm`

Use these for memory and context recovery.

They help agents recover past work, but they are not part of core bead creation or bead execution.

## Practical Summary

If you want the shortest correct summary:

- `br` manages beads
- `bv` analyzes the bead graph
- `git` versions the bead state with the code

Everything else is supporting infrastructure around execution, coordination, or memory.
