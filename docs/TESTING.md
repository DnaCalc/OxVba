# TESTING.md

## Local lanes
- Fast lane: `./scripts/meta-check.ps1 -Fast`
- Full lane: `./scripts/meta-check.ps1`
- Matrix lane: `./scripts/meta-check.ps1 -Fast -Matrix`
- Formal lane (non-blocking): `./scripts/meta-check.ps1 -Fast -Formal`
- Formal lane (strict Kani via WSL): `./scripts/run-formal-kani-wsl.ps1 -ProfileScope mvp-formal-foundation-v3`
- Combined ladder lane: `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal`
- COM conformance (required registrationless lane): `./scripts/run-com-conformance.ps1`
- COM conformance (registrationless + registered external lane): `./scripts/run-com-conformance.ps1 -IncludeRegisteredLane -RegisteredProgIds "Scripting.Dictionary"`
- COM early-binding conformance (`E0..E6`): `./scripts/run-com-early-conformance.ps1 -IncludeFormalLane`
- COM early-binding perf baseline: `./scripts/run-com-early-perf.ps1 -Iterations 3`
- Integration fixture lint: `./scripts/lint-integration-fixtures.ps1`

## Async long-running formal steps
For long Kani runs in profile execution, use:
- Start: `./scripts/run-formal-kani-async.ps1 -Action Start -Name v3-kani -ProfileScope mvp-formal-foundation-v3`
- Status: `./scripts/run-formal-kani-async.ps1 -Action Status -Name v3-kani`
- Tail logs: `./scripts/run-formal-kani-async.ps1 -Action Tail -Name v3-kani`
- Wait for completion: `./scripts/run-formal-kani-async.ps1 -Action Wait -Name v3-kani`
- Stop: `./scripts/run-formal-kani-async.ps1 -Action Stop -Name v3-kani`
- Probe toolchain before starting/restarting: `./scripts/run-formal-kani-async.ps1 -Action Probe -Name v3-kani`
- Reconcile stale state/watchers: `./scripts/run-formal-kani-async.ps1 -Action Reconcile -Name v3-kani`
- Start watcher (10-minute liveness poll): `./scripts/run-formal-kani-async.ps1 -Action WatchStart -Name v3-kani -WatchPollSeconds 600`
- Stop watcher: `./scripts/run-formal-kani-async.ps1 -Action WatchStop -Name v3-kani`

Deferred formal gate policy:
- For profiles that declare deferred gates, async Kani start + DG register update is required in-cycle.
- Completion can be folded back in a later reconciliation profile if conformance/matrix gates are green.
- DG register path: `docs/evidence/formal/DEFERRED_GATES.md`

## Current coverage
- Syntax: lexer/parser smoke and error tests.
- Runtime: Variant payload, coercion, arithmetic unit tests.
- IR: lowering consistency tests.
- Compiler/Host: compile+execute smoke + control-flow compilation tests.
- VM: bytecode execution tests for arithmetic, branch/loop execution, and jump validation.
- Conformance: VM/JIT-toggle profile corpus with slot snapshots (including relational/boolean branches).
- Formal: profile-scoped non-blocking obligations via `scripts/run-formal.ps1`.
- COM client E2E:
  - registrationless controlled lane (`com_client_end_to_end`) is always runnable in Windows host-backed mode,
  - registered external lane (`com_client_registered_lane`) uses ignored tests and runs only via explicit script/`--ignored` invocation.

## COM Lane Policy
- Registered COM tests are `#[ignore]` by default to avoid accidental nondeterministic CI/local failures.
- Registered COM lanes must run single-threaded (`--test-threads=1`) due shared COM apartment + global process state sensitivity.
- Prefer engine/policy COM selector overrides for deterministic tests (`Engine::set_com_prog_id_override`) over environment-only mapping.
- COM lane artifacts are written under `docs/evidence/conformance/com/` by:
  - `scripts/run-com-registrationless.ps1`,
  - `scripts/run-com-registered.ps1`,
  - `scripts/run-com-conformance.ps1`.

## Next additions
- Property tests for parse/print and coercion matrices.
- Broader Kani lane coverage once tooling is available in CI/local environments.
- Office harness expansion beyond MVP profile scope.
