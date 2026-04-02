# Workset: Immediate Window And Live-Session REPL Execution

Date: 2026-04-02
Owner: Codex
Status: in-progress

## Near-Term Priority

This lane now has higher delivery priority than the debugger lane and than the current VS Code, wrapper/native-hosting, web/wasm, and XLL execution lanes.

The intended near-term order is:
1. non-debug Immediate Window / CLI REPL contract and core,
2. CLI shell and validation,
3. later debugger-context Immediate Window layering,
4. then debugger, VS Code, wrapper, web/wasm, and XLL follow-on lanes as separate consumers or siblings.

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
6. The non-debug Immediate Window lane is the current preferred interactive-host priority ahead of debugger and alternate-editor follow-on work.

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
5. add immediate-window-focused unit and behavior coverage
6. add transcript and real-example integration scenarios
7. document OxIde integration shape
8. define debugger-context extension path against the debugger workset

## Validation Strategy

Immediate Window delivery is not complete on evaluator shape or CLI shell alone.
This lane must carry strong validation in three layers:

### 1. Unit and behavior coverage

Required:
- expression evaluation
- statement execution
- `? expr` / `Print`-style result projection
- assignment and persistent session-state mutation
- diagnostics and error-reset behavior
- explicit reset and reload semantics

These tests should target the evaluator core directly and aim for high semantic-path coverage.

### 2. Real-example integration scenarios

Required:
- repeated immediate commands against one live session
- interaction with procedure-created and module-level state
- realistic CLI REPL transcripts over example projects
- later, paused debug-context evaluation over the same evaluator core

These scenarios should use real OxVba examples rather than only isolated evaluator stubs.

### 3. Transcript/golden behavior tests

Required:
- deterministic REPL transcripts
- deterministic typed-value output snapshots
- reset/reload transcript coverage

These provide regression protection for both CLI and future OxIde Immediate Window presentation.

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
- strong validation exists across unit, integration, and transcript lanes,
- and the debugger-context extension path is defined rather than implied.
