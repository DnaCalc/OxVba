# Workset: Immediate Window And Live-Session REPL Execution

Date: 2026-04-02
Owner: Codex
Status: planned

## Purpose

Define and deliver an OxVba-side Immediate Window / REPL capability that evaluates VBA statements and expressions against a live compiled runtime session instead of forcing a full compile/run/debug cycle for each interaction.

## Why This Exists

OxVba already has the core runtime substrate needed for this lane:
- `Engine::compile_and_prepare_session(...)`
- `Engine::invoke_procedure(...)`
- persistent `ProjectRuntimeSession`
- VM state that survives multiple procedure invocations

That makes an Immediate Window viable, but only if it is built as a live-session evaluator rather than a disguised one-shot runner.

## Governing Policy

1. The Immediate Window is a semantic OxVba feature, not a CLI-only trick.
2. It must execute against a live `ProjectRuntimeSession`.
3. A debug-time Immediate Window must share the same evaluator core as the non-debug REPL, then add break-context/frame access.
4. OxIde and CLI should consume one shared OxVba-side immediate evaluator service.
5. Do not implement this as “recompile and run a new script for every line.”

## Product Shapes

### CLI shape

The CLI-facing form is a REPL-style shell, for example:
- `oxvba repl <project>`
- or `oxvba immediate <project>`

That shell should:
- load/compile once,
- create one live runtime session,
- accept interactive lines,
- evaluate them against the session,
- print result or diagnostics immediately.

### OxIde shape

The OxIde-facing form is an Immediate Window pane over the same evaluator service.

### Debug shape

Later, during debug break state, the same evaluator should support:
- expression evaluation in the current frame,
- statement execution under bounded debug rules,
- watch/evaluate support.

## Required Outcomes

1. A typed OxVba-side immediate evaluator contract exists.
2. Non-debug live-session immediate evaluation works against a compiled project/session.
3. CLI exposes the evaluator through a bounded REPL surface.
4. The path to OxIde Immediate Window integration is explicit.
5. The later debugger-context Immediate Window path is explicit and layered on the debugger workset.

## Main Design Questions

1. What syntax is accepted interactively?
   - expressions only,
   - statement lines,
   - `? expr` shorthand,
   - `Print expr`,
   - assignments,
   - multi-line blocks or single-line only in v1.
2. What scope does non-debug evaluation run in?
   - synthetic immediate module,
   - project-global session context,
   - explicit target module,
   - or a bounded pseudo-procedure wrapper.
3. How are results surfaced?
   - printed values,
   - typed value display,
   - diagnostics with source spans,
   - host output stream hooks.
4. What reset semantics exist?
   - explicit `reset`,
   - project reload,
   - module/source overlay refresh,
   - host policy/profile changes.
5. What is forbidden in v1?
   - multi-line procedure definitions,
   - structural project edits,
   - unsupported break-context mutations,
   - host-destructive operations under strict policies.

## Execution Slices

1. publish the workset and lock the live-session evaluator policy
2. define the typed immediate evaluator contract
3. implement non-debug live-session evaluator core
4. add CLI REPL/immediate shell
5. document OxIde integration shape
6. define debugger-context extension path against the debugger workset

## Relationships

- Runtime substrate provenance:
  - `WORKSET_2026-03-23_ENGINE_INVOKE_PROCEDURE_P5.md`
- Debugger follow-on:
  - `WORKSET_2026-04-02_OXVBA_DEBUGGING_SERVICE_AND_HOST_INTEGRATION.md`
- OxIde host lane:
  - `WORKSET_2026-04-01_OXIDE_HOST_SURFACE_AND_VSCODE_ALTERNATE_EDITOR_EXECUTION.md`

## Non-Goals

- native debugger integration
- re-running the full program for each line
- Edit and Continue
- full VBA IDE parity in the first slice

## Exit Condition

This workset is complete only when:
- OxVba has a real live-session immediate evaluator,
- CLI exposes it as a REPL/immediate shell,
- the OxIde Immediate Window consumption path is explicit,
- and the debugger-context extension path is defined rather than implied.
