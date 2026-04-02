# Workset: Wrapper Build Target And Native Hosting Execution

Date: 2026-04-02
Owner: Codex
Status: in-progress

## Near-Term Priority Position

This packaging/native-hosting lane remains planned follow-on work and is lower priority than the current Immediate Window / live-session REPL lane.

## Purpose

Refresh and continue the older wrapper-builder planning into a current execution lane for:
- wrapper executables,
- wrapper dynamic libraries,
- build-target separation,
- and hostable packaged OxVba artifacts built on top of `.oxb`.

## Why This Exists

OxVba already has:
- canonical project loading,
- `.oxb` as the stable compiled artifact,
- `oxvba-run` as the bundle launcher,
- native export metadata in the project model.

What is still largely unstarted is the packaging/hosting layer above that:
- standalone wrapper executables,
- wrapper DLL/shared-library outputs,
- robust build-target separation from semantic `OutputType`,
- and the delivery path to COM/XLL packaging.

## Governing Policy

1. `.oxb` remains the canonical compiled semantic artifact.
2. Wrapper outputs are packaging layers over `.oxb`, not a second compiler.
3. `OutputType` chooses semantic project shape; `BuildTarget` chooses emitted packaging shape.
4. The first wrapper lanes should be deterministic and inspectable, not magical.

## Required Outcomes

1. A clear `BuildTarget` model exists beside `OutputType`.
2. OxVba can emit at least:
   - wrapper executable
   - wrapper DLL/shared library
3. Native export metadata is consumed by the wrapper builder rather than reinterpreted by hosts.
4. Wrapper outputs are testable and evidence-backed.

## Execution Slices

1. define and land `BuildTarget` semantics
2. implement wrapper EXE generation
3. implement wrapper DLL/shared-library generation
4. validate launch/config/reference behavior under wrapper outputs
5. establish the handoff boundary to COM server and XLL lanes

Current execution state:
- workset is now the active execution owner for wrapper/native-hosting work
- the explicit `BuildTarget` and wrapper-boundary model is now landed in the project system and `.basproj` spec
- wrapper EXE/DLL packaging remains the next real delivery lane on top of that substrate

## Relationship To Prior Work

This workset is the current execution continuation of:
- `WORKSET_2026-03-23_WRAPPER_BUILDER_P7.md`

The older workset contains useful design detail, but this newer one is the current execution owner.

## Non-Goals

- direct native-image compiler claims
- full COM server closure in the same lane
- XLL closure in the same lane

## Exit Condition

This workset is complete only when:
- wrapper EXE and wrapper DLL/shared-library lanes are real,
- `BuildTarget` is explicit and honest,
- and COM/XLL lanes are unblocked by a usable wrapper substrate.
