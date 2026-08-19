# CORE-7 JIT Module Extract

Date: 2026-08-19
Bead: `bd-59co.2.9.10`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

`oxvba-jit` is no longer a single 35k-line `lib.rs`. The public API is unchanged.
The split is mechanical and crate-internal:

| File | Role |
|---|---|
| `src/lib.rs` | crate docs, shared imports, public re-exports |
| `src/admission.rs` | whole-image/procedure admission and type predicates |
| `src/helpers.rs` | Cranelift symbol registration, imports, compiled-entry shims |
| `src/lowering.rs` | procedure lowering and compile-time call planning |
| `src/tests.rs` | former in-file unit tests |
| `src/runtime.rs` | engine, compiled image, frames, run-state helpers |
| `src/consts.rs` | constants and entry ABI |
| `src/error.rs` | `JitError`, `JitOutcome`, `JitFinalErr` |

No typed-primary-entry, session, COM, Declare, or lowering-plan rewrite landed.

## Commands

- `cargo test -p oxvba-jit -- --nocapture` — 168 passed
- `cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture` — 7 passed
- `cargo clippy -p oxvba-jit --all-targets -- -D warnings` — clean

## Residual

Remaining CORE-7 architecture stays with `bd-59co.2.9.9`. Windows COM, Declare
execution, pointers, sessions/cache and packaging stay outside this leaf.
