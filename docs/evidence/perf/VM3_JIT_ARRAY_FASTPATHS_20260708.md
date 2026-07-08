# VM3/JIT Array Fast Paths - 2026-07-08

## Scope

Performance pass for VM3 and JIT array-heavy Linux-safe benchmark rows, preserving VM3/JIT parity and generic fallback behavior.

## Implemented Optimizations

### Shared Runtime

- Borrow SAFEARRAY bounds for `len()` instead of cloning a `Vec<SafeArrayBound>`.
- Cache SAFEARRAY length inside element loops and element read/write bounds checks.
- Add raw `i32` SAFEARRAY element read for `VT_I4`/`VT_INT` arrays.
- Add raw `i32` SAFEARRAY element write for `VT_I4`/`VT_INT` arrays.
- Make raw SAFEARRAY bounds+length query compute length from one bounds clone instead of cloning through `bounds()` plus `len()`.
- Make raw `i32` probes decline record/non-intrinsic/non-i32 arrays instead of forcing generic element-kind decoding.

### VM3

- Direct Long array reads in `ArrayGet` fast path.
- Direct Long array writes in `ArraySet` fast path, including ParamArray alias mirroring.
- One-dimensional array flat-index shortcut.
- Non-`Preserve` scalar `ReDim` zero-fills typed SAFEARRAY payloads instead of building default `Variant` vectors.
- Fixed-size scalar `Erase` zero-fills typed SAFEARRAY payloads.
- Direct Long reads/writes for project field array fast paths.
- Direct Long reads/writes for value-array fallback paths.

### JIT

- Direct Long array reads inside generic runtime array get helper.
- Direct Long array writes inside generic runtime array set helper, including ParamArray alias mirroring.
- One-dimensional runtime flat-index shortcut.
- Dedicated one-dimensional typed `Long()` array get lowering.
- Dedicated one-dimensional typed `Long()` array set lowering.
- Direct Long reads/writes for project field/value array helpers.
- Fixed-size scalar `Erase` zero-fills typed SAFEARRAY payloads.
- `ReDim` scalar zero-fill predicate is shared with erase.
- Specialized 1D helpers avoid subscript stack-slice setup on hot typed Long rows.
- Specialized 1D set helper avoids generic `JitVariantOperandDesc` construction for i32-lowerable assignments.

## Correctness Checks

- `cargo fmt --check`
- `cargo test -p oxvba-runtime safe_array_ -- --format terse`
- `cargo test -p oxvba-differential --test record_array_access_vm3 -- --format terse`
- `cargo test -p oxvba-differential --test jit_linux_safe_generated generated_loop_array_and_string_cases_match_vm3 -- --format terse`
- `cargo test -p oxvba-differential --test jit_linux_safe_generated benchmark_udt_and_collection_fixtures_match_vm3 -- --format terse`
- `cargo test -p oxvba-differential --test jit_linux_safe_generated -- --format terse`
- `cargo test -p oxvba-differential --test jit_linux_safe_scope -- --format terse`
- `cargo test -p oxvba-differential --test jit_project_objects -- --format terse`
- `cargo test -p oxvba-differential --test jit_udt_class_aggregates -- --format terse`
- `cargo test -p oxvba-jit -- --format terse`

All passed. Existing unrelated warnings remain in runtime/com/hal/host crates.

## Benchmark Commands

Baseline was collected before edits with:

```sh
cargo bench -p oxvba-differential --bench jit_m4_baseline -- --quiet
```

After-change data was collected after edits with the same command.

## Runtime Median Comparison

Selected median rows from the same Criterion benchmark harness:

| row | before | after | result |
| --- | ---: | ---: | ---: |
| VM3 `array_redim` | 25.464 ms | 16.622 ms | 1.53x faster |
| VM3 `array_set_long` | 4.6104 ms | 4.5781 ms | 1.01x faster |
| VM3 `array_get_long` | 9.7915 ms | 9.4886 ms | 1.03x faster |
| VM3 `array_for_each_variant` | 10.313 us | 9.2738 us | 1.11x faster |
| VM3 `udt_fields` | 10.264 ms | 9.9343 ms | 1.03x faster |
| VM3 `variant_box_unbox` | 15.679 ms | 15.181 ms | 1.03x faster |
| VM3 `call_overhead` | 14.871 ms | 13.941 ms | 1.07x faster |
| JIT `array_loop` | 342.55 us | 278.63 us | 1.23x faster |
| JIT `array_redim` | 10.896 ms | 6.9366 ms | 1.57x faster |
| JIT `array_set_long` | 1.2826 ms | 1.0572 ms | 1.21x faster |
| JIT `array_get_long` | 2.4547 ms | 2.1776 ms | 1.13x faster |
| JIT `array_for_each_variant` | 6.3577 us | 5.9424 us | 1.07x faster |
| JIT `referenced_class_aggregates` | 7.5285 ms | 7.3493 ms | 1.02x faster |
| JIT `bundle_only_referenced_class_aggregates` | 8.4264 ms | 7.8780 ms | 1.07x faster |
| JIT `variant_box_unbox` | 5.2311 ms | 5.0839 ms | 1.03x faster |
| JIT `collection_ops` | 3.0940 ms | 3.0149 ms | 1.03x faster |

Neutral/noisy rows in this run included VM3 `array_loop`, VM3 object aggregate rows, JIT `call_overhead`, and JIT `error_loop`. The largest confirmed wins are scalar `ReDim`/fixed scalar allocation paths and typed one-dimensional Long array JIT access.

## Compile Cost Notes

Selected JIT compile medians:

| row | before | after | note |
| --- | ---: | ---: | --- |
| `array_loop` | 4.6331 ms | 5.0051 ms | slower; extra specialized branch/helper imports |
| `array_redim` | 2.1323 ms | 2.1282 ms | flat |
| `array_set_long` | 2.3777 ms | 2.4387 ms | slightly slower |
| `array_get_long` | 3.3191 ms | 3.2026 ms | slightly faster |
| `class_field_aggregates` | 7.1283 ms | 6.5178 ms | faster in this sample |

The runtime speedups outweigh the small compile-cost movement for long-running array loops.
