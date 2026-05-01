# RuntimeValue / IR Search Gate Evidence

Date: 2026-05-01
Bead: `bd-pn5i.8` / `cleanout-007`
Workset: `WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`

## Search commands and results

```text
rg -n "\bRuntimeValue\b" crates --glob '*.rs' | wc -l
```

Result: `2706` crate occurrences.

```text
rg -l "\bRuntimeValue\b" crates --glob '*.rs' | wc -l
```

Result: `58` crate files.

```text
rg -n "\bRuntimeValue\b" docs --glob '*.md' --glob '!docs/archive/**' --glob '!docs/spec/archive/**' | wc -l
rg -l "\bRuntimeValue\b" docs --glob '*.md' --glob '!docs/archive/**' --glob '!docs/spec/archive/**' | wc -l
```

Result: `666` non-archived doc occurrences in `45` files.

```text
rg -n "CfgIr|VbaHir|VbaMir" crates --glob '*.rs' | wc -l
```

Result: `0` active crate occurrences.

```text
rg -n "CfgIr|VbaHir|VbaMir" docs --glob '*.md' --glob '!docs/archive/**' --glob '!docs/spec/archive/**' | wc -l
```

Result: `7` non-archived doc occurrences. These are explanatory/current-gate
mentions in `docs/IR_DESIGN.md`, the RuntimeValue inventory evidence, and active
workset/search-gate text; no active fake IR crate APIs remain.

Presentation-specific search:

```text
rg -n "RuntimeValue" crates/oxvba-launcher/src crates/oxvba-web-host/src crates/oxvba-web-shell/src crates/oxvba-languageservice/src --glob '*.rs'
```

Result: no matches.

## Residual classification

The RuntimeValue crate search is not clean. It is reduced to approved residual
families after the delivery beads:

1. `oxvba_runtime` compatibility substrate:
   - `runtime_value.rs` still defines the legacy semantic carrier.
   - `compat.rs` is the only runtime root-level re-export path for
     `RuntimeValue`.
   - `coerce::compat` and `pointer_helpers::compat` host runtime-value helper
     wrappers.
   - `Variant` and `SafeArray` still expose bridge helpers for old callers and
     tests.
2. VM/JIT/host compatibility projections:
   - Normal snapshot/invoke/debug/immediate/embedded APIs use retained
     `Variant` DTOs.
   - Legacy access is through explicit `oxvba_vm::compat`,
     `oxvba_jit::compat`, `oxvba_host::compat`, `jit_context::compat`, or
     `slot_abi::compat` boundaries plus compatibility tests.
3. HAL compatibility lanes:
   - HAL traits still contain legacy RuntimeValue methods, but retained
     `_variant` companions are the execution-facing lane and adapter tests check
     direct variant implementation.
   - Residual HAL trait retirement is a phase-3 follow-up because it is a public
     adapter contract split, not a hidden search/replace.
4. COM compatibility lanes:
   - `oxvba_com::compat` owns explicit RuntimeValue/ComValue/Variant projection
     helpers.
   - `ComValue`/`DynamicValue` still have inherent RuntimeValue bridge methods;
     these are tracked for phase-3 retirement or public-API blocker recording.
5. Tests and evidence/docs:
   - Host COM, pointer-helper, VM/JIT, HAL, and runtime tests intentionally
     assert legacy projections through explicit compat imports where required.
   - Non-archived docs mention RuntimeValue to describe the migration state,
     evidence, and residual gates rather than to claim RuntimeValue is the normal
     execution carrier.

## Follow-up path

Residual work is tracked by follow-up bead `bd-9xmu.3.2`:

- **Title:** `value-clean-001 retire residual RuntimeValue bridge methods`
- **Scope:** retire or explicitly public-API-block remaining RuntimeValue bridge
  methods, including `Variant`/`SafeArray` inherent bridge helpers, COM
  model/dynamic-object bridge methods, and HAL legacy trait methods.
- **Gate:** phase-3 value substrate rollout either migrates residuals to
  explicit compat extension traits/modules or records a public API blocker with
  owner/removal date.

This search gate therefore closes phase 2 as "removed or isolated from active
execution/presentation surfaces," not as a claim that all `RuntimeValue` text is
gone from the repository.

## Verification

Recent checks for the phase-2 delivery sequence:

```text
cargo test -p oxvba-runtime -p oxvba-com -p oxvba-hal
cargo test -p oxvba-host --tests
cargo test -p oxvba-vm -p oxvba-jit
cargo test -p oxvba-launcher -p oxvba-web-host -p oxvba-web-shell -p oxvba-languageservice
cargo check --workspace
```

All listed checks passed during the cleanout sequence.
