# JIT UDT/Class Aggregate Parity And Array ReDim Perf - 2026-07-08

Status: implemented-subset

Scope: Linux-safe VM3/JIT parity for UDT and project-class aggregate behavior,
source-backed and bounded bundle-only OxVBA project references, plus focused
array performance retesting. This evidence excludes live Windows COM and Office
automation.

## Implemented JIT Coverage

- `RecordLSet`, `RecordArrayGet`, `RecordArraySet`, `FieldArrayGet`, and
  `FieldArraySet` now lower through JIT runtime helpers instead of explicit
  unsupported diagnostics.
- Compound `ReDim` and `Erase` targets now accept metadata-bearing `Variant`
  temps produced by OxIR materialize-and-write-back lowering for UDT/class
  fields.
- Source-backed referenced-project class aggregate rows run under JIT with VM3
  parity.
- Bundle-only referenced-project rows bind the entry project against synthesized
  compiled export surfaces, then run VM3/JIT over the loaded OxImage closure.
  Covered callable exports are public module procedures and public creatable
  classes with method/property calls over UDT/object aggregate state.
- Non-`Preserve` scalar `ReDim` now builds zeroed typed SAFEARRAY payloads
  directly, avoiding default `Variant` vector construction for scalar arrays.

## Correctness Coverage

`crates/oxvba-differential/tests/jit_udt_class_aggregates.rs` covers:

- UDT fixed-array fields with explicit, `Option Base`, negative, and
  multidimensional bounds.
- Deep nested UDTs, nested fixed arrays, dynamic arrays of UDTs, and UDT
  elements containing fixed-array fields.
- UDT `LSet` over fixed strings, integers, and fixed byte-array storage.
- UDT fixed-array field `Erase`, class dynamic-array `ReDim Preserve`, and
  class dynamic-array `Erase` error behavior.
- Class fixed-array fields through `With`, class dynamic object arrays, class
  dynamic UDT arrays with nested fixed arrays, and nested UDT records in class
  fields.
- Source-backed referenced-project class aggregate fields and object arrays.
- Bundle-only referenced class aggregate fields/object arrays and a
  bundle-only referenced module function in
  `crates/oxvba-differential/tests/jit_bundle_only_references.rs`.

Linux-safe generated/scope tests now include `udt_nested_arrays`,
`class_field_aggregates`, `referenced_class_aggregates`, and
`bundle_only_referenced_class_aggregates` benchmark fixtures.

## Validation

```text
cargo fmt --check
cargo test -p oxvba-runtime safe_array_zeroed_typed_scalar_payload_is_materialized_and_writable -- --nocapture
cargo test -p oxvba-differential --test jit_udt_class_aggregates -- --nocapture
cargo test -p oxvba-differential --test jit_bundle_only_references
cargo test -p oxvba-differential --test jit_linux_safe_generated -- --nocapture
cargo test -p oxvba-differential --test jit_linux_safe_scope -- --nocapture
cargo test -p oxvba-differential --test jit_project_objects -- --nocapture
cargo test -p oxvba-differential --test record_array_access_vm3 -- --nocapture
cargo test -p oxvba-jit -- --format terse
cargo test -p oxvba-differential --benches --no-run
cargo bench -p oxvba-differential --bench jit_m4_baseline bundle_only_referenced_class_aggregates -- --quiet
```

All checks passed. Existing unrelated warning noise remains in runtime/com/hal/host crates.

## Targeted Perf Evidence

Targeted Criterion filters were run with precompiled OxIR execution groups.

| fixture | VM3 median | JIT median | VM3/JIT ratio | JIT compile median | source-to-OxIR median |
|---|---:|---:|---:|---:|---:|
| `array_loop` | 1.2406 ms | 342.64 us | 3.62x | 4.3824 ms | 243.25 us |
| `array_redim` | 25.760 ms | 10.918 ms | 2.36x | 2.0539 ms | 119.22 us |
| `array_set_long` | 4.8116 ms | 1.3446 ms | 3.58x | 2.5205 ms | 135.15 us |
| `array_get_long` | 9.3285 ms | 2.4566 ms | 3.80x | 3.0820 ms | 150.68 us |
| `array_for_each_variant` | 12.348 us | 6.6990 us | 1.84x | 1.5847 ms | 125.04 us |
| `udt_nested_arrays` | 23.897 ms | 23.432 ms | 1.02x | 6.1222 ms | 366.88 us |
| `class_field_aggregates` | 11.617 ms | 9.5590 ms | 1.22x | 6.3207 ms | 504.32 us |
| `referenced_class_aggregates` | 10.349 ms | 7.5314 ms | 1.37x | 6.4586 ms | 714.91 us |
| `bundle_only_referenced_class_aggregates` | 9.4993 ms | 7.0992 ms | 1.34x | 6.1744 ms | 719.24 us |

The explicit array perf target is met: `array_loop`, `array_get_long`,
`array_set_long`, and `array_redim` are all more than 2x faster than VM3 in the
targeted runs after the zeroed scalar `ReDim` fast path. `For Each` over a small
Variant array and aggregate UDT/class fixtures remain helper-bound but execute
without fallback and are now covered by parity tests before being counted.

## Residual Scope

- Bundle-only project references are claimed only for compiled CoreProgram/OxIR
  artifacts whose exported surface contains public module procedures and public
  class methods/properties. Compiled const/enum publication, optional default
  values, cross-bundle public field exports, exact `VB_Creatable` preservation,
  and bundle-only project-reference manifest loading remain outside this slice.
- Live Windows COM and Office/VBA oracle execution remain outside this Linux
  evidence slice.
