# V0.2 Non-Primary Host Validation Rollout

Date: 2026-04-27

Bead: `bd-bqm8.9.1`

Parent: `bd-bqm8.9`

## Scope

This rollout splits `bd-bqm8.9` into executable delivery beads for non-primary
host validation breadth.

## Child Beads

- `bd-bqm8.9.1`: roll out the child beads and current product-truth target.
- `bd-bqm8.9.2`: add active Linux, macOS, and wasm validation jobs to CI.
- `bd-bqm8.9.3`: publish the V0.2 non-primary host product-truth matrix.
- `bd-bqm8.9.4`: run the final checklist and close `bd-bqm8.9` only if the
  CI/job matrix, docs, governance, and residual boundaries match.

## Product-Truth Target

The V0.2 non-primary host claim is validation breadth, not parity with
Windows-primary COM behavior.

In scope:

- Linux ready checks.
- macOS ready checks.
- wasm32/WASI HAL conformance build/run checks.
- explicit documentation that COM/Office automation remains Windows-primary and
  non-primary hosts use portable deterministic rejection/projection behavior.

Out of scope:

- non-Windows Office COM parity,
- browser UI packaging parity,
- macOS-specific Office automation,
- Linux native desktop integration beyond current CLI/HAL/runtime checks.

## Checks Run

- `Select-String -Path .beads/issues.jsonl -Pattern 'bd-bqm8\\.9'`
- `Select-String -Path docs/worksets/WORKSET_2026-04-06_V0_2_SCOPE_ROUNDOUT_EXECUTION.md -Pattern 'v02\\.9|bd-bqm8\\.9|non-primary' -Context 3,8`

## Result

`bd-bqm8.9.1` is complete as a rollout bead. The parent lane remains
in-progress pending executable CI coverage, product-truth docs, and final
checklist evidence.
