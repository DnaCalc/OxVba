# Wrapper Generation EXE

Date: 2026-05-24
Bead: `bd-hjys.10`

## Implementation summary

Added `crates/oxvba-build/src/reflection_exe.rs` with a generated command-line executable wrapper example:

- `generate_reflection_exe_wrapper(...)` produces an introspection EXE wrapper plan and generated Rust source skeleton.
- `ReflectionExeWrapper` is the in-process test harness for the same generated command contract.
- Commands supported:
  - `list`
  - `describe Module.Proc`
  - `call Module.Proc [typed args...]`

The wrapper loads bundle bytes through `VbaHost`, uses descriptor reflection for list/describe, parses positional CLI arguments from descriptor-declared VBA types, and invokes through `PreparedVbaProject::invoke_callable_typed`.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| Generated EXE supports list, describe Module.Proc, and call Module.Proc commands. | `reflection_exe::tests::generated_exe_lists_describes_and_calls_from_descriptors` exercises all three commands. |
| list/describe output comes from descriptors, not manually hard-coded wrapper lists. | `ReflectionExeWrapper` stores `ProjectReflection` from `VbaHost` bundle loading; `list()` and `describe()` iterate `reflection.procedures`. |
| call parses typed CLI arguments and invokes neutral typed runtime path. | `call()` converts descriptor types to `TypedValue` and calls `PreparedVbaProject::invoke_callable_typed`; test returns `7` from typed Long arguments. |
| Negative cases cover unknown procedure, arity mismatch, unsupported type, parser error, and runtime diagnostic propagation. | `generated_exe_reports_cli_negative_cases` covers unknown, arity, unsupported Variant parser, and parse errors. `generated_exe_propagates_runtime_diagnostics` covers runtime failure propagation. |
| Evidence artifact required. | This file: `docs/evidence/host_callable/WRAPPER_GENERATION_EXE.md`. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-build reflection_exe -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- The wrapper is descriptor-driven and uses bundle `ProjectReflection` instead of duplicated function lists.
- The executable profile is a wrapper-plan profile, not XLL or UDF substrate.
- Runtime invocation goes through neutral callable ID typed invocation.
