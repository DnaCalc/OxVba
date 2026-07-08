# JIT Linux-Safe Fuzz/Scope/Perf Ratchet Evidence

Date: 2026-07-08

Scope: Linux-safe VM3-vs-JIT differential fuzz/scope expansion and paired execution benchmarks. This evidence avoids live COM, Excel, Office automation, and Windows-only host behavior.

## Commands

```text
cargo test -p oxvba-differential --test jit_linux_safe_generated -- --nocapture
cargo test -p oxvba-differential --test jit_linux_safe_scope -- --nocapture
cargo test -p oxvba-jit -- --format terse
cargo test -p oxvba-differential --test jit_project_objects -- --nocapture
cargo test -p oxvba-differential --benches --no-run
cargo bench -p oxvba-differential --bench jit_m4_baseline -- --quiet
cargo bench -p oxvba-differential --bench jit_m4_baseline bundle_only_referenced_class_aggregates -- --quiet
```

## Correctness

- Deterministic generated cases now cover scalar arithmetic/coercions, loops,
  dynamic arrays, string conversion/length boundaries, statement-form `Mid`,
  string concatenation, simple calls, error routing, UDT field read/write,
  built-in `Collection`, fixture-backed `CreateObject("OxVba.TestDispatch")`,
  project-object dispatch, source-backed project references, and bounded
  bundle-only project references.
- Every generated case accepted by JIT matched VM3 on snapshot, raised state, final `Err`, and live-handle balance.
- The remaining scope decline in `crates/oxvba-differential/jit_linux_safe_scope.snap` is `unsupported/native_declare`, which is native/COM declaration scope rather than a Linux-safe benchmark fixture.
- No VM fallback is used by the JIT path.

## Execution-Only Benchmark

Fixtures are precompiled OxIR. Source-to-OxIR compile, JIT image compile, and image-load costs are reported separately below.

| fixture | VM3 median | JIT median | VM3/JIT ratio | JIT status / unsupported reason |
|---|---:|---:|---:|---|
| `scalar_loop` | 12.993 ms | 3.1464 ms | 4.13x | supported |
| `string_concat` | 4.7311 ms | 3.2069 ms | 1.48x | supported |
| `array_loop` | 1.2632 ms | 353.39 us | 3.57x | supported |
| `array_redim` | 26.088 ms | 12.813 ms | 2.04x | supported |
| `array_set_long` | 4.7107 ms | 1.3282 ms | 3.55x | supported |
| `array_get_long` | 9.7443 ms | 2.5001 ms | 3.90x | supported |
| `array_for_each_variant` | 10.511 us | 6.2541 us | 1.68x | supported |
| `udt_fields` | 10.134 ms | 4.6596 ms | 2.17x | supported |
| `call_overhead` | 13.465 ms | 5.0149 ms | 2.68x | supported |
| `error_loop` | 10.306 ms | 2.7376 ms | 3.76x | supported |
| `variant_box_unbox` | 17.102 ms | 4.9004 ms | 3.49x | supported |
| `project_object_calls` | 11.641 ms | 6.3347 ms | 1.84x | supported |
| `dynamic_dispatch_helpers` | 9.1825 ms | 5.4762 ms | 1.68x | supported |
| `collection_ops` | 5.9681 ms | 3.0986 ms | 1.93x | supported |
| `com_late_vs_early` | 10.882 ms | 4.7390 ms | 2.30x | supported, fixture-backed `OxVba.TestDispatch` only |
| `bundle_only_referenced_class_aggregates` | 9.4993 ms | 7.0992 ms | 1.34x | supported, compiled CoreProgram surface subset only |

## Compile And Load Costs

| fixture | source-to-OxIR median | JIT image compile median |
|---|---:|---:|
| `scalar_loop` | 141.83 us | 3.1586 ms |
| `string_concat` | 193.91 us | 4.3839 ms |
| `array_loop` | 253.02 us | 4.7247 ms |
| `array_redim` | 124.33 us | 2.2255 ms |
| `array_set_long` | 131.84 us | 2.4158 ms |
| `array_get_long` | 150.54 us | 3.1018 ms |
| `array_for_each_variant` | 121.62 us | 1.6340 ms |
| `udt_fields` | 235.65 us | 3.6605 ms |
| `call_overhead` | 247.69 us | 3.7477 ms |
| `error_loop` | 184.38 us | 3.7909 ms |
| `variant_box_unbox` | 176.39 us | 3.2281 ms |
| `project_object_calls` | 254.86 us | 3.9028 ms |
| `dynamic_dispatch_helpers` | 275.51 us | 3.8968 ms |
| `collection_ops` | 171.36 us | 3.4793 ms |
| `com_late_vs_early` | 286.68 us | 5.3917 ms |
| `bundle_only_referenced_class_aggregates` | 719.24 us | 6.1744 ms |

| fixture | image-load median |
|---|---:|
| `image_load_json_parse/from_bytes_validate` | 35.930 us |

## Tuning Notes

- All current Linux-safe benchmark fixtures run under JIT with VM3 parity coverage before being counted in the performance table.
- The prior `array_loop` regression was traced to `ArrayGet`/`ArraySet` checking `as_safearray()` on every element access, cloning the whole descriptor/payload per access. The JIT helpers now use cheap tag/bounds checks, reducing `array_set_long` from the prior 423.08 ms to 1.3282 ms and `array_get_long` from 863.35 ms to 2.5001 ms.
- UDT scalar/fixed-string record field get/set, Variant temp/place
  arithmetic/coercion, built-in `As New Collection`, fixture-backed
  `CreateObject("OxVba.TestDispatch")`, source-backed project references, and
  bounded bundle-only OxVBA project references are now implemented in this
  Linux-safe slice.
- Bundle-only project references use synthesized compiled export surfaces for
  public module procedures and public class methods/properties. Compiled
  const/enum publication, optional default values, cross-bundle public field
  exports, exact `VB_Creatable` preservation, and product manifest loading of
  referenced `.oxb` artifacts remain outside this measured subset.
- Live Windows COM activation and broader imported library classes remain outside this Linux evidence; the COM benchmark support is intentionally limited to the portable `OxVba.TestDispatch` fixture path.

## 2026-07-08 Aggregate/ReDim Supplement

Focused follow-up evidence is recorded in
`docs/evidence/jit/JIT_UDT_CLASS_AGGREGATES_20260708.md`.

- New benchmark fixtures covered by VM3/JIT parity before perf claims:
  `udt_nested_arrays`, `class_field_aggregates`, and
  `referenced_class_aggregates`; the follow-up bundle-only pass adds
  `bundle_only_referenced_class_aggregates`.
- `array_redim` was retested after the zeroed typed scalar SAFEARRAY payload
  fast path: VM3 median 25.760 ms, JIT median 10.918 ms, ratio 2.36x,
  JIT compile median 2.0539 ms, source-to-OxIR median 119.22 us.
- Aggregate targeted medians:
  `udt_nested_arrays` VM3 23.897 ms / JIT 23.432 ms;
  `class_field_aggregates` VM3 11.617 ms / JIT 9.5590 ms;
  `referenced_class_aggregates` VM3 10.349 ms / JIT 7.5314 ms.
- Bundle-only referenced aggregate targeted median:
  `bundle_only_referenced_class_aggregates` VM3 9.4993 ms / JIT 7.0992 ms,
  ratio 1.34x, JIT compile median 6.1744 ms, source-to-OxIR median 719.24 us.
