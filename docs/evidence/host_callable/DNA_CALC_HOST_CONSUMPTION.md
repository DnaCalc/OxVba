# DNA Calc Host Consumption

Date: 2026-05-24
Bead: `bd-hjys.13`

## Implementation summary

Added host consumption examples in `crates/oxvba-host/tests/dna_calc_host_consumption_examples.rs` showing DnaOneCalc- and OxIde-style use of the neutral host API.

## Acceptance coverage

| Acceptance criterion | Evidence |
| --- | --- |
| DnaOneCalc-style host loads a project, reflects callables, admits one through host policy, and invokes via typed API. | `dnaonecalc_style_host_loads_reflects_admits_and_invokes_without_registry_mirror` uses `VbaHost`, `UdfAdmissionPolicy`, and `invoke_callable_typed`, returning `TypedValue::Long(42)`. |
| OxIde-style host inspects inventory without preparing execution. | `oxide_style_host_inspects_inventory_without_preparing_execution` reads `LoadedVbaProject::reflection()` and never calls `prepare()`. |
| Descriptor fingerprint supports cache invalidation examples. | `descriptor_fingerprint_supports_host_cache_invalidation` changes a procedure signature and asserts fingerprint change. |
| No formula binding/name precedence or comprehensive DnaOneCalc-local function mirror is implemented in OxVba. | The tests keep only an admitted request from host policy and do not create registry, formula binding, or name precedence structures. |
| Evidence artifact required. | This file. |

## Checks run

```text
cargo fmt
cargo test -p oxvba-host --test dna_calc_host_consumption_examples -- --nocapture
cargo check --workspace --all-targets
```

Results: all passed.

## Fresh-eyes review notes

- Examples consume neutral reflection and typed invocation only.
- UDF admission remains host-owned and produces a request shape, not registry mutation.
- OxIde inspection remains load/reflect-only.
