# Variant-Native Numeric Helper Evidence

Date: 2026-05-01
Bead: `bd-9xmu.3.4` / `value-clean-003`
Workset: `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md`

## Outcome

The normal VM/JIT numeric helper paths are retained-`Variant` paths. The
remaining `RuntimeValue` arithmetic/coercion helpers in `semantics.rs` are
legacy compatibility/test helpers, not interpreter/JIT runtime-helper call sites.

## Search evidence

The normal interpreter/JIT helper search for legacy arithmetic/math helpers was
clean:

```text
rg -n "legacy_(add|sub|mul|pow|div|intdiv|mod|truthy|increment|neg)_|runtime_(abs|sgn|round|sqr|sin|cos|log|exp|atn|tan)_bounded\(" crates/oxvba-vm/src/interpreter.rs crates/oxvba-jit/src/runtime_helpers.rs --glob '*.rs'
```

Result: no matches.

Representative retained-Variant helper call sites in normal paths include:

- `crates/oxvba-vm/src/interpreter.rs` calls
  `runtime_abs_variant_bounded`, `runtime_round_variant_bounded`,
  `runtime_variant_to_i32_compat`, `runtime_variant_to_numeric_compat`,
  `typed_compare_variants`, and variant date/numeric helpers.
- `crates/oxvba-jit/src/runtime_helpers.rs` mirrors those retained-Variant
  helper calls for JIT runtime helpers.

## Verification

Passed:

```text
cargo test -p oxvba-vm semantics::tests
cargo test -p oxvba-jit runtime_
```

These cover VM semantic helper families and JIT runtime helper lanes reading
retained `Variant` carriers.

## Residual

The next beads own deeper behavior specification rather than carrier migration:

- `bd-9xmu.3.5`: mixed numeric result matrix and regression tests.
- `bd-9xmu.3.6`: exact `Currency`, `Decimal`, `Date`, and Boolean carrier
  expectations.
