# RuntimeValue Bridge Public-API Blockers

Date: 2026-05-01
Bead: `bd-9xmu.3.2` / `value-clean-001`
Workset: `WORKSET_2026-04-30_VALUE_SUBSTRATE_NUMERIC_UDT_CLEANUP.md`

## Outcome

The residual RuntimeValue bridge families left by the phase-2 search gate are
not treated as normal value-substrate semantics. They are recorded here as
public-API blockers with owners, removal targets, and unblocking steps.

This bead does not claim every `RuntimeValue` occurrence is removed. It closes
the phase-3 bridge-retirement gate by making the remaining public bridge
contracts explicit blockers rather than hidden execution dependencies.

## Blocker register

| ID | Surface | Current shape | Owner | Removal target | Unblocking steps |
|---|---|---|---|---|---|
| RV-BRIDGE-001 | `Variant` / `SafeArray` inherent RuntimeValue bridge helpers | `Variant::{try_from_runtime_value, from_runtime_value, to_runtime_value}` and `SafeArray::{from_values, from_typed_values, elements, replace_elements}` remain public compatibility bridge helpers. | Phase-3 value substrate | Before native compiler/linker prerequisite checklist closes | Introduce explicit `oxvba_runtime::compat` extension traits or module functions; migrate tests/callers; keep retained `Variant`/SAFEARRAY APIs as normal path; remove or deprecate inherent bridge methods. |
| RV-BRIDGE-002 | HAL legacy trait methods | HAL traits still expose RuntimeValue methods paired with retained `_variant` companions. | Phase-3 HAL/value substrate | Before HAL native ABI work begins | Split legacy RuntimeValue methods into `oxvba_hal::compat` extension traits or document a semver/public adapter blocker; verify adapters implement variant companions directly. |
| RV-BRIDGE-003 | COM model/dynamic-object bridge methods | `ComValue` and `DynamicValue` keep inherent `from_runtime_value` / `to_runtime_value` bridge methods; `oxvba_com::compat` also exposes explicit conversion helpers. | Phase-3 COM/value substrate | Before COM/native boundary is treated as native-ready | Move inherent bridge methods behind `oxvba_com::compat` extension traits or record a public API compatibility blocker; keep `Variant`/`ComValue` carriers normal. |
| RV-BRIDGE-004 | VM/JIT/host compatibility DTOs and tests | Legacy RuntimeValue access remains under explicit compat modules and tests. | Phase-3 value substrate | Before final Native-Ready umbrella search gate | Retain only if external compatibility is intentionally supported; otherwise delete compatibility extension traits after downstream tests/callers use variant APIs. |

## Guardrails

- New execution, snapshot, observation, presentation, numeric, UDT, or native ABI
  APIs must not import `RuntimeValue` except from an explicit `compat` module.
- New tests that need legacy projections must import the relevant compatibility
  trait/module explicitly.
- Any blocker above that survives phase 3 must be copied into `CURRENT_BLOCKERS.md`
  with an owner/removal date before the umbrella terminal gate can close.

## Verification

Search baseline feeding this register:

```text
rg -n "\bRuntimeValue\b" crates --glob '*.rs' | wc -l
# 2706
rg -l "\bRuntimeValue\b" crates --glob '*.rs' | wc -l
# 58
```

Validation command:

```text
cargo check --workspace
```

Result: passed.
