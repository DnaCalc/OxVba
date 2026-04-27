# V0.2 Native Compilation Path Rollout

Date: 2026-04-27

Bead: `bd-bqm8.10.1`

Parent: `bd-bqm8.10`

## Scope

This rollout splits the native compilation path epic into executable child
beads. The parent remains in-progress until OxVba has a documented path
selection, ABI and packaging obligations, an executable validation scaffold, and
a final checklist.

## Child Beads

- `bd-bqm8.10.1`: roll out the child beads and execution boundaries.
- `bd-bqm8.10.2`: publish the native compilation path decision.
- `bd-bqm8.10.3`: publish ABI, packaging, and deployment obligations.
- `bd-bqm8.10.4`: add the first executable native validation scaffold.
- `bd-bqm8.10.5`: run the final native compilation path checklist.

## Existing Substrate

- `crates/oxvba-jit` already owns the Cranelift-backed execution substrate.
- `crates/oxvba-build` already emits wrapper-oriented DLL, EXE, COM server, XLL,
  manifest, IDL, and registration scaffolds.
- `scripts/run-bench.ps1`, `scripts/run-v02-performance.ps1`, and host tests
  provide validation surfaces for staged execution and packaging claims.

## Boundary

This rollout does not select or close the native compilation direction. It
creates the child beads required to make the selection and leave behind an
executable scaffold.

## Result

`bd-bqm8.10.1` is complete as a support rollout bead. Parent `bd-bqm8.10`
remains in-progress; the next ready delivery bead is `bd-bqm8.10.2`.
