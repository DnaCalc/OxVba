# JIT M4-3 Cranelift Skeleton Evidence

Date: 2026-07-03

Scope: `bd-h4oh.4` / `M4-3` first compiled Cranelift execution slice.

## Landed

- Pinned `cranelift-codegen`, `cranelift-frontend`, `cranelift-jit`, `cranelift-module`, and `cranelift-native` to `0.133.1`; only `oxvba-jit` consumes the Cranelift crates.
- Replaced the `oxvba-jit` stub with a real whole-image compiler for the single-`OxProgram` M4-3 slice.
- Added a `JitRun` frame carrier and compiled entry ABI `unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32`.
- Registered runtime helper symbols through `JITBuilder::symbol`, including JIT slot load/store helpers and checked `Long` arithmetic shims from `oxvba-rt-abi`.
- Lowered the straight-line source-level slice: `StmtBoundary`, `SetLineNumber`, `Assign`, binder-inserted `Numeric(Long)` `Coerce`, checked `Long` add/sub/mul via shims, `Jump`, `Return`, `Halt`, and no-handler `FaultDispatch` propagation.
- Added whole-program validation and clean `Unsupported` declines for classes, imports, native/COM calls, non-`Long` places, parameters, function return locals, branches/loops, and other unlowered instructions.
- Added the JIT run driver over the shared M4-2 protocol: initialize globals, reset pending terminations, run entry, drain pending terminations after entry, surface values plus final `Err` state, and never fall back to VM3.
- Wired `oxvba-host` so `HostConfig::jit()` compiles and runs manifests through the JIT backend; closure snapshots preserve the existing globals-only host contract.
- Wired `oxvba-differential::Executor::Jit` to the host JIT snapshot path instead of the old blanket not-implemented decline.
- Added source-level differential coverage for VM3/JIT equality on straight-line `Long` arithmetic, overflow error-number parity, clean loop decline, and the `crates/oxvba-differential/jit_scope.snap` ratchet.

## Compatibility Boundaries

- M4-3 is deliberately narrow: straight-line `Long` code only. Unsupported constructs are whole-program declines, not mixed execution.
- The JIT stores and snapshots retained `Variant` carriers, matching the differential comparison surface. The compiled machine values are only transient helper-call operands.
- `DrainTerminations` in compiled code is a no-op for this scalar-only slice; the driver still performs the shared post-entry drain. Object/lifecycle statement-boundary drain timing remains M4-8 scope.
- The JIT module must stay owned by `CompiledImage` for finalized function-pointer validity.

## Checks

- `cargo fmt`
- `cargo test -p oxvba-jit -- --nocapture`
- `cargo test -p oxvba-host jit_unsupported_phase_diagnostic_exposes_stable_code -- --nocapture`
- `cargo test -p oxvba-differential jit_ -- --nocapture`
- `cargo test -p oxvba-host --lib -- --nocapture`
- `cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture`
- `cargo test --workspace --no-run`
- `git diff --check`

## Cargo Tree Invariants

- VM3 remains Cranelift-free and does not depend on `oxvba-jit`.
- `oxvba-jit` depends on Cranelift and `oxvba-rt-abi`, but not on `oxvba-vm3`.
- Cranelift is confined to the JIT crate consumption path.

## Non-Blocking Formal Lane

`pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/run-formal.ps1 -Quiet -NoArtifacts`
and the `powershell` equivalent both failed with `command not found`. The formal lane remains
non-blocking under the current ladder policy and is tracked in
`docs/evidence/formal/EXTENDED_TODO.md` as `FTODO-M4-3-001`.

## Fresh-Eyes Review

- Rechecked run-protocol sequencing and found one issue before closure: global-initializer statuses were invoked but not interpreted, and pending terminations were reset after initializer execution. Fixed by resetting before initializers and mapping initializer `ST_FAULT`/`ST_HALT` the same way as entry execution.
- Added `jit_initializer_fault_status_is_observed` to pin initializer fault propagation through the same `Err` surface as entry faults.
- Rechecked that unsupported JIT shapes remain whole-program declines with no VM fallback in host or differential paths.
- Rechecked that JIT closure snapshots preserve the existing host globals-only contract by truncating completed JIT manifest snapshots for the closure entry point.
- Rechecked cargo-tree invariants after adding Cranelift: VM3 has no JIT/Cranelift path, and JIT has no VM3 path.
