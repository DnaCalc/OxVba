# Debugger Host Harness V1

This note records the bounded direct-host debugger harness now present in `oxvba-host`.

Harness anchor:
- `crates/oxvba-host/tests/debug_session_host_harness.rs`

Purpose:
- prove that an OxIde-class direct host can drive `DebugSession`
- prove that stop reasons, frame stacks, locals, and bounded paused evaluation are consumable without LSP or native debuggers

Current bounded scenarios:
- start on entry pause
- step into a callee
- inspect stacked frames
- inspect current-frame locals
- evaluate a current-frame identifier while paused
- step out back to the caller
- prove an empty `Sub Main` session completes without leaving stale pause state

Transcript-style expectation covered by the harness:
- `start:entry:module1:main:2`
- `step:2:main:foo`
- `eval:y=4`
- `local:z:local`
- `step_out:completed`

This is intentionally direct-host evidence, not a DAP or LSP transcript.
It exists to show that the semantic debugger core is already usable from an OxIde-style consumer.
