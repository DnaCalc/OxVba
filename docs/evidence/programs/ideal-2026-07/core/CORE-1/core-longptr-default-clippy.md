# CoreLongPtrWidth derived-default cleanup

Date: 2026-07-11

Bead: `bd-59co.2.2.19`

Baseline: `c0ff1de7a61dce2df6c83eff730582f2fd6f969b`

Status: focused strict-Clippy repair complete; no broader `LongPtr` compatibility
claim.

## Scope and contract trace

`oxvba_bundle::coreir::CoreLongPtrWidth` now derives `Default`, with
`CoreLongPtrWidth::Bits64` marked as the explicit default variant. This removes
the manual implementation rejected by `clippy::derivable_impls` without a lint
suppression and preserves the existing public default.

The default continues to serve hand-built Core IR and test construction. Real
binder output still supplies the manifest's conditional-compilation target.
That separation is consistent with `PROFILE-WIN-001`, `COMP-BIND-001`, and
`IR-CORE-001`: the active Windows profile is x64, while explicit target facts
remain part of Core IR rather than being inferred by a backend.

## Acceptance evidence

Run in the isolated `codex/bd-59co-2-2-19-longptr-clippy` worktree on Windows
x64:

| Command | Result |
|---|---|
| `cargo fmt -p oxvba-bundle -- --check` | pass |
| `cargo test -p oxvba-bundle core_long_ptr_width_default -- --nocapture` | pass; 1 passed, 0 failed, 8 filtered out |
| `cargo clippy -p oxvba-bundle --all-targets -- -D warnings` | pass; zero warnings |
| `cargo test -p oxvba-oxir longptr -- --nocapture` | pass; 1 passed, 0 failed, 80 filtered out |
| `cargo test -p oxvba-bundle` | pass; 9 unit tests and doc tests, 0 failed |

The focused regression asserts directly that
`CoreLongPtrWidth::default() == CoreLongPtrWidth::Bits64`. The neighboring OxIR
test continues to prove that an explicit `Bits32` target lowers `LongPtr` to
`Long` and an explicit `Bits64` target lowers it to `LongLong`.

## Observable axes

- Result: the public default remains `Bits64`; the focused regression and all
  `oxvba-bundle` tests pass.
- Full Err: this type-level construction change does not create, modify, or
  transport VBA `Err`, `Erl`, or `LastDllError` state.
- Side effects: deriving `Default` performs the same pure enum construction as
  the removed manual implementation; there are no host or runtime side effects.
- Lifecycle/order: the enum is `Copy` and owns no resources, so initialization,
  clone, move, and destruction order are unchanged.
- Transport: the enum variants and explicit `Bits32`/`Bits64` lowering paths are
  unchanged. No serialized format, ABI, call transport, or target-selection
  path is modified.
- Balance: the enum allocates and releases no BSTR, object, SAFEARRAY, record,
  descriptor, or other tracked carrier; carrier balances are therefore not
  applicable to this repair.

## Residual boundary

This bead removes one strict-Clippy finding and locks the existing x64-oriented
construction default. It does not claim complete VBA `LongPtr`, conditional
compilation, pointer-operation, Declare, VM3/JIT, or Windows interop parity.
Those broader capabilities remain owned by their existing Core and Windows
delivery rows and beads. No additional residual was exposed by this change or
its focused neighboring tests.
