# Workset: XLL Add-In Realization Execution

Date: 2026-04-02
Owner: Codex
Status: in-progress

## Near-Term Priority Position

This XLL lane remains downstream of both wrapper/native-hosting substrate work and the current higher-priority Immediate Window / live-session REPL lane.

## Purpose

Execute the first honest OxVba XLL/add-in delivery lane on top of the wrapper/native-hosting substrate.

## Why This Exists

XLL support has been envisioned for a while, but it is still mostly design-only.
The project needs a current execution owner that reflects the actual dependency ladder:
- wrapper build targets first,
- then Excel/XLL registration and marshaling work.

## Governing Policy

1. XLL is a specialized packaging/integration lane over OxVba semantics.
2. Do not overclaim full Excel extensibility parity.
3. Keep XLL-specific marshaling and registration code separate from the core language/runtime.
4. Treat Excel/XLL hosting as Windows-primary unless and until a broader story is real.

## Required Outcomes

1. `OutputType=Addin` has a concrete delivery meaning.
2. XLL entry points and registration flows are generated from canonical project/native-export metadata.
3. The XLL lane is validated against real Excel-facing registration/invocation expectations.
4. The limits of the first XLL lane are explicit.

## Execution Slices

1. refine XLL target semantics on top of the wrapper substrate
2. implement XLL entry-point generation
3. implement XLOPER12 marshaling
4. wire metadata and registration surfaces
5. capture validation and evidence

Current execution state:
- wrapper/native-hosting substrate is now closed as a generated-source handoff
  boundary, so this XLL workset is active
- `bd-xll1.2` is complete: generated XLL source now carries a registration
  table derived from `NativeExportDescriptor` metadata and an explicit
  `Excel12v` / `xlfRegister` source path
- evidence:
  [XLL_REGISTRATION_SOURCE_GENERATION_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/XLL_REGISTRATION_SOURCE_GENERATION_2026-04-27.md)
- next delivery slice: bridge XLOPER12 invocation arguments/results to
  `RuntimeValue` procedure invocation without claiming Excel-loaded parity

## Relationship To Prior Work

This workset is the current execution continuation of:
- `WORKSET_2026-03-23_XLL_ADDIN_SUPPORT_P8.md`

## Dependencies

- wrapper build-target substrate
- native export mechanism
- host invocation surface

## Non-Goals

- all Excel add-in models
- macOS Excel parity
- full Office extensibility closure beyond the XLL lane

## Exit Condition

This workset is complete only when:
- XLL packaging is real,
- registration/invocation works for the intended bounded slice,
- and the public docs state the exact supported subset honestly.
