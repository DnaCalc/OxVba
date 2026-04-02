# OxVba Immediate Evaluator Contract V1

This spec defines the first bounded OxVba-side contract for Immediate Window and CLI REPL work.

The design target is a semantic live-session evaluator for OxVba code:
- backed by a persistent `ProjectRuntimeSession`,
- directly consumable from OxIde,
- exposed through a CLI REPL later,
- and reusable during debugger break mode after the non-debug lane is real.

## Current Runtime Starting Point

OxVba already has the core substrate needed for a real Immediate Window:
- persistent `ProjectRuntimeSession` in `oxvba-host`,
- `Engine::compile_and_prepare_session(...)`,
- `Engine::invoke_procedure(...)`,
- a live `Vm` whose state survives repeated procedure dispatch.

That means Immediate Window work should be built as an extension of the current live-session runtime model, not as a repeated compile-run wrapper.

## Governing Rules

1. Immediate evaluation truth is semantic OxVba runtime state.
2. The first lane must execute against a live `ProjectRuntimeSession`.
3. Direct hosts consume typed Rust APIs.
4. CLI and OxIde share one evaluator contract.
5. Debugger-context evaluation is later layering over the same core, not a second evaluator.
6. Re-running the whole project for every line is explicitly out of scope.

## Required Core Types

The first Immediate Window surface should converge on these typed concepts:
- `ImmediateSession`
- `ImmediateEvaluationRequest`
- `ImmediateInputKind`
- `ImmediateDisplayStyle`
- `ImmediateEvaluationResult`
- `ImmediateEvaluationOutput`
- `ImmediateValueProjection`
- `ImmediateResetKind`
- `ImmediateSessionError`

Names may evolve, but the shape should remain:
- one host-visible live session wrapper,
- one typed request model for expressions/statements/query shorthand,
- one typed output model for values, printed text, empty results, and reset events,
- explicit reset semantics and explicit session-targeting policy.

## First Contract Surface

`oxvba-host` now carries the bounded contract surface:
- `Engine::prepare_immediate_session(...)`
- `ImmediateSession`
- `ImmediateEvaluationRequest`
- `ImmediateEvaluationResult`
- related input/output/projection enums

In the current V1 slice:
- `ImmediateSession` owns the live `ProjectRuntimeSession`
- the host may set a default target module explicitly
- evaluation is bounded to existing project procedure invocation on that live session
- typed value projection and deterministic reset/reload are present
- arbitrary ad hoc expression compilation and multi-line statement evaluation are still future slices

This is intentional: the contract now exists as a real OxVba-side API, and the first executable evaluator is honest about what it can already do.

## Session Model

`ImmediateSession` should remain the single semantic owner for non-debug interactive evaluation.

It currently supports:
- default target module selection,
- snapshot access for tests/debugging,
- explicit reset/reload operations,
- bounded live evaluator entrypoints for existing procedures.

It should later support:
- default target module selection,
- snapshot access for tests/debugging,
- explicit reset/reload operations,
- live evaluator entrypoints,
- later paused-frame/debug-context extension.

Hosts should not own their own parallel runtime-session bookkeeping.

## Request Model

The bounded request model supports:
- `Auto`
- `Expression`
- `Statement`
- `Query`

In the current evaluator core, these route into:
- `?` shorthand for value-oriented invocation,
- `Call ...` or statement-mode invocation,
- explicit module-qualified or default-module procedure targets,
- literal arguments for strings, integers, booleans, and `Empty`.

This still leaves room for later:
- explicit `Print expr`
- assignment statements
- broader bounded single-line statements

Multi-line structural editing remains outside this contract.

## Output Model

Immediate output should remain typed rather than stringly.

The first output model supports:
- `Empty`
- `Value(ImmediateValueProjection)`
- `PrintedLine(String)`
- `Reset`

That shape is intended to compose into:
- CLI transcript output,
- OxIde Immediate Window panes,
- later paused-context debugger evaluation output.

## Relationship To Debugger

Immediate Window and debugger work should compose in this order:
1. non-debug live-session evaluator core,
2. CLI and OxIde consumption,
3. debugger pause/frame context added later,
4. same evaluator semantics reused for watch/evaluate paths.

The debugger lane should not invent a separate evaluation model.

## First Executable Ladder

1. publish this contract and bounded host API surface
2. implement non-debug evaluator behavior over `ImmediateSession`
3. add CLI REPL/immediate shell
4. add validation and transcript coverage
5. document OxIde consumption
6. layer debugger-context evaluation later

Current published guidance:
- `docs/OXIDE_IMMEDIATE_WINDOW_INTEGRATION_GUIDANCE.md`

## Exit Condition For V1 Contract

V1 contract is complete when:
- a real typed OxVba-side immediate-session API exists,
- the session is explicitly live-runtime backed,
- request/result/output shapes are no longer implicit,
- and the next evaluator implementation slice is directly ready rather than inferred.
